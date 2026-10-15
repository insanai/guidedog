//! `release build`: builds the release programs the way the release workflow does,
//! copies them into build/, and records the exact command in build/build.command for the
//! archives' SOURCE.txt.
//!
//! The Typst compiler is embedded (gd-typst's default `embedded` feature) and tree-sitter
//! is compiled in. Graphviz is linked statically from the libraries `cargo xtask graphviz`
//! builds (built first when missing), so the programs need no Graphviz at run time. On
//! macOS the build targets macOS 12.

use std::path::PathBuf;
use std::process::Command;

use super::PROGRAMS;
use crate::args::{self, Spec};
use crate::fsutil::{copy, create_dir, write};
use crate::{Result, process, repository_root};

const HELP: &str = "\
Builds guidedog, guidedog-pages and graphdog the way the release workflow
does (cargo build --release --locked) and copies them into build/.

Usage: cargo xtask release build [--target TRIPLE] [--dry-run]

  --target TRIPLE   build for this Rust target, such as aarch64-apple-darwin
  --dry-run         print the cargo command and change nothing

Graphviz is linked statically: GUIDEDOG_GRAPHVIZ_STATIC names its libraries, or
`cargo xtask graphviz` builds them into target/graphviz first. build/build.command and
build/rustc.version record how the programs were built, for the archives' SOURCE.txt.
";

/// What `release build` builds.
#[derive(Debug, Clone, Default)]
pub(crate) struct Options {
    /// The Rust target triple; `None` is the host's.
    pub target: Option<String>,
    /// Print the command instead of running it.
    pub dry_run: bool,
}

/// Runs `release build`.
pub(super) fn main(args: &[String]) -> Result<()> {
    let spec = Spec {
        command: "release build",
        flags: &["--dry-run"],
        options: &["--target"],
        help: HELP,
    };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(0, &spec)?;
    let options = Options {
        target: parsed.option("--target").map(String::from),
        dry_run: parsed.has("--dry-run"),
    };
    build(&options)
}

/// Builds the programs into build/ as [`Options`] says.
pub(crate) fn build(options: &Options) -> Result<()> {
    let root = repository_root();
    let mut cargo = Command::new("cargo");
    cargo.current_dir(&root).args(["build", "--release", "--locked"]);
    for program in PROGRAMS {
        cargo.arg("-p").arg(format!("{program}-cmd"));
    }
    if let Some(target) = &options.target {
        cargo.args(["--target", target]);
    }
    let graphviz = static_graphviz(options.dry_run)?;
    let shown = graphviz.strip_prefix(&root).unwrap_or(&graphviz).display().to_string();
    let line = format!("GUIDEDOG_GRAPHVIZ_STATIC={shown} {}", process::describe(&cargo));
    println!("{line}");
    if options.dry_run {
        return Ok(());
    }
    cargo.env("GUIDEDOG_GRAPHVIZ_STATIC", &graphviz);
    prepare_macos(&mut cargo);
    let build = root.join("build");
    create_dir(&build)?;
    write(&build.join("build.command"), format!("{line}\n"))?;
    process::run(&mut cargo)?;
    let out = target_dir(&root).join(options.target.as_deref().unwrap_or("")).join("release");
    let exe = std::env::consts::EXE_SUFFIX;
    for program in PROGRAMS {
        let name = format!("{program}{exe}");
        copy(&out.join(&name), &build.join(&name))?;
    }
    let rustc = process::output(Command::new("rustc").arg("--version"))?;
    write(&build.join("rustc.version"), rustc)?;
    println!("build/: {}", PROGRAMS.join(" "));
    Ok(())
}

/// Cargo's target folder: CARGO_TARGET_DIR (relative to the repository) or target.
pub(super) fn target_dir(root: &std::path::Path) -> PathBuf {
    let dir = std::env::var_os("CARGO_TARGET_DIR").filter(|d| !d.is_empty());
    match dir.map(PathBuf::from) {
        Some(dir) if dir.is_absolute() => dir,
        Some(dir) => root.join(dir),
        None => root.join("target"),
    }
}

/// The static Graphviz install folder: GUIDEDOG_GRAPHVIZ_STATIC when set, otherwise the
/// default one of `cargo xtask graphviz`, built now unless it is there or this is a dry run.
fn static_graphviz(dry_run: bool) -> Result<PathBuf> {
    let dir = crate::graphviz::install_dir();
    if dry_run || std::env::var_os("GUIDEDOG_GRAPHVIZ_STATIC").is_some_and(|d| !d.is_empty()) {
        return Ok(dir);
    }
    crate::graphviz::ensure_default(4)
}

/// On macOS, targets macOS 12 unless MACOSX_DEPLOYMENT_TARGET says otherwise.
fn prepare_macos(cargo: &mut Command) {
    if cfg!(target_os = "macos") && std::env::var_os("MACOSX_DEPLOYMENT_TARGET").is_none() {
        cargo.env("MACOSX_DEPLOYMENT_TARGET", "12.0");
    }
}
