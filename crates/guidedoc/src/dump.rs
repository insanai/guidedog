//! A readable, deterministic outline of a document. Tests use it as a
//! renderer-independent golden format:
//!
//! ```text
//! document
//!   section value=1
//!     heading value=1
//!       text "Title"
//! ```

use crate::document::Document;
use crate::kinds::{Payload, kind_name, payload_of};
use crate::text::TextSink;
use crate::types::{AttributeKey, LinkInfo, LinkKind, NONE, NodeFlag, NodeId};

/// Writes the outline of `doc` into `out`, one node per line, indented two spaces per
/// level.
pub fn dump_tree(doc: Document<'_>, out: &mut (impl TextSink + ?Sized)) {
    let mut ends: Vec<u32> = Vec::new();
    for (i, n) in doc.nodes().iter().enumerate() {
        while ends.last().is_some_and(|&end| end as usize <= i) {
            ends.pop();
        }
        for _ in 0..ends.len() {
            out.put_str("  ");
        }
        out.put_str(kind_name(n.kind));
        if n.value != 0 {
            out.put_str(" value=");
            out.put_str(&n.value.to_string());
        }
        for f in NodeFlag::ALL.iter().filter(|f| n.flags.has(**f)) {
            out.put_str(" +");
            out.put_str(f.name());
        }
        if n.text.len > 0 {
            out.put_char(' ');
            dump_string(out, doc.text(n.text));
        }
        let id = NodeId::from_index(i);
        dump_payload(doc, id, out);
        for a in doc.attributes(id) {
            out.put_str(" @");
            let name = if a.key == AttributeKey::Custom { doc.text(a.name) } else { a.key.name() };
            out.put_str(name);
            out.put_char('=');
            dump_string(out, doc.text(a.value));
        }
        out.put_char('\n');
        ends.push(n.subtree_end);
    }
}

/// Returns the outline of `doc` as a new string (for tests and debugging).
pub fn dump_to_string(doc: Document<'_>) -> String {
    let mut out = String::new();
    dump_tree(doc, &mut out);
    out
}

fn dump_payload(doc: Document<'_>, id: NodeId, out: &mut (impl TextSink + ?Sized)) {
    if doc.node(id).payload == NONE {
        return;
    }
    match payload_of(doc.kind(id)) {
        Payload::None => {}
        Payload::List => {
            let l = doc.list_info(id);
            out.put_str(" list=");
            out.put_str(l.style.name());
            out.put_char(',');
            if l.delimiter != 0 {
                out.put_char(char::from(l.delimiter));
            }
            out.put_char(',');
            out.put_str(&l.start.to_string());
        }
        Payload::Cell => {
            let c = doc.cell_info(id);
            out.put_str(&format!(" span={}x{}", c.row_span, c.col_span));
        }
        Payload::Link => dump_link(doc, doc.link_info(id).unwrap_or(LinkInfo::UNRESOLVED), out),
    }
}

fn dump_link(doc: Document<'_>, l: LinkInfo, out: &mut (impl TextSink + ?Sized)) {
    out.put_str(" link=");
    out.put_str(l.kind.name());
    let parts = [(" dest=", l.destination), (" title=", l.title), (" name=", l.name)];
    for (label, text) in parts {
        if text.len > 0 {
            out.put_str(label);
            dump_string(out, doc.text(text));
        }
    }
    if l.kind == LinkKind::Internal {
        out.put_str(" ->");
        out.put_str(&l.resolved.0.to_string());
    }
}

/// Writes `s` quoted, with quotes, backslashes, newlines, and tabs escaped.
fn dump_string(out: &mut (impl TextSink + ?Sized), s: &str) {
    out.put_char('"');
    for c in s.chars() {
        match c {
            '"' => out.put_str("\\\""),
            '\\' => out.put_str("\\\\"),
            '\n' => out.put_str("\\n"),
            '\t' => out.put_str("\\t"),
            _ => out.put_char(c),
        }
    }
    out.put_char('"');
}
