//! Includes, anchors, and steady-state allocation.

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;

use common::{TOY_READER, toy_registry};
use guidedoc::{
    AttributeKey, Fail, NodeFlags, NodeKind, ReadContext, Reader, RenderOptions, Request,
    Resource, ResourceProvider, ResourceStatus, SourceId, SourceSpan, SourceUse, Workspace,
    build_anchors, convert_text, read_document,
};

/// Counts allocations made on the current thread, so a test can assert a conversion
/// in steady state allocates nothing.
struct Counting;

thread_local! {
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

// SAFETY: every call is forwarded unchanged to the system allocator; the counter is a
// thread-local cell that never allocates.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.with(|c| c.set(c.get() + 1));
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract, passed on as is.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `System.alloc` with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCATIONS.with(|c| c.set(c.get() + 1));
        // SAFETY: as for `dealloc`; `size` is the caller's valid new size.
        unsafe { System.realloc(ptr, layout, size) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

#[test]
fn steady_state_conversion_does_not_allocate() {
    let mut ws = Workspace::new();
    let registry = toy_registry();
    let text = "one\ntwo\n!\nthree\n";
    assert!(convert_text(&mut ws, &registry, "toy", "toy", text).is_ok());
    let before = ALLOCATIONS.with(Cell::get);
    let html = convert_text(&mut ws, &registry, "toy", "toy", text).map(str::len);
    let after = ALLOCATIONS.with(Cell::get);
    assert_eq!(html, Ok(32));
    assert_eq!(after - before, 0);
}

/// Provides "a.txt" (which includes "b.txt"), "b.txt", and "loop.txt" (itself).
struct Files;

impl ResourceProvider for Files {
    fn provide(&self, _from: &str, name: &str) -> Result<Resource, ResourceStatus> {
        let text = match name {
            "a.txt" => "alpha",
            "b.txt" => "beta",
            "loop.txt" => "again",
            "secret" => return Err(ResourceStatus::Refused),
            _ => return Err(ResourceStatus::Missing),
        };
        Ok(Resource { text: Arc::from(text), canonical: name.to_owned() })
    }
}

/// Includes each line's file name and writes the included text as a paragraph.
fn include_read(ctx: &mut ReadContext<'_>) -> Result<(), Fail> {
    let source = ctx.source_of(ctx.source);
    let mut offset = 0u32;
    for name in source.split('\n').filter(|l| !l.is_empty()) {
        let span = SourceSpan::new(ctx.source, offset, offset + name.len() as u32);
        offset += name.len() as u32 + 1;
        let Some(id) = ctx.include_source(ctx.source, name, span, SourceUse::Parsed) else {
            continue;
        };
        let included = ctx.source_of(id);
        if name == "loop.txt" {
            let again = SourceSpan::new(id, 0, 5);
            assert!(ctx.include_source(id, "loop.txt", again, SourceUse::Parsed).is_none());
        }
        let whole = SourceSpan::new(id, 0, included.len() as u32);
        ctx.b.with_node(NodeKind::Paragraph, whole, |b, _| {
            b.add_text_leaf(NodeKind::Text, whole, &included).map(|_| ())
        })?;
    }
    Ok(())
}

#[test]
fn includes_are_checked_and_recorded() {
    let reader = Reader { read: include_read, ..TOY_READER };
    let mut ws = Workspace::new();
    let mut request =
        Request { resources: Some(&Files), ..common::toy_request("a.txt\nloop.txt") };
    request.policy.keep_documents_with_errors = true;
    let texts = read_document(&mut ws, &reader, &request).map(|doc| {
        (1..doc.sources().len())
            .map(|i| doc.source_str(SourceId(i as u16)).to_owned())
            .collect::<Vec<_>>()
    });
    assert_eq!(texts, Ok(vec!["alpha".to_owned(), "again".to_owned()]));
    let codes: Vec<_> = ws.reports().items().iter().map(|d| d.code).collect();
    assert_eq!(codes, ["core.include.cycle"]);
    let refused = Request { resources: Some(&Files), ..common::toy_request("secret\nnone") };
    let fail = read_document(&mut ws, &reader, &refused).err();
    assert_eq!(fail.map(|f| f.code()), Some("core.include.refused"));
    let codes: Vec<_> = ws.reports().items().iter().map(|d| d.code).collect();
    assert_eq!(codes, ["core.include.refused", "core.include.missing"]);
    let none = common::toy_request("a.txt");
    let fail = read_document(&mut ws, &reader, &none).err();
    assert_eq!(fail.map(|f| f.code()), Some("core.include.no-provider"));
}

/// Two sections titled alike, a contents table, a label, and a footnote reference.
fn anchor_read(ctx: &mut ReadContext<'_>) -> Result<(), Fail> {
    let b = &mut ctx.b;
    let span = SourceSpan::new(SourceId::MAIN, 0, 1);
    let contents = b.begin_node(NodeKind::Contents, span)?;
    b.end_node(contents)?;
    for title in ["Hello World", "Hello World"] {
        let section = b.begin_node(NodeKind::Section, span)?;
        b.set_value(section, 1);
        let heading = b.begin_node(NodeKind::Heading, span)?;
        b.set_value(heading, 1);
        b.add_text_leaf(NodeKind::Text, span, title)?;
        b.end_node(heading)?;
        b.with_node(NodeKind::Paragraph, span, |b, p| {
            b.add_attribute_string(p, AttributeKey::Id, "explicit")?;
            let note = b.add_text_leaf(NodeKind::FootnoteReference, span, "1")?;
            b.add_flags(note, NodeFlags::EMPTY);
            Ok(())
        })?;
        b.end_node(section)?;
    }
    Ok(())
}

#[test]
fn anchors_follow_docutils_order() {
    let reader = Reader { read: anchor_read, ..TOY_READER };
    let mut ws = Workspace::new();
    let request = common::toy_request("x");
    let Ok(doc) = read_document(&mut ws, &reader, &request) else {
        panic!("the anchor document reads")
    };
    let options = RenderOptions { anchor_prefix: "Doc", ..RenderOptions::default() };
    let anchors = build_anchors(doc, &options, true);
    let section = |n: usize| doc.children(guidedoc::ROOT).nth(n).unwrap_or(guidedoc::ROOT);
    assert_eq!(anchors.anchor_of(section(0)), Some("doc-contents"));
    assert_eq!(anchors.anchor_of(section(1)), Some("doc-hello-world"));
    assert_eq!(anchors.anchor_of(section(2)), Some("doc-id3"));
    let paragraph = section(1) + 3;
    assert_eq!(anchors.anchor_ids(paragraph).collect::<Vec<_>>(), ["doc-explicit"]);
    assert_eq!(anchors.anchor_of(paragraph + 1), Some("doc-id1"));
    let extras = anchors.anchor_extras(section(0));
    assert_eq!(extras.len(), 2);
    assert_eq!(anchors.extra_anchor(section(0), section(2)), Some("doc-id5"));
}
