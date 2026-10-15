//! The reader's own resolution, when no extension resolves: footnotes get their
//! numbers and references to them, internal targets lead to the element after them,
//! substitution references find their definitions, and links to heading anchors reach
//! their sections. The shapes are the ones reStructuredText resolution produces.

use std::collections::HashMap;

use guidedoc::{
    BUILDER_OPEN, Fail, LinkInfo, LinkKind, NodeFlag, NodeFlags, NodeId, NodeKind, Text,
    normalize_label,
};

use crate::host::Host;
use crate::messages::REFERENCE_UNRESOLVED;

/// Links `[](#slug)` to the section a heading anchor names.
pub(crate) fn resolve_anchors(h: &mut Host<'_, '_>) {
    let anchors = std::mem::take(&mut h.ms.anchors);
    for &(row, slug) in &anchors {
        if let Some(node) = h.ms.slugs[slug].node {
            h.b().resolve_link(row, node, LinkKind::Internal, Text::default());
        }
    }
    h.ms.anchors = anchors;
}

/// The key a footnote or substitution is found by: its name without "#",
/// case-folded, after a letter that keeps the two namespaces apart.
fn name_key(kind: NodeKind, text: &str) -> String {
    let mut key = String::from(if kind == NodeKind::Footnote { "f" } else { "s" });
    normalize_label(&mut key, text.strip_prefix('#').unwrap_or(text));
    key
}

/// Resolves footnotes, footnote and substitution references, and targets.
pub(crate) fn resolve_document(h: &mut Host<'_, '_>) -> Result<(), Fail> {
    let names = index_names(h);
    number_notes(h);
    let count = h.b().snap.nodes.len();
    for i in 1..count {
        let id = NodeId::from_index(i);
        match h.b().snap.nodes[id].kind {
            NodeKind::FootnoteReference => resolve_named(h, &names, id, NodeKind::Footnote)?,
            NodeKind::SubstitutionReference => {
                resolve_named(h, &names, id, NodeKind::SubstitutionDefinition)?;
            }
            NodeKind::Target => resolve_target(h, id),
            _ => {}
        }
    }
    Ok(())
}

/// Records footnotes and substitution definitions by name, the first of each name.
fn index_names(h: &mut Host<'_, '_>) -> HashMap<String, NodeId> {
    let mut names = HashMap::new();
    let snap = &h.b().snap;
    for (i, n) in snap.nodes.iter().enumerate().skip(1) {
        if n.kind != NodeKind::Footnote && n.kind != NodeKind::SubstitutionDefinition {
            continue;
        }
        let key = name_key(n.kind, snap.pool_str(n.text));
        names.entry(key).or_insert(NodeId::from_index(i));
    }
    names
}

/// Gives auto-numbered footnotes, in document order, the lowest numbers no manually
/// numbered footnote uses.
fn number_notes(h: &mut Host<'_, '_>) {
    let nodes = &mut h.b().snap.nodes;
    let is_note = |n: &guidedoc::Node| n.kind == NodeKind::Footnote;
    let autos = nodes.iter().filter(|n| is_note(n) && n.flags.has(NodeFlag::Auto)).count();
    if autos == 0 {
        return;
    }
    let mut taken = vec![false; 2 * autos + 2];
    for n in nodes.iter().filter(|n| is_note(n) && !n.flags.has(NodeFlag::Auto)) {
        if let Some(slot) = usize::try_from(n.value).ok().and_then(|v| taken.get_mut(v)) {
            *slot = true;
        }
    }
    let mut next = 1;
    for n in nodes.iter_mut().filter(|n| is_note(n) && n.flags.has(NodeFlag::Auto)) {
        while next < taken.len() && taken[next] {
            next += 1;
        }
        n.value = next as i32;
        next += 1;
    }
}

/// Links a footnote or substitution reference to the node of its name.
fn resolve_named(
    h: &mut Host<'_, '_>,
    names: &HashMap<String, NodeId>,
    id: NodeId,
    kind: NodeKind,
) -> Result<(), Fail> {
    let b = h.b();
    let node = b.snap.nodes[id];
    let key = name_key(kind, b.snap.pool_str(node.text));
    let Some(&target) = names.get(&key) else {
        h.report_span(REFERENCE_UNRESOLVED, node.span);
        return Ok(());
    };
    let mut row = node.payload;
    if row as usize >= b.snap.links.len() {
        row = b.set_link(id, LinkInfo::UNRESOLVED)?;
    }
    if kind == NodeKind::Footnote {
        b.snap.nodes[id].value = b.snap.nodes[target].value;
        b.snap.nodes[target].flags.insert(NodeFlags::REFERENCED);
    }
    b.resolve_link(row, target, LinkKind::Internal, Text::default());
    Ok(())
}

/// The end of a node's subtree; open nodes reach the end of the table.
fn node_end(h: &mut Host<'_, '_>, id: NodeId) -> usize {
    let nodes = &h.b().snap.nodes;
    match nodes[id].subtree_end {
        BUILDER_OPEN => nodes.len(),
        end => end as usize,
    }
}

/// Leads an internal target to the next element that is not a target or comment.
fn resolve_target(h: &mut Host<'_, '_>, id: NodeId) {
    let row = h.b().snap.nodes[id].payload as usize;
    let Some(info) = h.b().snap.links.get(row).copied() else { return };
    if info.kind != LinkKind::Unresolved || info.destination.len > 0 || info.name.len > 0 {
        return;
    }
    let mut i = node_end(h, id);
    while i < h.b().snap.nodes.len() {
        let next = NodeId::from_index(i);
        let k = h.b().snap.nodes[next].kind;
        if k != NodeKind::Target && k != NodeKind::Comment {
            let b = h.b();
            b.resolve_link(row as u32, next, LinkKind::Internal, Text::default());
            b.snap.nodes[next].flags.insert(NodeFlags::REFERENCED);
            return;
        }
        i = node_end(h, next);
    }
}
