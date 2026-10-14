//! String literals as Python evaluates and `repr`s them, and docstring preparation.

use std::borrow::Cow;
use std::fmt::Write;

use crate::syntax::Node;

/// What a string literal makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StringKind {
    /// A `str`.
    #[default]
    Str,
    /// A `bytes`: each byte is held as the `char` of the same value (Latin-1).
    Bytes,
    /// An f-string or t-string: not a constant.
    Formatted,
}

/// Evaluates a `string` or `concatenated_string` node as Python does: prefixes, escape
/// sequences, and adjacent literals joined. `None` for other nodes.
pub fn string_value<'a>(n: Node<'a>) -> Option<(Cow<'a, str>, StringKind)> {
    match n.kind() {
        "string" => one_string(n),
        "concatenated_string" => {
            let mut out = String::new();
            let mut kind = StringKind::Str;
            for part in n.named_children() {
                let (v, k) = one_string(part)?;
                if k != StringKind::Str {
                    kind = k;
                }
                out.push_str(&v);
            }
            Some((Cow::Owned(out), kind))
        }
        _ => None,
    }
}

/// [`string_value`] for `str` constants only, looking through parentheses.
pub fn str_value<'a>(n: Node<'a>) -> Option<Cow<'a, str>> {
    let mut node = n;
    while node.kind() == "parenthesized_expression" {
        node = node.named_child(0);
    }
    match string_value(node)? {
        (v, StringKind::Str) => Some(v),
        _ => None,
    }
}

fn one_string<'a>(n: Node<'a>) -> Option<(Cow<'a, str>, StringKind)> {
    if n.kind() != "string" {
        return None;
    }
    let start = n.child(0);
    let end = n.child(n.child_count().checked_sub(1)?);
    if start.kind() != "string_start" || end.kind() != "string_end" {
        return None;
    }
    let mut raw = false;
    let mut kind = StringKind::Str;
    for c in start.text().chars() {
        match c {
            'r' | 'R' => raw = true,
            'b' | 'B' => kind = StringKind::Bytes,
            'f' | 'F' | 't' | 'T' => kind = StringKind::Formatted,
            _ => {}
        }
    }
    let body = n.source_slice(start.end_byte(), end.start_byte());
    let bytes = kind == StringKind::Bytes;
    if raw {
        if bytes && !body.is_ascii() {
            return Some((Cow::Owned(body.bytes().map(char::from).collect()), kind));
        }
        return Some((Cow::Borrowed(body), kind));
    }
    Some((unescape(body, bytes), kind))
}

/// Processes Python's escape sequences. Unknown escapes keep their backslash, as Python
/// does (with a warning); `\N{...}` is kept as written.
fn unescape(s: &str, bytes: bool) -> Cow<'_, str> {
    if !s.contains('\\') {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'\\' || i + 1 >= b.len() {
            let ch = s[i..].chars().next().unwrap_or('\u{fffd}');
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        i += 1 + escape(&mut out, &s[i + 1..], bytes);
    }
    Cow::Owned(out)
}

/// Writes the escape `s` starts with (after the backslash); returns the bytes it used.
fn escape(out: &mut String, s: &str, bytes: bool) -> usize {
    let b = s.as_bytes();
    let simple = match b[0] {
        b'\n' => Some(None),
        c @ (b'\\' | b'\'' | b'"') => Some(Some(c as char)),
        b'n' => Some(Some('\n')),
        b't' => Some(Some('\t')),
        b'r' => Some(Some('\r')),
        b'a' => Some(Some('\u{7}')),
        b'b' => Some(Some('\u{8}')),
        b'f' => Some(Some('\u{c}')),
        b'v' => Some(Some('\u{b}')),
        _ => None,
    };
    if let Some(c) = simple {
        out.extend(c);
        return 1;
    }
    match b[0] {
        b'0'..=b'7' => {
            let mut n = 0;
            let mut value = 0u32;
            while n < 3 && n < b.len() && (b'0'..=b'7').contains(&b[n]) {
                value = value * 8 + u32::from(b[n] - b'0');
                n += 1;
            }
            write_code(out, value, bytes);
            n
        }
        b'x' => hex_escape(out, s, 2, bytes),
        b'u' if !bytes => hex_escape(out, s, 4, false),
        b'U' if !bytes => hex_escape(out, s, 8, false),
        _ => {
            out.push('\\');
            0
        }
    }
}

fn hex_escape(out: &mut String, s: &str, digits: usize, bytes: bool) -> usize {
    let value = s.get(1..1 + digits).and_then(|h| {
        if h.bytes().all(|c| c.is_ascii_hexdigit()) {
            u32::from_str_radix(h, 16).ok()
        } else {
            None
        }
    });
    match value {
        Some(v) => {
            write_code(out, v, bytes);
            1 + digits
        }
        None => {
            out.push('\\');
            0
        }
    }
}

fn write_code(out: &mut String, code: u32, bytes: bool) {
    if bytes {
        out.push(char::from(code as u8));
    } else {
        out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
    }
}

/// Python's `repr` of a `str`: single quotes unless the text has one and no double
/// quote, and escapes for what is not printable.
pub fn repr_str(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut b = String::with_capacity(s.len() + 2);
    b.push(quote);
    for r in s.chars() {
        match r {
            _ if r == quote || r == '\\' => {
                b.push('\\');
                b.push(r);
            }
            '\n' => b.push_str("\\n"),
            '\r' => b.push_str("\\r"),
            '\t' => b.push_str("\\t"),
            '\u{fffd}' => b.push_str("\\ufffd"),
            _ if !printable(r) => write_hex_escape(&mut b, r as u32),
            _ => b.push(r),
        }
    }
    b.push(quote);
    b
}

/// Python's `repr` of `bytes` held as [`StringKind::Bytes`] text (one `char` per byte).
pub fn repr_bytes(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { b'"' } else { b'\'' };
    let mut b = String::with_capacity(s.len() + 3);
    b.push('b');
    b.push(quote as char);
    let mut utf8 = [0u8; 4];
    for ch in s.chars() {
        let bytes: &[u8] = match u8::try_from(ch as u32) {
            Ok(byte) => {
                utf8[0] = byte;
                &utf8[..1]
            }
            Err(_) => ch.encode_utf8(&mut utf8).as_bytes(),
        };
        for &c in bytes {
            match c {
                _ if c == quote || c == b'\\' => {
                    b.push('\\');
                    b.push(c as char);
                }
                b'\n' => b.push_str("\\n"),
                b'\r' => b.push_str("\\r"),
                b'\t' => b.push_str("\\t"),
                _ if !(0x20..0x7f).contains(&c) => write_hex_escape(&mut b, u32::from(c)),
                _ => b.push(c as char),
            }
        }
    }
    b.push(quote as char);
    b
}

fn write_hex_escape(b: &mut String, r: u32) {
    let _ = match r {
        0..=0xff => write!(b, "\\x{r:02x}"),
        0x100..=0xffff => write!(b, "\\u{r:04x}"),
        _ => write!(b, "\\U{r:08x}"),
    };
}

/// Approximates `str.isprintable`: control characters, separators other than the
/// space, and format characters are not.
fn printable(r: char) -> bool {
    !matches!(r as u32,
        0..=0x1f | 0x7f..=0xa0 | 0xad | 0x2000..=0x200f | 0x2028..=0x202f
            | 0x205f..=0x206f | 0x3000 | 0xfeff | 0xd800..=0xdfff | 0xe000..=0xf8ff)
}

/// `str.expandtabs(size)`.
pub(crate) fn expand_tabs(s: &str, size: usize) -> Cow<'_, str> {
    if !s.contains('\t') {
        return Cow::Borrowed(s);
    }
    let mut b = String::with_capacity(s.len() + 16);
    let mut column = 0;
    for r in s.chars() {
        match r {
            '\t' => {
                let n = if size > 0 { size - column % size } else { 0 };
                b.extend(std::iter::repeat_n(' ', n));
                column += n;
            }
            '\n' | '\r' => {
                b.push(r);
                column = 0;
            }
            _ => {
                b.push(r);
                column += 1;
            }
        }
    }
    Cow::Owned(b)
}

/// Sphinx's `prepare_docstring`: lines of reST with the common indentation of the lines
/// after the first removed, the first line stripped, leading blank lines dropped, and a
/// blank line at the end.
pub fn prepare_docstring(s: &str, tab_width: usize) -> Vec<String> {
    let expanded = expand_tabs(s, tab_width);
    let lines = split_lines(&expanded);
    let margin = lines
        .iter()
        .skip(1)
        .filter_map(|l| {
            let content = l.trim_start();
            (!content.is_empty()).then(|| l.len() - content.len())
        })
        .min();
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + 1);
    for (i, l) in lines.iter().enumerate() {
        let line = match (i, margin) {
            (0, _) => l.trim_start(),
            (_, Some(m)) => l.get(m.min(l.len())..).unwrap_or(l.trim_start()),
            (_, None) => l,
        };
        if out.is_empty() && line.is_empty() {
            continue; // leading blank lines are dropped.
        }
        out.push(line.to_string());
    }
    if out.last().is_some_and(|l| !l.is_empty()) {
        out.push(String::new());
    }
    out
}

/// What Python 3.13's compiler does to docstrings it stores in `__doc__`: tabs
/// expanded, the first line's leading spaces removed, and up to the common indentation
/// of the later lines removed from each of them.
pub(crate) fn compile_cleandoc(s: &str) -> String {
    let doc = expand_tabs(s, 8);
    let margin = doc
        .split('\n')
        .skip(1)
        .filter_map(|line| {
            let spaces = line.bytes().take_while(|&c| c == b' ').count();
            (spaces < line.len()).then_some(spaces)
        })
        .min()
        .unwrap_or(0);
    let mut b = String::with_capacity(doc.len());
    for (i, line) in doc.split('\n').enumerate() {
        if i == 0 {
            b.push_str(line.trim_start_matches(' '));
            continue;
        }
        b.push('\n');
        let cut = line.bytes().take(margin).take_while(|&c| c == b' ').count();
        b.push_str(&line[cut..]);
    }
    b
}

/// `inspect.cleandoc`: [`prepare_docstring`] without the final blank line, and with
/// trailing blank lines dropped.
pub(crate) fn cleandoc(s: &str) -> String {
    let mut lines = prepare_docstring(s, 8);
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

/// `str.splitlines` for `\n`, `\r\n`, `\r` and the other line boundaries Python knows;
/// a final line break does not start another line.
pub fn split_lines(s: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut it = s.char_indices().peekable();
    while let Some((i, r)) = it.next() {
        match r {
            '\n' | '\u{b}' | '\u{c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}'
            | '\u{2029}' => {
                lines.push(&s[start..i]);
                start = i + r.len_utf8();
            }
            '\r' => {
                lines.push(&s[start..i]);
                start = i + 1;
                if let Some(&(_, '\n')) = it.peek() {
                    it.next();
                    start += 1;
                }
            }
            _ => {}
        }
    }
    if start < s.len() {
        lines.push(&s[start..]);
    }
    lines
}
