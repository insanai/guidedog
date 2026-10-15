#![cfg(feature = "artifact")]
//! Sessions, described diagnostics, and the `convert_artifact` route, with toy adapters.

mod common;

use common::{TempDir, toy_registry};
use gd_host::*;
use guidedoc::{CORE_DEPTH, SourceId, SourceSpan, Status, arg_int, default_request, diagnostic};
use std::fs;

/// Runs a toy conversion of `text` named `name` and returns the session.
fn session_of(name: &str, text: &str) -> Session {
    let mut session = Session::new();
    let _ = session.run(&toy_registry(), &default_request(name, text, "rst", "html"));
    session
}

#[test]
fn templates_and_positions() {
    let session = session_of("doc.rst", "one\ntwo \x1b[31mred\n");
    let d = diagnostic(CORE_DEPTH, SourceSpan::new(SourceId::MAIN, 4, 7), arg_int(3), arg_int(0));
    let r = describe(session.workspace(), &d);
    assert_eq!(r.code, "core.limit.depth");
    assert!(r.message.contains('3'), "{}", r.message);
    assert_eq!((r.line, r.column, r.end_column), (2, 1, 4));
    assert_eq!(r.path, "doc.rst");
    let text = gd_report::render_text(&r, &gd_report::Style::default());
    assert!(text.contains("doc.rst:2:1"), "{text}");
    assert!(!text.contains('\x1b'), "{text}");
    assert_eq!(exit_code(Status::Capacity), 3);
    assert_eq!(exit_code(Status::Cancelled), 130);
    assert_eq!(exit_code(Status::Ok), 0);
}

#[test]
fn span_arguments_are_quoted_and_cut() {
    let long = "é".repeat(70);
    let session = session_of("x.rst", &format!("!\n{long}\n"));
    let reports = session.reports();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].code, "toy.warning");
    assert_eq!(reports[0].message, "A warning line `!`.");
    let span = SourceSpan::new(SourceId::MAIN, 2, 2 + long.len() as u32);
    let quoted = diagnostic(common::TOY_WARNING, span, guidedoc::arg_span(span), arg_int(0));
    let message = describe(session.workspace(), &quoted).message;
    assert!(message.contains(&format!("{}…", "é".repeat(60))), "{message}");
}

#[test]
fn diagnostic_coordinates_survive_terminal_escaping() {
    let text = "e\u{301}界\t\x1bwrong";
    let start = text.find("wrong").unwrap_or(0) as u32;
    let session = session_of("unicode.rst", text);
    let span = SourceSpan::new(SourceId::MAIN, start, text.len() as u32);
    let r = describe(session.workspace(), &diagnostic(CORE_DEPTH, span, arg_int(3), arg_int(0)));
    assert_eq!((r.column, r.end_column), (6, 11));
    assert_eq!(r.excerpt.map(|e| e.text).as_deref(), Some(text));
}

#[test]
fn sessions_run_read_and_report() {
    let registry = toy_registry();
    let mut session = Session::new();
    let request = default_request("a.rst", "one\ntwo\n", "rst", "html");
    assert_eq!(session.run(&registry, &request), Ok("<p>one</p><p>two</p>"));
    assert!(session.reports().is_empty());
    let failed = default_request("a.rst", "deep\n", "rst", "html");
    let status = session.run(&registry, &failed).map(|_| ()).map_err(|f| f.status);
    assert_eq!(status, Err(Status::Limit));
    let reports = session.reports();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].severity, gd_report::Severity::Error);
    let reader = &registry.readers[0];
    let count = session.read(reader, &request).map(|doc| doc.child_count(guidedoc::ROOT));
    assert_eq!(count.ok(), Some(2));
    assert!(session.document().is_some());
}

#[test]
fn problems_render_as_reports() {
    let p =
        Problem::new("host.io", "CANNOT READ FILE", "doc.rst", "It is gone.", "Restore it.", 4);
    let r = p.report();
    assert_eq!((r.code.as_str(), r.path.as_str()), ("host.io", "doc.rst"));
    assert_eq!(r.hints, vec!["Restore it.".to_string()]);
    assert!(gd_report::render_json(&r).contains("\"code\":\"host.io\""));
    assert_eq!(interrupted_problem().exit, 130);
    assert!(p.to_string().contains("CANNOT READ FILE"));
}

#[test]
fn display_paths_are_relative_when_shorter() {
    let cwd = std::env::current_dir().unwrap_or_default();
    let inside = cwd.join("docs/a.rst");
    assert_eq!(display_path(inside.to_str().unwrap_or_default()), "docs/a.rst");
    assert_eq!(display_path("-"), "-");
    assert_eq!(display_path("rel/x.rst"), "rel/x.rst");
}

fn html_request<'a>(input: &'a str, output: &'a str) -> ArtifactRequest<'a> {
    ArtifactRequest { input, output, from: Format::Rst, to: Target::Html, ..Default::default() }
}

#[test]
fn artifacts_are_converted_and_published() {
    let dir = TempDir::new("artifact");
    dir.write("part.rst", "included");
    let input = dir.write("main.rst", "first\ninclude part.rst\n!\n");
    let output = dir.join("out/main.html");
    let (input, output_str) = (input.to_str().unwrap_or_default(), output.to_str().unwrap_or(""));
    let result = convert_artifact(&toy_registry(), &html_request(input, output_str));
    assert_eq!(result.exit, 0, "{:?}", result.problem);
    assert_eq!(result.delivered, Delivered::File(output.clone()));
    assert_eq!(result.reports.len(), 1); // the warning.
    assert_eq!(fs::read_to_string(&output).ok().as_deref(), Some("<p>first</p><p>included</p>"));
    // An existing output is refused and left as it was.
    dir.write("main.rst", "changed\n");
    let again = convert_artifact(&toy_registry(), &html_request(input, output_str));
    assert_eq!(again.exit, 2);
    assert_eq!(again.problem.map(|p| p.code), Some("host.output.exists"));
    assert_eq!(fs::read_to_string(&output).ok().as_deref(), Some("<p>first</p><p>included</p>"));
    // Standard output gets the bytes; nothing is written.
    let printed = convert_artifact(&toy_registry(), &html_request(input, "-"));
    assert_eq!(printed.delivered, Delivered::Stdout(b"<p>changed</p>".to_vec()));
}

#[test]
fn artifact_failures_keep_their_reports() {
    let dir = TempDir::new("failure");
    let input = dir.write("main.rst", "deep\n");
    let output = dir.join("main.html");
    let input = input.to_str().unwrap_or_default();
    let result =
        convert_artifact(&toy_registry(), &html_request(input, output.to_str().unwrap_or("")));
    assert_eq!(result.exit, 3);
    assert_eq!(result.reports.first().map(|r| r.code.as_str()), Some("core.limit.depth"));
    assert!(!output.exists());
    let missing = convert_artifact(&toy_registry(), &html_request("/no/such/file.rst", "-"));
    assert_eq!(missing.problem.map(|p| p.code), Some("host.io"));
    // An include outside the input's folder is refused by the confinement.
    let inner = dir.write("inner/main.rst", "include ../main.rst\n");
    let refused =
        convert_artifact(&toy_registry(), &html_request(inner.to_str().unwrap_or(""), "-"));
    assert_eq!(refused.reports.first().map(|r| r.code.as_str()), Some("core.include.refused"));
}

#[test]
fn includes_are_charged_to_the_budget() {
    let dir = TempDir::new("budget");
    dir.write("big.rst", &"x".repeat(8192));
    let input = dir.write("main.rst", "include big.rst\n");
    let request =
        ArtifactRequest { budget: 4096, ..html_request(input.to_str().unwrap_or_default(), "-") };
    let result = convert_artifact(&toy_registry(), &request);
    assert_eq!(result.reports.first().map(|r| r.code.as_str()), Some("core.include.limit"));
    assert_ne!(result.exit, 0);
}

#[test]
fn typst_source_can_be_emitted_beside_html() {
    let dir = TempDir::new("emit");
    let input = dir.write("main.rst", "one\n");
    let typst = dir.join("main.typ");
    let request = ArtifactRequest {
        emit_typst: Some(&typst),
        ..html_request(input.to_str().unwrap_or_default(), "-")
    };
    let result = convert_artifact(&toy_registry(), &request);
    assert_eq!(result.exit, 0, "{:?}", result.problem);
    assert_eq!(fs::read_to_string(&typst).ok().as_deref(), Some("one\n\n"));
}

#[test]
fn pdfs_are_compiled_and_typst_errors_point_at_the_source() {
    if !gd_typst::EMBEDDED {
        return;
    }
    let dir = TempDir::new("pdf");
    let input = dir.write("main.rst", "Hello\n");
    let output = dir.join("main.pdf");
    let request = ArtifactRequest {
        to: Target::Pdf,
        typst_options: gd_typst::Options { offline: true, ..Default::default() },
        ..html_request(input.to_str().unwrap_or_default(), output.to_str().unwrap_or_default())
    };
    let result = convert_artifact(&toy_registry(), &request);
    assert_eq!(result.exit, 0, "{:?} {:?}", result.problem, result.reports);
    assert!(fs::read(&output).map(|pdf| pdf.starts_with(b"%PDF")).unwrap_or(false));

    let broken = dir.write("broken.rst", "fine\nbad\n");
    let broken_out = dir.join("broken.pdf");
    let request = ArtifactRequest {
        input: broken.to_str().unwrap_or_default(),
        output: broken_out.to_str().unwrap_or_default(),
        typst_options: gd_typst::Options { offline: true, ..Default::default() },
        ..request
    };
    let result = convert_artifact(&toy_registry(), &request);
    assert_eq!(result.exit, 4);
    assert_eq!(result.problem.map(|p| p.code), Some("typst.compile"));
    let mapped = result.reports.iter().find(|r| r.path.ends_with("broken.rst"));
    assert_eq!(mapped.map(|r| r.line), Some(2), "{:?}", result.reports);
    assert!(!broken_out.exists());
}
