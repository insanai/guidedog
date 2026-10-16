//! `release smoke`, `release smoke-discussion`: each program
//! run the way a new user runs it, in a temporary folder, with an empty PATH where the
//! program must carry what it needs.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::args::{self, Spec};
use crate::fsutil::{TempDir, create_dir, read_text, write};
use crate::{Failure, Result, fail, process};

mod ssg;

const SMOKE_HELP: &str = "\
Smoke-tests a guidedog program the way a new user runs it: quickstart, then an HTML site
and a PDF book from a project with a Graphviz diagram, an equation, Markdown, and API
documentation read from Python source without running Python.

Usage: cargo xtask release smoke [--external-typst] PROGRAM

By default the program runs with an empty PATH, so it must carry everything itself except
the system libraries it links (libc): the embedded Typst compiler, tree-sitter and
Graphviz. --external-typst keeps PATH, for builds without the `embedded` feature that
run an installed typst program.
";

const DISCUSSION_HELP: &str = "\
Smoke-tests guidedog dsc, the discussion catalogs, the way a new user runs them: a new
catalog, a draft, a promotion, a decision, and its HTML site and PDF books.

Usage: cargo xtask release smoke-discussion PROGRAM

PROGRAM is guidedog. It runs with an empty PATH, so it must link the discussion plugin
and build with the Typst compiler embedded in it.
";

/// The variables a program keeps when it runs with an otherwise empty environment, where
/// set (the Windows ones let a Windows program start at all).
const KEPT_VARIABLES: [&str; 8] =
    ["TMPDIR", "SYSTEMROOT", "WINDIR", "USERPROFILE", "LOCALAPPDATA", "APPDATA", "TEMP", "TMP"];

/// A smoke test's failure: what went wrong, for the step named by `test`.
fn smoke_failure(test: &str, what: &str) -> Failure {
    fail(
        "smoke",
        "SMOKE TEST FAILED",
        &format!("{test} smoke test: {what}."),
        "Run the program by hand in a fresh folder to see the problem, then fix and rebuild it.",
    )
}

/// Runs a program the way the smoke tests do: in `dir`, with only HOME, an empty PATH,
/// and [`KEPT_VARIABLES`], unless `keep_env`.
struct Runner {
    program: PathBuf,
    home: PathBuf,
    bin: PathBuf,
    keep_env: bool,
}

impl Runner {
    fn new(program: PathBuf, work: &Path, keep_env: bool) -> Result<Runner> {
        let (home, bin) = (work.join("home"), work.join("bin"));
        create_dir(&home)?;
        create_dir(&bin)?;
        Ok(Runner { program, home, bin, keep_env })
    }

    fn command(&self, dir: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(&self.program);
        command.current_dir(dir).args(args);
        if !self.keep_env {
            command.env_clear().env("HOME", &self.home).env("PATH", &self.bin);
            for name in KEPT_VARIABLES {
                if let Some(value) = std::env::var_os(name) {
                    command.env(name, value);
                }
            }
        }
        command
    }

    fn run(&self, dir: &Path, args: &[&str]) -> Result<()> {
        process::run(&mut self.command(dir, args))
    }

    fn output(&self, dir: &Path, args: &[&str]) -> Result<String> {
        process::output(&mut self.command(dir, args))
    }
}

/// Runs `release smoke`.
pub(super) fn main(args: &[String]) -> Result<()> {
    let spec = Spec {
        command: "release smoke",
        flags: &["--external-typst"],
        options: &[],
        help: SMOKE_HELP,
    };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(1, &spec)?;
    let program = super::given_path(parsed.positional(0, "the guidedog program", &spec)?);
    smoke(&program, parsed.has("--external-typst"))
}

const SMOKE_CONF: &str = "\
project = \"Smoke\"
author = \"Release\"
extensions = [\"sphinx.ext.graphviz\", \"sphinx.ext.mathjax\", \
    \"myst_parser\", \"sphinx.ext.autodoc\"]
autodoc_source_paths = [\".\"]
";

const SMOKE_INDEX: &str = "\
Smoke
=====

.. toctree::

   notes

.. digraph:: flow

   source -> html
   source -> pdf

.. math::

   e^{i \\pi} + 1 = 0

.. autofunction:: tool.run

.. glossary::

   Admission
      Letting a request through.

   Gate
      The surface without inspection.

First :envvar:`FIRST`.

Second :envvar:`SECOND`.
";

/// Smoke-tests a guidedog program; `external_typst` keeps the environment.
pub(crate) fn smoke(program: &Path, external_typst: bool) -> Result<()> {
    let work = TempDir::new("smoke")?;
    let dir = work.path();
    let runner = Runner::new(program.to_path_buf(), dir, external_typst)?;
    let version = runner.output(dir, &["--version"])?;
    print!("{version}");
    if !external_typst && !version.lines().any(|l| l.starts_with("typst 0.15.1 (embedded)")) {
        return Err(smoke_failure(
            "guidedog",
            &format!("{} does not embed Typst 0.15.1", program.display()),
        ));
    }
    runner.run(dir, &["quickstart", "docs", "-q", "-p", "Smoke", "-a", "Release"])?;
    write(&dir.join("docs/conf.toml"), SMOKE_CONF)?;
    write(&dir.join("docs/index.rst"), SMOKE_INDEX)?;
    write(&dir.join("docs/tool.py"), "def run(x, y=1):\n    \"\"\"Runs x.\"\"\"\n")?;
    write(&dir.join("docs/notes.md"), "# Notes\n\nSome *Markdown*, and ``code``.\n")?;
    runner.run(dir, &["build", "html", "docs", "-W", "--color=never"])?;
    runner.run(dir, &["build", "pdf", "docs", "-W", "--color=never"])?;
    let html_path = dir.join("docs/_build/html/index.html");
    let html = read_text(&html_path).map_err(|_| smoke_failure("guidedog", "no index.html"))?;
    let checks = [
        (dir.join("docs/_build/html/notes.html").is_file(), "no notes.html from Markdown"),
        (html.contains("class=\"graphviz\""), "the diagram is missing from index.html"),
        (html.contains("<svg"), "no SVG in index.html"),
        (html.contains("id=\"tool.run\""), "autodoc did not document tool.run"),
    ];
    if let Some((_, what)) = checks.iter().find(|(ok, _)| !ok) {
        return Err(smoke_failure("guidedog", what));
    }
    let bytes = check_pdf(&dir.join("docs/_build/pdf/smoke-en.pdf"), "guidedog")?;
    smoke_translation(&runner, dir)?;
    smoke_publication(&runner, dir)?;
    ssg::run(&runner, dir)?;
    println!("smoke test passed: {bytes} byte PDF and an HTML site");
    Ok(())
}

/// Qualifies the linked publication commands and article compiler in every archive.
fn smoke_publication(runner: &Runner, dir: &Path) -> Result<()> {
    let docs = dir.join("docs");
    runner.run(
        &docs,
        &["ssg", "new", "article.rst", "--title", "Publication", "--now=2026-10-16T09:00:00Z"],
    )?;
    let path = docs.join("article.rst");
    let source = read_text(&path)?;
    write(&path, format!(":pdf: article\n{source}"))?;
    write(
        &docs.join("conf.toml"),
        format!(
            "{SMOKE_CONF}\n[site]\n\
        base_url = \"https://example.org/smoke\"\nfeeds = true\nsitemap = true\n"
        ),
    )?;
    write(&docs.join("index.rst"), format!("{SMOKE_INDEX}\n\n.. postlist::\n"))?;
    runner.run(&docs, &["ssg", "schedule", "article.rst", "--at=2026-10-17T00:00:00Z"])?;
    runner.run(&docs, &["build", "html", "--now=2026-10-16T23:59:59Z", "-W"])?;
    let html = docs.join("_build/html");
    if html.join("article.html").exists() || html.join("article.pdf").exists() {
        return Err(smoke_failure("ssg", "scheduled article leaked before release"));
    }
    runner.run(&docs, &["build", "html", "--now=2026-10-17T00:00:00Z", "-W"])?;
    check_pdf(&html.join("article.pdf"), "ssg")?;
    for file in ["article.html", "atom.xml", "rss.xml", "sitemap.xml"] {
        if !html.join(file).is_file() {
            return Err(smoke_failure("ssg", &format!("missing {file}")));
        }
    }
    runner.run(&docs, &["ssg", "draft", "article.rst"])?;
    runner.run(&docs, &["build", "html", "--now=2026-10-17T00:00:00Z", "-W"])?;
    if html.join("article.html").exists() || html.join("article.pdf").exists() {
        return Err(smoke_failure("ssg", "withheld article artifacts were retained"));
    }
    runner.run(&docs, &["ssg", "publish", "article.rst", "--now=2026-10-17T00:00:00Z"])?;
    runner.run(&docs, &["ssg", "status", "--now=2026-10-17T00:00:00Z"])?;
    println!("ssg smoke test passed: lifecycle, collection, discovery and article PDF");
    Ok(())
}

/// Checks translated metadata with the shipped executable on every release platform.
fn smoke_translation(runner: &Runner, dir: &Path) -> Result<()> {
    let po = "msgid \"Admission\"\nmsgstr \"許可\"\n\n\
              msgid \"First :envvar:`FIRST`.\"\nmsgstr \"最初 :envvar:`FIRST`.\"\n\n\
              msgid \"Second :envvar:`SECOND`.\"\nmsgstr \"次 :envvar:`SECOND`.\"\n";
    let catalogs = dir.join("docs/locales/ja/LC_MESSAGES");
    create_dir(&catalogs)?;
    write(&catalogs.join("index.po"), po)?;
    runner.run(dir, &["build", "html", "docs", "-W", "-D", "language=ja", "--color=never"])?;
    let index = read_text(&dir.join("docs/_build/html/genindex.html"))?;
    let page = read_text(&dir.join("docs/_build/html/index.html"))?;
    for word in ["許可", "Gate"] {
        if !index.contains(word) {
            return Err(smoke_failure("translation", &format!("the index lost {word}")));
        }
    }
    for anchor in ["term-Admission", "index-0", "index-1"] {
        if !index.contains(&format!("index.html#{anchor}"))
            || page.matches(&format!("id=\"{anchor}\"")).count() != 1
        {
            return Err(smoke_failure(
                "translation",
                &format!("index target {anchor} is invalid"),
            ));
        }
    }
    Ok(())
}

/// Fails unless the file is a PDF; returns its size.
fn check_pdf(path: &Path, test: &str) -> Result<u64> {
    let bytes =
        std::fs::read(path).map_err(|_| smoke_failure(test, &format!("no {}", path.display())))?;
    if !bytes.starts_with(b"%PDF-") {
        return Err(smoke_failure(test, &format!("{} is not a PDF", path.display())));
    }
    Ok(bytes.len() as u64)
}

/// Runs `release smoke-discussion`.
pub(super) fn discussion_main(args: &[String]) -> Result<()> {
    let spec = Spec {
        command: "release smoke-discussion",
        flags: &[],
        options: &[],
        help: DISCUSSION_HELP,
    };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(1, &spec)?;
    let program = super::given_path(parsed.positional(0, "the guidedog program", &spec)?);
    smoke_discussion(&program)
}

/// Smoke-tests `guidedog dsc`, the discussion plugin linked into guidedog.
pub(crate) fn smoke_discussion(program: &Path) -> Result<()> {
    let work = TempDir::new("discussion")?;
    let runner = Runner::new(program.to_path_buf(), work.path(), false)?;
    let help = runner.output(work.path(), &["dsc", "--help"])?;
    if !help.contains("dsc build") {
        return Err(smoke_failure(
            "discussion",
            &format!("{} does not link the discussion plugin", program.display()),
        ));
    }
    runner.run(
        work.path(),
        &["dsc", "init", "eds", "--prefix=eds", "--name=Smoke Discussions", "--author=Release"],
    )?;
    let eds = work.path().join("eds");
    let template = read_text(&eds.join("_templates/discussion.typ"))?;
    let theme = read_text(&eds.join("_templates/theme.typ"))?;
    if !template.contains("record: (id:") || !theme.contains("configure-document") {
        return Err(smoke_failure("discussion", "init did not install the discussion PDF layout"));
    }
    runner.run(&eds, &["dsc", "new", "Cache policy", "--labels=architecture"])?;
    runner.run(&eds, &["dsc", "promote", "cache-policy"])?;
    runner.run(
        &eds,
        &["dsc", "state", "1", "--to=accepted", "--reason=Reviewed in the release smoke test"],
    )?;
    runner.run(&eds, &["dsc", "check"])?;
    runner.run(&eds, &["dsc", "build", "--color=never"])?;
    let html = eds.join("_build/html/records/0001-cache-policy.html");
    let text = read_text(&html)
        .map_err(|_| smoke_failure("discussion", &format!("no {}", html.display())))?;
    if !text.contains("Authors") {
        return Err(smoke_failure(
            "discussion",
            "the record page does not show the metadata as Authors",
        ));
    }
    check_pdf(&eds.join("_build/pdf/eds-0001-cache-policy-en.pdf"), "discussion")?;
    let source = read_text(&eds.join("_build/pdf/sources/eds-0001-cache-policy-en.pdf.typ"))?;
    if !source.contains("#import \"/_templates/discussion.typ\": book")
        || !source.contains("record: (id: \"EDS 0001\"")
        || source.contains("#gd-fields(")
    {
        return Err(smoke_failure("discussion", "the PDF did not use the discussion layout"));
    }
    let name = program.file_name().unwrap_or_default().to_string_lossy();
    println!("discussion smoke test passed: {name} dsc, an HTML site and PDF books");
    Ok(())
}
