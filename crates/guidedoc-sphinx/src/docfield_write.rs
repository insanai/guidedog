//! Step 2 of Sphinx's info field transform: the rebuilt list is written after the
//! object's content read so far, the content of the fields as written is copied into
//! it, and it then takes the old list's place (docfields.rs).
//!
//! Shapes, with Sphinx's nodes: Field{Custom docfield = the type's name} > Field_Name
//! (the translated label) and Field_Body. A group's body is List{Bullet} > List_Item >
//! Paragraph per use, or the Paragraph alone when a group of one collapses. A use's
//! paragraph holds the argument (Strong{literal}, Sphinx's literal_strong), its type in
//! " (" and ")" (Emphasis{literal}), " -- " (an en dash once smart quotes apply), and
//! the field's inline content. Content of several blocks, which Sphinx puts inside that
//! paragraph, follows it instead, since a paragraph holds inlines only.

use guidedoc::{
    Builder, CORE_NODES, Fail, ListInfo, ListStyle, NodeId, NodeKind, Region, SourceSpan, arg_int,
    diagnostic,
};

use crate::docfields::{
    DocField, DocFields, FieldItem, FieldShape, field_parts, one_text, single_paragraph,
};
use crate::domains::{Cx, XrefParts, text_leaf};
use crate::roles::{RoleCx, field_role, xref_begin};
use crate::text::{has_at, trim_space};

/// The node a cross-reference of a field shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Display {
    Text,
    Emphasis,
    /// Sphinx's literal_emphasis: Emphasis{literal}.
    LiteralEmphasis,
    /// literal_strong: Strong{literal}.
    LiteralStrong,
}

/// Begins a field of a type, with its label as the name, and its body.
fn open_field(
    cx: &mut Cx<'_, '_, '_>,
    df: &DocField,
    span: SourceSpan,
) -> Result<(NodeId, NodeId), Fail> {
    let field = cx.b().begin_node(NodeKind::Field, span)?;
    cx.custom(field, "docfield", df.name)?;
    let first = cx.node_count();
    let label = cx.config.label(df.label).to_owned();
    cx.leaf_in(NodeKind::FieldName, span, &label)?;
    cx.generated(first);
    let body = cx.b().begin_node(NodeKind::FieldBody, span)?;
    Ok((field, body))
}

/// Writes a field kept as written, under its new name; a type it links replaces its
/// body's text.
pub(crate) fn write_kept(
    cx: &mut Cx<'_, '_, '_>,
    t: &DocFields,
    entry: usize,
) -> Result<(), Fail> {
    let e = &t.entries[entry];
    let b = cx.b();
    let (Some(name), Some(body)) = field_parts(b, e.field) else {
        let end = b.snap.nodes[e.field].subtree_end as usize;
        return copy_nodes(b, e.field.index(), end);
    };
    let field = reopen(b, e.field)?;
    let n = reopen(b, name)?;
    let name_span = b.snap.nodes[name].span;
    text_leaf(b, name_span, &e.name)?;
    let (first, end) = (name.index() + 1, b.snap.nodes[name].subtree_end as usize);
    if first < end {
        let after = b.snap.nodes[first].subtree_end as usize;
        copy_nodes(b, after, end)?;
    }
    b.end_node(n)?;
    if let Some((kind, text)) = &e.link {
        let df = t.fields[*kind];
        let span = b.snap.nodes[body].span;
        let bd = reopen(b, body)?;
        let para = reopen(b, NodeId::from_index(body.index() + 1))?;
        field_xrefs(cx, t, &df, df.typerole, text, Display::Emphasis, (true, false), span)?;
        cx.b().end_node(para)?;
        cx.b().end_node(bd)?;
    } else {
        let end = b.snap.nodes[body].subtree_end as usize;
        copy_nodes(b, body.index(), end)?;
    }
    cx.b().end_node(field)
}

/// Writes a use of a plain field type: its content in a paragraph, a body of one text
/// linked when the type has a role for it, or the blocks as they are.
pub(crate) fn write_plain(
    cx: &mut Cx<'_, '_, '_>,
    t: &DocFields,
    entry: usize,
) -> Result<(), Fail> {
    let Some(kind) = t.entries[entry].kind else { return Ok(()) };
    let df = t.fields[kind];
    let Some(item) = t.items.iter().rev().find(|it| it.entry == entry) else { return Ok(()) };
    let span = cx.b().snap.nodes[item.body].span;
    let (field, body) = open_field(cx, &df, span)?;
    let b = cx.b();
    let body_end = b.snap.nodes[item.body].subtree_end as usize;
    if let Some(para) = single_paragraph(b, item.body) {
        let text = one_text(b, para);
        let pa = b.begin_node(NodeKind::Paragraph, span)?;
        match text {
            Some(text) if !df.bodyrole.is_empty() => {
                let first = cx.node_count();
                field_xrefs(
                    cx,
                    t,
                    &df,
                    df.bodyrole,
                    &text,
                    Display::Emphasis,
                    (true, true),
                    span,
                )?;
                cx.generated(first);
            }
            _ => {
                let end = b.snap.nodes[para].subtree_end as usize;
                copy_nodes(b, para.index() + 1, end)?;
            }
        }
        cx.b().end_node(pa)?;
    } else if body_end > item.body.index() + 1 {
        copy_nodes(b, item.body.index() + 1, body_end)?;
    } else {
        let pa = b.begin_node(NodeKind::Paragraph, span)?;
        b.end_node(pa)?;
    }
    cx.b().end_node(body)?;
    cx.b().end_node(field)
}

/// Writes the uses of a grouped or typed field type in one field.
pub(crate) fn write_group(
    cx: &mut Cx<'_, '_, '_>,
    t: &mut DocFields,
    entry: usize,
) -> Result<(), Fail> {
    let Some(kind) = t.entries[entry].kind else { return Ok(()) };
    let df = t.fields[kind];
    let items: Vec<FieldItem> = t.items.iter().filter(|it| it.entry == entry).cloned().collect();
    let span = cx.b().snap.nodes[t.entries[entry].field].span;
    let (field, body) = open_field(cx, &df, span)?;
    if items.len() == 1 && df.collapse {
        write_use(cx, t, &df, &items[0])?;
    } else {
        let b = cx.b();
        let list = b.begin_node(NodeKind::List, span)?;
        b.set_list(list, ListInfo { style: ListStyle::Bullet, delimiter: b'*', start: 1 })?;
        for it in &items {
            let item_span = cx.b().snap.nodes[it.body].span;
            let li = cx.b().begin_node(NodeKind::ListItem, item_span)?;
            write_use(cx, t, &df, it)?;
            cx.b().end_node(li)?;
        }
        cx.b().end_node(list)?;
    }
    cx.b().end_node(body)?;
    cx.b().end_node(field)
}

/// Writes one use: argument, type, separator, and content. A typed use shows the
/// separator and content only when the content has text.
fn write_use(
    cx: &mut Cx<'_, '_, '_>,
    t: &mut DocFields,
    df: &DocField,
    it: &FieldItem,
) -> Result<(), Fail> {
    let span = cx.b().snap.nodes[it.body].span;
    let para = cx.b().begin_node(NodeKind::Paragraph, span)?;
    let grouped = df.shape == FieldShape::Grouped;
    let first = cx.node_count();
    field_xrefs(cx, t, df, df.role, &it.arg, Display::LiteralStrong, (false, grouped), span)?;
    let mut content = true;
    if !grouped {
        let kind = t.entries[it.entry].kind.unwrap_or(0);
        if let Some(k) = t.take_type(kind, &it.arg) {
            cx.text_leaf(span, " (")?;
            match t.types[k].text.clone() {
                Some(text) => {
                    field_xrefs(
                        cx,
                        t,
                        df,
                        df.typerole,
                        &text,
                        Display::LiteralEmphasis,
                        (false, true),
                        span,
                    )?;
                }
                None => {
                    let p = t.types[k].para;
                    let end = cx.b().snap.nodes[p].subtree_end as usize;
                    copy_nodes(cx.b(), p.index() + 1, end)?;
                }
            }
            cx.text_leaf(span, ")")?;
        }
        content = has_text(cx.b(), it.body);
    }
    if content {
        cx.text_leaf(span, " -- ")?;
    }
    cx.generated(first);
    let b = cx.b();
    let inner = single_paragraph(b, it.body);
    if content && let Some(inner) = inner {
        let end = b.snap.nodes[inner].subtree_end as usize;
        copy_nodes(b, inner.index() + 1, end)?;
    }
    b.end_node(para)?;
    if content && inner.is_none() {
        let end = b.snap.nodes[it.body].subtree_end as usize;
        copy_nodes(b, it.body.index() + 1, end)?;
    }
    Ok(())
}

/// Tells whether a body has text other than white space, as Sphinx tests a typed use's
/// content.
fn has_text(b: &Builder<'_>, body: NodeId) -> bool {
    let end = b.snap.nodes[body].subtree_end as usize;
    b.snap.nodes[body.index() + 1..end]
        .iter()
        .any(|n| n.kind != NodeKind::Target && !trim_space(b.pool_string(n.text)).is_empty())
}

/// Links a field's argument, type, or body text with a role, as Sphinx's make_xrefs:
/// without a role the text is shown alone; Python's fields split it at delimiters
/// (PyXrefMixin); other domains read it with the role when Sphinx has an inliner for
/// it, and otherwise make one Xref. `how` is (contnode, inliner): contnode shows plain
/// text, as Sphinx does when the field's own text node is the display.
#[allow(clippy::too_many_arguments)]
fn field_xrefs(
    cx: &mut Cx<'_, '_, '_>,
    t: &DocFields,
    df: &DocField,
    role: &str,
    text: &str,
    inner: Display,
    how: (bool, bool),
    span: SourceSpan,
) -> Result<(), Fail> {
    let (contnode, inliner) = how;
    let shown = if contnode { Display::Text } else { inner };
    if df.python {
        py_xrefs(cx, role, text, inner, contnode, span)
    } else if role.is_empty() {
        show(cx.b(), span, shown, text)
    } else if inliner {
        let mut rx = RoleCx { ctx: cx.p.ctx, st: cx.st, config: cx.config };
        field_role(&mut rx, t.domain, role, text, span)
    } else {
        let reftype = format!("{}:{role}", t.domain);
        field_xref(cx, span, (t.domain, &reftype), text, shown, text)
    }
}

/// PyXrefMixin.make_xrefs: the text is split at brackets, commas, "or", "of", "|", and
/// "..."; delimiters are shown, the rest linked, and everything after a Literal is
/// shown unlinked.
fn py_xrefs(
    cx: &mut Cx<'_, '_, '_>,
    role: &str,
    text: &str,
    inner: Display,
    contnode: bool,
    span: SourceSpan,
) -> Result<(), Fail> {
    let shown = if contnode { Display::Text } else { inner };
    let mut literal = false;
    let mut i = 0;
    while i < text.len() {
        let (piece, delimiter) = next_piece(text, &mut i);
        if literal || delimiter {
            show(cx.b(), span, shown, piece)?;
        } else {
            py_xref(cx, role, piece, inner, contnode, span)?;
        }
        if matches!(piece, "Literal" | "typing.Literal" | "~typing.Literal") {
            literal = true;
        }
    }
    Ok(())
}

/// PyXrefMixin.make_xref for one part: an Xref of the role, refspecific, in the Python
/// context. A "~", "." or "typing." prefix changes what is shown, and then the target
/// loses the prefix and the type becomes py:class, or py:obj for None and typing names,
/// as parse_reftarget decides.
fn py_xref(
    cx: &mut Cx<'_, '_, '_>,
    role: &str,
    target: &str,
    inner: Display,
    contnode: bool,
    span: SourceSpan,
) -> Result<(), Fail> {
    let plain = if contnode { Display::Text } else { inner };
    if role.is_empty() {
        return show(cx.b(), span, plain, target);
    }
    let (mut goal, mut title) = (target, target);
    if let Some(rest) = target.strip_prefix('.') {
        (goal, title) = (rest, rest);
    } else if let Some(rest) = target.strip_prefix('~') {
        goal = rest;
        title = rest.rfind('.').map_or(rest, |k| &rest[k + 1..]);
    } else if let Some(rest) = target.strip_prefix("typing.") {
        title = rest;
    }
    if goal == title {
        let reftype = format!("py:{role}");
        return field_xref(cx, span, ("py", &reftype), target, plain, target);
    }
    let typing = goal == "None" || has_at(goal, 0, "typing.");
    let reftype = if typing { "py:obj" } else { "py:class" };
    field_xref(cx, span, ("py", reftype), goal, inner, title)
}

/// Returns the text from `i` to the next delimiter, or the delimiter at `i`, and moves
/// `i` past it.
fn next_piece<'t>(text: &'t str, i: &mut usize) -> (&'t str, bool) {
    let start = *i;
    for k in start..text.len() {
        let n = delimiter_at(text, k);
        if n == 0 {
            continue;
        }
        if k > start {
            *i = k;
            return (&text[start..k], false);
        }
        *i = k + n;
        return (&text[k..k + n], true);
    }
    *i = text.len();
    (&text[start..], false)
}

/// The length of the delimiter at `k`, or 0, as Sphinx's expression
/// `(\s*[\[\]\(\),](?:\s*o[rf]\s)?\s*|\s+o[rf]\s+|\s*\|\s*|\.\.\.)` matches.
fn delimiter_at(text: &str, k: usize) -> usize {
    let t = text.as_bytes();
    let spaces =
        |i: usize| (i..t.len()).find(|&j| !guidedoc::is_ascii_space(t[j])).unwrap_or(t.len());
    let or_of = |i: usize| {
        i + 2 < t.len()
            && t[i] == b'o'
            && (t[i + 1] == b'r' || t[i + 1] == b'f')
            && guidedoc::is_ascii_space(t[i + 2])
    };
    let mut j = spaces(k);
    if j < t.len() && matches!(t[j], b'[' | b']' | b'(' | b')' | b',') {
        j += 1;
        let g = spaces(j);
        if or_of(g) {
            j = g + 3;
        }
        return spaces(j) - k;
    }
    if j > k && or_of(j) {
        return spaces(j + 3) - k;
    }
    if j < t.len() && t[j] == b'|' {
        return spaces(j + 1) - k;
    }
    if has_at(text, k, "...") {
        return 3;
    }
    0
}

/// Writes an Xref an info field makes: refspecific in Python, its display without the
/// classes a role's display has (Custom unstyled). `kind` is (domain, reftype).
fn field_xref(
    cx: &mut Cx<'_, '_, '_>,
    span: SourceSpan,
    kind: (&str, &str),
    target: &str,
    shown: Display,
    text: &str,
) -> Result<(), Fail> {
    let (domain, reftype) = kind;
    let x = XrefParts { target: target.into(), specific: domain == "py", ..XrefParts::default() };
    let id = xref_begin(&mut cx.p.ctx.b, cx.st, span, reftype, domain, &x)?;
    cx.custom(id, "unstyled", "1")?;
    show(cx.b(), span, shown, text)?;
    cx.b().end_node(id)
}

/// Writes text as the display a field gives it.
fn show(b: &mut Builder<'_>, span: SourceSpan, d: Display, text: &str) -> Result<(), Fail> {
    if d == Display::Text {
        return text_leaf(b, span, text);
    }
    let kind = if d == Display::LiteralStrong { NodeKind::Strong } else { NodeKind::Emphasis };
    let id = b.begin_node(kind, span)?;
    if d != Display::Emphasis {
        b.add_custom_attribute(id, "literal", "1")?;
    }
    text_leaf(b, span, text)?;
    b.end_node(id)
}

/// Begins a node like `old`: its kind, span, attributes, value, flags, and text,
/// without children.
pub(crate) fn reopen(b: &mut Builder<'_>, old: NodeId) -> Result<NodeId, Fail> {
    let row = b.snap.nodes[old];
    let id = b.begin_node(row.kind, row.span)?;
    let n = &mut b.snap.nodes[id];
    n.attributes = row.attributes;
    n.value = row.value;
    n.flags = row.flags;
    n.text = row.text;
    Ok(id)
}

/// Copies the sibling subtrees [a, e) as the last children of the open node. A copy
/// shares its original's attributes and payload row: the original is removed before
/// the document is finished.
pub(crate) fn copy_nodes(b: &mut Builder<'_>, a: usize, e: usize) -> Result<(), Fail> {
    if e <= a {
        return Ok(());
    }
    let n = e - a;
    b.spend(n as u64)?;
    let used = b.snap.nodes.len();
    if used + n > b.limits.max_nodes {
        let max = i64::try_from(b.limits.max_nodes).unwrap_or(i64::MAX);
        let span = b.snap.nodes[a].span;
        return Err(b.fail(diagnostic(CORE_NODES, span, arg_int(max), guidedoc::Arg::None)));
    }
    if b.snap.nodes.try_reserve(n).is_err() {
        return Err(b.capacity(Region::Nodes, n as u64));
    }
    let shift = (used - a) as u32;
    for i in a..e {
        let mut row = b.snap.nodes[i];
        row.subtree_end += shift;
        b.snap.nodes.push(row);
    }
    Ok(())
}

/// Puts the nodes written since `start` in place of the subtree [a, e) and returns the
/// index after them. Every node from `a` on is closed and inside the open object
/// content, and reading has linked none of them yet, so only their subtree ends change.
pub(crate) fn replace_nodes(
    b: &mut Builder<'_>,
    a: usize,
    e: usize,
    start: usize,
) -> Result<usize, Fail> {
    let used = b.snap.nodes.len();
    b.spend((used - a) as u64)?;
    let (before, moved) = ((start - e) as u32, (used - start) as u32);
    let rows = &mut b.snap.nodes;
    for (i, row) in rows.iter_mut().enumerate().take(used).skip(e) {
        if i < start {
            row.subtree_end += moved;
        } else {
            row.subtree_end -= before;
        }
    }
    rows[e..used].rotate_left(start - e);
    let k = e - a;
    rows.drain(a..e);
    for row in &mut rows[a..] {
        row.subtree_end -= k as u32;
    }
    Ok(a + moved as usize)
}
