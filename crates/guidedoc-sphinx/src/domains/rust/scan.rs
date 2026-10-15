//! Reading Rust declarations as text: brackets (with `->` and `=>` not closing an
//! angle bracket), string, char and lifetime tokens, and the head of a signature.
//! Every position these helpers return is at an ASCII byte or at the end, so slicing
//! there is safe.

use guidedoc::is_ascii_space;

use crate::text::{is_identifier_byte, trim_space};

/// The qualifiers a signature wrote before its keyword.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Qualifiers {
    pub is_const: bool,
    pub is_async: bool,
    pub is_unsafe: bool,
}

/// A signature split at its name: what is written before it (attributes, visibility,
/// qualifiers, keyword), the name, and what follows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Head<'s> {
    /// The text before the name, as written and trimmed ("pub const unsafe fn").
    pub annotation: &'s str,
    /// The object type's keyword was written ("fn", "struct", ...).
    pub keyword: bool,
    /// The declared name; "" for impls and uses, which have none.
    pub name: &'s str,
    /// Everything after the name (after the keyword for impls and uses), trimmed.
    pub rest: &'s str,
    pub qualifiers: Qualifiers,
}

/// Returns the keyword that declares an object type: "fn" for functions, "" for
/// fields and variants, which have none.
pub(super) fn keyword_of(objtype: &str) -> &'static str {
    match objtype {
        "function" | "method" => "fn",
        "struct" => "struct",
        "enum" => "enum",
        "union" => "union",
        "trait" => "trait",
        "type" | "associatedtype" => "type",
        "const" | "associatedconst" => "const",
        "static" => "static",
        "macro" => "macro_rules!",
        "impl" => "impl",
        "use" => "use",
        _ => "",
    }
}

/// Returns the index after the token starting with a quote at `i`: a string, a char
/// literal, or a lifetime (`'a`, which has no closing quote).
pub(super) fn skip_quoted(b: &[u8], i: usize) -> usize {
    let quote = b[i];
    if quote == b'\'' && b.get(i + 1).is_some_and(|&c| is_identifier_byte(c)) {
        let mut k = i + 1;
        while k < b.len() && is_identifier_byte(b[k]) {
            k += 1;
        }
        if b.get(k) != Some(&b'\'') {
            return k;
        }
    }
    let mut k = i + 1;
    while k < b.len() && b[k] != quote {
        if b[k] == b'\\' {
            k += 1;
        }
        k += 1;
    }
    (k + 1).min(b.len())
}

/// The change of bracket depth the byte at `k` makes: `(`, `[`, `{`, `<` open; their
/// pairs close, except the `>` of `->` and `=>`.
fn depth_change(b: &[u8], k: usize) -> i32 {
    match b[k] {
        b'(' | b'[' | b'{' | b'<' => 1,
        b')' | b']' | b'}' => -1,
        b'>' if k == 0 || !matches!(b[k - 1], b'-' | b'=') => -1,
        _ => 0,
    }
}

/// Returns the index of the bracket closing the one at `i`.
pub(super) fn close(s: &str, i: usize) -> Option<usize> {
    let b = s.as_bytes();
    let mut depth = 0i32;
    let mut k = i;
    while k < b.len() {
        if matches!(b[k], b'"' | b'\'') {
            k = skip_quoted(b, k);
            continue;
        }
        depth += depth_change(b, k);
        if depth == 0 {
            return Some(k);
        }
        k += 1;
    }
    None
}

/// Returns the index of the first byte outside brackets and quotes that `stop`
/// accepts, or `s.len()`.
pub(super) fn find_top(s: &str, stop: impl Fn(&[u8], usize) -> bool) -> usize {
    let b = s.as_bytes();
    let mut depth = 0i32;
    let mut k = 0;
    while k < b.len() {
        if depth == 0 && stop(b, k) {
            return k;
        }
        if matches!(b[k], b'"' | b'\'') {
            k = skip_quoted(b, k);
            continue;
        }
        depth += depth_change(b, k);
        k += 1;
    }
    b.len()
}

/// Returns the index of a top-level `c`, or `s.len()`.
pub(super) fn find_byte(s: &str, c: u8) -> usize {
    find_top(s, |b, k| b[k] == c)
}

/// Returns the index of a top-level single colon (not part of `::`), or `s.len()`.
pub(super) fn find_colon(s: &str) -> usize {
    find_top(s, |b, k| b[k] == b':' && b.get(k + 1) != Some(&b':') && (k == 0 || b[k - 1] != b':'))
}

/// Returns the index of a top-level `=` that is not part of `==`, `=>`, `<=`, `>=` or
/// `!=`, or `s.len()`.
pub(super) fn find_assign(s: &str) -> usize {
    find_top(s, |b, k| {
        let next = b.get(k + 1).copied();
        b[k] == b'='
            && !matches!(next, Some(b'=' | b'>'))
            && (k == 0 || !matches!(b[k - 1], b'=' | b'<' | b'>' | b'!'))
    })
}

/// Returns the index of a top-level word `word` that stands alone, or `s.len()`.
pub(super) fn find_word(s: &str, word: &str) -> usize {
    find_top(s, |b, k| {
        b[k..].starts_with(word.as_bytes())
            && (k == 0 || !is_identifier_byte(b[k - 1]))
            && b.get(k + word.len()).is_none_or(|&c| !is_identifier_byte(c))
    })
}

/// Returns the end of the identifier (or raw identifier `r#name`) at `i`; `i` when
/// there is none.
pub(super) fn ident_end(b: &[u8], i: usize) -> usize {
    let mut k = i;
    if b.get(k) == Some(&b'r') && b.get(k + 1) == Some(&b'#') {
        k += 2;
    }
    let start = k;
    while k < b.len() && is_identifier_byte(b[k]) {
        k += 1;
    }
    if k == start { i } else { k }
}

fn skip_space(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && is_ascii_space(b[i]) {
        i += 1;
    }
    i
}

/// Splits a signature at its name, for an object of type `objtype`; `None` when no
/// name can be found (or, for impls and uses, no keyword).
pub(super) fn parse_head<'s>(sig: &'s str, objtype: &str) -> Option<Head<'s>> {
    let t = trim_space(sig);
    let b = t.as_bytes();
    let keyword = keyword_of(objtype).trim_end_matches('!');
    let mut head = Head::default();
    let mut i = skip_space(b, 0);
    while i < b.len() {
        if b[i] == b'#' {
            let open = i + 1 + usize::from(b.get(i + 1) == Some(&b'!'));
            i = skip_space(b, close(t, open)? + 1);
            continue;
        }
        let end = ident_end(b, i);
        let word = &t[i..end];
        if word.is_empty() {
            break;
        }
        if !keyword.is_empty() && word == keyword {
            head.keyword = true;
            i = after_keyword(t, end, keyword);
            break;
        }
        match word {
            "pub" => {
                i = skip_space(b, end);
                if b.get(i) == Some(&b'(') {
                    i = close(t, i)? + 1;
                }
            }
            "const" => head.qualifiers.is_const = true,
            "async" => head.qualifiers.is_async = true,
            "unsafe" => head.qualifiers.is_unsafe = true,
            "extern" => {
                i = skip_space(b, end);
                if b.get(i) == Some(&b'"') {
                    i = skip_quoted(b, i);
                }
            }
            "default" | "auto" | "safe" => {}
            _ => break,
        }
        if end > i {
            i = end;
        }
        i = skip_space(b, i);
    }
    head.annotation = trim_space(&t[..i]);
    if matches!(objtype, "impl" | "use") {
        head.rest = trim_space(&t[i..]);
        return head.keyword.then_some(head);
    }
    let start = skip_space(b, i);
    let end = ident_end(b, start);
    let name = &t[start..end];
    let digits = name.as_bytes().first().is_some_and(u8::is_ascii_digit);
    if name.is_empty() || (digits && objtype != "field") {
        return None;
    }
    head.name = name;
    head.rest = trim_space(&t[end..]);
    Some(head)
}

/// Returns where the name starts after a keyword ending at `end`: after the `!` of
/// `macro_rules!` and the `mut` of `static mut`.
fn after_keyword(t: &str, end: usize, keyword: &str) -> usize {
    let b = t.as_bytes();
    let mut i = end;
    if keyword == "macro_rules" && b.get(i) == Some(&b'!') {
        i += 1;
    }
    let k = skip_space(b, i);
    if keyword == "static" && ident_end(b, k) == k + 3 && &t[k..k + 3] == "mut" {
        i = k + 3;
    }
    i
}

/// Returns the names a generic parameter list (`<T: Copy, const N: usize, 'a>`, with
/// its brackets) declares: types and constants, not lifetimes.
pub(super) fn generic_names(list: &str) -> impl Iterator<Item = &str> {
    let inner = list.strip_prefix('<').and_then(|l| l.strip_suffix('>')).unwrap_or("");
    let mut rest = inner;
    std::iter::from_fn(move || {
        while !rest.is_empty() {
            let k = find_byte(rest, b',');
            let item = trim_space(&rest[..k]);
            rest = &rest[(k + 1).min(rest.len())..];
            let item = trim_space(item.strip_prefix("const ").unwrap_or(item));
            let end = ident_end(item.as_bytes(), 0);
            if end > 0 {
                return Some(&item[..end]);
            }
        }
        None
    })
}

/// Splits leading generic parameters (`<...>`) from `s`: (the list with brackets, the
/// rest trimmed); ("", s) without one.
pub(super) fn split_generics(s: &str) -> (&str, &str) {
    if !s.starts_with('<') {
        return ("", s);
    }
    match close(s, 0) {
        Some(end) => (&s[..=end], trim_space(&s[end + 1..])),
        None => ("", s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(sig: &str, objtype: &str) -> (String, String, String) {
        let h = parse_head(sig, objtype).expect("a head");
        (h.annotation.to_owned(), h.name.to_owned(), h.rest.to_owned())
    }

    fn owned(a: &str, n: &str, r: &str) -> (String, String, String) {
        (a.to_owned(), n.to_owned(), r.to_owned())
    }

    #[test]
    fn heads_split_at_the_name() {
        assert_eq!(
            head("pub const unsafe fn f<T>(x: &T) -> u8", "function"),
            owned("pub const unsafe fn", "f", "<T>(x: &T) -> u8")
        );
        assert_eq!(
            head("pub unsafe extern \"C\" fn c_draw(x: i32)", "function"),
            owned("pub unsafe extern \"C\" fn", "c_draw", "(x: i32)")
        );
        assert_eq!(head("pub(crate) x: T", "field"), owned("pub(crate)", "x", ": T"));
        assert_eq!(head("0: u8", "field"), owned("", "0", ": u8"));
        assert_eq!(
            head("pub static mut DRAWN: u64", "static"),
            owned("pub static mut", "DRAWN", ": u64")
        );
        assert_eq!(head("macro_rules! shape", "macro"), owned("macro_rules!", "shape", ""));
        assert_eq!(
            head("pub const MAX: usize = 4", "const"),
            owned("pub const", "MAX", ": usize = 4")
        );
        assert_eq!(head("f(x: u8)", "function"), owned("", "f", "(x: u8)"));
        assert_eq!(head("impl<T> X<T>", "impl"), owned("impl", "", "<T> X<T>"));
        assert_eq!(
            head("#[must_use] pub fn g()", "function"),
            owned("#[must_use] pub fn", "g", "()")
        );
        assert!(parse_head("Point<T>", "impl").is_none());
        assert!(parse_head("pub fn 3()", "function").is_none());
        let q = parse_head("pub async unsafe fn f()", "function").map(|h| h.qualifiers);
        assert_eq!(q, Some(Qualifiers { is_const: false, is_async: true, is_unsafe: true }));
    }

    #[test]
    fn brackets_skip_arrows_and_lifetimes() {
        assert_eq!(close("<F: Fn() -> u8>", 0), Some(14));
        assert_eq!(close("<'a, T>", 0), Some(6));
        assert_eq!(close("('a')", 0), Some(4));
        assert_eq!(find_colon("x: std::fmt::Formatter<'_>"), 1);
        assert_eq!(find_colon("std::fmt::X"), 11);
        assert_eq!(find_assign("T: X<Item = u8> = u8"), 16);
        assert_eq!(find_word("Display for Point<T>", "for"), 8);
        assert_eq!(find_word("Fn(&mut Formatter)", "for"), 18);
    }

    #[test]
    fn generic_names_leave_out_lifetimes() {
        let names: Vec<&str> =
            generic_names("<'a, T: Copy + 'a, const N: usize, U = f32>").collect();
        assert_eq!(names, ["T", "N", "U"]);
        assert_eq!(split_generics("<T> X<T>"), ("<T>", "X<T>"));
        assert_eq!(split_generics("X"), ("", "X"));
    }
}
