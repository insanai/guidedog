//! Python parameter lists: parsed as Python would parse them, or, for lists Python
//! would refuse (optional parts in brackets), read as Sphinx's pseudo argument lists.

use guidedoc::{Builder, Fail, NodeKind, SourceSpan};

use super::annotation_xrefs;
use crate::domains::{Cx, leaf_in};
use crate::text::{is_identifier_byte, trim_space, trim_suffix};

/// One parameter, or a "/" or "*" separator.
#[derive(Debug, Default)]
struct PyParam<'s> {
    /// 1 for *args, 2 for **kwargs.
    stars: usize,
    name: &'s str,
    annotation: Option<&'s str>,
    default: Option<&'s str>,
    /// b'/' or b'*' for a separator.
    marker: Option<u8>,
}

/// Emits a parameter list as Python would read it; one Python would refuse falls back
/// to Sphinx's bracket reading.
pub(super) fn py_arglist(
    cx: &mut Cx<'_, '_, '_>,
    arglist: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    let Some(params) = split_params(arglist) else {
        return pseudo_arglist(cx.b(), arglist, span);
    };
    let list = cx.b().begin_node(NodeKind::DescParameterlist, span)?;
    for q in &params {
        py_param(cx, q, span)?;
    }
    cx.b().end_node(list)
}

/// Splits at top-level commas and checks each parameter's form and order.
fn split_params(arglist: &str) -> Option<Vec<PyParam<'_>>> {
    let mut params = Vec::new();
    if trim_space(arglist).is_empty() {
        return Some(params);
    }
    let a = arglist.as_bytes();
    let (mut depth, mut quote, mut start) = (0i32, 0u8, 0);
    let (mut seen_default, mut keyword_only) = (false, false);
    for i in 0..=a.len() {
        if i < a.len() {
            let c = a[i];
            if quote != 0 {
                if c == quote {
                    quote = 0;
                }
                continue;
            }
            match c {
                b'\'' | b'"' => quote = c,
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                _ => {}
            }
            if depth < 0 {
                return None;
            }
            if c != b',' || depth > 0 {
                continue;
            }
        }
        let part = trim_space(&arglist[start..i]);
        start = i + 1;
        if part.is_empty() {
            if i == a.len() && !params.is_empty() {
                break; // a trailing comma.
            }
            return None;
        }
        let q = parse_param(part)?;
        if q.marker == Some(b'*') || q.stars > 0 {
            keyword_only = true;
        }
        if q.marker.is_none() && q.stars == 0 && !keyword_only {
            if q.default.is_some() {
                seen_default = true;
            } else if seen_default {
                return None;
            }
        }
        params.push(q);
    }
    (depth == 0 && quote == 0).then_some(params)
}

/// Reads one parameter: "*"/"**", a name, ": annotation", "= default".
fn parse_param(part: &str) -> Option<PyParam<'_>> {
    if part == "/" || part == "*" {
        return Some(PyParam { marker: Some(part.as_bytes()[0]), ..PyParam::default() });
    }
    let mut q = PyParam::default();
    let mut t = part;
    while q.stars < 2 && t.starts_with('*') {
        q.stars += 1;
        t = &t[1..];
    }
    let k = t.bytes().position(|c| !is_identifier_byte(c)).unwrap_or(t.len());
    if k == 0 || t.as_bytes()[0].is_ascii_digit() {
        return None;
    }
    q.name = &part[..q.stars + k];
    let mut rest = trim_space(&t[k..]);
    if let Some(after) = rest.strip_prefix(':') {
        let e = top_level_index(after, b'=');
        let annotation = trim_space(&after[..e]);
        if annotation.is_empty() {
            return None;
        }
        q.annotation = Some(annotation);
        rest = &after[e..];
    }
    if let Some(after) = rest.strip_prefix('=') {
        let default = trim_space(after);
        if q.stars > 0 || default.is_empty() {
            return None;
        }
        q.default = Some(default);
        rest = "";
    }
    rest.is_empty().then_some(q)
}

/// Finds `c` outside brackets and quotes, or returns `s.len()`.
fn top_level_index(s: &str, c: u8) -> usize {
    let (mut depth, mut quote) = (0i32, 0u8);
    for (i, &x) in s.as_bytes().iter().enumerate() {
        if quote != 0 {
            if x == quote {
                quote = 0;
            }
            continue;
        }
        match x {
            b'\'' | b'"' => quote = x,
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            _ if x == c && depth == 0 => return i,
            _ => {}
        }
    }
    s.len()
}

/// Emits Desc_Parameter: the name, ": " and its annotation, "=" and its default.
fn py_param(cx: &mut Cx<'_, '_, '_>, q: &PyParam<'_>, span: SourceSpan) -> Result<(), Fail> {
    let id = cx.b().begin_node(NodeKind::DescParameter, span)?;
    if let Some(m) = q.marker {
        cx.text_leaf(span, if m == b'/' { "/" } else { "*" })?;
        return cx.b().end_node(id);
    }
    cx.text_leaf(span, q.name)?;
    if let Some(a) = q.annotation {
        cx.text_leaf(span, ": ")?;
        annotation_xrefs(cx, a, span)?;
    }
    if let Some(d) = q.default {
        cx.text_leaf(span, if q.annotation.is_some() { " = " } else { "=" })?;
        cx.text_leaf(span, d)?;
    }
    cx.b().end_node(id)
}

/// Sphinx's reading of argument lists that are not Python: commas separate parameters
/// and brackets mark optional ones (Desc_Optional). Unbalanced brackets make the whole
/// list one parameter.
pub(crate) fn pseudo_arglist(
    b: &mut Builder<'_>,
    arglist: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    let list = b.begin_node(NodeKind::DescParameterlist, span)?;
    if pseudo_walk(None, arglist, span)? {
        pseudo_walk(Some(b), arglist, span)?;
    } else {
        leaf_in(b, NodeKind::DescParameter, span, arglist)?;
    }
    b.end_node(list)
}

/// Checks the bracket structure (`b` None), or emits it; false for unbalanced brackets.
fn pseudo_walk(
    mut b: Option<&mut Builder<'_>>,
    arglist: &str,
    span: SourceSpan,
) -> Result<bool, Fail> {
    let mut depth = 0usize;
    for part in arglist.split(',') {
        let mut arg = trim_space(part);
        let (mut ends_open, mut ends_close) = (0, 0);
        while let Some(rest) = arg.strip_prefix('[') {
            open(&mut b, span, &mut depth)?;
            arg = trim_space(rest);
        }
        while let Some(rest) = arg.strip_prefix(']') {
            if !close(&mut b, &mut depth)? {
                return Ok(false);
            }
            arg = trim_space(rest);
        }
        while arg.ends_with(']') && trim_suffix(arg, "[]") == arg {
            ends_close += 1;
            arg = trim_space(&arg[..arg.len() - 1]);
        }
        while let Some(rest) = arg.strip_suffix('[') {
            ends_open += 1;
            arg = trim_space(rest);
        }
        if !arg.is_empty()
            && let Some(b) = b.as_deref_mut()
        {
            leaf_in(b, NodeKind::DescParameter, span, arg)?;
        }
        for _ in 0..ends_open {
            open(&mut b, span, &mut depth)?;
        }
        for _ in 0..ends_close {
            if !close(&mut b, &mut depth)? {
                return Ok(false);
            }
        }
    }
    Ok(depth == 0)
}

fn open(
    b: &mut Option<&mut Builder<'_>>,
    span: SourceSpan,
    depth: &mut usize,
) -> Result<(), Fail> {
    if let Some(b) = b.as_deref_mut() {
        b.begin_node(NodeKind::DescOptional, span)?;
    }
    *depth += 1;
    Ok(())
}

fn close(b: &mut Option<&mut Builder<'_>>, depth: &mut usize) -> Result<bool, Fail> {
    if *depth == 0 {
        return Ok(false);
    }
    if let Some(b) = b.as_deref_mut()
        && let Some(current) = b.current()
    {
        b.end_node(current)?;
    }
    *depth -= 1;
    Ok(true)
}
