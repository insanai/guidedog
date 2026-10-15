//! Guidedog's repository tasks, run as `cargo xtask <task>` (an alias in
//! `.cargo/config.toml`): the website, the CJK fonts, the release steps and their smoke
//! tests, the data tables generated from `tools/data`, and the verification tasks that
//! compare Guidedog with Sphinx and the RST corpus with Docutils.
//!
//! Every task is plain Rust: external programs (cargo, guidedog, git, gh, curl, tar,
//! gzip) run through `std::process::Command` with argument vectors, never a shell, and
//! their output streams to the terminal. A failure prints an Elm-style report with a
//! hint and exits 1 (2 for a usage problem).

#![forbid(unsafe_code)]

mod args;
mod docutils;
mod evidence;
mod fonts;
mod fsutil;
mod graphviz;
mod process;
mod release;
mod site;
mod sphinxdiff;
mod tables;

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use gd_report::Report;

/// The tasks, for `cargo xtask --help`.
const HELP: &str = "\
Guidedog's repository tasks.

Usage: cargo xtask <TASK> [OPTIONS]

Tasks:
  build-site [--guidedog PATH] [--out DIR] [--no-pdf]
                     build the website: landing page, manual, PDF books, discussions
  fetch-fonts [DIR]  fetch the Noto Serif CJK fonts the CJK PDF books use, checked
                     against their pinned SHA-256
  graphviz [--out DIR] [--jobs N]
                     build the static Graphviz libraries release binaries link
  rstref [--ids] DOCUTILS FIXTURES_DIR [SOURCE...]
                     store the Docutils reference trees (or ids.txt) of the RST
                     conformance corpus, with a pinned Python Docutils
  release <STEP>     one step of a release (build, package, checksums, source, formula,
                     publish, check-version, check-run, credential-store, smoke,
                     smoke-discussion, smoke-graphdog, smoke-pages,
                     plugin-compatibility)
  sphinxdiff SPHINX_HTML GUIDEDOG_HTML [-v]
             [--warnings SPHINX_LOG GUIDEDOG_JSON_LOG SOURCE_DIR]
                     compare a project's HTML built by Sphinx with Guidedog's
  sphinxdiff-roots SPHINX_BUILD ROOTS_DIR WORK_DIR [RECORD] [--guidedog PATH]
                     build Sphinx's test roots with both builders and record the
                     comparison
  tables [--check]   regenerate the Unicode, HTML entity and Pygments style tables from
                     tools/data; --check only compares them with the committed files
  verify-docutils DOCUTILS WORK_DIR [RECORD]
                     regenerate the RST conformance corpus with a pinned Docutils and
                     compare it byte for byte with the committed files

`cargo xtask <TASK> --help` explains a task and its options.
";

/// A task that failed: the report to show, and whether it was a usage problem.
#[derive(Debug)]
pub(crate) struct Failure {
    report: Box<Report>,
    usage: bool,
}

/// What every task returns.
pub(crate) type Result<T> = std::result::Result<T, Failure>;

/// Makes the failure of a task, with a title, a plain explanation and a hint.
pub(crate) fn fail(code: &str, title: &str, message: &str, hint: &str) -> Failure {
    let report = Report::error(&format!("xtask.{code}"), title, message).hint(hint);
    Failure { report: Box::new(report), usage: false }
}

/// Makes a usage failure, which exits with status 2.
pub(crate) fn usage(message: &str, hint: &str) -> Failure {
    Failure { usage: true, ..fail("usage", "USAGE", message, hint) }
}

/// The repository's root, where the workspace's Cargo.toml is: two folders above this
/// crate. Tasks run their builds from there, as the old scripts did.
pub(crate) fn repository_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest.ancestors().nth(2).unwrap_or(manifest).to_path_buf()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            let style = gd_report::Style { color: std::io::stderr().is_terminal(), width: 80 };
            eprint!("{}", gd_report::render_text(&failure.report, &style));
            ExitCode::from(if failure.usage { 2 } else { 1 })
        }
    }
}

fn run(args: &[String]) -> Result<()> {
    let rest = args.get(1..).unwrap_or_default();
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help" | "help") => {
            print!("{HELP}");
            Ok(())
        }
        Some("build-site") => site::main(rest),
        Some("fetch-fonts") => fonts::main(rest),
        Some("graphviz") => graphviz::main(rest),
        Some("release") => release::main(rest),
        Some("rstref") => docutils::main(rest),
        Some("sphinxdiff") => sphinxdiff::main(rest),
        Some("sphinxdiff-roots") => sphinxdiff::roots_main(rest),
        Some("tables") => tables::main(rest),
        Some("verify-docutils") => docutils::verify_main(rest),
        Some(other) => {
            let tasks = [
                "build-site",
                "fetch-fonts",
                "graphviz",
                "release",
                "rstref",
                "sphinxdiff",
                "sphinxdiff-roots",
                "tables",
                "verify-docutils",
            ];
            let mut hint = String::from("`cargo xtask --help` lists the tasks.");
            if let Some(near) = gd_report::suggest(other, tasks) {
                hint.push_str(&format!(" Did you mean `{near}`?"));
            }
            Err(usage(&format!("There is no task named `{other}`."), &hint))
        }
    }
}
