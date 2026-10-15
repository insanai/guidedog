//! Entity and numeric character references, and backslash-escape decoding.

use super::entity_table::ENTITIES;

/// Decodes an entity or numeric character reference at the start of `s`: the text it
/// stands for and the bytes it takes. `None` when `s` does not start with a valid,
/// known reference.
pub(crate) fn match_entity(s: &[u8]) -> Option<(Decoded, usize)> {
    if s.len() < 3 || s[0] != b'&' {
        return None;
    }
    if s[1] == b'#' {
        return match_numeric(s);
    }
    if !s[1].is_ascii_alphabetic() {
        return None;
    }
    let mut i = 2;
    while i < s.len() && i <= 32 && s[i].is_ascii_alphanumeric() {
        i += 1;
    }
    if i >= s.len() || s[i] != b';' {
        return None;
    }
    let name = std::str::from_utf8(&s[1..i]).ok()?;
    let k = ENTITIES.binary_search_by(|(n, _)| (*n).cmp(name)).ok()?;
    Some((Decoded::Named(ENTITIES[k].1), i + 1))
}

/// What a reference decodes to.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Decoded {
    Named(&'static str),
    Char(char),
}

impl Decoded {
    /// Appends the decoded text to `out`.
    pub(crate) fn push_to(self, out: &mut String) {
        match self {
            Decoded::Named(s) => out.push_str(s),
            Decoded::Char(c) => out.push(c),
        }
    }
}

fn match_numeric(s: &[u8]) -> Option<(Decoded, usize)> {
    let mut i = 2;
    let hex = matches!(s.get(i), Some(b'x' | b'X'));
    if hex {
        i += 1;
    }
    let start = i;
    let limit = if hex { 6 } else { 7 };
    let mut value: u32 = 0;
    while i < s.len() && i - start < limit {
        let d = if hex { (s[i] as char).to_digit(16) } else { (s[i] as char).to_digit(10) };
        let Some(d) = d else { break };
        value = value * if hex { 16 } else { 10 } + d;
        i += 1;
    }
    if i == start || i >= s.len() || s[i] != b';' {
        return None;
    }
    let c = if value == 0 { None } else { char::from_u32(value) };
    Some((Decoded::Char(c.unwrap_or('\u{fffd}')), i + 1))
}

/// Appends `s` to `out` with backslash escapes and entity references decoded.
pub(crate) fn push_unescaped(out: &mut String, s: &str) {
    let b = s.as_bytes();
    let (mut run, mut i) = (0, 0);
    while i < b.len() {
        let c = b[i];
        if c == b'\\' && i + 1 < b.len() && b[i + 1].is_ascii_punctuation() {
            out.push_str(&s[run..i]);
            out.push(b[i + 1] as char);
            i += 2;
            run = i;
            continue;
        }
        if c == b'&'
            && let Some((decoded, used)) = match_entity(&b[i..])
        {
            out.push_str(&s[run..i]);
            decoded.push_to(out);
            i += used;
            run = i;
            continue;
        }
        i += 1;
    }
    out.push_str(&s[run..]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(s: &str) -> String {
        let mut out = String::new();
        push_unescaped(&mut out, s);
        out
    }

    #[test]
    fn references_decode_as_html5_names_them() {
        assert_eq!(
            decode("&Agr; &blank; &lowast; &lang; &#0; &#x110000; &ngE;"),
            "&Agr; \u{2423} \u{2217} \u{27e8} \u{fffd} \u{fffd} \u{2267}\u{338}"
        );
        assert_eq!(decode("\\*a&amp;b\\q"), "*a&b\\q");
        assert!(ENTITIES.windows(2).all(|w| w[0].0 < w[1].0));
    }
}
