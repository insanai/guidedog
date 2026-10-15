//! HTML escaping (MarkupSafe), striptags, entity decoding and URL quoting.

use std::borrow::Cow;
use std::fmt::Write as _;
use std::rc::Rc;

use crate::convert;
use crate::text;
use crate::value::Value;

/// Appends text with the five HTML special characters replaced, exactly as MarkupSafe
/// does (`&amp; &lt; &gt; &#34; &#39;`).
pub fn write_escaped(out: &mut String, s: &str) {
    let mut start = 0;
    for (i, b) in s.bytes().enumerate() {
        let replacement = match b {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'"' => "&#34;",
            b'\'' => "&#39;",
            _ => continue,
        };
        out.push_str(&s[start..i]);
        out.push_str(replacement);
        start = i + 1;
    }
    out.push_str(&s[start..]);
}

/// Replaces the five HTML special characters as MarkupSafe does; text that has none is
/// borrowed.
pub fn escape_string(s: &str) -> Cow<'_, str> {
    if !s.bytes().any(|b| matches!(b, b'&' | b'<' | b'>' | b'"' | b'\'')) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 16);
    write_escaped(&mut out, s);
    Cow::Owned(out)
}

/// Converts a value to Markup: Markup stays as it is, everything else is converted to a
/// string and escaped (MarkupSafe's `escape()`).
pub fn escape(v: &Value) -> Value {
    if v.is_markup() {
        return v.clone();
    }
    Value::Markup(Rc::from(escape_string(&convert::to_str(v)).as_ref()))
}

/// The escaped text of v (Markup as it is).
pub(crate) fn escaped_text(v: &Value) -> Cow<'_, str> {
    match v {
        Value::Markup(s) => Cow::Borrowed(s),
        Value::Str(s) => escape_string(s),
        _ => Cow::Owned(escape_string(&convert::to_str(v)).into_owned()),
    }
}

/// Joins two values the way Markup's + does when either side is Markup: the plain side
/// is escaped.
pub(crate) fn markup_concat(a: &Value, b: &Value) -> Value {
    let mut out = String::from(escaped_text(a));
    out.push_str(&escaped_text(b));
    Value::Markup(Rc::from(out))
}

/// MarkupSafe's striptags: drops comments and tags, collapses whitespace, and decodes
/// character references.
pub(crate) fn striptags(s: &str) -> String {
    let mut stripped = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        if let Some(body) = rest.strip_prefix("<!--")
            && let Some(end) = body.find("-->")
        {
            i += 4 + end + 3;
            continue;
        }
        if rest.starts_with('<')
            && let Some(end) = rest[1..].find('>')
        {
            i += end + 2;
            continue;
        }
        let c = rest.chars().next().unwrap_or(' ');
        stripped.push(c);
        i += c.len_utf8();
    }
    let words: Vec<&str> = text::split_whitespace(&stripped).collect();
    unescape_html(&words.join(" ")).into_owned()
}

const ENTITIES: &[(&str, &str)] = &[
    ("amp", "&"),
    ("lt", "<"),
    ("gt", ">"),
    ("quot", "\""),
    ("apos", "'"),
    ("nbsp", "\u{a0}"),
    ("copy", "©"),
    ("reg", "®"),
    ("trade", "™"),
    ("hellip", "…"),
    ("mdash", "—"),
    ("ndash", "–"),
    ("lsquo", "‘"),
    ("rsquo", "’"),
    ("ldquo", "“"),
    ("rdquo", "”"),
    ("laquo", "«"),
    ("raquo", "»"),
    ("middot", "·"),
    ("bull", "•"),
    ("para", "¶"),
    ("sect", "§"),
    ("deg", "°"),
    ("times", "×"),
    ("divide", "÷"),
    ("euro", "€"),
    ("pound", "£"),
    ("yen", "¥"),
    ("cent", "¢"),
    ("larr", "←"),
    ("rarr", "→"),
    ("uarr", "↑"),
    ("darr", "↓"),
    ("shy", "\u{ad}"),
    ("iexcl", "¡"),
    ("iquest", "¿"),
    ("szlig", "ß"),
];

/// Replaces character references: numeric ones and the common named ones.
pub(crate) fn unescape_html(s: &str) -> Cow<'_, str> {
    if !s.contains('&') {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        if !rest.starts_with('&') {
            let c = rest.chars().next().unwrap_or('&');
            out.push(c);
            i += c.len_utf8();
            continue;
        }
        if let Some((text, used)) = reference(rest) {
            out.push_str(&text);
            i += used;
        } else {
            out.push('&');
            i += 1;
        }
    }
    Cow::Owned(out)
}

/// Decodes the character reference at the start of s: its text and its length.
fn reference(s: &str) -> Option<(Cow<'static, str>, usize)> {
    let semi = s.find(';')?;
    if !(2..40).contains(&semi) {
        return None;
    }
    let name = &s[1..semi];
    if let Some(number) = name.strip_prefix('#') {
        let code = match number.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => number.parse::<u32>().ok()?,
        };
        let c = if code == 0 || code > 0x10ffff {
            '\u{fffd}'
        } else {
            char::from_u32(code).unwrap_or('\u{fffd}')
        };
        return Some((Cow::Owned(c.to_string()), semi + 1));
    }
    let (_, value) = ENTITIES.iter().find(|(n, _)| *n == name)?;
    Some((Cow::Borrowed(value), semi + 1))
}

/// Appends s quoted as urllib.parse.quote does, keeping the characters of safe.
pub(crate) fn write_url_quoted(out: &mut String, s: &str, safe: &str, plus_spaces: bool) {
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'~' => {
                out.push(b as char);
            }
            _ if safe.as_bytes().contains(&b) => out.push(b as char),
            b' ' if plus_spaces => out.push('+'),
            _ => {
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
}
