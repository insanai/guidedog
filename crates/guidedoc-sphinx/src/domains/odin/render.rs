//! Rendering what follows an Odin object's name: procedures, records, types, values,
//! and fields, with type names as cross-references.

use guidedoc::{Fail, NodeKind, SourceSpan, is_ascii_space};

use super::scan::{close, find, first_word, name_end, results_end, skip_quote};
use super::{is_private, mapped};
use crate::domains::{Cx, XrefParts};
use crate::text::{has_at, is_identifier_byte, trim_space};

/// Odin's built-in types: they are never links.
const BUILTINS: &[&str] = &[
    "bool",
    "b8",
    "b16",
    "b32",
    "b64",
    "int",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "uint",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "uintptr",
    "i16le",
    "i32le",
    "i64le",
    "i128le",
    "u16le",
    "u32le",
    "u64le",
    "u128le",
    "i16be",
    "i32be",
    "i64be",
    "i128be",
    "u16be",
    "u32be",
    "u64be",
    "u128be",
    "f16",
    "f32",
    "f64",
    "f16le",
    "f32le",
    "f64le",
    "f16be",
    "f32be",
    "f64be",
    "complex32",
    "complex64",
    "complex128",
    "quaternion64",
    "quaternion128",
    "quaternion256",
    "rune",
    "string",
    "cstring",
    "string16",
    "cstring16",
    "rawptr",
    "typeid",
    "any",
    "byte",
    "Maybe",
];

/// Odin's keywords and built-in procedures: they are never links.
const KEYWORDS: &[&str] = &[
    "proc",
    "struct",
    "union",
    "enum",
    "bit_set",
    "bit_field",
    "map",
    "matrix",
    "dynamic",
    "distinct",
    "using",
    "context",
    "nil",
    "true",
    "false",
    "where",
    "or_else",
    "or_return",
    "cast",
    "transmute",
    "auto_cast",
    "in",
    "not_in",
    "size_of",
    "align_of",
    "offset_of",
    "type_of",
    "len",
    "cap",
    "min",
    "max",
    "import",
    "foreign",
    "package",
];

/// What rendering one signature needs besides the domain context: its span, its
/// polymorphic names, and those of the types it is nested in.
#[derive(Debug, Clone, Default)]
pub(super) struct SigCx<'s> {
    pub span: SourceSpan,
    polys: Vec<&'s str>,
    /// The enclosing types' polymorphic names, separated by spaces.
    outer: String,
}

impl<'s> SigCx<'s> {
    /// The context of `sig`, nested in types whose polymorphic names are `outer`: it
    /// notes the names the signature declares with "$".
    pub(super) fn new(sig: &'s str, span: SourceSpan, outer: String) -> SigCx<'s> {
        let b = sig.as_bytes();
        let mut polys = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if b[i] != b'$' {
                i += 1;
                continue;
            }
            let mut k = i + 1;
            while k < b.len() && is_identifier_byte(b[k]) {
                k += 1;
            }
            if k > i + 1 {
                polys.push(&sig[i + 1..k]);
            }
            i = k;
        }
        SigCx { span, polys, outer }
    }

    /// Tells whether `word` is a polymorphic name here.
    fn is_poly(&self, word: &str) -> bool {
        self.polys.contains(&word) || self.outer.split(' ').any(|name| name == word)
    }

    /// Returns the polymorphic names a type's members see: the outer ones and its own.
    pub(super) fn polys_text(&self) -> String {
        let mut text = self.outer.clone();
        for name in &self.polys {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(name);
        }
        text
    }
}

/// Emits two text leaves: `before` and `text`.
fn plain(cx: &mut Cx<'_, '_, '_>, c: &SigCx<'_>, before: &str, text: &str) -> Result<(), Fail> {
    cx.text_leaf(c.span, before)?;
    cx.text_leaf(c.span, text)
}

/// Emits what follows the name, by its first characters.
pub(super) fn rest(
    cx: &mut Cx<'_, '_, '_>,
    c: &SigCx<'_>,
    objtype: &str,
    r: &str,
) -> Result<(), Fail> {
    let span = c.span;
    if r.is_empty() {
        let keyword = match objtype {
            "procedure" => {
                cx.leaf_in(NodeKind::DescAnnotation, span, " :: proc")?;
                return cx.leaf_in(NodeKind::DescParameterlist, span, "");
            }
            "struct" => " :: struct",
            "union" => " :: union",
            "enum" => " :: enum",
            "bitset" => " :: bit_set",
            "bitfield" => " :: bit_field",
            _ => return Ok(()),
        };
        return cx.leaf_in(NodeKind::DescAnnotation, span, keyword);
    }
    if r.starts_with('(') {
        return match objtype {
            "procedure" => {
                cx.leaf_in(NodeKind::DescAnnotation, span, " :: proc")?;
                procedure(cx, c, r)
            }
            "struct" | "union" => {
                cx.leaf_in(NodeKind::DescAnnotation, span, &format!(" :: {objtype}"))?;
                record(cx, c, r)
            }
            _ => plain(cx, c, " ", r),
        };
    }
    if let Some(v) = r.strip_prefix("::") {
        return value(cx, c, objtype, trim_space(v));
    }
    if let Some(v) = r.strip_prefix(":=") {
        return plain(cx, c, " := ", trim_space(v));
    }
    if let Some(v) = r.strip_prefix(':') {
        return typed(cx, c, trim_space(v));
    }
    if let Some(v) = r.strip_prefix('=') {
        return plain(cx, c, " = ", trim_space(v));
    }
    plain(cx, c, " ", r)
}

/// Emits ": T" and what may follow the type: " = value" (a variable), " : value" (a
/// typed constant), a field's tag, or a bit field's size.
fn typed(cx: &mut Cx<'_, '_, '_>, c: &SigCx<'_>, r: &str) -> Result<(), Fail> {
    cx.text_leaf(c.span, ": ")?;
    let end = find(r, b'=').min(find(r, b':')).min(find(r, b'`')).min(find(r, b'|'));
    odin_type(cx, c, trim_space(&r[..end]), "")?;
    let after = trim_space(&r[end..]);
    if !after.is_empty() {
        plain(cx, c, " ", after)?;
    }
    Ok(())
}

/// Emits " :: " and a declaration's value: a procedure, a procedure group, a record or
/// other type, or a constant's value as written.
fn value(cx: &mut Cx<'_, '_, '_>, c: &SigCx<'_>, objtype: &str, v: &str) -> Result<(), Fail> {
    if matches!(objtype, "const" | "enumerator" | "var") {
        return plain(cx, c, " :: ", v);
    }
    let (mut word, mut after) = first_word(v);
    let mut lead = "";
    if matches!(word, "#force_inline" | "#force_no_inline" | "#type") {
        lead = word;
        (word, after) = first_word(after);
    }
    let head = if lead.is_empty() { format!(" :: {word}") } else { format!(" :: {lead} {word}") };
    let span = c.span;
    match word {
        "proc" => {
            cx.leaf_in(NodeKind::DescAnnotation, span, &head)?;
            if after.starts_with('{') { group(cx, c, after) } else { procedure(cx, c, after) }
        }
        "struct" | "union" => {
            cx.leaf_in(NodeKind::DescAnnotation, span, &head)?;
            record(cx, c, after)
        }
        "enum" | "bit_field" | "distinct" => {
            cx.leaf_in(NodeKind::DescAnnotation, span, &head)?;
            if !after.is_empty() {
                cx.text_leaf(span, " ")?;
            }
            odin_type(cx, c, after, "")
        }
        "bit_set" => {
            cx.leaf_in(NodeKind::DescAnnotation, span, &head)?;
            odin_type(cx, c, after, "bit_set")
        }
        _ => {
            cx.text_leaf(span, " :: ")?;
            odin_type(cx, c, v, "")
        }
    }
}

/// Emits a procedure group's members, "{a, b}", each a cross-reference to a procedure
/// unless the package keeps it private.
fn group(cx: &mut Cx<'_, '_, '_>, c: &SigCx<'_>, r: &str) -> Result<(), Fail> {
    let span = c.span;
    let Some(end) = close(r, 0) else { return cx.text_leaf(span, r) };
    cx.text_leaf(span, "{")?;
    let mut inner = &r[1..end];
    let mut first = true;
    while !inner.is_empty() {
        let k = find(inner, b',');
        let member = trim_space(&inner[..k]);
        inner = &inner[(k + 1).min(inner.len())..];
        if member.is_empty() {
            continue;
        }
        if !first {
            cx.text_leaf(span, ", ")?;
        }
        first = false;
        if is_private(cx.st, member) {
            cx.text_leaf(span, member)?;
        } else {
            xref(cx, c, member, "odin:proc")?;
        }
    }
    cx.text_leaf(span, &r[end..])
}

/// Emits a procedure's calling convention, parameters, results, tags, and where
/// clause; `r` starts after "proc".
fn procedure(cx: &mut Cx<'_, '_, '_>, c: &SigCx<'_>, r: &str) -> Result<(), Fail> {
    let span = c.span;
    let mut rest = trim_space(r);
    if rest.starts_with('"') {
        let k = skip_quote(rest.as_bytes(), 0);
        cx.text_leaf(span, &format!(" {} ", &rest[..k]))?;
        rest = trim_space(&rest[k..]);
    }
    if !rest.starts_with('(') {
        cx.leaf_in(NodeKind::DescParameterlist, span, "")?;
        if !rest.is_empty() {
            plain(cx, c, " ", rest)?;
        }
        return Ok(());
    }
    let Some(end) = close(rest, 0) else { return plain(cx, c, "", rest) };
    fields(cx, c, &rest[1..end], true)?;
    rest = trim_space(&rest[end + 1..]);
    if let Some(after) = rest.strip_prefix("->") {
        cx.text_leaf(span, " -> ")?;
        let after = trim_space(after);
        let end = results_end(after);
        let results = trim_space(&after[..end]);
        let list = results.len() > 1
            && results.starts_with('(')
            && close(results, 0) == Some(results.len() - 1);
        if list {
            fields(cx, c, &results[1..results.len() - 1], true)?;
        } else {
            odin_type(cx, c, results, "")?;
        }
        rest = trim_space(&after[end..]);
    }
    if !rest.is_empty() {
        plain(cx, c, " ", rest)?;
    }
    Ok(())
}

/// Emits a struct's or union's polymorphic parameters, then its directives, variants,
/// and where clause.
fn record(cx: &mut Cx<'_, '_, '_>, c: &SigCx<'_>, r: &str) -> Result<(), Fail> {
    let mut rest = trim_space(r);
    if rest.starts_with('(')
        && let Some(end) = close(rest, 0)
    {
        fields(cx, c, &rest[1..end], true)?;
        rest = trim_space(&rest[end + 1..]);
    }
    if rest.is_empty() {
        return Ok(());
    }
    cx.text_leaf(c.span, " ")?;
    odin_type(cx, c, rest, "")
}

/// Emits a parameter or field list. As in Odin, when any item has a type (":"), items
/// without one are names ("a, b: int"); otherwise every item is a type ("(int,
/// bool)"). With `nodes`, the list is a DescParameterlist of DescParameters; without,
/// text in a type.
fn fields(cx: &mut Cx<'_, '_, '_>, c: &SigCx<'_>, text: &str, nodes: bool) -> Result<(), Fail> {
    let span = c.span;
    let named = items(text).any(|item| find(item, b':') < item.len());
    let list = if nodes {
        Some(cx.b().begin_node(NodeKind::DescParameterlist, span)?)
    } else {
        cx.text_leaf(span, "(")?;
        None
    };
    for (i, part) in items(text).map(trim_space).filter(|p| !p.is_empty()).enumerate() {
        if !nodes && i > 0 {
            cx.text_leaf(span, ", ")?;
        }
        let param =
            if nodes { Some(cx.b().begin_node(NodeKind::DescParameter, span)?) } else { None };
        field(cx, c, part, named)?;
        if let Some(param) = param {
            cx.b().end_node(param)?;
        }
    }
    match list {
        Some(list) => cx.b().end_node(list),
        None => cx.text_leaf(span, ")"),
    }
}

/// Iterates over the items of a list separated by commas outside brackets.
fn items(text: &str) -> impl Iterator<Item = &str> {
    let mut rest = text;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let k = find(rest, b',');
        let item = &rest[..k];
        rest = &rest[(k + 1).min(rest.len())..];
        Some(item)
    })
}

/// Emits one parameter: "name: T = default", "name := default", a name, or a type.
fn field(cx: &mut Cx<'_, '_, '_>, c: &SigCx<'_>, part: &str, named: bool) -> Result<(), Fail> {
    if !named {
        return odin_type(cx, c, part, "");
    }
    let colon = find(part, b':');
    if colon >= part.len() {
        return cx.text_leaf(c.span, part);
    }
    cx.text_leaf(c.span, trim_space(&part[..colon]))?;
    let after = &part[colon + 1..];
    if let Some(default) = after.strip_prefix('=') {
        return plain(cx, c, " := ", trim_space(default));
    }
    cx.text_leaf(c.span, ": ")?;
    let eq = find(after, b'=');
    odin_type(cx, c, trim_space(&after[..eq]), "")?;
    if eq < after.len() {
        plain(cx, c, " = ", trim_space(&after[eq + 1..]))?;
    }
    Ok(())
}

/// Emits a type as written, with its type names as cross-references. A procedure
/// type's parameters are read as a list; from "where" on the text is shown as written,
/// and so are an array's length and an enumeration's names, which are values. `after`
/// is the word before the text, when it has one ("bit_set").
fn odin_type(cx: &mut Cx<'_, '_, '_>, c: &SigCx<'_>, text: &str, after: &str) -> Result<(), Fail> {
    let b = text.as_bytes();
    let (mut pos, mut i) = (0, 0);
    // The word before a bracket tells what it holds.
    let mut last = after;
    let mut in_enum = false;
    while i < b.len() {
        let ch = b[i];
        if matches!(ch, b'"' | b'`' | b'\'') {
            i = skip_quote(b, i);
        } else if ch == b'#' || ch == b'$' || ch.is_ascii_digit() {
            i += 1;
            while i < b.len() && is_identifier_byte(b[i]) {
                i += 1;
            }
            last = "";
        } else if (ch == b'[' && last != "bit_set" && last != "map") || (ch == b'{' && in_enum) {
            i = close(text, i).map_or(b.len(), |end| end + 1);
            in_enum = in_enum && ch != b'{';
        } else if is_identifier_byte(ch) {
            let end = name_end(b, i);
            let word = &text[i..end];
            last = word;
            match word {
                "where" => break,
                "enum" => in_enum = true,
                "proc" => {
                    cx.text_leaf(c.span, &text[pos..end])?;
                    pos = proc_type(cx, c, text, end)?;
                    i = pos;
                    continue;
                }
                _ => {}
            }
            if linkable(cx, c, text, (i, end), word) {
                cx.text_leaf(c.span, &text[pos..i])?;
                xref(cx, c, word, "odin:type")?;
                pos = end;
            }
            i = end;
        } else {
            i += 1;
            if !is_ascii_space(ch) {
                last = "";
            }
        }
    }
    cx.text_leaf(c.span, &text[pos..])
}

/// Emits a procedure type's convention and parameter list, starting at `from` (after
/// "proc"), and returns where the rest of the type continues.
fn proc_type(
    cx: &mut Cx<'_, '_, '_>,
    c: &SigCx<'_>,
    text: &str,
    from: usize,
) -> Result<usize, Fail> {
    let b = text.as_bytes();
    let skip_space = |mut k: usize| {
        while k < b.len() && is_ascii_space(b[k]) {
            k += 1;
        }
        k
    };
    let mut k = skip_space(from);
    if b.get(k) == Some(&b'"') {
        k = skip_quote(b, k);
    }
    k = skip_space(k);
    if b.get(k) != Some(&b'(') {
        return Ok(from);
    }
    let Some(end) = close(text, k) else { return Ok(from) };
    cx.text_leaf(c.span, &text[from..k])?;
    fields(cx, c, &text[k + 1..end], false)?;
    Ok(end + 1)
}

/// Tells whether a word of a type names a type to link: not a keyword, a built-in
/// type, a private type, a polymorphic name, a field's name ("a:"), or an implicit
/// selector (".A").
fn linkable(
    cx: &Cx<'_, '_, '_>,
    c: &SigCx<'_>,
    text: &str,
    at: (usize, usize),
    word: &str,
) -> bool {
    if KEYWORDS.contains(&word) || BUILTINS.contains(&word) {
        return false;
    }
    if is_private(cx.st, word) || c.is_poly(word) {
        return false;
    }
    let before =
        text[..at.0].trim_end_matches(|ch: char| ch.is_ascii() && is_ascii_space(ch as u8));
    if before.ends_with('.') {
        return false;
    }
    let after =
        text[at.1..].trim_start_matches(|ch: char| ch.is_ascii() && is_ascii_space(ch as u8));
    !(after.starts_with(':') && !has_at(after, 0, "::"))
}

/// Emits a cross-reference to `name`; an import alias the package declares is followed
/// to the package it names ("gd.Node_Id" finds "core.Node_Id").
fn xref(cx: &mut Cx<'_, '_, '_>, c: &SigCx<'_>, name: &str, reftype: &str) -> Result<(), Fail> {
    let target = mapped(cx.st, name).unwrap_or_else(|| name.to_owned());
    let x =
        XrefParts { title: name.into(), target: target.into(), explicit: false, specific: false };
    cx.xref(c.span, reftype, "odin", &x, false)
}
