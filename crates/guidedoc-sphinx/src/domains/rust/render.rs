//! Rendering what follows a Rust object's name: generics, parameters, return types,
//! bounds, fields and values, with the paths in type positions as cross-references.

use guidedoc::{Fail, NodeKind, SourceSpan, is_ascii_space};

use super::resolve_path;
use super::scan::{
    close, find_assign, find_byte, find_colon, find_word, generic_names, ident_end, skip_quoted,
    split_generics,
};
use crate::domains::{Cx, XrefParts};
use crate::text::{is_identifier_byte, trim_space};

/// Rust's primitive types: they are never links.
const PRIMITIVES: &[&str] = &[
    "bool", "char", "str", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64",
    "i128", "isize", "f16", "f32", "f64", "f128",
];

/// Rust's keywords (and `_`): they are never links.
const KEYWORDS: &[&str] = &[
    "_",
    "as",
    "async",
    "auto",
    "await",
    "const",
    "crate",
    "default",
    "dyn",
    "else",
    "enum",
    "extern",
    "false",
    "fn",
    "for",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "macro_rules",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "return",
    "safe",
    "self",
    "Self",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "type",
    "union",
    "unsafe",
    "use",
    "where",
    "while",
];

/// How deep array types (`[[T; 2]; 3]`) are read before the rest is shown as text.
const MAX_ARRAY_DEPTH: usize = 16;

/// The reftype of the paths a signature links: any Rust object may be named there.
pub(super) const SIGNATURE_REFTYPE: &str = "rust:any";

/// What rendering one signature needs besides the domain context: its span, the generic
/// names it declares, and those of the types and impls it is nested in.
#[derive(Debug, Clone, Default)]
pub(super) struct SigCx<'s> {
    pub span: SourceSpan,
    generics: Vec<&'s str>,
    /// The enclosing scopes' generic names, separated by spaces.
    outer: String,
}

impl<'s> SigCx<'s> {
    /// The context of a signature nested in scopes whose generic names are `outer`.
    pub(super) fn new(span: SourceSpan, outer: String) -> SigCx<'s> {
        SigCx { span, generics: Vec::new(), outer }
    }

    /// Notes the names a generic parameter list declares.
    pub(super) fn declare(&mut self, list: &'s str) {
        self.generics.extend(generic_names(list));
    }

    fn is_generic(&self, word: &str) -> bool {
        self.generics.contains(&word) || self.outer.split(' ').any(|name| name == word)
    }

    /// Returns the generic names a scope's members see: the outer ones and its own.
    pub(super) fn generics_text(&self) -> String {
        let mut text = self.outer.clone();
        for name in &self.generics {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(name);
        }
        text
    }
}

/// Emits what follows the name of an object of type `objtype`.
pub(super) fn rest<'s>(
    cx: &mut Cx<'_, '_, '_>,
    c: &mut SigCx<'s>,
    objtype: &str,
    rest: &'s str,
) -> Result<(), Fail> {
    match objtype {
        "function" | "method" => function(cx, c, rest),
        "field" => typed(cx, c, rest, false),
        "const" | "static" | "associatedconst" => typed(cx, c, rest, true),
        "variant" => variant(cx, c, rest),
        "macro" => separated(cx, c, rest, false),
        "use" => Ok(()),
        _ => item(cx, c, rest),
    }
}

/// Emits a function's generics, parameters, return type and where clause.
fn function<'s>(cx: &mut Cx<'_, '_, '_>, c: &mut SigCx<'s>, rest: &'s str) -> Result<(), Fail> {
    let (generics, mut r) = split_generics(rest);
    c.declare(generics);
    type_text(cx, c, generics)?;
    match r.starts_with('(').then(|| close(r, 0)).flatten() {
        Some(end) => {
            parameters(cx, c, &r[1..end])?;
            r = trim_space(&r[end + 1..]);
        }
        None => cx.leaf_in(NodeKind::DescParameterlist, c.span, "")?,
    }
    if let Some(after) = r.strip_prefix("->") {
        let after = trim_space(after);
        let end = find_word(after, "where");
        cx.text_leaf(c.span, " -> ")?;
        type_text(cx, c, trim_space(&after[..end]))?;
        r = &after[end..];
    }
    separated(cx, c, r, true)
}

/// Emits a parameter list as DescParameterlist > DescParameter per parameter:
/// "pattern: Type", or `self` in its forms.
fn parameters(cx: &mut Cx<'_, '_, '_>, c: &SigCx<'_>, text: &str) -> Result<(), Fail> {
    let list = cx.b().begin_node(NodeKind::DescParameterlist, c.span)?;
    for part in items(text).map(trim_space).filter(|p| !p.is_empty()) {
        let param = cx.b().begin_node(NodeKind::DescParameter, c.span)?;
        let colon = find_colon(part);
        if colon < part.len() {
            cx.text_leaf(c.span, trim_space(&part[..colon]))?;
            cx.text_leaf(c.span, ": ")?;
            type_text(cx, c, trim_space(&part[colon + 1..]))?;
        } else {
            type_text(cx, c, part)?;
        }
        cx.b().end_node(param)?;
    }
    cx.b().end_node(list)
}

/// Iterates over the items of a list separated by commas outside brackets.
fn items(text: &str) -> impl Iterator<Item = &str> {
    let mut rest = text;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let k = find_byte(rest, b',');
        let item = &rest[..k];
        rest = &rest[(k + 1).min(rest.len())..];
        Some(item)
    })
}

/// Emits ": Type" and, for constants and statics, " = value" as written.
fn typed(cx: &mut Cx<'_, '_, '_>, c: &SigCx<'_>, rest: &str, valued: bool) -> Result<(), Fail> {
    let Some(r) = rest.strip_prefix(':') else { return separated(cx, c, rest, false) };
    let end = if valued { find_assign(r) } else { r.len() };
    cx.text_leaf(c.span, ": ")?;
    type_text(cx, c, trim_space(&r[..end]))?;
    if end < r.len() {
        cx.text_leaf(c.span, " = ")?;
        cx.text_leaf(c.span, trim_space(&r[end + 1..]))?;
    }
    Ok(())
}

/// Emits a variant's fields, "(u8)" or "{ width: f32 }", or its discriminant "= 9".
fn variant(cx: &mut Cx<'_, '_, '_>, c: &SigCx<'_>, rest: &str) -> Result<(), Fail> {
    if let Some(value) = rest.strip_prefix('=') {
        cx.text_leaf(c.span, " = ")?;
        return cx.text_leaf(c.span, trim_space(value));
    }
    if rest.starts_with('(') {
        return type_text(cx, c, rest);
    }
    separated(cx, c, rest, true)
}

/// Emits a type's generics and what follows them: a tuple struct's fields, bounds
/// (": Clone + Send"), an alias's type ("= Type"), or a where clause. An impl's text
/// after `impl` reads the same way.
fn item<'s>(cx: &mut Cx<'_, '_, '_>, c: &mut SigCx<'s>, rest: &'s str) -> Result<(), Fail> {
    let (generics, r) = split_generics(rest);
    c.declare(generics);
    type_text(cx, c, generics)?;
    if r.starts_with('(') {
        return type_text(cx, c, r);
    }
    if let Some(bounds) = r.strip_prefix(':') {
        let end = find_assign(bounds);
        cx.text_leaf(c.span, ": ")?;
        type_text(cx, c, trim_space(&bounds[..end]))?;
        return aliased(cx, c, &bounds[end..]);
    }
    if r.starts_with('=') {
        return aliased(cx, c, r);
    }
    separated(cx, c, r, true)
}

/// Emits " = Type" and a where clause for text starting at "=", or nothing.
fn aliased(cx: &mut Cx<'_, '_, '_>, c: &SigCx<'_>, r: &str) -> Result<(), Fail> {
    let Some(aliased) = r.strip_prefix('=') else { return separated(cx, c, r, true) };
    cx.text_leaf(c.span, " = ")?;
    type_text(cx, c, trim_space(aliased))
}

/// Emits " " and `text`, with links when `linked`; nothing for empty text.
fn separated(
    cx: &mut Cx<'_, '_, '_>,
    c: &SigCx<'_>,
    text: &str,
    linked: bool,
) -> Result<(), Fail> {
    let text = trim_space(text);
    if text.is_empty() {
        return Ok(());
    }
    cx.text_leaf(c.span, " ")?;
    if linked { type_text(cx, c, text) } else { cx.text_leaf(c.span, text) }
}

/// Emits a type, bounds, or a where clause as written, with its paths as
/// cross-references. Keywords, primitive types, generic names, lifetimes, numbers and
/// names that are declared rather than used ("x:" and "Item =") stay text, and so do
/// array lengths, which are values.
pub(super) fn type_text(cx: &mut Cx<'_, '_, '_>, c: &SigCx<'_>, text: &str) -> Result<(), Fail> {
    type_text_at(cx, c, text, 0)
}

fn type_text_at(
    cx: &mut Cx<'_, '_, '_>,
    c: &SigCx<'_>,
    text: &str,
    depth: usize,
) -> Result<(), Fail> {
    let b = text.as_bytes();
    let (mut pos, mut i) = (0, 0);
    while i < b.len() {
        let ch = b[i];
        if matches!(ch, b'"' | b'\'') {
            i = skip_quoted(b, i);
        } else if ch.is_ascii_digit() {
            while i < b.len() && is_identifier_byte(b[i]) {
                i += 1;
            }
        } else if ch == b'[' && depth < MAX_ARRAY_DEPTH {
            let Some(end) = close(text, i) else { break };
            let inner = &text[i + 1..end];
            let semi = find_byte(inner, b';');
            if semi < inner.len() {
                cx.text_leaf(c.span, &text[pos..=i])?;
                type_text_at(cx, c, &inner[..semi], depth + 1)?;
                pos = i + 1 + semi;
                i = end + 1;
            } else {
                i += 1;
            }
        } else if is_identifier_byte(ch) {
            let end = path_end(b, i);
            if linkable(c, text, (i, end)) {
                cx.text_leaf(c.span, &text[pos..i])?;
                xref(cx, c, &text[i..end])?;
                pos = end;
            }
            i = end;
        } else {
            i += 1;
        }
    }
    cx.text_leaf(c.span, &text[pos..])
}

/// Returns the end of the path starting at `i`: identifiers joined by `::`.
fn path_end(b: &[u8], i: usize) -> usize {
    let mut end = ident_end(b, i).max(i + 1);
    while b.get(end) == Some(&b':') && b.get(end + 1) == Some(&b':') {
        let next = ident_end(b, end + 2);
        if next == end + 2 {
            break;
        }
        end = next;
    }
    end
}

/// Tells whether the path `text[at.0..at.1]` names something to link.
fn linkable(c: &SigCx<'_>, text: &str, at: (usize, usize)) -> bool {
    let path = &text[at.0..at.1];
    let first = path.split("::").next().unwrap_or(path);
    let single = first.len() == path.len();
    if single && (KEYWORDS.contains(&path) || PRIMITIVES.contains(&path)) {
        return false;
    }
    if c.is_generic(first) || path == "Self" {
        return false;
    }
    let after =
        text[at.1..].trim_start_matches(|ch: char| ch.is_ascii() && is_ascii_space(ch as u8));
    let a = after.as_bytes();
    let declared_name = match a.first() {
        Some(b':') => a.get(1) != Some(&b':'),
        Some(b'=') => !matches!(a.get(1), Some(b'=' | b'>')),
        Some(b'!') => true,
        _ => false,
    };
    !declared_name
}

/// Emits a cross-reference to `path`, its target resolved from the current module.
fn xref(cx: &mut Cx<'_, '_, '_>, c: &SigCx<'_>, path: &str) -> Result<(), Fail> {
    let target = resolve_path(cx.st, path).unwrap_or_else(|| path.to_owned());
    let x =
        XrefParts { title: path.into(), target: target.into(), explicit: false, specific: false };
    cx.xref(c.span, SIGNATURE_REFTYPE, "rust", &x, false)
}
