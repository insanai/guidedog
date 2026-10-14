//! The conversion pipeline: read, transform, check, render, and the one-call forms.
//! Every stage works on the workspace's current document, so there is no handle that
//! could go stale; the returned `Document` and output borrow the workspace.

use crate::adapter::{Reader, Registry, RenderContext, Renderer};
use crate::builder::Builder;
use crate::document::Document;
use crate::emit::Emitter;
use crate::options::{Limits, Pass, PassOutcome, Policy, Request, default_request};
use crate::report::{
    Arg, CORE_NO_DOCUMENT, CORE_OUTPUT_UTF8, CORE_PASS, CORE_STRICT, CORE_UNKNOWN_READER,
    CORE_UNKNOWN_RENDERER, Fail, arg_int, arg_text,
};
use crate::snapshot::Snapshot;
use crate::types::{CapacityNeed, ROOT, Region, SourceSpan, Status};
use crate::workspace::Workspace;

/// Reads the request's source with `reader` into a new conversion and validates it.
/// The document borrows the workspace until its next mutable use. Content errors fail
/// it unless `Policy::keep_documents_with_errors`; strict policy fails on warnings.
pub fn read_document<'w>(
    ws: &'w mut Workspace,
    reader: &Reader,
    request: &Request<'_>,
) -> Result<Document<'w>, Fail> {
    if let Err(fail) = run_reader(ws, reader, request) {
        return Err(ws.failure(fail.status, fail.need, Some(*fail.diagnostic)));
    }
    ws.settle(0, &request.limits, &request.policy)?;
    current_document(ws)
}

/// Runs the reader and closes the root; the builder's failure wins over the reader's
/// result, since the builder knows what ran out.
fn run_reader(ws: &mut Workspace, reader: &Reader, request: &Request<'_>) -> Result<(), Fail> {
    let mut ctx = ws.begin_read(request)?;
    let result = (reader.read)(&mut ctx).and_then(|()| ctx.b.finish());
    match result {
        Err(fail) if !ctx.b.is_ok() => {
            Err(Fail { status: ctx.b.status(), need: ctx.b.need(), ..fail })
        }
        other => other,
    }
}

/// Returns the current document, or the failure of a stage that needs one.
fn current_document(ws: &mut Workspace) -> Result<Document<'_>, Fail> {
    if !ws.has_document {
        ws.reports.message(CORE_NO_DOCUMENT, SourceSpan::default(), Arg::None, Arg::None);
        return Err(ws.failure(Status::InvalidInput, CapacityNeed::default(), None));
    }
    let Workspace { banks, active, main, sources, .. } = ws;
    Ok(Document::new(&banks[*active], main, sources))
}

/// Runs `passes` in order on the current document. A rebuilding pass writes the other
/// snapshot, which is validated before it becomes current; when a pass fails, the
/// previous document stays current. Passes keep the read's error policy.
pub fn transform_document<'w>(
    ws: &'w mut Workspace,
    passes: &[Pass<'_>],
    limits: &Limits,
) -> Result<Document<'w>, Fail> {
    current_document(ws)?;
    for pass in passes {
        let dest = 1 - ws.active;
        let (outcome, status, need) = run_pass(ws, pass, limits);
        match outcome {
            PassOutcome::Unchanged => {}
            PassOutcome::Failed => {
                if status == Status::Ok {
                    ws.reports.message(
                        CORE_PASS,
                        SourceSpan::default(),
                        arg_text(pass.name),
                        Arg::None,
                    );
                    return Err(ws.failure(Status::InvalidInput, need, None));
                }
                return Err(ws.failure(status, need, None));
            }
            PassOutcome::Rebuilt if status != Status::Ok => {
                return Err(ws.failure(status, need, None));
            }
            PassOutcome::Rebuilt => {
                ws.settle(dest, limits, &Policy { strict: false, ..Policy::default() })?
            }
        }
    }
    current_document(ws)
}

/// Runs one pass from the current snapshot into the other: its outcome, and the
/// builder's status and need once a rebuild is finished.
fn run_pass(
    ws: &mut Workspace,
    pass: &Pass<'_>,
    limits: &Limits,
) -> (PassOutcome, Status, CapacityNeed) {
    let Workspace { banks, active, main, sources, reports, open, .. } = ws;
    let (current, dest) = split_banks(banks, *active);
    let doc = Document::new(current, main, sources);
    let root = doc.node(ROOT).span;
    let mut b = Builder::new(dest, reports, open, *limits, root);
    let outcome = if b.is_ok() { (pass.run)(doc, &mut b) } else { PassOutcome::Failed };
    if outcome == PassOutcome::Rebuilt {
        // A failure poisons the builder, whose status is returned below.
        let _ = b.finish();
    }
    (outcome, b.status(), b.need())
}

/// Returns the snapshot at `active` to read and the other one to write.
fn split_banks(banks: &mut [Snapshot; 2], active: usize) -> (&Snapshot, &mut Snapshot) {
    let [first, second] = banks;
    if active == 0 { (first, second) } else { (second, first) }
}

/// Which emitter a render uses.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RenderMode {
    Check,
    Write,
}

/// Runs the renderer over the current document and returns the bytes it wrote (or
/// would have written). Errors it reported fail the render, as in Odin.
fn run_renderer(
    ws: &mut Workspace,
    renderer: &Renderer,
    request: &Request<'_>,
    mode: RenderMode,
) -> Result<usize, Fail> {
    current_document(ws)?;
    let Workspace { banks, active, main, sources, reports, output, source_map, .. } = ws;
    output.clear();
    source_map.clear();
    let doc = Document::new(&banks[*active], main, sources);
    let mut out = match mode {
        RenderMode::Check => Emitter::check(),
        RenderMode::Write => Emitter::with_limit(output, request.limits.max_output_bytes),
    };
    if mode == RenderMode::Write && request.source_map {
        out.set_source_map(Some(source_map));
    }
    let errors_before = reports.error_count();
    let mut ctx = RenderContext {
        doc,
        out,
        options: &request.render,
        policy: request.policy,
        limits: request.limits,
        reports,
    };
    let result = (renderer.render)(&mut ctx);
    let written = ctx.out.written();
    match result {
        Err(fail) => Err(ws.failure(fail.status, fail.need, Some(*fail.diagnostic))),
        Ok(()) if ws.reports.error_count() > errors_before => {
            let status = ws.reports.primary().map_or(Status::Internal, |d| d.status);
            Err(ws.failure(status, CapacityNeed::default(), None))
        }
        Ok(()) => Ok(written),
    }
}

/// Runs the renderer without storing output and returns the exact output size; every
/// support problem is reported as a render would report it.
pub fn check_render(
    ws: &mut Workspace,
    renderer: &Renderer,
    request: &Request<'_>,
) -> Result<usize, Fail> {
    run_renderer(ws, renderer, request, RenderMode::Check)
}

/// Renders the current document into the workspace's output and returns it, valid
/// until the next conversion. Output past `Limits::max_output_bytes` fails with
/// `Capacity` and the exact size needed; strict policy fails on warnings.
pub fn render_document<'w>(
    ws: &'w mut Workspace,
    renderer: &Renderer,
    request: &Request<'_>,
) -> Result<&'w str, Fail> {
    let written = run_renderer(ws, renderer, request, RenderMode::Write)?;
    if written > request.limits.max_output_bytes {
        ws.output.clear();
        let fail = Fail::capacity(Region::Output, written as u64, true);
        return Err(ws.failure(Status::Capacity, fail.need, Some(*fail.diagnostic)));
    }
    let warnings = ws.reports.warning_count();
    if request.policy.strict && warnings > 0 {
        let count = i64::try_from(warnings).unwrap_or(i64::MAX);
        ws.reports.message(CORE_STRICT, SourceSpan::default(), arg_int(count), Arg::None);
        return Err(ws.failure(Status::Policy, CapacityNeed::default(), None));
    }
    if let Err(e) = std::str::from_utf8(&ws.output) {
        let at = i64::try_from(e.valid_up_to()).unwrap_or(i64::MAX);
        ws.reports.message(CORE_OUTPUT_UTF8, SourceSpan::default(), arg_int(at), Arg::None);
        return Err(ws.failure(Status::Internal, CapacityNeed::default(), None));
    }
    Ok(std::str::from_utf8(&ws.output).unwrap_or(""))
}

/// Reads, transforms (when the request has passes), and renders one source, with the
/// adapters `registry` names. It checks the registry first. The output borrows the
/// workspace until its next conversion; on failure the workspace's reports explain it
/// and the `Fail` carries the primary diagnostic.
pub fn convert<'w>(
    ws: &'w mut Workspace,
    registry: &Registry,
    request: &Request<'_>,
) -> Result<&'w str, Fail> {
    if let Some(d) = registry.problem() {
        ws.begin_generation(&request.limits);
        ws.reports.report(d);
        return Err(ws.failure(Status::InvalidInput, CapacityNeed::default(), None));
    }
    let Some(reader) = registry.find_reader(request.reader) else {
        ws.begin_generation(&request.limits);
        ws.reports.message(
            CORE_UNKNOWN_READER,
            SourceSpan::default(),
            arg_text(request.reader),
            Arg::None,
        );
        return Err(ws.failure(Status::Unsupported, CapacityNeed::default(), None));
    };
    let Some(renderer) = registry.find_renderer(request.renderer) else {
        ws.begin_generation(&request.limits);
        ws.reports.message(
            CORE_UNKNOWN_RENDERER,
            SourceSpan::default(),
            arg_text(request.renderer),
            Arg::None,
        );
        return Err(ws.failure(Status::Unsupported, CapacityNeed::default(), None));
    };
    read_document(ws, reader, request)?;
    if !request.passes.is_empty() {
        transform_document(ws, request.passes, &request.limits)?;
    }
    render_document(ws, renderer, request)
}

/// Converts `text` with `default_request` and the workspace's limits: the one-call
/// path. `guidedoc::convert_text(&mut ws, &registry, "commonmark", "html", text)`.
pub fn convert_text<'w>(
    ws: &'w mut Workspace,
    registry: &Registry,
    reader: &str,
    renderer: &str,
    text: &str,
) -> Result<&'w str, Fail> {
    let request =
        Request { limits: *ws.limits(), ..default_request("input", text, reader, renderer) };
    convert(ws, registry, &request)
}
