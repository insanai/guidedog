//! An object is a document serialized after reading and resolving, so a build can reuse
//! it instead of parsing its source again. Fields are written one by one in
//! little-endian order, never as raw structs, so the bytes are the same on every
//! platform and build. The source table travels with the object: spans stay meaningful
//! and diagnostics can quote source text. Loading validates everything, as a reader's
//! output is validated.
//!
//! Format version 2: magic `GDOB`,
//! the format and schema versions, seven counts (nodes, attributes, lists, links, cells,
//! text bytes, sources), the sources (name, text, parent, include span, usage), the
//! tables in that order, and the text pool.

use std::sync::Arc;

use crate::SCHEMA_VERSION;
use crate::document::Document;
use crate::options::{Limits, Policy};
use crate::report::{Arg, Fail, Message, Severity, arg_static};
use crate::snapshot::{SourceEntry, SourceUse};
use crate::types::{
    Align, Attribute, AttributeKey, AttributeRange, CapacityNeed, CellInfo, LinkInfo, LinkKind,
    ListInfo, ListStyle, Node, NodeFlags, NodeId, NodeKind, SourceId, SourceSpan, Status, Text,
};
use crate::workspace::Workspace;

const OBJECT_MAGIC: &[u8; 4] = b"GDOB";
const OBJECT_VERSION: u32 = 2;

/// The bytes do not hold a usable document.
const OBJECT_INVALID: Message = Message {
    code: "core.object.invalid",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "CACHED DOCUMENT IS UNUSABLE",
    message: "The cached document is {0}.",
    hint: "Rebuild without the cache (guidedog build -E); the cache is rewritten.",
};

/// Appends `doc`, with its sources, to `out` as object bytes.
pub fn write_object(doc: Document<'_>, out: &mut Vec<u8>) {
    let s = doc.snapshot();
    out.extend_from_slice(OBJECT_MAGIC);
    put_u32(out, OBJECT_VERSION);
    put_u32(out, SCHEMA_VERSION);
    let counts = [
        s.nodes.len(),
        s.attributes.len(),
        s.lists.len(),
        s.links.len(),
        s.cells.len(),
        s.text.len(),
        doc.sources().len(),
    ];
    for c in counts {
        put_u32(out, u32::try_from(c).unwrap_or(u32::MAX));
    }
    for (i, src) in doc.sources().iter().enumerate() {
        put_bytes(out, src.name.as_bytes());
        put_bytes(out, doc.source_str(SourceId(u16::try_from(i).unwrap_or(u16::MAX))).as_bytes());
        put_u32(out, u32::from(src.parent.0));
        put_span(out, src.included_at);
        out.push(src.usage as u8);
    }
    for n in &s.nodes {
        write_node(out, n);
    }
    for a in &s.attributes {
        out.push(a.key as u8);
        put_text(out, a.name);
        put_text(out, a.value);
    }
    for l in &s.lists {
        out.push(l.style as u8);
        out.push(l.delimiter);
        put_u32(out, l.start as u32);
    }
    for l in &s.links {
        put_text(out, l.destination);
        put_text(out, l.title);
        put_text(out, l.name);
        put_u32(out, l.resolved.0);
        out.push(l.kind as u8);
    }
    for c in &s.cells {
        put_u32(out, u32::from(c.row_span) | (u32::from(c.col_span) << 16));
        out.push(c.align as u8);
    }
    out.extend_from_slice(&s.text);
}

fn write_node(out: &mut Vec<u8>, n: &Node) {
    out.push(n.kind as u8);
    out.push(n.flags.0);
    put_u32(out, n.subtree_end);
    put_u32(out, n.value as u32);
    put_text(out, n.text);
    put_u32(out, n.payload);
    put_span(out, n.span);
    put_u32(out, n.attributes.first);
    put_u32(out, n.attributes.count);
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_text(out: &mut Vec<u8>, t: Text) {
    put_u32(out, t.start);
    put_u32(out, t.len);
}

fn put_span(out: &mut Vec<u8>, s: SourceSpan) {
    put_u32(out, u32::from(s.source.0));
    put_u32(out, s.start);
    put_u32(out, s.end);
}

fn put_bytes(out: &mut Vec<u8>, b: &[u8]) {
    put_u32(out, u32::try_from(b.len()).unwrap_or(u32::MAX));
    out.extend_from_slice(b);
}

/// Reads fields from object bytes; a read past the end or a bad value marks it bad.
struct ObjectReader<'a> {
    data: &'a [u8],
    at: usize,
    bad: bool,
}

impl<'a> ObjectReader<'a> {
    fn bytes(&mut self, n: usize) -> &'a [u8] {
        match self.data.get(self.at..self.at.saturating_add(n)) {
            Some(b) if !self.bad => {
                self.at += n;
                b
            }
            _ => {
                self.bad = true;
                &[]
            }
        }
    }

    fn u8(&mut self) -> u8 {
        self.bytes(1).first().copied().unwrap_or(0)
    }

    fn u32(&mut self) -> u32 {
        let b = self.bytes(4);
        b.try_into().map(u32::from_le_bytes).unwrap_or(0)
    }

    fn text(&mut self) -> Text {
        Text { start: self.u32(), len: self.u32() }
    }

    fn span(&mut self) -> SourceSpan {
        let source = self.u32();
        let (start, end) = (self.u32(), self.u32());
        let source = u16::try_from(source).unwrap_or_else(|_| {
            self.bad = true;
            0
        });
        SourceSpan::new(SourceId(source), start, end)
    }

    fn string(&mut self) -> &'a str {
        let n = self.u32() as usize;
        let b = self.bytes(n);
        std::str::from_utf8(b).unwrap_or_else(|_| {
            self.bad = true;
            ""
        })
    }

    /// Returns an enum value from its table, marking the reader bad when out of range.
    fn pick<T: Copy + Default>(&mut self, all: &[T]) -> T {
        let v = self.u8() as usize;
        all.get(v).copied().unwrap_or_else(|| {
            self.bad = true;
            T::default()
        })
    }

    /// Tells whether `count` rows of at least `row_bytes` each can still follow, so a
    /// damaged count cannot make the loader reserve memory the bytes cannot fill.
    fn can_hold(&self, count: usize, row_bytes: usize) -> bool {
        count.saturating_mul(row_bytes) <= self.data.len().saturating_sub(self.at)
    }
}

/// Rebuilds a document from bytes `write_object` wrote, as a new conversion of `ws`,
/// and validates it. The bytes are copied; they need not outlive the document.
pub fn load_object<'w>(
    ws: &'w mut Workspace,
    bytes: &[u8],
    limits: &Limits,
) -> Result<Document<'w>, Fail> {
    ws.begin_generation(limits);
    let mut r = ObjectReader { data: bytes, at: 0, bad: false };
    if r.bytes(4) != OBJECT_MAGIC || r.u32() != OBJECT_VERSION {
        return Err(object_invalid(ws, "from another format version"));
    }
    if r.u32() != SCHEMA_VERSION {
        return Err(object_invalid(ws, "from another document schema"));
    }
    let mut counts = [0usize; 7];
    for c in &mut counts {
        *c = r.u32() as usize;
    }
    if r.bad || !r.can_hold(counts[6], 21) {
        return Err(object_invalid(ws, "truncated"));
    }
    read_sources(&mut r, ws, counts[6]);
    read_tables(&mut r, ws, counts);
    if r.bad || r.at != bytes.len() {
        return Err(object_invalid(ws, "truncated or damaged"));
    }
    ws.settle(0, limits, &Policy { strict: false, ..Policy::default() })?;
    ws.document().ok_or_else(|| Fail::new(Status::Internal))
}

fn object_invalid(ws: &mut Workspace, why: &'static str) -> Fail {
    ws.banks[0].clear();
    ws.sources.clear();
    ws.main.clear();
    ws.reports.message(OBJECT_INVALID, SourceSpan::default(), arg_static(why), Arg::None);
    ws.failure(Status::InvalidInput, CapacityNeed::default(), None)
}

fn read_sources(r: &mut ObjectReader<'_>, ws: &mut Workspace, count: usize) {
    for i in 0..count {
        let name = r.string().to_owned();
        let text = r.string();
        let parent = SourceId(u16::try_from(r.u32()).unwrap_or(0));
        let included_at = r.span();
        let usage = r.pick(&[SourceUse::Parsed, SourceUse::Literal]);
        if r.bad {
            return;
        }
        let mut entry = SourceEntry { name, parent, included_at, usage, shared: None };
        if i == 0 {
            ws.main.push_str(text);
        } else {
            entry.shared = Some(Arc::from(text));
        }
        ws.sources.push(entry);
    }
}

fn read_tables(r: &mut ObjectReader<'_>, ws: &mut Workspace, c: [usize; 7]) {
    let s = &mut ws.banks[0];
    let sizes = [42, 17, 6, 33, 5, 1];
    if (0..6).any(|i| !r.can_hold(c[i], sizes[i])) {
        r.bad = true;
        return;
    }
    s.nodes.reserve(c[0]);
    for _ in 0..c[0] {
        s.nodes.push(read_node(r));
    }
    for _ in 0..c[1] {
        let key = r.pick(&AttributeKey::ALL);
        s.attributes.push(Attribute { key, name: r.text(), value: r.text() });
    }
    for _ in 0..c[2] {
        let style = r.pick(&ListStyle::ALL);
        s.lists.push(ListInfo { style, delimiter: r.u8(), start: r.u32() as i32 });
    }
    for _ in 0..c[3] {
        let (destination, title, name) = (r.text(), r.text(), r.text());
        let resolved = NodeId(r.u32());
        let kind = r.pick(&LinkKind::ALL);
        s.links.push(LinkInfo { destination, title, name, resolved, kind });
    }
    for _ in 0..c[4] {
        let spans = r.u32();
        let align = r.pick(&Align::ALL);
        s.cells.push(CellInfo {
            row_span: (spans & 0xffff) as u16,
            col_span: (spans >> 16) as u16,
            align,
        });
    }
    s.text.extend_from_slice(r.bytes(c[5]));
}

fn read_node(r: &mut ObjectReader<'_>) -> Node {
    let kind = r.pick(NodeKind::ALL.as_slice());
    // Every byte is a valid flag set: NodeFlag has eight members.
    let flags = NodeFlags(r.u8());
    let subtree_end = r.u32();
    let value = r.u32() as i32;
    let text = r.text();
    let payload = r.u32();
    let span = r.span();
    let attributes = AttributeRange { first: r.u32(), count: r.u32() };
    Node { kind, flags, subtree_end, value, text, payload, span, attributes }
}
