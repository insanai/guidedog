//! `cargo xtask release <step>`: the steps of a release, as .github/workflows/release.yml
//! runs them (RELEASING.md tells the whole story). Each step also runs by hand.

mod build;
mod compat;
mod files;
mod github;
mod package;
mod pages;
mod smoke;
mod version;
mod windows;

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{Result, fail, process, usage};

const HELP: &str = "\
The steps of a release (RELEASING.md tells the whole story).

Usage: cargo xtask release <STEP> [ARGS]

Steps:
  build [--target TRIPLE] [--dry-run]
                      build guidedog, guidedog-pages and graphdog the way the
                      release workflow does, into build/
  package PACKAGE     write dist/guidedog-PACKAGE.tar.gz
                      (linux-amd64, linux-arm64, macos-arm64, macos-amd64, windows-amd64)
  source              write dist/guidedog-VERSION-source.tar.gz from the commit
  checksums DIR       write DIR/SHA256SUMS for the release files in DIR
  formula DIR         render the Homebrew formula DIR/guidedog.rb
  publish [--check] [--dry-run] TAG [DIR]
                      publish DIR as the GitHub release TAG, never replacing one
  check-version [PROGRAM]
                      check that VERSION, Cargo.toml, the tag and PROGRAM agree
  check-run RUN TAG   check that a release workflow run may be published
  credential-store    test gd-pages' credential store against a private Secret Service
  smoke [--external-typst] PROGRAM
                      smoke-test guidedog: an HTML site and a PDF book
  smoke-discussion PROGRAM
                      smoke-test guidedog dsc: a discussion catalog, HTML and PDF
  smoke-graphdog PROGRAM
                      smoke-test graphdog with its built-in Graphviz
  smoke-pages PAGES GUIDEDOG
                      smoke-test guidedog-pages and guidedog publish, offline
  plugin-compatibility [--work DIR]
                      qualify plugin protocol 1.0 against the 0.6.0 release, both ways

OUT sets the output folder of package and source (default: dist). GitHub steps read
GH_TOKEN and GITHUB_REPOSITORY. `cargo xtask release <STEP> --help` explains a step.
";

/// The programs a release builds, in the order the archives list them.
pub(crate) const PROGRAMS: [&str; 3] = ["guidedog", "guidedog-pages", "graphdog"];

/// The steps, for "Did you mean" hints.
const STEPS: [&str; 14] = [
    "build",
    "package",
    "source",
    "checksums",
    "formula",
    "publish",
    "check-version",
    "check-run",
    "credential-store",
    "smoke",
    "smoke-discussion",
    "smoke-graphdog",
    "smoke-pages",
    "plugin-compatibility",
];

/// Runs `cargo xtask release <step>`.
pub(crate) fn main(args: &[String]) -> Result<()> {
    let rest = args.get(1..).unwrap_or_default();
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help" | "help") => {
            print!("{HELP}");
            Ok(())
        }
        Some("build") => build::main(rest),
        Some("package") => package::main(rest),
        Some("source") => package::source_main(rest),
        Some("checksums") => files::checksums_main(rest),
        Some("formula") => files::formula_main(rest),
        Some("publish") => github::publish_main(rest),
        Some("check-version") => version::main(rest),
        Some("check-run") => github::check_run_main(rest),
        Some("credential-store") => pages::credential_store_main(rest),
        Some("smoke") => smoke::main(rest),
        Some("smoke-discussion") => smoke::discussion_main(rest),
        Some("smoke-graphdog") => smoke::graphdog_main(rest),
        Some("smoke-pages") => pages::smoke_main(rest),
        Some("plugin-compatibility") => compat::main(rest),
        Some(other) => {
            let mut hint = String::from("`cargo xtask release --help` lists the steps.");
            if let Some(near) = gd_report::suggest(other, STEPS) {
                hint.push_str(&format!(" Did you mean `{near}`?"));
            }
            Err(usage(&format!("There is no release step named `{other}`."), &hint))
        }
    }
}

/// The folder package and source write to: OUT, or dist in the repository.
fn dist_dir(root: &Path) -> PathBuf {
    match std::env::var_os("OUT").filter(|out| !out.is_empty()) {
        Some(out) => crate::fsutil::absolute(Path::new(&out)),
        None => root.join("dist"),
    }
}

/// Fails unless the working tree is clean, so an archive matches its commit.
fn require_clean_tree(root: &Path) -> Result<()> {
    let mut git = Command::new("git");
    git.arg("-C").arg(root).args(["status", "--porcelain"]);
    let status = process::output(&mut git)?;
    if status.trim().is_empty() {
        return Ok(());
    }
    let mut message = String::from("The working tree has changes:\n\n");
    for line in status.lines().take(20) {
        message.push_str("    ");
        message.push_str(line);
        message.push('\n');
    }
    Err(fail(
        "release-dirty",
        "UNCOMMITTED CHANGES",
        message.trim_end(),
        "Check the changes in with git commit, so the archive matches its commit.",
    ))
}

/// The file name of an archive of the release for `package`.
fn archive_name(name: &str, package: &str) -> String {
    format!("{name}-{package}.tar.gz")
}

/// An absolute path for a program or file given on the command line.
fn given_path(path: &str) -> PathBuf {
    crate::fsutil::absolute(Path::new(path))
}
