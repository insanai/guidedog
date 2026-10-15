//! Names: reference names, class identifiers, URIs, and numbers as the specification
//! writes them.

use guidedoc::{Fail, Text, normalize_label};

use crate::lines::{Line, is_space};
use crate::parser::Parser;

/// Writes `s` without backslash escapes into `out`. An escaped white space character
/// disappears; any other escaped character is kept without its backslash.
pub(crate) fn unescape_into(out: &mut String, s: &str) {
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some(e) if e.is_ascii() && is_space(e as u8) => {}
            Some(e) => out.push(e),
            None => out.push('\\'),
        }
    }
}

/// Tells whether `s` is non-empty and all ASCII digits.
pub(crate) fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())
}

/// Reads an optionally signed decimal integer prefix; it saturates rather than wraps.
pub(crate) fn parse_int(s: &str) -> i64 {
    let (neg, t) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let mut n: i64 = 0;
    for c in t.bytes().take_while(u8::is_ascii_digit) {
        n = (n * 10 + i64::from(c - b'0')).min(1 << 40);
    }
    if neg { -n } else { n }
}

/// Matches a simple reference name: word characters with isolated hyphens,
/// underscores, periods, colons, and plus signs inside.
pub(crate) fn is_simplename(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut after_punct = true;
    for c in s.bytes() {
        if c.is_ascii_alphanumeric() || c >= 0x80 {
            after_punct = false;
            continue;
        }
        if !matches!(c, b'-' | b'.' | b'_' | b'+' | b':') || after_punct {
            return false;
        }
        after_punct = true;
    }
    !after_punct
}

/// Appends `s` to `out` as a class or identifier: lowercase ASCII letters and digits,
/// every other run turned into one hyphen; leading digits and hyphens are dropped.
/// Returns whether anything was written.
pub(crate) fn write_id(out: &mut String, s: &str) -> bool {
    let (mut started, mut dash) = (false, false);
    for c in s.bytes() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_lowercase() || (c.is_ascii_digit() && started) {
            if dash {
                out.push('-');
            }
            out.push(c as char);
            (started, dash) = (true, false);
        } else if c >= 0x80 || c.is_ascii_digit() {
            // Dropped without a hyphen.
        } else {
            dash = started;
        }
    }
    started
}

/// Recognizes the link block of an indirect target: "name_" or "`phrase`_". Returns
/// the name, still escaped.
pub(crate) fn is_reference(s: &str) -> Option<&str> {
    let n = s.len();
    if n >= 4 && s.starts_with('`') && s.ends_with("`_") {
        let inner = &s[1..n - 2];
        let b = inner.as_bytes();
        if b.is_empty() || b[0] == b' ' || matches!(b[b.len() - 1], b' ' | b'\\') {
            return None;
        }
        return Some(inner);
    }
    let name = s.strip_suffix('_')?;
    is_simplename(name).then_some(name)
}

/// Recognizes a bare e-mail address, which targets turn into mailto links.
pub(crate) fn is_email(s: &str) -> bool {
    let b = s.as_bytes();
    let mut at = None;
    for (i, &c) in b.iter().enumerate() {
        match c {
            b'@' if at.is_some() => return false,
            b'@' => at = Some(i),
            b':' | b'/' | b' ' | b'<' | b'>' | b'"' | b'(' | b')' | b',' | b';' | b'[' | b']'
            | b'\\' => return false,
            _ => {}
        }
    }
    match at {
        Some(at) if at > 0 && at + 1 < b.len() => b[at + 1..b.len() - 1].contains(&b'.'),
        _ => false,
    }
}

/// Appends a link block to `out` as a URI: unescaped white space is removed, escaped
/// white space becomes one space, and other escapes lose their backslash.
pub(crate) fn write_uri(out: &mut String, lines: &[Line<'_>]) {
    for l in lines {
        let mut chars = l.text.chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => match chars.next() {
                    Some(e) if e.is_ascii() && is_space(e as u8) => out.push(' '),
                    Some(e) => out.push(e),
                    None => out.push('\\'),
                },
                c if c.is_ascii() && is_space(c as u8) => {}
                c => out.push(c),
            }
        }
    }
}

/// Returns a link block as a URI (`write_uri`).
pub(crate) fn uri_string(lines: &[Line<'_>]) -> String {
    let mut out = String::new();
    write_uri(&mut out, lines);
    out
}

impl Parser<'_, '_> {
    /// Writes the normalized reference name of `s` to the text pool: escapes removed,
    /// white space collapsed, case folded.
    pub(crate) fn pool_name(&mut self, s: &str) -> Result<Text, Fail> {
        let mut raw = String::with_capacity(s.len());
        unescape_into(&mut raw, s);
        let mut norm = String::with_capacity(raw.len());
        normalize_label(&mut norm, &raw);
        self.ctx.b.add_text(&norm)
    }

    /// Writes white-space separated class names as identifiers, one space apart, to the
    /// text pool; `None` when a name has no letters or there is none.
    pub fn pool_classes(&mut self, s: &str) -> Result<Option<Text>, Fail> {
        let mut out = String::with_capacity(s.len());
        for word in s.split(|c: char| c.is_ascii() && is_space(c as u8)).filter(|w| !w.is_empty())
        {
            if !out.is_empty() {
                out.push(' ');
            }
            if !write_id(&mut out, word) {
                return Ok(None);
            }
        }
        if out.is_empty() {
            return Ok(None);
        }
        self.ctx.b.add_text(&out).map(Some)
    }

    /// Writes `s` in lower case (ASCII) to the text pool.
    pub fn pool_lower(&mut self, s: &str) -> Result<Text, Fail> {
        self.ctx.b.add_text(&s.to_ascii_lowercase())
    }

    /// Pushes `lines` joined by `sep` onto the text run being built (between
    /// `Builder::text_open` and `text_close`).
    pub fn push_lines(&mut self, lines: &[Line<'_>], sep: &str) -> Result<(), Fail> {
        for (i, l) in lines.iter().enumerate() {
            if i > 0 {
                self.ctx.b.text_push(sep)?;
            }
            self.ctx.b.text_push(l.text)?;
        }
        Ok(())
    }

    /// Writes `lines` joined by `sep` to the text pool.
    pub fn pool_lines(&mut self, lines: &[Line<'_>], sep: &str) -> Result<Text, Fail> {
        let start = self.ctx.b.text_open();
        self.push_lines(lines, sep)?;
        Ok(self.ctx.b.text_close(start))
    }

    /// Writes `lines` to the text pool, each ending in "\n" (code).
    pub fn code_text(&mut self, lines: &[Line<'_>]) -> Result<Text, Fail> {
        let start = self.ctx.b.text_open();
        for l in lines {
            self.ctx.b.text_push(l.text)?;
            self.ctx.b.text_push_byte(b'\n')?;
        }
        Ok(self.ctx.b.text_close(start))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_ids() {
        assert!(is_simplename("a-b.c"));
        assert!(!is_simplename("a--b"));
        let mut s = String::new();
        assert!(write_id(&mut s, "9 Hello  World!"));
        assert_eq!(s, "hello-world");
        assert_eq!(is_reference("`a b`_"), Some("a b"));
        assert_eq!(is_reference("name_"), Some("name"));
        assert!(is_email("a@b.c"));
        assert!(!is_email("a@bc"));
        assert_eq!(parse_int("-12x"), -12);
    }
}
