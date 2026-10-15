//! `release smoke`, `release smoke-discussion` and `release smoke-graphdog`: each program
//! run the way a new user runs it, in a temporary folder, with an empty PATH where the
//! program must carry what it needs.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::args::{self, Spec};
use crate::fsutil::{TempDir, create_dir, read_text, write};
use crate::{Failure, Result, fail, process};

const SMOKE_HELP: &str = "\
Smoke-tests a guidedog program the way a new user runs it: quickstart, then an HTML site
and a PDF book from a project with a Graphviz diagram, an equation, Markdown, and API
documentation read from Python source without running Python.

Usage: cargo xtask release smoke [--external-typst] PROGRAM

By default the program runs with an empty PATH, so it must carry everything itself except
the system libraries it links (libc and Graphviz): the embedded Typst compiler and
tree-sitter. --external-typst keeps PATH, for builds without the `embedded` feature that
run an installed typst program.
";

const DISCUSSION_HELP: &str = "\
Smoke-tests the discussion programs the way a new user runs them: a new catalog, a draft,
a promotion, a decision, and its HTML site and PDF books.

Usage: cargo xtask release smoke-discussion PROGRAM [GUIDEDOG]

  dsc builds with the GUIDEDOG it is given; dscc, given alone, with the Guidedog built
  into it. Each runs with an empty PATH.
";

const GRAPHDOG_HELP: &str = "\
Smoke-tests graphdog with the system's Graphviz: a graph to SVG, and a syntax error
reported with its location.

Usage: cargo xtask release smoke-graphdog PROGRAM
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
    println!("smoke test passed: {bytes} byte PDF and an HTML site");
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
    parsed.at_most(2, &spec)?;
    let program = super::given_path(parsed.positional(0, "the dsc or dscc program", &spec)?);
    let guidedog = parsed.positionals.get(1).map(|g| super::given_path(g));
    smoke_discussion(&program, guidedog.as_deref())
}

/// Smoke-tests dsc with `guidedog`, or dscc without one.
pub(crate) fn smoke_discussion(program: &Path, guidedog: Option<&Path>) -> Result<()> {
    let work = TempDir::new("discussion")?;
    let runner = Runner::new(program.to_path_buf(), work.path(), false)?;
    let version = runner.output(work.path(), &["--version"])?;
    print!("{version}");
    let engine = guidedog.map(|g| format!("--guidedog={}", g.display()));
    if engine.is_none() && !version.contains("Typst: built in") {
        return Err(smoke_failure(
            "discussion",
            &format!("{} has no Typst built in", program.display()),
        ));
    }
    runner.run(
        work.path(),
        &["init", "eds", "--prefix=eds", "--name=Smoke Discussions", "--author=Release"],
    )?;
    let eds = work.path().join("eds");
    runner.run(&eds, &["new", "Cache policy", "--labels=architecture"])?;
    runner.run(&eds, &["promote", "cache-policy"])?;
    runner.run(
        &eds,
        &["state", "1", "--to=accepted", "--reason=Reviewed in the release smoke test"],
    )?;
    runner.run(&eds, &["check"])?;
    let mut build = vec!["build", "--color=never"];
    build.extend(engine.as_deref());
    runner.run(&eds, &build)?;
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
    if guidedog.is_some() {
        let missing = format!("--guidedog={}", work.path().join("no-guidedog").display());
        let out = process::capture(&mut runner.command(&eds, &["build", &missing]))?;
        if out.success {
            return Err(smoke_failure("discussion", "dsc built without a guidedog"));
        }
    }
    let name = program.file_name().unwrap_or_default().to_string_lossy();
    println!("discussion smoke test passed: {name}, an HTML site and PDF books");
    Ok(())
}

/// Runs `release smoke-graphdog`.
pub(super) fn graphdog_main(args: &[String]) -> Result<()> {
    let spec =
        Spec { command: "release smoke-graphdog", flags: &[], options: &[], help: GRAPHDOG_HELP };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(1, &spec)?;
    let program = super::given_path(parsed.positional(0, "the graphdog program", &spec)?);
    smoke_graphdog(&program)
}

/// Smoke-tests graphdog: a valid graph renders, a broken one fails.
pub(crate) fn smoke_graphdog(program: &Path) -> Result<()> {
    process::run(Command::new(program).arg("--version"))?;
    let valid = process::feed(
        Command::new(program).arg("-Tsvg"),
        b"digraph { source -> html; source -> pdf }\n",
    )?;
    if !valid.success || !valid.stdout.contains("<svg") {
        eprint!("{}", valid.stderr);
        return Err(smoke_failure("graphdog", "no SVG from a valid graph"));
    }
    let broken =
        process::feed(Command::new(program).arg("-Tsvg"), b"digraph {\n    a -> -> b\n}\n")?;
    if broken.success {
        return Err(smoke_failure("graphdog", "a broken graph rendered"));
    }
    println!("graphdog smoke test passed");
    Ok(())
}
