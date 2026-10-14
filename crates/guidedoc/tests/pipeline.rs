//! The pipeline end to end with the toy adapters: the Rust form of the Odin
//! `core_test.odin`. Tests of the slab and bank machinery the port dropped are replaced
//! by tests of what took their place (limits, `Option` documents, lifetimes).

mod common;

use common::{TOY_ERROR, TOY_READER, TOY_RENDERER, copy_all, toy_registry, toy_request};
use guidedoc::{
    Builder, CapacityNeed, Conformance, Fail, Limits, NodeKind, Pass, PassOutcome, ROOT, Reader,
    Region, Registry, Renderer, Request, Severity, SourceId, SourceSpan, Status, Workspace,
    check_render, check_utf8, convert, convert_text, dump_to_string, kept_message, load_object,
    read_document, render_document, transform_document, validate, write_object,
};

#[test]
fn conversions_report_warnings_and_honour_strict() {
    let mut ws = Workspace::new();
    let registry = toy_registry();
    let html = convert(&mut ws, &registry, &toy_request("a\nb\n!\n"));
    assert_eq!(html, Ok("<p>a</p><p>b</p>"));
    assert_eq!(ws.reports().items().len(), 1);
    let mut strict = toy_request("a\n!\n");
    strict.policy.strict = true;
    assert_eq!(convert(&mut ws, &registry, &strict).map_err(|f| f.status), Err(Status::Policy));
    let bad = check_utf8(SourceId::MAIN, b"\xff").map_err(|f| f.status);
    assert_eq!(bad, Err(Status::InvalidInput));
    assert_eq!(
        convert_text(&mut ws, &registry, "toy", "toy", "one\ntwo\n"),
        Ok("<p>one</p><p>two</p>")
    );
}

#[test]
fn output_limit_is_exact() {
    let mut ws = Workspace::new();
    let registry = toy_registry();
    let mut request = toy_request("abc\n");
    request.limits.max_output_bytes = 9;
    let fail = convert(&mut ws, &registry, &request).err();
    assert_eq!(fail.as_ref().map(|f| f.status), Some(Status::Capacity));
    let need = CapacityNeed { region: Region::Output, minimum: 10, exact: true };
    assert_eq!(fail.map(|f| f.need), Some(need));
    request.limits.max_output_bytes = 10;
    assert_eq!(convert(&mut ws, &registry, &request).map(str::len), Ok(10));
    assert_eq!(check_render(&mut ws, &TOY_RENDERER, &request), Ok(10));
}

#[test]
fn node_and_text_limits_name_their_cause() {
    let mut ws = Workspace::new();
    let registry = toy_registry();
    // Root plus one paragraph and its text need exactly three nodes.
    let mut request = toy_request("a");
    request.limits.max_nodes = 3;
    assert!(convert(&mut ws, &registry, &request).is_ok());
    request.limits.max_nodes = 2;
    let fail = convert(&mut ws, &registry, &request).err();
    assert_eq!(fail.map(|f| (f.status, f.code())), Some((Status::Limit, "core.limit.nodes")));
    let mut request = toy_request("abcdef");
    request.limits.max_text_bytes = 4;
    let fail = convert(&mut ws, &registry, &request).err();
    assert_eq!(fail.as_ref().map(|f| f.need.region), Some(Region::Text));
    assert_eq!(fail.map(|f| f.code()), Some("core.capacity"));
}

#[test]
fn stages_need_a_document() {
    let mut ws = Workspace::new();
    let request = toy_request("a");
    let fail = render_document(&mut ws, &TOY_RENDERER, &request).err();
    assert_eq!(fail.map(|f| f.code()), Some("core.document.none"));
    assert!(read_document(&mut ws, &TOY_READER, &request).is_ok());
    assert!(ws.document().is_some());
    ws.reset();
    assert!(ws.document().is_none());
}

#[test]
fn passes_rebuild_into_the_other_snapshot() {
    let mut ws = Workspace::new();
    let request = toy_request("a\nb");
    let before = read_document(&mut ws, &TOY_READER, &request).map(dump_to_string);
    let pass = Pass { name: "copy", run: &copy_all };
    let after = transform_document(&mut ws, &[pass, pass], &request.limits).map(dump_to_string);
    assert_eq!(before, after);
    assert_eq!(render_document(&mut ws, &TOY_RENDERER, &request), Ok("<p>a</p><p>b</p>"));
}

#[test]
fn failed_passes_keep_the_previous_document() {
    let mut ws = Workspace::new();
    let request = toy_request("a");
    assert!(read_document(&mut ws, &TOY_READER, &request).is_ok());
    let failing = |_: guidedoc::Document<'_>, _: &mut Builder<'_>| PassOutcome::Failed;
    let pass = Pass { name: "broken", run: &failing };
    let fail = transform_document(&mut ws, &[pass], &request.limits).err();
    assert_eq!(fail.map(|f| f.code()), Some("core.pass.failed"));
    assert_eq!(ws.document().map(|d| d.node_count()), Some(3));
}

#[test]
fn invalid_registries_are_refused() {
    let mut ws = Workspace::new();
    let twice = Registry { readers: vec![TOY_READER, TOY_READER], ..toy_registry() };
    let unnamed =
        Registry { renderers: vec![Renderer { version: "", ..TOY_RENDERER }], ..toy_registry() };
    for registry in [twice, unnamed] {
        assert_eq!(registry.validate().map_err(|f| f.status), Err(Status::InvalidInput));
        let fail = convert(&mut ws, &registry, &toy_request("a")).err();
        assert_eq!(fail.map(|f| f.code()), Some("core.registry.invalid"));
    }
    assert!(toy_registry().validate().is_ok());
    let request = Request { reader: "none", ..toy_request("a") };
    let fail = convert(&mut ws, &toy_registry(), &request).err();
    assert_eq!(
        fail.map(|f| (f.status, f.code())),
        Some((Status::Unsupported, "core.reader.unknown"))
    );
}

#[test]
fn malformed_trees_fail_validation() {
    let mut ws = Workspace::new();
    let request = toy_request("a");
    let snap = read_document(&mut ws, &TOY_READER, &request).map(|d| d.snapshot().clone());
    let Ok(mut snap) = snap else { panic!("the toy reads one line") };
    let limits = Limits::default();
    snap.nodes[2].subtree_end = 9;
    assert_eq!(validate(&snap, &[1], &limits).map_err(|f| f.code()), Err("core.document.invalid"));
    snap.nodes[2].subtree_end = 3;
    snap.nodes[2].kind = NodeKind::Section;
    assert!(validate(&snap, &[1], &limits).is_err());
    snap.nodes[2].kind = NodeKind::Text;
    snap.nodes[2].text = guidedoc::Text { start: 250, len: 10 };
    assert_eq!(validate(&snap, &[1], &limits).map_err(|f| f.status), Err(Status::Internal));
    snap.nodes[2].text = guidedoc::Text::default();
    snap.text = vec![0xff];
    assert_eq!(validate(&snap, &[1], &limits).map_err(|f| f.status), Err(Status::Internal));
}

#[test]
fn objects_round_trip_and_reject_damage() {
    let mut ws = Workspace::new();
    let request = toy_request("alpha\nbeta");
    let mut bytes = Vec::new();
    let before = read_document(&mut ws, &TOY_READER, &request).map(|doc| {
        write_object(doc, &mut bytes);
        dump_to_string(doc)
    });
    let limits = Limits::default();
    let after = load_object(&mut ws, &bytes, &limits).map(dump_to_string);
    assert_eq!(before, after);
    assert_eq!(
        ws.document().and_then(|d| d.source_entry(SourceId::MAIN)).map(|e| e.name.as_str()),
        Some("toy.txt")
    );
    assert_eq!(ws.source_str(SourceId::MAIN), "alpha\nbeta");
    let mut damaged = bytes.clone();
    damaged[20] = 0xee; // the node count
    assert!(load_object(&mut ws, &damaged, &limits).is_err());
    assert!(load_object(&mut ws, &bytes[..10], &limits).is_err());
    assert!(ws.document().is_none());
}

#[test]
fn content_errors_fail_unless_the_policy_keeps_the_document() {
    let mut ws = Workspace::new();
    let registry = toy_registry();
    let refused = convert(&mut ws, &registry, &toy_request("a\n?\nb\n")).err();
    assert_eq!(refused.map(|f| f.status), Some(Status::Unsupported));
    let mut keep = toy_request("a\n?\nb\n");
    keep.policy.keep_documents_with_errors = true;
    assert_eq!(convert(&mut ws, &registry, &keep), Ok("<p>a</p><p>b</p>"));
    let reports = ws.reports();
    assert_eq!(reports.items().len(), 1);
    assert_eq!(reports.items()[0].severity, Severity::Error);
    assert_eq!(reports.primary().map(|d| d.code), Some("toy.error"));
    keep.source.text = "a\n?\n%\n";
    assert_eq!(convert(&mut ws, &registry, &keep).map_err(|f| f.status), Err(Status::Capacity));
    keep.source.text = "a\n?\n";
    keep.policy.strict = true;
    assert_eq!(convert(&mut ws, &registry, &keep).map_err(|f| f.status), Err(Status::Policy));
    assert_eq!(kept_message(&keep.policy, TOY_ERROR).severity, Severity::Warning);
    assert_eq!(kept_message(&Default::default(), TOY_ERROR).severity, Severity::Error);
}

#[test]
fn passes_keep_documents_with_content_errors() {
    let mut ws = Workspace::new();
    let mut keep = toy_request("a\n?\n");
    keep.policy.keep_documents_with_errors = true;
    assert!(read_document(&mut ws, &TOY_READER, &keep).is_ok());
    let pass = Pass { name: "copy", run: &copy_all };
    assert!(transform_document(&mut ws, &[pass], &keep.limits).is_ok());
}

fn deep_read(ctx: &mut guidedoc::ReadContext<'_>) -> Result<(), Fail> {
    for _ in 0..400 {
        ctx.b.begin_node(NodeKind::BlockQuote, SourceSpan::default())?;
    }
    while let Some(top) = ctx.b.current().filter(|&id| id != ROOT) {
        ctx.b.end_node(top)?;
    }
    Ok(())
}

#[test]
fn passes_copy_deeper_than_256_levels() {
    let deep = Reader { id: "deep", read: deep_read, ..TOY_READER };
    let registry = Registry { readers: vec![deep], ..toy_registry() };
    let pass = Pass { name: "copy", run: &copy_all };
    let passes = [pass];
    let mut request = Request { reader: "deep", passes: &passes, ..toy_request("x") };
    request.limits.max_depth = 1000;
    assert_eq!(convert(&mut Workspace::new(), &registry, &request), Ok("<p></p>"));
    let mut ws = Workspace::new();
    assert!(read_document(&mut ws, &deep, &request).is_ok());
    assert_eq!(
        transform_document(&mut ws, &passes, &request.limits).map(|d| d.node_count()),
        Ok(401)
    );
}

fn hungry_render(_: &mut guidedoc::RenderContext<'_>) -> Result<(), Fail> {
    Err(Fail::capacity(Region::Scratch, 1 << 20, false))
}

#[test]
fn renderer_capacity_needs_reach_the_caller() {
    let hungry = Renderer {
        id: "hungry",
        render: hungry_render,
        conformance: Conformance::Planned,
        ..TOY_RENDERER
    };
    let registry = Registry { renderers: vec![hungry], ..toy_registry() };
    let request = Request { renderer: "hungry", ..toy_request("a") };
    let fail = convert(&mut Workspace::new(), &registry, &request).err();
    assert_eq!(fail.as_ref().map(|f| f.need.region), Some(Region::Scratch));
    assert_eq!(fail.map(|f| f.code()), Some("core.capacity"));
}

#[test]
fn offset_in_accepts_only_slices_of_the_text() {
    let text = "alpha beta gamma";
    assert_eq!(guidedoc::offset_in(text, &text[6..10]), Some(6));
    assert_eq!(guidedoc::offset_in(text, &text[11..]), Some(11));
    assert_eq!(guidedoc::offset_in(text, "beta"), None);
    assert_eq!(guidedoc::offset_in(&text[..8], &text[6..10]), None);
    assert_eq!(guidedoc::offset_in(text, ""), None);
}
