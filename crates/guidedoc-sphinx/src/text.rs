//! ASCII text helpers of the Sphinx layer. Directive arguments, option values, and names
//! are compared and trimmed as Docutils does, by ASCII rules only.

use std::borrow::Cow;

use guidedoc::is_ascii_space;

pub(crate) use guidedoc_rst::equal_fold;

/// Removes ASCII white space from both ends of `s`.
pub(crate) fn trim_space(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_ascii() && is_ascii_space(c as u8))
}

/// Tells whether `s` is a non-empty run of ASCII digits.
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

/// Tells whether `sub` occurs in `s` at byte `i`.
pub(crate) fn has_at(s: &str, i: usize, sub: &str) -> bool {
    s.as_bytes().get(i..i + sub.len()) == Some(sub.as_bytes())
}

/// Returns `s` without `suffix` at its end, when it has one.
pub(crate) fn trim_suffix<'s>(s: &'s str, suffix: &str) -> &'s str {
    s.strip_suffix(suffix).unwrap_or(s)
}

/// Returns `s` without `prefix` at its start, when it has one.
pub(crate) fn trim_prefix<'s>(s: &'s str, prefix: &str) -> &'s str {
    s.strip_prefix(prefix).unwrap_or(s)
}

/// Joins runs of ASCII white space into single spaces.
pub(crate) fn collapse(s: &str) -> Cow<'_, str> {
    let b = s.as_bytes();
    let doubled = b.windows(2).any(|w| is_ascii_space(w[0]) && is_ascii_space(w[1]));
    if !doubled && !b.iter().any(|&c| c == b'\t' || c == b'\n') {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii() && is_ascii_space(c as u8) {
            if !out.ends_with(' ') || out.is_empty() {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
    }
    Cow::Owned(out)
}

/// Returns what follows the last `sep` in `name`, or `name`.
pub(crate) fn last_part<'s>(name: &'s str, sep: &str) -> &'s str {
    name.rfind(sep).map_or(name, |k| &name[k + sep.len()..])
}

/// Splits `s` at the first `sep`: the head, whether it was found, and the tail.
pub(crate) fn partition(s: &str, sep: char) -> (&str, bool, &str) {
    match s.split_once(sep) {
        Some((h, t)) => (h, true, t),
        None => (s, false, ""),
    }
}

/// Tells whether `c` may be part of a Python (or Odin) identifier.
pub(crate) fn is_identifier_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c >= 0x80
}

/// Counts the decimal digits of `n`.
pub(crate) fn digits_of(n: i64) -> usize {
    let mut d = 1;
    let mut v = n;
    while v >= 10 {
        v /= 10;
        d += 1;
    }
    d
}

/// Splits "Title <target>" as Sphinx's explicit title pattern does: (title, target,
/// explicit). Without a title both parts are the whole text. An escaped "<" (a NUL
/// before it in marked text) does not start a target.
pub(crate) fn split_title(s: &str) -> (&str, &str, bool) {
    let b = s.as_bytes();
    let n = b.len();
    if n < 3 || b[n - 1] != b'>' {
        return (s, s, false);
    }
    let Some(k) = (1..n - 1).find(|&i| b[i] == b'<' && b[i - 1] != 0) else {
        return (s, s, false);
    };
    let title = s[..k].trim_end_matches(|c: char| c.is_ascii() && is_ascii_space(c as u8));
    if title.is_empty() {
        return (s, s, false);
    }
    (title, &s[k + 1..n - 1], true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapse_joins_white_space() {
        assert_eq!(collapse("a  b\tc"), "a b c");
        assert_eq!(collapse("ab"), "ab");
    }

    #[test]
    fn parse_int_saturates() {
        assert_eq!(parse_int("-12x"), -12);
        assert_eq!(parse_int("99999999999999999"), 1 << 40);
    }

    #[test]
    fn titles_split_at_the_last_angle_bracket() {
        assert_eq!(split_title("Title <target>"), ("Title", "target", true));
        assert_eq!(split_title("<target>"), ("<target>", "<target>", false));
        assert_eq!(split_title("plain"), ("plain", "plain", false));
    }
}
