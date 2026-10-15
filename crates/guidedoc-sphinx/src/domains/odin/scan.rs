//! Reading Odin declarations as text: brackets, strings, and the parts of a signature.
//! Every position these helpers return is at an ASCII byte or at the end, so slicing
//! there is safe.

use crate::text::{has_at, is_identifier_byte, trim_space};
use guidedoc::is_ascii_space;

/// A signature split into its attributes, the written prefix and name, and what
/// follows the name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct OdinSig<'s> {
    /// `@(require_results)` and other attributes, as written.
    pub attrs: &'s str,
    /// A written qualifier ending in a dot ("Type."), or "".
    pub prefix: &'s str,
    /// The declared name.
    pub name: &'s str,
    /// Everything after the name, trimmed.
    pub rest: &'s str,
}

/// Returns the index after the string or rune literal that starts at `i`.
pub(super) fn skip_quote(s: &[u8], i: usize) -> usize {
    let quote = s[i];
    let mut k = i + 1;
    while k < s.len() && s[k] != quote {
        if s[k] == b'\\' && quote != b'`' {
            k += 1;
        }
        k += 1;
    }
    (k + 1).min(s.len())
}

/// Returns the index of the bracket closing the one at `i`, skipping strings and
/// nested brackets.
pub(super) fn close(s: &str, i: usize) -> Option<usize> {
    let b = s.as_bytes();
    let mut depth = 0i32;
    let mut k = i;
    while k < b.len() {
        match b[k] {
            b'"' | b'`' | b'\'' => {
                k = skip_quote(b, k);
                continue;
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(k);
                }
            }
            _ => {}
        }
        k += 1;
    }
    None
}

/// Returns the index of `c` outside brackets and strings, or `s.len()`.
pub(super) fn find(s: &str, c: u8) -> usize {
    let b = s.as_bytes();
    let mut depth = 0i32;
    let mut k = 0;
    while k < b.len() {
        let x = b[k];
        if depth == 0 && x == c {
            return k;
        }
        match x {
            b'"' | b'`' | b'\'' => {
                k = skip_quote(b, k);
                continue;
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            _ => {}
        }
        k += 1;
    }
    b.len()
}

/// Returns where a procedure's results end: at a tag ("#optional_ok") or a where
/// clause outside brackets.
pub(super) fn results_end(s: &str) -> usize {
    let b = s.as_bytes();
    let mut depth = 0i32;
    let mut k = 0;
    while k < b.len() {
        let after_space = depth == 0 && k > 0 && is_ascii_space(b[k - 1]);
        match b[k] {
            b'"' | b'`' | b'\'' => {
                k = skip_quote(b, k);
                continue;
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'#' if after_space => return k,
            b'w' if after_space
                && has_at(s, k, "where")
                && b.get(k + 5).is_none_or(|&c| !is_identifier_byte(c)) =>
            {
                return k;
            }
            _ => {}
        }
        k += 1;
    }
    b.len()
}

/// Splits a signature; `None` for text with no name. A foreign import's signature may
/// start with "foreign import".
pub(super) fn parse_sig(sig: &str, foreign_import: bool) -> Option<OdinSig<'_>> {
    let t = trim_space(sig);
    let b = t.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i] == b'@' {
        let mut k = i + 1;
        if b.get(k) == Some(&b'(') {
            k = close(t, k)? + 1;
        } else {
            while k < b.len() && is_identifier_byte(b[k]) {
                k += 1;
            }
        }
        i = k;
        while i < b.len() && is_ascii_space(b[i]) {
            i += 1;
        }
    }
    let attrs = trim_space(&t[..i]);
    let mut t = &t[i..];
    if foreign_import {
        t = after_word(after_word(t, "foreign"), "import");
    }
    let b = t.as_bytes();
    let k = b.iter().position(|&c| !(is_identifier_byte(c) || c == b'.')).unwrap_or(b.len());
    let head = &t[..k];
    let split = head.rfind('.').map_or(0, |dot| dot + 1);
    let (prefix, name) = head.split_at(split);
    if name.is_empty() || name.as_bytes()[0].is_ascii_digit() {
        return None;
    }
    Some(OdinSig { attrs, prefix, name, rest: trim_space(&t[k..]) })
}

/// Drops a leading word and the space after it.
fn after_word<'s>(t: &'s str, word: &str) -> &'s str {
    let boundary = t.as_bytes().get(word.len()).is_none_or(|&c| !is_identifier_byte(c));
    match t.strip_prefix(word) {
        Some(rest) if boundary => trim_space(rest),
        _ => t,
    }
}

/// Splits the first word (an identifier, or "#" and one) from the rest.
pub(super) fn first_word(v: &str) -> (&str, &str) {
    let b = v.as_bytes();
    let mut k = usize::from(b.first() == Some(&b'#'));
    while k < b.len() && is_identifier_byte(b[k]) {
        k += 1;
    }
    (&v[..k], trim_space(&v[k..]))
}

/// Returns where a name starting at `i` ends: an identifier, or a package's name, a
/// dot, and an identifier.
pub(super) fn name_end(b: &[u8], i: usize) -> usize {
    let mut end = i;
    while end < b.len() && is_identifier_byte(b[end]) {
        end += 1;
    }
    let dotted = b.get(end) == Some(&b'.')
        && b.get(end + 1).is_some_and(|&c| is_identifier_byte(c) && !c.is_ascii_digit());
    if dotted {
        end += 1;
        while end < b.len() && is_identifier_byte(b[end]) {
            end += 1;
        }
    }
    end
}

/// Returns a package's name as code qualifies with it: its last path segment.
pub(super) fn short_name(pkg: &str) -> &str {
    pkg.rfind(['/', ':']).map_or(pkg, |k| &pkg[k + 1..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures_split_into_attributes_prefix_and_name() {
        let s = parse_sig("@(require_results) clone :: proc()", false);
        assert_eq!(
            s,
            Some(OdinSig {
                attrs: "@(require_results)",
                prefix: "",
                name: "clone",
                rest: ":: proc()"
            })
        );
        let s = parse_sig("Small.data: int", false);
        assert_eq!(s.map(|s| (s.prefix, s.name)), Some(("Small.", "data")));
        let s = parse_sig("foreign import libc \"system:c\"", true);
        assert_eq!(s.map(|s| (s.name, s.rest)), Some(("libc", "\"system:c\"")));
        assert_eq!(parse_sig("3 :: x", false), None);
        assert_eq!(parse_sig("@(x", false), None);
    }

    #[test]
    fn brackets_and_strings_are_skipped() {
        assert_eq!(close("(a, \")\", (b))", 0), Some(12));
        assert_eq!(close("(a", 0), None);
        assert_eq!(find("a: [x, y], b", b','), 9);
        assert_eq!(find("abc", b','), 3);
        assert_eq!(results_end("[]K where K == int"), 4);
        assert_eq!(results_end("int #optional_ok"), 4);
        assert_eq!(results_end("int wherever"), 12);
    }

    #[test]
    fn names_and_packages() {
        assert_eq!(short_name("core:fmt"), "fmt");
        assert_eq!(short_name("readers/sphinx"), "sphinx");
        assert_eq!(short_name("box"), "box");
        assert_eq!(name_end(b"gd.Node_Id x", 0), 10);
        assert_eq!(name_end(b"a.0", 0), 1);
        assert_eq!(first_word("#force_inline proc()"), ("#force_inline", "proc()"));
    }
}
