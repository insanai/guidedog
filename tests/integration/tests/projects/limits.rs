//! Memory budgets and conversion limits of a build, as a user meets them without a
//! terminal (the questions asked at a terminal are covered in gd-cli and gd-project), and
//! builds on a machine with no Python.

use guidedog_tests::{Scratch, has, lacks};

use crate::support::{both, build, cli, project};

/// A template that renders 60 MB, more than a 4 MiB budget holds.
const BIG_LAYOUT: &str = "{{ \"x\" * 60000000 }}\n";

/// Without a terminal a build never asks what to do at its budget: a template that
/// builds more than the budget stops the build with the budget's report at the template
/// line, naming --budget; a larger budget builds the page. The disk memory modes of
/// earlier versions are explained, not ignored.
#[test]
fn templates_render_within_the_budget() {
    let dir = project("memory");
    dir.write("conf.toml", "project = \"Demo\"\ntemplates_path = [\"_templates\"]\n");
    dir.write("_templates/layout.html", BIG_LAYOUT);
    for mode in [&[][..], &["--memory=ram"][..]] {
        let r = build(&dir.path, 3, &[&["html", "--budget=4"][..], mode].concat());
        has(&r.err, "MEMORY BUDGET REACHED");
        has(&r.err, "layout.html:1");
        has(&r.err, "{{ \"x\" * 60000000 }}");
        has(&r.err, "--budget");
        assert!(!dir.exists("_build/html/index.html"));
    }
    build(&dir.path, 0, &["html", "--budget=64"]);
    assert_eq!(dir.read("_build/html/index.html").trim().len(), 60_000_000);
    for flag in ["--memory=disk", "--disk-budget=0"] {
        let r = build(&dir.path, 2, &["html", flag]);
        has(&r.err, "WORKING MEMORY ON DISK IS NOT AVAILABLE");
    }
    let r = build(&dir.path, 2, &["html", "--memory=swap"]);
    has(&r.err, "INVALID MEMORY MODE");
}

/// A build holds one budget, and reading a document and rendering a page both draw on
/// it: one conversion's text and output may use at most the whole budget. The document
/// here is small, but its substitutions make a 3 MB page.
#[test]
fn one_budget_bounds_reading_and_rendering() {
    let dir = project("budget");
    dir.write("index.rst", "Demo\n====\n\n.. include:: part.rst\n\n.. toctree::\n\n   other\n");
    dir.write("part.rst", "Included text.\n");
    let mut other = format!("Other\n=====\n\n.. |x| replace:: {}\n\n", "word ".repeat(400));
    other.push_str(&"|x|\n\n".repeat(1500));
    dir.write("other.rst", &other);
    build(&dir.path, 0, &["html"]);
    has(&dir.read("_build/html/index.html"), "Included text.");
    // Nothing is read again, so only the pages meet the one-MiB budget.
    let r = build(&dir.path, 1, &["html", "-a", "--budget=1"]);
    has(&r.err, "core.capacity");
    has(&r.err, "--budget=N");
    has(&both(&r), "0 document(s) parsed");
    // A budget that holds the work builds as before, without reading again.
    let r = build(&dir.path, 0, &["html", "-a", "--budget=64"]);
    has(&r.out, "0 document(s) parsed");
    build(&dir.path, 2, &["html", "--budget=0"]);
    // Reading meets it too: a 1.4 MB source does not fit.
    dir.write(
        "other.rst",
        &format!("Other\n=====\n\n{}", "Words and more words.\n\n".repeat(60_000)),
    );
    let r = build(&dir.path, 3, &["html", "-E", "--budget=1"]);
    has(&r.err, "MEMORY BUDGET REACHED");
    has(&r.err, "other.rst");
}

/// Files a build reads into memory are charged to its budget before they are read.
#[test]
fn files_are_read_within_the_budget() {
    let dir = project("files-budget");
    dir.write("index.rst", "Demo\n====\n\nShort.\n");
    let big = "word ".repeat(3 << 18); // 3.75 MiB
    dir.write("_static/big.txt", &big);
    let r = build(&dir.path, 3, &["html", "--budget=3"]);
    has(&r.err, "MEMORY BUDGET REACHED");
    has(&r.err, "big.txt");
    std::fs::remove_file(dir.join("_static/big.txt")).unwrap();
    dir.write("big.rst", &big);
    dir.write("index.rst", "Demo\n====\n\n.. include:: big.rst\n");
    dir.write("conf.toml", "project = \"Demo\"\nexclude_patterns = [\"big.rst\"]\n");
    let r = build(&dir.path, 1, &["html", "--budget=3"]);
    has(&r.err, "core.include.limit");
    // convert charges includes to its --budget the same way.
    let r = cli(&dir.path, 3, &["convert", "index.rst", "--to", "html", "--budget=3", "-o", "-"]);
    has(&r.err, "core.include.limit");
}

/// A build takes convert's limits, and their hints name the flags that raise them.
#[test]
fn builds_take_the_conversion_limits() {
    let dir = project("conversion-limits");
    let mut text = String::from("Demo\n====\n\n");
    for i in 0..40 {
        text.push_str(&"  ".repeat(i));
        text.push_str("Quoted.\n\n");
    }
    dir.write("index.rst", &text);
    build(&dir.path, 0, &["html"]);
    let r = build(&dir.path, 1, &["html", "--stack-kib=8"]);
    has(&r.err, "core.limit.stack");
    has(&r.err, "raise --stack-kib=N");
    lacks(&r.err, "stack_bytes");
    let r = build(&dir.path, 1, &["html", "--max-depth=10"]);
    has(&r.err, "core.limit.depth");
    has(&r.err, "raise --max-depth=N");
    lacks(&r.err, "max_depth");
    // Worker threads have 8 MiB stacks, which hold a budget of 6 MiB (gd_host).
    let r = build(&dir.path, 2, &["html", "--stack-kib=6145"]);
    has(&r.err, "STACK BUDGET TOO LARGE");
    build(&dir.path, 0, &["html", "-j", "2", "--stack-kib=2048", "--max-depth=1000"]);
    let r = build(&dir.path, 2, &["html", "--max-nodes=0"]);
    has(&r.err, "INVALID NUMBER");
}

/// Guidedog needs no Python: builds run with an environment holding only HOME and an
/// empty PATH (no python, no sphinx, no dot), with graphs, MyST, autodoc and migrate.
#[test]
fn builds_run_without_python() {
    let dir = Scratch::new("no-python");
    std::fs::create_dir(dir.join("bin")).unwrap();
    let home = std::env::var("HOME").unwrap_or_default();
    let path = dir.join("bin").to_string_lossy().into_owned();
    let bare = |at: &std::path::Path, args: &[&str]| {
        let mut command = std::process::Command::new(guidedog_tests::program("guidedog"));
        command.args(args).current_dir(at).env_clear().env("HOME", &home).env("PATH", &path);
        guidedog_tests::run_command(&mut command).expect(0)
    };
    bare(&dir.path, &["quickstart", "docs", "-q", "-p", "Demo", "-a", "Ada"]);
    let docs = dir.join("docs");
    dir.write(
        "docs/conf.toml",
        "project = \"Demo\"\nextensions = [\"sphinx.ext.graphviz\", \"myst_parser\", \
         \"sphinx.ext.autodoc\"]\nautodoc_source_paths = [\".\"]\n",
    );
    dir.write("docs/tool.py", "def run(x, y=1):\n    \"\"\"Runs x.\"\"\"\n");
    dir.write(
        "docs/index.rst",
        "Demo\n====\n\n.. toctree::\n\n   notes\n\n.. digraph:: flow\n\n   a -> b\n\n\
         .. py:function:: f(x)\n\nSee :py:func:`f`.\n\n.. autofunction:: tool.run\n",
    );
    dir.write("docs/notes.md", "# Notes\n\nSome *Markdown*.\n");
    dir.write("docs/old.py", "project = 'Old'\nextensions = ['myst_parser']\n");
    for builder in ["html", "singlehtml", "text", "pdf"] {
        bare(&docs, &["build", builder, "-W", "--color=never"]);
    }
    assert!(dir.exists("docs/_build/html/notes.html"));
    let index = dir.read("docs/_build/html/index.html");
    has(&index, "class=\"graphviz\"");
    has(&index, "id=\"tool.run\"");
    assert!(dir.exists("docs/_build/pdf/demo-en.pdf"));
    let r = bare(&docs, &["migrate", "old.py", "-o", "-", "--color=never"]);
    has(&r.out, "project = \"Old\"");
}
