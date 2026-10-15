//! C declarations and expressions name other declarations, which Sphinx's C domain
//! links as it describes them. `c_linked` writes C text with every identifier that is
//! not a keyword, does not follow "." or "->" (a member), and is not the name being
//! declared as an Xref{reftype "c:identifier", Custom "c:parent" = the scope it is
//! looked up from}; the host resolves it from that scope outward and leaves it as text
//! when nothing is found. A function's parameter names are noted for the host too.

use guidedoc::{Builder, Fail, INVALID_NODE, LinkInfo, LinkKind, NodeId, NodeKind, SourceSpan};

use super::decl::find_close;
use crate::domains::text_leaf;
use crate::text::{is_identifier_byte, trim_space};

/// Writes C text with its names linked, looked up from `scope`; `declared` is the name
/// being declared, which stays text.
pub(crate) fn c_linked(
    b: &mut Builder<'_>,
    span: SourceSpan,
    text: &str,
    scope: &str,
    declared: &str,
) -> Result<(), Fail> {
    let t = text.as_bytes();
    let (mut plain, mut i) = (0, 0);
    while i < t.len() {
        let c = t[i];
        if c == b'"' || c == b'\'' {
            i = literal_end(t, i);
        } else if c.is_ascii_digit() {
            while i < t.len() && is_identifier_byte(t[i]) {
                i += 1;
            }
        } else if is_identifier_byte(c) || c == b'@' {
            let start = i;
            i += 1;
            while i < t.len() && is_identifier_byte(t[i]) {
                i += 1;
            }
            let word = &text[start..i];
            if !linkable(t, start, word, declared) {
                continue;
            }
            text_leaf(b, span, &text[plain..start])?;
            c_identifier_xref(b, span, word, word, scope)?;
            plain = i;
        } else {
            i += 1;
        }
    }
    text_leaf(b, span, &text[plain..])
}

/// Writes `shown` as a cross-reference to `target`, looked up from `scope`.
pub(crate) fn c_identifier_xref(
    b: &mut Builder<'_>,
    span: SourceSpan,
    target: &str,
    shown: &str,
    scope: &str,
) -> Result<(), Fail> {
    let id = b.begin_node(NodeKind::Xref, span)?;
    let name = b.add_text(target)?;
    b.set_link(
        id,
        LinkInfo {
            name,
            kind: LinkKind::Unresolved,
            resolved: INVALID_NODE,
            ..LinkInfo::UNRESOLVED
        },
    )?;
    b.add_custom_attribute(id, "reftype", "c:identifier")?;
    b.add_custom_attribute(id, "unstyled", "1")?;
    if !scope.is_empty() {
        b.add_custom_attribute(id, "c:parent", scope)?;
    }
    b.add_text_leaf(NodeKind::Text, span, shown)?;
    b.end_node(id)
}

fn linkable(t: &[u8], start: usize, word: &str, declared: &str) -> bool {
    if word == declared || word.starts_with('@') || c_reserved(word) {
        return false;
    }
    let mut j = start;
    while j > 0 && (t[j - 1] == b' ' || t[j - 1] == b'\t') {
        j -= 1;
    }
    if j > 0 && t[j - 1] == b'.' {
        return false;
    }
    !(j > 1 && t[j - 2] == b'-' && t[j - 1] == b'>')
}

fn literal_end(t: &[u8], i: usize) -> usize {
    let quote = t[i];
    let mut j = i + 1;
    while j < t.len() && t[j] != quote {
        if t[j] == b'\\' {
            j += 1;
        }
        j += 1;
    }
    (j + 1).min(t.len())
}

/// C's keywords and the words Sphinx's C domain reads as part of a type.
fn c_reserved(word: &str) -> bool {
    matches!(
        word,
        "auto"
            | "break"
            | "case"
            | "char"
            | "const"
            | "continue"
            | "default"
            | "do"
            | "double"
            | "else"
            | "enum"
            | "extern"
            | "float"
            | "for"
            | "goto"
            | "if"
            | "inline"
            | "int"
            | "long"
            | "register"
            | "restrict"
            | "return"
            | "short"
            | "signed"
            | "sizeof"
            | "static"
            | "struct"
            | "switch"
            | "typedef"
            | "union"
            | "unsigned"
            | "void"
            | "volatile"
            | "while"
            | "_Alignas"
            | "_Alignof"
            | "_Atomic"
            | "_Bool"
            | "_Complex"
            | "_Generic"
            | "_Imaginary"
            | "_Noreturn"
            | "_Static_assert"
            | "_Thread_local"
            | "alignas"
            | "alignof"
            | "bool"
            | "complex"
            | "imaginary"
            | "noreturn"
            | "static_assert"
            | "thread_local"
            | "true"
            | "false"
            | "__int64"
            | "__int128"
    )
}

/// Writes the qualification of a declared name ("Sphinx." in "Sphinx.version") as
/// Desc_Addname, each part a link to what it names ("Sphinx"); an anonymous part
/// ("@data") shows as "[anonymous]".
pub(crate) fn c_qualifier(
    b: &mut Builder<'_>,
    span: SourceSpan,
    qualifier: &str,
    scope: &str,
) -> Result<(), Fail> {
    let id = b.begin_node(NodeKind::DescAddname, span)?;
    let mut start = 0;
    let mut pending = String::new();
    for (i, c) in qualifier.bytes().enumerate() {
        if c != b'.' {
            continue;
        }
        let part = &qualifier[start..i];
        if part.is_empty() || part.starts_with('@') {
            pending.push_str(if part.is_empty() { "." } else { "[anonymous]." });
        } else {
            text_leaf(b, span, &pending)?;
            c_identifier_xref(b, span, &qualifier[..i], part, scope)?;
            pending = ".".to_owned();
        }
        start = i + 1;
    }
    pending.push_str(&qualifier[start..]);
    text_leaf(b, span, &pending)?;
    b.end_node(id)
}

/// Splits a parameter list at top-level commas, trimmed, empty parts skipped.
fn split_params(params: &str) -> impl Iterator<Item = &str> {
    let b = params.as_bytes();
    let mut depth = 0i32;
    let mut cuts = vec![0];
    for (i, &c) in b.iter().enumerate() {
        match c {
            b'(' | b'[' | b'{' | b'<' => depth += 1,
            b')' | b']' | b'}' | b'>' => depth -= 1,
            b',' if depth <= 0 => cuts.push(i + 1),
            _ => {}
        }
    }
    cuts.push(b.len() + 1);
    let parts: Vec<&str> = cuts.windows(2).map(|w| trim_space(&params[w[0]..w[1] - 1])).collect();
    parts.into_iter().filter(|p| !p.is_empty())
}

/// Emits a parameter list; with `links`, names in each parameter link from it.
pub(crate) fn c_params(
    b: &mut Builder<'_>,
    params: &str,
    span: SourceSpan,
    links: &str,
) -> Result<(), Fail> {
    let list = b.begin_node(NodeKind::DescParameterlist, span)?;
    for part in split_params(params) {
        let param = b.begin_node(NodeKind::DescParameter, span)?;
        if links.is_empty() {
            text_leaf(b, span, part)?;
        } else {
            c_linked(b, span, part, links, c_param_name(part))?;
        }
        b.end_node(param)?;
    }
    b.end_node(list)
}

/// Emits declarator suffixes: parameter lists as Desc_Parameterlist, the closing
/// parentheses and array bounds as text.
pub(crate) fn c_suffix_nodes(
    b: &mut Builder<'_>,
    s: &str,
    span: SourceSpan,
    links: &str,
) -> Result<(), Fail> {
    let t = s.as_bytes();
    let (mut start, mut i) = (0, 0);
    while i < t.len() {
        if t[i] != b'(' && t[i] != b'[' {
            i += 1;
            continue;
        }
        let close = find_close(s, i);
        if t[i] == b'[' {
            i = close + 1;
            continue;
        }
        text_leaf(b, span, &s[start..i])?;
        c_params(b, &s[i + 1..close.min(t.len())], span, links)?;
        i = close + 1;
        start = i.min(t.len());
    }
    text_leaf(b, span, &s[start.min(t.len())..])
}

/// Adds Custom `params` to a C function's Desc_Signature: the names of its parameters,
/// one per line, which Sphinx's C domain declares as function parameters. Parameters
/// without a name ("void", "...", a type alone) give none.
pub(crate) fn c_function_params(
    b: &mut Builder<'_>,
    id: NodeId,
    params: &str,
) -> Result<(), Fail> {
    let names: Vec<&str> =
        split_params(params).map(c_param_name).filter(|n| !n.is_empty()).collect();
    if names.is_empty() {
        return Ok(());
    }
    b.add_custom_attribute(id, "params", &names.join("\n"))
}

/// The name a parameter declares: the identifier of its declarator, as in "const char
/// *name", "int values[4]", or "int (*callback)(int)".
pub(crate) fn c_param_name(param: &str) -> &str {
    if let Some(k) = param.find('(') {
        // A function pointer: the name follows the '*' inside the first parentheses.
        let inner = &param[k + 1..];
        let Some(star) = inner.find('*') else { return "" };
        let rest = trim_space(&inner[star + 1..]);
        let e = rest.bytes().position(|c| !is_identifier_byte(c)).unwrap_or(rest.len());
        return &rest[..e];
    }
    let mut t = param;
    while t.ends_with(']') {
        let k = t.rfind('[').unwrap_or(0);
        t = trim_space(&t[..k]);
    }
    let b = t.as_bytes();
    let mut s = b.len();
    while s > 0 && is_identifier_byte(b[s - 1]) {
        s -= 1;
    }
    let name = &t[s..];
    let before = trim_space(&t[..s]);
    let keyword = matches!(
        name,
        "void"
            | "char"
            | "short"
            | "int"
            | "long"
            | "float"
            | "double"
            | "signed"
            | "unsigned"
            | "_Bool"
            | "bool"
            | "_Complex"
            | "complex"
            | "const"
            | "volatile"
            | "restrict"
    );
    if name.is_empty()
        || before.is_empty()
        || keyword
        || matches!(before, "struct" | "union" | "enum")
    {
        return "";
    }
    name
}
