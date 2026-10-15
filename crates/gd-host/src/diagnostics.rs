//! Engine diagnostics as `gd_report::Report`s: templates filled, span arguments quoted
//! from the source table, positions resolved to lines and Unicode-scalar columns, and
//! paths shown relative to the working directory.

use std::path::{Component, Path, PathBuf};

use gd_report::Report;
use guidedoc::{Arg, Diagnostic, Severity, SourceSpan, Status, Workspace};

/// The most characters of source a span argument quotes; longer ones end in "…".
const MAX_QUOTE: usize = 60;

/// Maps a conversion status to the exit status Guidedog's commands use: 0 ok, 1 invalid
/// input, unsupported or policy, 3 capacity or limits, 5 internal, 130 cancelled.
pub fn exit_code(status: Status) -> i32 {
    match status {
        Status::Ok => 0,
        Status::InvalidInput | Status::Unsupported | Status::Policy => 1,
        Status::Capacity | Status::Limit => 3,
        Status::Cancelled => 130,
        Status::Internal => 5,
    }
}

/// `name` relative to the working directory when that reads shorter (not three or more
/// folders above it), and `name` itself otherwise, for "" and "-", and for names that
/// are not absolute paths.
pub fn display_path(name: &str) -> String {
    let path = Path::new(name);
    if name.is_empty() || name == "-" || !path.is_absolute() {
        return name.to_string();
    }
    let Ok(cwd) = std::env::current_dir() else { return name.to_string() };
    match relative_to(path, &cwd) {
        Some(rel) if !rel.starts_with("../../..") => rel.to_string_lossy().into_owned(),
        _ => name.to_string(),
    }
}

/// `path` relative to the folder `base`, lexically; `None` when they share no root.
fn relative_to(path: &Path, base: &Path) -> Option<PathBuf> {
    let mut a = path.components().peekable();
    let mut b = base.components().peekable();
    if a.peek() != b.peek() {
        return None;
    }
    while a.peek().is_some() && a.peek() == b.peek() {
        a.next();
        b.next();
    }
    let mut out: PathBuf = b.filter(|c| matches!(c, Component::Normal(_))).map(|_| "..").collect();
    out.extend(a);
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    Some(out)
}

/// Turns a diagnostic of the conversion `ws` holds into a report: its message and hint
/// with `{0}`/`{1}` filled (span arguments quoted from the sources, cut at 60
/// characters), the source's path as `display_path` shows it, and the line, columns,
/// and excerpt of its span. The report copies what it needs and outlives `ws`.
pub fn describe(ws: &Workspace, d: &Diagnostic) -> Report {
    let quote = |span: SourceSpan| quoted(ws.source_text(span));
    let mut report = Report {
        code: d.code.to_string(),
        severity: severity(d.severity),
        title: d.title.to_string(),
        ..Report::default()
    };
    d.format_into(d.message, &quote, &mut report.message);
    let mut hint = String::new();
    d.format_into(d.hint, &quote, &mut hint);
    report.hints.push(hint);
    let Some(source) = ws.sources().get(usize::from(d.span.source.0)) else { return report };
    report.path = display_path(&source.name);
    let positioned = d.span.start != 0 || d.span.end != 0 || matches!(d.args[0], Arg::Span(_));
    if positioned {
        let text = ws.source_str(d.span.source);
        gd_report::set_span(&mut report, text, d.span.start as usize, d.span.end as usize);
    }
    report
}

/// Describes every diagnostic the last conversion in `ws` reported, in order; the
/// primary error too when the store was full before it, and a note counting the
/// diagnostics the store had no room for.
pub fn describe_all(ws: &Workspace) -> Vec<Report> {
    let reports = ws.reports();
    let mut out: Vec<Report> = reports.items().iter().map(|d| describe(ws, d)).collect();
    if let Some(primary) = reports.primary()
        && !reports.items().contains(primary)
    {
        out.push(describe(ws, primary));
    }
    let suppressed = reports.suppressed_count();
    if suppressed > 0 {
        out.push(suppressed_note(suppressed));
    }
    out
}

fn suppressed_note(count: usize) -> Report {
    Report::note(
        "host.reports.suppressed",
        "MORE DIAGNOSTICS",
        &format!("{count} more diagnostics were reported but not kept."),
    )
    .hint("Fix the problems above and convert again, or raise limits.max_reports.")
}

fn severity(s: Severity) -> gd_report::Severity {
    match s {
        Severity::Error => gd_report::Severity::Error,
        Severity::Warning => gd_report::Severity::Warning,
        Severity::Note => gd_report::Severity::Note,
    }
}

/// `text` cut at `MAX_QUOTE` characters with "…"; the core adds the quotes.
fn quoted(text: &str) -> String {
    match text.char_indices().nth(MAX_QUOTE) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_string(),
    }
}
