//! `guidedog convert` and `formats`: diagnostics with directions, the stack budget,
//! confined includes, Typst sources and packages. Plain HTML, PDF, `--force` and
//! standard input are covered by gd-cli's `commands.rs` and guidedog's `process.rs`.

use guidedog_tests::{Scratch, guidedog, has, lacks};

use crate::flat;

#[test]
fn convert_reports_problems_with_directions() {
    let dir = Scratch::new("convert-problems");
    dir.write("broken.rst", "See missing_ here.\n");
    let args = ["convert", "broken.rst", "--output", "broken.html", "--color=never"];
    let r = guidedog(&dir.path, 1, &args);
    has(&r.err, "rst.reference.unresolved");
    has(&r.err, "broken.rst:1:");
    has(&r.err, "Hint:");
    has(&flat(&r.err), "No output was written.");
    assert!(!dir.exists("broken.html"));
    let args = ["convert", "broken.rst", "--output", "broken.html", "--diagnostics=json"];
    let r = guidedog(&dir.path, 1, &args);
    let first = r.err.lines().next().unwrap_or_default();
    let report = gd_json::parse(first).unwrap_or_else(|e| panic!("{e}: {first}"));
    assert_eq!(report["schema"].as_i64(), Some(1), "{first}");
    assert_eq!(report["code"].as_str(), Some("rst.reference.unresolved"), "{first}");
    assert_eq!(report["line"].as_i64(), Some(1), "{first}");
    assert!(report["hint"].as_str().is_some_and(|h| !h.is_empty()), "{first}");
    dir.write("raw.md", "<div>raw</div>\n");
    let r = guidedog(&dir.path, 1, &["convert", "raw.md", "--output", "raw.html"]);
    has(&r.err, "--raw");
    guidedog(&dir.path, 0, &["convert", "raw.md", "--output", "raw.html", "--raw=allow"]);
    has(&dir.read("raw.html"), "<div>raw</div>");
}

/// `--stack-kib` bounds the recursive reader and renderers: past it nesting is a limit
/// error naming the flag, never a crash, and a larger budget converts deeper nesting.
#[test]
fn convert_takes_a_stack_budget() {
    let dir = Scratch::new("convert-stack");
    let deep: String = (0..40).map(|i| format!("{}Quoted.\n\n", "  ".repeat(i))).collect();
    dir.write("deep.rst", &deep);
    guidedog(&dir.path, 0, &["convert", "deep.rst", "--output", "deep.html"]);
    let args = ["convert", "deep.rst", "--output", "small.html", "--stack-kib=8", "--color=never"];
    let r = guidedog(&dir.path, 3, &args);
    has(&r.err, "core.limit.stack");
    has(&r.err, "raise --stack-kib=N");
    assert!(!dir.exists("small.html"));
    let args = ["convert", "deep.rst", "-o", "big.html", "--stack-kib=2048", "--max-depth=1000"];
    guidedog(&dir.path, 0, &args);
    let r =
        guidedog(&dir.path, 2, &["convert", "deep.rst", "-o", "x.html", "--stack-kib=99999999"]);
    has(&r.err, "STACK BUDGET");
}

#[test]
fn convert_usage_errors_and_missing_inputs_have_their_statuses() {
    let dir = Scratch::new("convert-usage");
    dir.write("notes.txt", "text\n");
    let r = guidedog(&dir.path, 2, &["convert", "notes.txt", "--output", "x.html"]);
    has(&r.err, "--from");
    guidedog(&dir.path, 2, &["convert", "notes.txt", "--from", "rst", "--output", "x.docx"]);
    guidedog(&dir.path, 2, &["convert", "notes.txt", "--from", "rst"]);
    let r = guidedog(&dir.path, 4, &["convert", "missing.rst", "--output", "x.html"]);
    assert!(!r.err.is_empty());
    assert!(!dir.exists("x.html"));
}

#[test]
fn includes_are_confined_to_the_source_root() {
    let root = Scratch::new("convert-include");
    root.write("docs/parts/intro.rst", "Included *text*.\n");
    root.write("secret.rst", "Secret.\n");
    root.write("docs/main.rst", "Start.\n\n.. include:: parts/intro.rst\n");
    let docs = root.join("docs");
    guidedog(&docs, 0, &["convert", "main.rst", "--output", "main.html"]);
    has(&root.read("docs/main.html"), "Included <em>text</em>");
    root.write("docs/escape.rst", ".. include:: ../secret.rst\n");
    let r = guidedog(&docs, 1, &["convert", "escape.rst", "--output", "escape.html"]);
    has(&r.err, "core.include.refused");
    root.write("docs/loop.rst", ".. include:: loop.rst\n");
    let r = guidedog(&docs, 1, &["convert", "loop.rst", "--output", "loop.html"]);
    has(&r.err, "core.include.cycle");
}

#[test]
fn typst_sources_convert_to_pdf_and_html() {
    let dir = Scratch::new("convert-typst");
    dir.write("native.typ", "= Native\n\nTypst *markup*.\n");
    guidedog(&dir.path, 0, &["convert", "native.typ", "--output", "native.pdf"]);
    assert!(std::fs::read(dir.join("native.pdf")).unwrap().starts_with(b"%PDF-"));
    guidedog(&dir.path, 0, &["convert", "native.typ", "--output", "native.html"]);
    has(&dir.read("native.html"), "Native");
    dir.write("notes.md", "# Title\n\nText.\n");
    let args = ["convert", "notes.md", "--output", "x.pdf", "--typst=/not/a/typst"];
    let r = guidedog(&dir.path, 4, &args);
    has(&r.err, "typst.missing");
    assert!(!dir.exists("x.pdf"));
}

/// A package in a private cache proves package resolution without the network.
#[test]
fn typst_packages_resolve_from_the_cache() {
    let dir = Scratch::new("convert-packages");
    dir.write(
        "cache/preview/demo/0.1.0/typst.toml",
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\n",
    );
    dir.write("cache/preview/demo/0.1.0/lib.typ", "#let shout(body) = upper(body)\n");
    dir.write("uses.typ", "#import \"@preview/demo:0.1.0\": shout\n#shout[from a package]\n");
    let cache = dir.join("cache");
    let cache = cache.to_str().unwrap();
    let args =
        ["convert", "uses.typ", "--output", "uses.html", "--offline", "--package-cache", cache];
    guidedog(&dir.path, 0, &args);
    has(&dir.read("uses.html"), "FROM A PACKAGE");
    dir.write("missing.typ", "#import \"@preview/absent:9.9.9\": x\n");
    let args = [
        "convert",
        "missing.typ",
        "--output",
        "m.pdf",
        "--offline",
        "--package-cache",
        cache,
        "--color=never",
    ];
    let r = guidedog(&dir.path, 4, &args);
    has(&r.err, "absent");
}

#[test]
fn formats_report_the_adapters_and_the_typst_version() {
    let dir = Scratch::new("formats");
    let r = guidedog(&dir.path, 0, &["formats", "--color=never"]);
    for name in ["commonmark", "rst", "html", "typst", "0.15.1"] {
        has(&r.out, name);
    }
    lacks(&r.out, "\x1b[");
    let r = guidedog(&dir.path, 0, &["--version"]);
    has(&r.out, &format!("guidedog {}", guidedog_tests::repository_text("VERSION").trim()));
    has(&r.out, "guidedoc");
}
