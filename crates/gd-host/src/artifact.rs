//! The route `guidedog convert` takes: a source file (or standard input) converted to
//! HTML, or through the Typst renderer and compiler to PDF, and published atomically.

use std::fmt;
use std::path::{Path, PathBuf};

use gd_report::Report;
use guidedoc::{Limits, Policy, ReadOptions, Registry, RenderOptions, Request, Source};

use crate::budget::{Budget, DEFAULT_BUDGET, Ledger, budget_problem, limits_within};
use crate::confine::Files;
use crate::diagnostics::{display_path, exit_code};
use crate::files::{MAX_SOURCE_BYTES, SourceFile, read_source};
use crate::interrupt::{cancel_probe, was_interrupted};
use crate::problem::{Problem, interrupted_problem, io_problem};
use crate::publish::{Stage, exists_problem, publish};
use crate::session::Session;

/// The markup `convert_artifact` reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Format {
    /// reStructuredText (reader `rst`).
    #[default]
    Rst,
    /// CommonMark (reader `commonmark`).
    Commonmark,
    /// Native Typst, compiled as it is.
    Typst,
}

/// The artifact `convert_artifact` publishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Target {
    /// An HTML page (renderer `html`; Typst's HTML export for Typst input).
    #[default]
    Html,
    /// A PDF document (renderer `typst`, then the Typst compiler).
    Pdf,
}

/// One conversion of `guidedog convert`. Everything is borrowed for the call; fill it
/// with struct-update syntax from `Default`.
#[derive(Default)]
pub struct ArtifactRequest<'a> {
    /// The input path, or `"-"` for standard input.
    pub input: &'a str,
    /// The markup of the input.
    pub from: Format,
    /// The artifact to publish.
    pub to: Target,
    /// The output path, or `"-"` for standard output.
    pub output: &'a str,
    /// Where to publish the intermediate Typst source too, if anywhere.
    pub emit_typst: Option<&'a Path>,
    /// Replace existing outputs.
    pub force: bool,
    /// The conversion's trust decisions.
    pub policy: Policy,
    /// The conversion's bounds; their byte bounds are lowered to the budget.
    pub limits: Limits,
    /// Read options.
    pub read: ReadOptions<'a>,
    /// Render options.
    pub render: RenderOptions<'a>,
    /// The Typst program name or path; `""` or `"typst"` uses the embedded compiler.
    pub typst_program: &'a str,
    /// Typst packages and fonts; `reserve` is set by the route (it charges the budget).
    pub typst_options: gd_typst::Options<'a>,
    /// Approved roots for included files; the input's folder when empty.
    pub roots: &'a [PathBuf],
    /// Memory budget in bytes for what the route holds; 0 is `DEFAULT_BUDGET`.
    pub budget: usize,
}

impl fmt::Debug for ArtifactRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArtifactRequest")
            .field("input", &self.input)
            .field("from", &self.from)
            .field("to", &self.to)
            .field("output", &self.output)
            .field("force", &self.force)
            .field("budget", &self.budget)
            .finish_non_exhaustive()
    }
}

/// Where a successful conversion's artifact went.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Delivered {
    /// Nothing was delivered (the conversion failed).
    #[default]
    Nothing,
    /// Published at this path.
    File(PathBuf),
    /// The bytes to print, for output `"-"`.
    Stdout(Vec<u8>),
}

/// What `convert_artifact` did. Reports are kept whatever the outcome; `exit` is 0 on
/// success, else the status the command ends with, explained by `problem` or by the
/// error reports.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArtifactResult {
    /// The process exit status.
    pub exit: i32,
    /// The conversion's and Typst's diagnostics, ready to render.
    pub reports: Vec<Report>,
    /// The host failure, if one stopped the route.
    pub problem: Option<Problem>,
    /// Where the artifact went.
    pub delivered: Delivered,
}

impl ArtifactResult {
    fn failed(problem: Problem, reports: Vec<Report>) -> ArtifactResult {
        ArtifactResult {
            exit: problem.exit,
            reports,
            problem: Some(problem),
            ..Default::default()
        }
    }

    fn done(delivered: Delivered, reports: Vec<Report>) -> ArtifactResult {
        ArtifactResult { reports, delivered, ..Default::default() }
    }
}

/// The input as read, and where its includes and Typst files are resolved from.
struct Input {
    file: SourceFile,
    root: PathBuf,
}

/// Converts one file to a published HTML or PDF artifact: reads the input within
/// `MAX_SOURCE_BYTES`, converts it with `registry` (readers `rst`/`commonmark`,
/// renderers `html`/`typst`) and includes confined to the roots, runs Typst for PDF,
/// and publishes atomically, or returns the bytes for `"-"`. The budget bounds what the
/// route holds: the source, its includes, and the PDF the embedded compiler returns.
/// A failure leaves any existing output unchanged.
pub fn convert_artifact(registry: &Registry, req: &ArtifactRequest<'_>) -> ArtifactResult {
    let budget = Budget::new(if req.budget == 0 { DEFAULT_BUDGET } else { req.budget });
    let ledger = Ledger::new(&budget);
    let input = match read_input(req, &ledger) {
        Ok(input) => input,
        Err(problem) => return ArtifactResult::failed(problem, Vec::new()),
    };
    if req.from == Format::Typst {
        return native_typst(req, &input, &ledger);
    }
    let roots = if req.roots.is_empty() { std::slice::from_ref(&input.root) } else { req.roots };
    let files = Files::new(roots).with_ledger(&ledger);
    let request = Request {
        source: Source { name: &input.file.canonical, text: &input.file.text },
        reader: if req.from == Format::Rst { "rst" } else { "commonmark" },
        renderer: if req.to == Target::Html { "html" } else { "typst" },
        read: req.read,
        render: req.render,
        policy: req.policy,
        limits: limits_within(req.limits, budget.limit()),
        resources: Some(&files),
        cancel: Some(cancel_probe()),
        source_map: req.to == Target::Pdf,
        ..Request::default()
    };
    let mut session = Session::new();
    let generated = session.run(registry, &request).map(str::to_owned);
    let reports = session.reports();
    let generated = match generated {
        Ok(text) => text,
        Err(fail) => {
            return ArtifactResult { exit: exit_code(fail.status), reports, ..Default::default() };
        }
    };
    if let Some(path) = req.emit_typst
        && let Err(problem) = emit_typst(registry, req, &request, &generated, path)
    {
        return ArtifactResult::failed(problem, reports);
    }
    match req.to {
        Target::Html => deliver(req, generated.into_bytes(), reports),
        Target::Pdf => compile_generated(req, &session, &generated, &input.root, &ledger, reports),
    }
}

/// Reads the input, charging its size, and finds the root its includes resolve from.
fn read_input(req: &ArtifactRequest<'_>, ledger: &Ledger<'_>) -> Result<Input, Problem> {
    let file = read_source(req.input, MAX_SOURCE_BYTES)?;
    if !ledger.charge(file.text.len()) {
        let budget = ledger.budget();
        return Err(budget_problem("Reading this file", req.input, file.text.len(), budget));
    }
    let root = if file.canonical == "-" {
        // Includes resolve to absolute paths, so the root for standard input must be too.
        let cwd = std::env::current_dir().map_err(|e| io_problem(Path::new("."), &e))?;
        cwd.canonicalize().unwrap_or(cwd)
    } else {
        Path::new(&file.canonical).parent().map(Path::to_path_buf).unwrap_or_default()
    };
    Ok(Input { file, root })
}

/// Publishes the intermediate Typst source: the generated text for PDF, or a second
/// conversion with the Typst renderer for HTML.
fn emit_typst(
    registry: &Registry,
    req: &ArtifactRequest<'_>,
    request: &Request<'_>,
    generated: &str,
    path: &Path,
) -> Result<(), Problem> {
    if req.to == Target::Pdf {
        return publish(path, generated.as_bytes(), req.force);
    }
    let typst_request = Request { renderer: "typst", source_map: false, ..*request };
    let mut session = Session::new();
    match session.run(registry, &typst_request) {
        Ok(typst) => publish(path, typst.as_bytes(), req.force),
        Err(fail) => Err(Problem::new(
            "host.emit-typst",
            "CANNOT GENERATE TYPST",
            &path.display().to_string(),
            "The Typst renderer refused this document; HTML was not affected.",
            "Run the conversion with --to pdf to see the Typst diagnostics.",
            exit_code(fail.status),
        )),
    }
}

/// Publishes HTML bytes, or hands them back for standard output.
fn deliver(req: &ArtifactRequest<'_>, bytes: Vec<u8>, reports: Vec<Report>) -> ArtifactResult {
    if req.output == "-" {
        return ArtifactResult::done(Delivered::Stdout(bytes), reports);
    }
    let path = Path::new(req.output);
    match publish(path, &bytes, req.force) {
        Ok(()) => ArtifactResult::done(Delivered::File(path.to_path_buf()), reports),
        Err(problem) => ArtifactResult::failed(problem, reports),
    }
}

/// The problem of a Typst program that cannot be found (`typst.missing`, exit 4).
pub fn backend_problem(program: &str) -> Problem {
    Problem::new(
        "typst.missing",
        "TYPST WAS NOT FOUND",
        program,
        "PDF output and native Typst input use the Typst compiler, which is not installed.",
        "Install Typst 0.15.1, or pass --typst=/path/to/typst.",
        4,
    )
}

/// Compiles the generated Typst into a staged PDF, mapping Typst's positions back to
/// the source through the session's source map.
fn compile_generated(
    req: &ArtifactRequest<'_>,
    session: &Session,
    generated: &str,
    root: &Path,
    ledger: &Ledger<'_>,
    mut reports: Vec<Report>,
) -> ArtifactResult {
    let Some(backend) = gd_typst::locate(req.typst_program) else {
        return ArtifactResult::failed(backend_problem(req.typst_program), reports);
    };
    let stage = match staging(req) {
        Ok(stage) => stage,
        Err(problem) => return ArtifactResult::failed(problem, reports),
    };
    let reserve = |bytes: usize| ledger.charge(bytes);
    let options = typst_options(req, Target::Pdf, &reserve);
    let outcome = gd_typst::compile_text(&backend, generated, root, stage.path(), &options);
    if was_interrupted() {
        return ArtifactResult::failed(interrupted_problem(), reports);
    }
    match outcome {
        Ok(compiled) => {
            reports.extend(compiled.diagnostics.iter().map(|d| map_typst(session, generated, d)));
            finish_stage(req, stage, reports)
        }
        Err(error) => {
            if let gd_typst::Error::Compile { diagnostics, .. } = &error {
                reports.extend(diagnostics.iter().map(|d| map_typst(session, generated, d)));
            }
            ArtifactResult::failed(compile_problem(&error), reports)
        }
    }
}

/// Compiles native Typst input to PDF or HTML.
fn native_typst(req: &ArtifactRequest<'_>, input: &Input, ledger: &Ledger<'_>) -> ArtifactResult {
    let Some(backend) = gd_typst::locate(req.typst_program) else {
        return ArtifactResult::failed(backend_problem(req.typst_program), Vec::new());
    };
    let stage = match staging(req) {
        Ok(stage) => stage,
        Err(problem) => return ArtifactResult::failed(problem, Vec::new()),
    };
    let reserve = |bytes: usize| ledger.charge(bytes);
    let options = typst_options(req, req.to, &reserve);
    let (root, text) = (&input.root, input.file.text.as_str());
    let outcome = if input.file.canonical == "-" {
        gd_typst::compile_text(&backend, text, root, stage.path(), &options)
    } else {
        let source = Path::new(&input.file.canonical);
        gd_typst::compile_file(&backend, source, root, stage.path(), &options)
    };
    if was_interrupted() {
        return ArtifactResult::failed(interrupted_problem(), Vec::new());
    }
    let describe = |d: &gd_typst::Diagnostic| native_report(input, d);
    match outcome {
        Ok(compiled) => {
            finish_stage(req, stage, compiled.diagnostics.iter().map(describe).collect())
        }
        Err(error) => {
            let reports = match &error {
                gd_typst::Error::Compile { diagnostics, .. } => {
                    diagnostics.iter().map(describe).collect()
                }
                _ => Vec::new(),
            };
            ArtifactResult::failed(compile_problem(&error), reports)
        }
    }
}

/// A Typst diagnostic of native input, with an excerpt when it is in the input itself.
fn native_report(input: &Input, d: &gd_typst::Diagnostic) -> Report {
    let in_input = d.stdin
        || input.root.join(d.file.trim_start_matches('/')) == Path::new(&input.file.canonical);
    let mut report = d.report(in_input.then_some(input.file.text.as_str()));
    if in_input {
        report.path = display_path(&input.file.canonical);
    }
    report
}

/// The request's Typst options for `target`, charging the output through `reserve`.
fn typst_options<'a>(
    req: &ArtifactRequest<'_>,
    target: Target,
    reserve: &'a dyn Fn(usize) -> bool,
) -> gd_typst::Options<'a> {
    let o = &req.typst_options;
    gd_typst::Options {
        target: match target {
            Target::Html => gd_typst::Target::Html,
            Target::Pdf => gd_typst::Target::Pdf,
        },
        offline: o.offline,
        package_cache: o.package_cache.clone(),
        embedded_fonts_only: o.embedded_fonts_only,
        font_paths: o.font_paths.clone(),
        read_roots: o.read_roots.clone(),
        reserve: Some(reserve),
    }
}

/// Where Typst writes: a stage beside the destination, or in the temporary folder for
/// standard output. An existing destination is refused first unless forced.
fn staging(req: &ArtifactRequest<'_>) -> Result<Stage, Problem> {
    if req.output == "-" {
        return Stage::beside(&std::env::temp_dir().join("guidedog-out"));
    }
    let output = Path::new(req.output);
    if !req.force && std::fs::symlink_metadata(output).is_ok() {
        return Err(exists_problem(output));
    }
    Stage::beside(output)
}

/// Publishes Typst's staged artifact, or reads it back for standard output.
fn finish_stage(req: &ArtifactRequest<'_>, stage: Stage, reports: Vec<Report>) -> ArtifactResult {
    let size = std::fs::metadata(stage.path()).map(|m| m.len()).unwrap_or(0);
    if size == 0 {
        let problem = Problem::new(
            "typst.empty",
            "TYPST PRODUCED NO ARTIFACT",
            req.output,
            "The compiler returned an empty file.",
            "Check that --typst names Typst 0.15.1.",
            4,
        );
        return ArtifactResult::failed(problem, reports);
    }
    if req.output == "-" {
        return match std::fs::read(stage.path()) {
            Ok(data) => ArtifactResult::done(Delivered::Stdout(data), reports),
            Err(e) => ArtifactResult::failed(io_problem(stage.path(), &e), reports),
        };
    }
    let output = Path::new(req.output);
    match stage.commit(output, req.force) {
        Ok(()) => ArtifactResult::done(Delivered::File(output.to_path_buf()), reports),
        Err(problem) => ArtifactResult::failed(problem, reports),
    }
}

/// The problem a failed Typst compile ends with (exit 4).
fn compile_problem(error: &gd_typst::Error) -> Problem {
    match error {
        gd_typst::Error::Compile { .. } => Problem::new(
            "typst.compile",
            "TYPST COULD NOT BUILD THE ARTIFACT",
            "",
            "The compiler reported the problems above. No output was written.",
            "Fix the reported source location; any previous artifact is unchanged.",
            4,
        ),
        other => {
            let mut problem =
                Problem::new(other.code(), other.title(), "", other.message(), other.hint(), 4);
            problem.path = other.report().path;
            problem
        }
    }
}

/// A Typst diagnostic in the generated text, moved back to the RST or CommonMark source
/// through the source map: the last entry at or before Typst's position names the span.
fn map_typst(session: &Session, generated: &str, d: &gd_typst::Diagnostic) -> Report {
    let ws = session.workspace();
    let entries = ws.source_map();
    if !d.stdin || d.line == 0 || entries.is_empty() {
        return d.report(d.stdin.then_some(generated));
    }
    let offset = gd_typst::offset_of(generated, d.line, d.column) as u64;
    let Some(entry) = entries.iter().rev().find(|e| u64::from(e.offset) <= offset) else {
        return d.report(Some(generated));
    };
    let span = entry.span;
    let mut report = d.report(None);
    if let Some(source) = ws.sources().get(usize::from(span.source.0)) {
        report.path = display_path(&source.name);
        let text = ws.source_str(span.source);
        gd_report::set_span(&mut report, text, span.start as usize, span.end as usize);
    }
    report.hints = vec![
        "Check the raw Typst at this line of the source; if there is none, report this \
         Guidedog renderer bug with the source that caused it."
            .to_string(),
    ];
    report
}
