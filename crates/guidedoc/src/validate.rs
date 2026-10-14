//! Validation checks every structural invariant of a finished snapshot, so renderers can
//! index it without checks of their own. Readers' and passes' output, and loaded
//! objects, are validated before they become the workspace's document.

use crate::kinds::{Payload, child_allowed, kind_name, payload_of};
use crate::options::Limits;
use crate::report::{
    Arg, CORE_DEPTH, CORE_INVALID, CORE_NESTING, Fail, arg_int, arg_static, diagnostic,
};
use crate::snapshot::Snapshot;
use crate::types::{CellInfo, LinkKind, NONE, NodeKind, Region, SourceSpan, Text};

/// A validation failure: the status and the diagnostic explaining it.
type Invalid = Fail;

fn invalid(id: usize, why: &'static str, span: SourceSpan) -> Invalid {
    let id = i64::try_from(id).unwrap_or(i64::MAX);
    Fail::from_diagnostic(diagnostic(CORE_INVALID, span, arg_int(id), arg_static(why)))
}

/// Checks a finished snapshot. `source_lengths[i]` is the byte length of source `i`.
/// It allocates its own working stack; the engine uses `validate_with` and a reused one.
pub fn validate(snap: &Snapshot, source_lengths: &[usize], limits: &Limits) -> Result<(), Fail> {
    let mut stack = Vec::new();
    validate_with(snap, source_lengths, limits, &mut stack)
}

/// `validate` with a caller-owned stack of open subtree ends, cleared first.
pub(crate) fn validate_with(
    snap: &Snapshot,
    source_lengths: &[usize],
    limits: &Limits,
    open: &mut Vec<u32>,
) -> Result<(), Invalid> {
    check_pool(snap, limits)?;
    let nodes = &snap.nodes;
    if nodes.first().map(|n| n.kind) != Some(NodeKind::Document) {
        return Err(invalid(0, "the root is not a document", SourceSpan::default()));
    }
    if nodes[0].subtree_end as usize != nodes.len() {
        return Err(invalid(0, "the root does not cover every node", SourceSpan::default()));
    }
    open.clear();
    for (i, n) in nodes.iter().enumerate() {
        while open.last().is_some_and(|&end| end as usize <= i) {
            open.pop();
        }
        if n.subtree_end as usize <= i || n.subtree_end as usize > nodes.len() {
            return Err(invalid(i, "its subtree end is out of order", n.span));
        }
        if i > 0 {
            let Some(&parent_end) = open.last() else {
                return Err(invalid(i, "it has no parent", n.span));
            };
            if n.subtree_end > parent_end {
                return Err(invalid(i, "it extends past its parent", n.span));
            }
        }
        check_node(snap, source_lengths, i)?;
        if open.len() > limits.max_depth + 1 {
            let max = i64::try_from(limits.max_depth).unwrap_or(i64::MAX);
            return Err(Fail::from_diagnostic(diagnostic(
                CORE_DEPTH,
                n.span,
                arg_int(max),
                Arg::None,
            )));
        }
        open.push(n.subtree_end);
    }
    check_parents(snap)?;
    check_links(snap)
}

/// The pool must be within its limit and UTF-8, so documents slice it as text.
fn check_pool(snap: &Snapshot, limits: &Limits) -> Result<(), Invalid> {
    let max = limits.max_text_bytes.min(u32::MAX as usize);
    if snap.text.len() > max {
        let f = Fail::capacity(Region::Text, (snap.text.len() - max) as u64, false);
        return Err(f);
    }
    if let Err(e) = std::str::from_utf8(&snap.text) {
        // A pool-wide problem: the argument names the pool offset, not a node.
        return Err(invalid(
            e.valid_up_to(),
            "the text pool is not UTF-8 at this offset",
            SourceSpan::default(),
        ));
    }
    Ok(())
}

/// A text is valid when it lies in the pool on character boundaries.
fn text_ok(snap: &Snapshot, t: Text) -> bool {
    let range = t.range();
    if range.end > snap.text.len() {
        return false;
    }
    // A UTF-8 continuation byte is 0b10xx_xxxx; a boundary is any other byte or the end.
    let is_boundary = |at: usize| snap.text.get(at).is_none_or(|&c| c & 0xC0 != 0x80);
    is_boundary(range.start) && is_boundary(range.end)
}

fn check_node(snap: &Snapshot, source_lengths: &[usize], i: usize) -> Result<(), Invalid> {
    let n = &snap.nodes[i];
    if !text_ok(snap, n.text) {
        return Err(invalid(i, "its text is outside the pool", n.span));
    }
    let Some(&source_len) = source_lengths.get(n.span.source.index()) else {
        return Err(invalid(i, "its source is unknown", n.span));
    };
    if n.span.start > n.span.end || n.span.end as usize > source_len {
        return Err(invalid(i, "its source span is out of range", n.span));
    }
    let Some(attributes) = snap.attributes.get(n.attributes.range()) else {
        return Err(invalid(i, "its attributes are out of range", n.span));
    };
    if attributes.iter().any(|a| !text_ok(snap, a.value) || !text_ok(snap, a.name)) {
        return Err(invalid(i, "an attribute's text is outside the pool", n.span));
    }
    if n.payload != NONE && n.payload as usize >= payload_rows(snap, n.kind) {
        return Err(invalid(i, "its payload row is out of range", n.span));
    }
    match n.kind {
        NodeKind::Heading | NodeKind::Section if n.value < 1 => {
            Err(invalid(i, "its level is not positive", n.span))
        }
        NodeKind::Cell => {
            let c = snap.cells.get(n.payload as usize).copied().unwrap_or(CellInfo::default());
            if c.row_span < 1 || c.col_span < 1 {
                return Err(invalid(i, "a cell span is not positive", n.span));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Returns the rows of the table a kind's payload indexes; 0 for kinds without one.
fn payload_rows(snap: &Snapshot, k: NodeKind) -> usize {
    match payload_of(k) {
        Payload::None => 0,
        Payload::List => snap.lists.len(),
        Payload::Cell => snap.cells.len(),
        Payload::Link => snap.links.len(),
    }
}

/// Checks every parent-child pair against `child_allowed`.
fn check_parents(snap: &Snapshot) -> Result<(), Invalid> {
    let nodes = &snap.nodes;
    for (i, parent) in nodes.iter().enumerate() {
        let mut child = i + 1;
        while child < parent.subtree_end as usize {
            let c = &nodes[child];
            if !child_allowed(parent.kind, c.kind) {
                let d = diagnostic(
                    CORE_NESTING,
                    c.span,
                    arg_static(kind_name(parent.kind)),
                    arg_static(kind_name(c.kind)),
                );
                return Err(Fail::from_diagnostic(d));
            }
            child = c.subtree_end as usize;
        }
    }
    Ok(())
}

fn check_links(snap: &Snapshot) -> Result<(), Invalid> {
    for l in &snap.links {
        if !text_ok(snap, l.destination) || !text_ok(snap, l.title) || !text_ok(snap, l.name) {
            return Err(invalid(0, "a link's text is outside the pool", SourceSpan::default()));
        }
        if l.kind == LinkKind::Internal && l.resolved.index() >= snap.nodes.len() {
            return Err(invalid(
                0,
                "an internal link resolves to a missing node",
                SourceSpan::default(),
            ));
        }
    }
    Ok(())
}
