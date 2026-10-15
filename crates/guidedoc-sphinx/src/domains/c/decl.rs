//! Finding the declared name, parameters, and the rest of a C declaration. Plain
//! declarators are scanned; parenthesized ones ("int (*visitproc)(PyObject *, void *)")
//! are parsed as Sphinx parses them: pointer operators, then either a name with its
//! suffixes or "( declarator )" followed by suffixes.

use guidedoc::is_ascii_space;

use super::c_scope_kind;
use crate::text::{has_at, is_identifier_byte, trim_space};

/// A declaration split around its declared name.
#[derive(Debug, Default)]
pub(crate) struct CDecl<'s> {
    /// Text before the name (return type, keywords).
    pub before: &'s str,
    /// "S." written before the name.
    pub qualifier: &'s str,
    /// The declared name.
    pub name: &'s str,
    /// Inside the parameter list's parentheses.
    pub params: Option<&'s str>,
    /// Suffixes of a parenthesized declarator: ")" and parameters.
    pub mid: &'s str,
    /// Text after the name or the parameter list.
    pub after: &'s str,
}

/// Returns the index of the bracket closing `s[i]`, or `s.len()`.
pub(crate) fn find_close(s: &str, i: usize) -> usize {
    let b = s.as_bytes();
    let mut depth = 0i32;
    for k in i..b.len() {
        match b[k] {
            b'(' | b'[' | b'{' | b'<' => depth += 1,
            b')' | b']' | b'}' | b'>' => {
                if b[k] == b'>' && k > 0 && b[k - 1] == b'-' {
                    continue;
                }
                depth -= 1;
                if depth == 0 {
                    return k;
                }
            }
            _ => {}
        }
    }
    b.len()
}

/// Returns the start of the qualified name that ends at `end`; with dots (C),
/// "S.member" is one name.
fn qualified_before(s: &str, end: usize, dots: bool) -> usize {
    let b = s.as_bytes();
    let mut i = end;
    while i > 0 {
        let c = b[i - 1];
        if is_identifier_byte(c)
            || c == b'~'
            || c == b'@'
            || (dots && c == b'.' && i >= 2 && is_identifier_byte(b[i - 2]))
        {
            i -= 1;
            continue;
        }
        if c == b':' && i >= 2 && b[i - 2] == b':' {
            i -= 2;
            continue;
        }
        if c == b'>' {
            let mut depth = 0;
            let mut k = i as isize - 1;
            while k >= 0 {
                match b[k as usize] {
                    b'>' => depth += 1,
                    b'<' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                k -= 1;
            }
            if k <= 0 {
                break;
            }
            i = k as usize;
            continue;
        }
        break;
    }
    i
}

/// Finds the declared name of a declaration of an object type.
pub(crate) fn parse_c_decl<'s>(sig: &'s str, objtype: &str) -> Option<CDecl<'s>> {
    let t = trim_space(sig);
    let mut d = CDecl::default();
    let (start, end);
    let paren = c_paren_declarator(t, objtype);
    if let Some(dd) = &paren {
        (start, end) = (dd.name_start, dd.name_end);
        let mut tail = dd.name_end;
        if let Some((open, close)) = dd.params {
            d.params = Some(t.get(open + 1..close)?);
            tail = close + 1;
        }
        (d.mid, d.after) = (t.get(tail..dd.end)?, &t[dd.end..]);
    } else if c_scope_kind(objtype) {
        (start, end) = scope_name_range(t);
    } else {
        (start, end) = plain_name_range(t, objtype, &mut d)?;
    }
    if start >= end {
        return None;
    }
    let name = &t[start..end];
    let split =
        name.bytes().enumerate().rfind(|&(_, c)| c == b'.').map_or(0, |(i, _)| i + 1);
    let split = name.rfind("::").map_or(split, |i| (i + 2).max(split));
    (d.before, d.qualifier, d.name) = (&t[..start], &name[..split], &name[split..]);
    if d.params.is_none() && paren.is_none() {
        d.after = &t[end..];
    }
    (!d.name.is_empty()).then_some(d)
}

/// The name of a struct, union, or enum: after its keywords; it may be nested ("a.b"),
/// its parts anonymous ("@a").
fn scope_name_range(t: &str) -> (usize, usize) {
    let b = t.as_bytes();
    let mut i = 0;
    loop {
        let k = (i..b.len()).find(|&k| !is_identifier_byte(b[k])).unwrap_or(b.len());
        if !matches!(&t[i..k], "class" | "struct" | "union" | "enum" | "concept") {
            break;
        }
        i = k;
        while i < b.len() && is_ascii_space(b[i]) {
            i += 1;
        }
    }
    let mut end = i;
    while end < b.len() {
        let c = b[end];
        if !is_identifier_byte(c) && c != b'@' && c != b'.' && !has_at(t, end, "::") {
            break;
        }
        end += if c == b':' { 2 } else { 1 };
    }
    (i, end.min(b.len()))
}

/// The name of a function (before its parameter list) or of another declaration (before
/// "=", "[", or ":"); fills the parameters and what follows.
fn plain_name_range<'s>(t: &'s str, objtype: &str, d: &mut CDecl<'s>) -> Option<(usize, usize)> {
    let b = t.as_bytes();
    let paren = top_paren(t);
    let takes_params = !matches!(objtype, "member" | "var" | "type" | "enumerator");
    if let Some(paren) = paren.filter(|_| takes_params) {
        let mut end = paren;
        while end > 0 && is_ascii_space(b[end - 1]) {
            end -= 1;
        }
        let start = operator_start(t, end).unwrap_or_else(|| qualified_before(t, end, true));
        let close = find_close(t, paren);
        if close >= b.len() {
            return None;
        }
        d.params = Some(&t[paren + 1..close]);
        d.after = &t[close + 1..];
        return Some((start, end));
    }
    let mut end = b.iter().position(|&c| c == b'=' || c == b'[' || c == b':').unwrap_or(b.len());
    while end > 0 && is_ascii_space(b[end - 1]) {
        end -= 1;
    }
    let start = qualified_before(t, end, true);
    d.after = &t[end..];
    Some((start, end))
}

fn top_paren(t: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, c) in t.bytes().enumerate() {
        match c {
            b'<' | b'[' | b'{' => depth += 1,
            b'>' | b']' | b'}' => depth -= 1,
            b'(' if depth <= 0 => return Some(i),
            _ => {}
        }
    }
    None
}

/// Returns the start of "operator..." ending at `end`.
fn operator_start(t: &str, end: usize) -> Option<usize> {
    let k = t[..end].find("operator")?;
    Some(qualified_before(t, k, false))
}

/// Tells whether `word` occurs in `s` as a word (its first occurrence).
pub(crate) fn has_word(s: &str, word: &str) -> bool {
    let Some(k) = s.find(word) else { return false };
    let b = s.as_bytes();
    let before = k == 0 || !is_identifier_byte(b[k - 1]);
    let after = k + word.len() >= b.len() || !is_identifier_byte(b[k + word.len()]);
    before && after
}

/// A parenthesized declarator: the declared name, the parameter list right after it,
/// and the end of the whole declarator.
#[derive(Debug, Default)]
struct CDeclarator {
    name_start: usize,
    name_end: usize,
    params: Option<(usize, usize)>,
    end: usize,
}

const C_SIMPLE_SPECS: [&str; 14] = [
    "static",
    "extern",
    "register",
    "_Thread_local",
    "thread_local",
    "inline",
    "restrict",
    "volatile",
    "const",
    "mutable",
    "constexpr",
    "virtual",
    "explicit",
    "friend",
];

const C_FUNDAMENTAL_WORDS: [&str; 22] = [
    "void",
    "_Bool",
    "bool",
    "char",
    "int",
    "float",
    "double",
    "__int64",
    "__int128",
    "short",
    "long",
    "signed",
    "unsigned",
    "_Complex",
    "complex",
    "_Imaginary",
    "imaginary",
    "wchar_t",
    "char8_t",
    "char16_t",
    "char32_t",
    "auto",
];

fn skip_space(t: &str, i: usize) -> usize {
    let b = t.as_bytes();
    (i..b.len()).find(|&k| !is_ascii_space(b[k])).unwrap_or(b.len().max(i))
}

/// Returns the identifier starting at `i`, or "".
fn word_at(t: &str, i: usize) -> &str {
    let b = t.as_bytes();
    let mut k = i;
    if k < b.len() && b[k] == b'@' {
        k += 1; // an anonymous entity's name.
    }
    while k < b.len() && is_identifier_byte(b[k]) {
        k += 1;
    }
    t.get(i..k).unwrap_or("")
}

/// Skips storage classes and qualifiers from `i`.
fn simple_specs(t: &str, i: usize) -> usize {
    let mut k = skip_space(t, i);
    loop {
        let w = word_at(t, k);
        if !C_SIMPLE_SPECS.contains(&w) {
            return k;
        }
        k = skip_space(t, k + w.len());
    }
}

/// Returns the end of the (qualified) name at `i`: identifiers joined by "."; `i` when
/// there is none.
fn nested_name_end(t: &str, i: usize) -> usize {
    let b = t.as_bytes();
    let mut k = i;
    loop {
        let w = word_at(t, k);
        if w.is_empty() {
            return k;
        }
        k += w.len();
        if k + 1 < b.len() && b[k] == b'.' {
            k += 1;
        } else {
            return k;
        }
    }
}

/// Returns the end of the declaration specifiers that start `t`, and whether they name
/// a type (Sphinx's trailing type specifier): fundamental type words, or a name
/// optionally after struct, union, or enum.
fn decl_specs(t: &str) -> (usize, bool) {
    let mut i = simple_specs(t, 0);
    let mut w = word_at(t, i);
    if C_FUNDAMENTAL_WORDS.contains(&w) {
        while C_FUNDAMENTAL_WORDS.contains(&w) {
            i = skip_space(t, i + w.len());
            w = word_at(t, i);
        }
        return (simple_specs(t, i), true);
    }
    if matches!(w, "struct" | "union" | "enum") {
        i = skip_space(t, i + w.len());
    }
    let k = nested_name_end(t, i);
    if k == i {
        return (i, false);
    }
    (simple_specs(t, k), true)
}

/// Skips pointer operators ("*" and its qualifiers).
fn ptr_ops(t: &str, i: usize) -> usize {
    let b = t.as_bytes();
    let mut k = skip_space(t, i);
    let mut pointer = false;
    while k < b.len() {
        if b[k] == b'*' {
            pointer = true;
            k = skip_space(t, k + 1);
            continue;
        }
        let w = word_at(t, k);
        if !pointer || !matches!(w, "const" | "volatile" | "restrict") {
            break;
        }
        k = skip_space(t, k + w.len());
    }
    k
}

/// Returns the end of the parameter lists and array bounds from `i`; `None` when one is
/// unclosed.
fn suffixes(t: &str, i: usize) -> Option<usize> {
    let b = t.as_bytes();
    let mut end = i;
    loop {
        let k = skip_space(t, end);
        if k >= b.len() || (b[k] != b'(' && b[k] != b'[') {
            return Some(end);
        }
        let close = find_close(t, k);
        if close >= b.len() {
            return None;
        }
        end = close + 1;
    }
}

/// Parses a declarator at `i`, recursing into parentheses; returns its end.
fn declarator(t: &str, i: usize, dd: &mut CDeclarator, depth: usize) -> Option<usize> {
    if depth > 64 {
        return None;
    }
    let b = t.as_bytes();
    let k = ptr_ops(t, i);
    if k < b.len() && b[k] == b'(' {
        let inner = skip_space(t, declarator(t, k + 1, dd, depth + 1)?);
        if inner >= b.len() || b[inner] != b')' {
            return None;
        }
        return suffixes(t, inner + 1);
    }
    let name_end = nested_name_end(t, k);
    if name_end == k {
        return None;
    }
    (dd.name_start, dd.name_end, dd.params) = (k, name_end, None);
    let open = skip_space(t, name_end);
    if open < b.len() && b[open] == b'(' {
        dd.params = Some((open, find_close(t, open)));
    }
    suffixes(t, name_end)
}

/// Parses a declaration whose declarator is parenthesized, as "int (*visitproc)(PyObject
/// *, void *)" or "T (*const hash)(void)".
fn c_paren_declarator(t: &str, objtype: &str) -> Option<CDeclarator> {
    if !matches!(objtype, "function" | "member" | "var" | "type") {
        return None;
    }
    let (specs, typed) = decl_specs(t);
    if !typed {
        return None;
    }
    let open = ptr_ops(t, specs);
    if open >= t.len() || t.as_bytes()[open] != b'(' {
        return None;
    }
    let mut dd = CDeclarator::default();
    dd.end = declarator(t, specs, &mut dd, 0)?;
    Some(dd)
}

/// Tells whether a C type declaration has a type, which Sphinx shows as "typedef"; a
/// bare name is shown as "type".
pub(crate) fn c_typedef(sig: &str) -> bool {
    let t = trim_space(sig);
    let (end, typed) = decl_specs(t);
    typed && !trim_space(&t[end..]).is_empty()
}
