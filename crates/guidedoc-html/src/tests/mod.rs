//! Renderer tests. Trees are built directly with the builder by a test reader, since
//! the readers are separate crates; each test installs the tree it needs.

mod blocks;
mod fields;
mod inlines;
mod sphinx;

use std::cell::Cell;

use guidedoc::{
    AttributeKey, Builder, Conformance, Fail, INVALID_NODE, LinkInfo, LinkKind, ListInfo,
    ListStyle, NodeFlags, NodeId, NodeKind, Policy, ReadContext, Reader, Registry, RenderOptions,
    Request, Severity, SourceSpan, Status, Text, Workspace, default_request, trusted_policy,
};

/// Builds a test tree.
pub type Build = fn(&mut Builder<'_>) -> Result<(), Fail>;

thread_local! {
    static CURRENT: Cell<Option<Build>> = const { Cell::new(None) };
}

fn tree_read(ctx: &mut ReadContext<'_>) -> Result<(), Fail> {
    match CURRENT.get() {
        Some(build) => build(&mut ctx.b),
        None => Ok(()),
    }
}

const TREE_READER: Reader = Reader {
    id: "tree",
    name: "Tree",
    version: "1",
    conformance: Conformance::Partial,
    read: tree_read,
    read_inline: None,
};

const SOURCE: &str = "Guidedoc HTML renderer test source text, long enough for spans.";

/// What a render gave.
#[derive(Debug)]
pub struct Rendered {
    pub html: String,
    pub status: Status,
    /// The primary error's code, or the first warning's.
    pub code: &'static str,
    pub warnings: usize,
}

/// Fragment options: the defaults without a standalone page wrapper.
pub fn frag() -> RenderOptions<'static> {
    RenderOptions { standalone: false, ..RenderOptions::default() }
}

/// Renders `build`'s tree with `options` under the trusted policy.
pub fn render(build: Build, options: RenderOptions<'_>) -> Rendered {
    render_with(build, options, trusted_policy(), |_| {})
}

/// Renders `build`'s tree with `options` and `policy`; `adjust` edits the request.
pub fn render_with(
    build: Build,
    options: RenderOptions<'_>,
    policy: Policy,
    adjust: impl FnOnce(&mut Request<'_>),
) -> Rendered {
    CURRENT.set(Some(build));
    let registry = Registry { readers: vec![TREE_READER], renderers: vec![crate::RENDERER] };
    let mut request = default_request("tree.txt", SOURCE, "tree", "html");
    request.render = options;
    request.policy = policy;
    adjust(&mut request);
    let mut ws = Workspace::new();
    let (html, status) = match guidedoc::convert(&mut ws, &registry, &request) {
        Ok(html) => (html.to_owned(), Status::Ok),
        Err(fail) => (String::new(), fail.status),
    };
    let reports = ws.reports();
    let warnings = reports.items().iter().filter(|d| d.severity == Severity::Warning);
    let code = reports
        .primary()
        .map(|d| d.code)
        .or_else(|| warnings.clone().next().map(|d| d.code))
        .unwrap_or("");
    Rendered { html, status, code, warnings: warnings.count() }
}

/// Asserts the output holds `fragment`.
#[track_caller]
pub fn has(r: &Rendered, fragment: &str) {
    assert!(r.html.contains(fragment), "missing {fragment:?} in:\n{}", r.html);
}

/// Asserts the output lacks `fragment`.
#[track_caller]
pub fn lacks(r: &Rendered, fragment: &str) {
    assert!(!r.html.contains(fragment), "unexpected {fragment:?} in:\n{}", r.html);
}

/// Opens a node; spans are empty ranges in the test source.
pub fn open(b: &mut Builder<'_>, kind: NodeKind) -> Result<NodeId, Fail> {
    b.begin_node(kind, SourceSpan::default())
}

/// Opens a node with a value.
pub fn open_v(b: &mut Builder<'_>, kind: NodeKind, value: i32) -> Result<NodeId, Fail> {
    let id = open(b, kind)?;
    b.set_value(id, value);
    Ok(id)
}

/// Closes the innermost open node.
pub fn shut(b: &mut Builder<'_>) -> Result<(), Fail> {
    let id = b.current().ok_or_else(|| Fail::new(Status::Internal))?;
    b.end_node(id)
}

pub fn text(b: &mut Builder<'_>, s: &str) -> Result<(), Fail> {
    b.add_text_leaf(NodeKind::Text, SourceSpan::default(), s).map(|_| ())
}

pub fn leaf(b: &mut Builder<'_>, kind: NodeKind, s: &str) -> Result<NodeId, Fail> {
    b.add_text_leaf(kind, SourceSpan::default(), s)
}

/// Sets the open node's text.
pub fn set_text(b: &mut Builder<'_>, id: NodeId, s: &str) -> Result<(), Fail> {
    let t = b.add_text(s)?;
    b.set_text(id, t);
    Ok(())
}

pub fn para(b: &mut Builder<'_>, s: &str) -> Result<(), Fail> {
    open(b, NodeKind::Paragraph)?;
    text(b, s)?;
    shut(b)
}

/// Opens `kind` holding the text `s`.
pub fn inline(b: &mut Builder<'_>, kind: NodeKind, s: &str) -> Result<(), Fail> {
    open(b, kind)?;
    text(b, s)?;
    shut(b)
}

/// Adds an attribute to the open node.
pub fn attr(b: &mut Builder<'_>, key: AttributeKey, value: &str) -> Result<(), Fail> {
    let id = b.current().ok_or_else(|| Fail::new(Status::Internal))?;
    b.add_attribute_string(id, key, value)
}

/// Adds a Custom attribute to the open node.
pub fn custom(b: &mut Builder<'_>, name: &str, value: &str) -> Result<(), Fail> {
    let id = b.current().ok_or_else(|| Fail::new(Status::Internal))?;
    b.add_custom_attribute(id, name, value)
}

/// Opens a link-like node with a destination and returns its link row.
pub fn link(b: &mut Builder<'_>, kind: NodeKind, dest: &str, k: LinkKind) -> Result<u32, Fail> {
    link_titled(b, kind, dest, k, "")
}

pub fn link_titled(
    b: &mut Builder<'_>,
    kind: NodeKind,
    dest: &str,
    k: LinkKind,
    title: &str,
) -> Result<u32, Fail> {
    let id = open(b, kind)?;
    let destination = b.add_text(dest)?;
    let title = b.add_text(title)?;
    let info =
        LinkInfo { destination, title, name: Text::default(), resolved: INVALID_NODE, kind: k };
    b.set_link(id, info)
}

/// Sets a node's link row to `kind`, resolved to `resolved`.
pub fn set_link(
    b: &mut Builder<'_>,
    id: NodeId,
    kind: LinkKind,
    resolved: NodeId,
) -> Result<u32, Fail> {
    b.set_link(id, LinkInfo { kind, resolved, ..LinkInfo::UNRESOLVED })
}

/// A list with one item per string.
pub fn list(
    b: &mut Builder<'_>,
    style: ListStyle,
    tight: bool,
    items: &[&str],
) -> Result<(), Fail> {
    let l = open(b, NodeKind::List)?;
    b.set_list(l, ListInfo { style, delimiter: b'*', start: 1 })?;
    if tight {
        b.add_flags(l, NodeFlags::TIGHT);
    }
    for item in items {
        open(b, NodeKind::ListItem)?;
        para(b, item)?;
        shut(b)?;
    }
    shut(b)
}
