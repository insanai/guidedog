use std::path::PathBuf;

use super::*;

/// A fresh, empty directory for one test, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("gd-typst-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create a test directory");
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn offline() -> Options<'static> {
    Options { offline: true, embedded_fonts_only: true, ..Options::default() }
}

#[test]
fn diagnostics_are_read_from_the_short_format() {
    let found = cli::parse_diagnostics(
        "book.typ:3:7: error: unknown variable: x\n  hint: define it first\n\
         C:\\docs\\a.typ:1:2: warning: unused\n<stdin>:4:1: error: bad\nnoise line\n",
    );
    assert_eq!(found.len(), 3);
    assert_eq!(found[0].file, "book.typ");
    assert_eq!((found[0].line, found[0].column), (3, 7));
    assert_eq!(found[0].message, "unknown variable: x");
    assert_eq!(found[0].hints, ["define it first"]);
    assert_eq!(found[1].file, "C:\\docs\\a.typ");
    assert_eq!(found[1].severity, Severity::Warning);
    assert!(found[2].stdin);
}

#[test]
fn offsets_count_characters_and_stop_at_the_end() {
    let text = "ab\nçd\n";
    assert_eq!(offset_of(text, 1, 1), 0);
    assert_eq!(offset_of(text, 2, 2), 5); // "ç" is two bytes.
    assert_eq!(offset_of(text, 2, 99), 6); // the line's end.
    assert_eq!(offset_of(text, 99, 1), text.len());
}

#[test]
fn missing_programs_are_not_found() {
    assert!(find_program("guidedog-no-such-typst").is_none());
    assert!(locate("/no/such/typst").is_none());
}

#[test]
fn sources_are_rooted_or_refused() {
    let root = Path::new("/book");
    assert_eq!(rooted_path(root, Path::new("/book/ch/a.typ")).as_deref(), Some("/ch/a.typ"));
    assert_eq!(rooted_path(root, Path::new("/book/./a.typ")).as_deref(), Some("/a.typ"));
    assert_eq!(rooted_path(root, Path::new("/elsewhere/a.typ")), None);
    assert_eq!(rooted_path(root, Path::new("/book/../a.typ")), None);
}

#[test]
fn diagnostics_render_in_the_elm_style() {
    let d = Diagnostic {
        file: "/book.typ".into(),
        line: 2,
        column: 2,
        span: Some(5..9),
        message: "unknown variable: nope".into(),
        hints: vec!["define `nope` with `#let`".into()],
        ..Diagnostic::default()
    };
    let report = d.render(Some("= A\n#nope\n"));
    assert!(report.starts_with("-- TYPST ERROR [typst.error] --"), "{report}");
    assert!(report.contains("/book.typ:2:2\n\nunknown variable: nope"), "{report}");
    assert!(report.contains("2| #nope\n    ^^^^\n\nHint: define `nope`"), "{report}");
    let error = Error::OutsideRoot { source: "/x.typ".into(), root: "/book".into() };
    let text = error.to_string();
    assert!(text.starts_with("-- SOURCE OUTSIDE ROOT [typst.outside-root] --"), "{text}");
    assert!(text.contains("\nHint: "), "{text}");
}

/// Compiles a document to PDF and a broken one to diagnostics with `backend`.
fn compile_both_ways(backend: &Backend, name: &str) {
    let dir = TempDir::new(name);
    let output = dir.0.join("out.pdf");
    let ok = compile_text(backend, "= Hello\n", &dir.0, &output, &offline());
    assert!(ok.is_ok(), "{ok:?}");
    // Only the embedded compiler hands back the laid-out pages.
    let pages = ok.ok().and_then(|c| c.pages);
    assert_eq!(pages.is_some(), backend.is_embedded(), "{pages:?}");
    let data = std::fs::read(&output).expect("the PDF was written");
    assert!(data.starts_with(b"%PDF-"));
    let bad = compile_text(backend, "#nope\n", &dir.0, &output, &offline());
    let Err(Error::Compile { diagnostics, .. }) = &bad else { panic!("{bad:?}") };
    assert!(diagnostics.first().is_some_and(|d| d.line == 1 && d.stdin), "{bad:?}");
    let source = dir.0.join("book.typ");
    std::fs::write(&source, "= Book\n#include \"part.typ\"\n").expect("write the source");
    std::fs::write(dir.0.join("part.typ"), "Part.\n").expect("write the part");
    let file = compile_file(backend, &source, &dir.0, &output, &offline());
    assert!(file.is_ok(), "{file:?}");
}

#[test]
fn the_installed_program_compiles_to_pdf() {
    let Some(path) = find_program("typst") else { return };
    let backend = locate(&path.to_string_lossy()).expect("the program is found");
    assert!(!backend.is_embedded());
    compile_both_ways(&backend, "cli");
}

#[test]
fn a_missing_program_reports_a_launch_error() {
    let backend = Backend::Program { path: "/no/such/typst".into(), version: None };
    let dir = TempDir::new("launch");
    let out = compile_text(&backend, "x", &dir.0, &dir.0.join("o.pdf"), &offline());
    assert!(matches!(out, Err(Error::Launch { .. })), "{out:?}");
}

#[test]
fn compile_failures_name_the_first_error() {
    let d = |severity, message: &str| Diagnostic {
        severity,
        message: message.into(),
        ..Diagnostic::default()
    };
    let diagnostics =
        vec![d(Severity::Warning, "unused"), d(Severity::Error, "unknown variable: x")];
    let error = Error::Compile { exit_code: Some(1), diagnostics, detail: String::new() };
    let text = error.to_string();
    assert!(text.contains("Typst reported an error: unknown variable: x"), "{text}");
}

#[cfg(feature = "embedded")]
mod embedded {
    use super::*;

    fn backend() -> Backend {
        let backend = locate("typst").expect("the embedded compiler");
        assert!(backend.is_embedded() && backend.is_pinned());
        backend
    }

    fn html() -> Options<'static> {
        Options { target: Target::Html, ..offline() }
    }

    fn diagnostics_of(result: Result<Compiled, Error>) -> Vec<Diagnostic> {
        match result {
            Err(Error::Compile { diagnostics, .. }) => diagnostics,
            other => panic!("expected a compile error: {other:?}"),
        }
    }

    #[test]
    fn the_embedded_compiler_compiles_to_pdf() {
        compile_both_ways(&backend(), "embedded");
    }

    #[test]
    fn the_embedded_compiler_reports_each_pages_text() {
        let dir = TempDir::new("pages");
        let output = dir.0.join("out.pdf");
        let text = "= Hello\nFirst page.\n#pagebreak()\nSecond page, `let x = 1;`\n";
        let compiled = compile_text(&backend(), text, &dir.0, &output, &offline());
        let pages = compiled.expect("the document compiles").pages.expect("the pages");
        assert_eq!(pages.len(), 2, "{pages:?}");
        assert!(pages[0].contains("Hello") && pages[0].contains("First page."), "{pages:?}");
        assert!(pages[1].contains("Second page, let x = 1;"), "{pages:?}");
    }

    #[test]
    fn embedded_diagnostics_carry_spans_and_html_compiles() {
        let dir = TempDir::new("spans");
        let output = dir.0.join("out.html");
        let text = "= Hi\n#nope\n";
        let bad = diagnostics_of(compile_text(&backend(), text, &dir.0, &output, &html()));
        let mut errors = bad.iter();
        let d = errors.find(|d| d.severity == Severity::Error).expect("an error");
        assert_eq!((d.file.as_str(), d.line, d.column), (GENERATED_MAIN, 2, 2), "{d:?}");
        assert_eq!(d.span.clone().map(|s| &text[s]), Some("nope"));
        assert!(d.render(Some(text)).contains("2| #nope\n    ^^^^"), "{}", d.render(Some(text)));
        let good = compile_text(&backend(), "= Hi\n", &dir.0, &output, &html());
        assert!(good.as_ref().is_ok_and(|c| c.pages.is_none()), "{good:?}");
        let html = std::fs::read_to_string(&output).expect("the HTML was written");
        assert!(html.contains("<h2>Hi</h2>") || html.contains("Hi</h"), "{html}");
    }

    #[test]
    fn the_embedded_compiler_confines_reads_and_asks_before_writing() {
        let dir = TempDir::new("confined");
        let output = dir.0.join("out.pdf");
        let text = "#read(\"../../etc/passwd\")\n";
        let out = compile_text(&backend(), text, &dir.0, &output, &offline());
        assert!(!diagnostics_of(out).is_empty());
        let asked = std::cell::Cell::new(0usize);
        let refuse = |bytes: usize| {
            asked.set(bytes);
            false
        };
        let options = Options { reserve: Some(&refuse), ..offline() };
        let out = compile_text(&backend(), "Hi", &dir.0, &output, &options);
        assert!(matches!(out, Err(Error::OutputTooLarge { .. })) && asked.get() > 0);
        let outside = compile_file(&backend(), Path::new("/x.typ"), &dir.0, &output, &offline());
        assert!(matches!(outside, Err(Error::OutsideRoot { .. })));
    }

    #[test]
    fn read_roots_narrow_what_the_root_lets_through() {
        let dir = TempDir::new("read-roots");
        let (allowed, private) = (dir.0.join("allowed"), dir.0.join("private"));
        std::fs::create_dir_all(&allowed).expect("a folder");
        std::fs::create_dir_all(&private).expect("a folder");
        std::fs::write(allowed.join("a.txt"), "open").expect("a file");
        std::fs::write(private.join("b.txt"), "secret").expect("a file");
        let output = dir.0.join("out.pdf");
        let options = Options { read_roots: vec![allowed.clone()], ..offline() };
        let open =
            compile_text(&backend(), "#read(\"/allowed/a.txt\")", &dir.0, &output, &options);
        assert!(open.is_ok(), "{open:?}");
        let text = "#read(\"/private/b.txt\")";
        let refused = diagnostics_of(compile_text(&backend(), text, &dir.0, &output, &options));
        assert!(refused.iter().any(|d| d.message.contains("denied")), "{refused:?}");
        #[cfg(unix)]
        {
            // A link inside an allowed folder may not lead out of it.
            std::os::unix::fs::symlink(&private, allowed.join("link")).expect("a link");
            let text = "#read(\"/allowed/link/b.txt\")";
            let out = compile_text(&backend(), text, &dir.0, &output, &options);
            assert!(!diagnostics_of(out).is_empty());
        }
    }
}

#[cfg(not(feature = "embedded"))]
#[test]
fn without_the_feature_the_program_is_located() {
    if let Some(backend) = locate("typst") {
        assert!(!backend.is_embedded());
    }
}
