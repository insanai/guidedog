//! `release build`: builds the release programs the way the release workflow does,
//! copies them into build/, and records the exact command in build/build.command for the
//! archives' SOURCE.txt.
//!
//! The Typst compiler is embedded (gd-typst's default `embedded` feature) and tree-sitter
//! is compiled in. Graphviz is linked from the system: its development files must be
//! installed first (Debian/Ubuntu: libgraphviz-dev; macOS: brew install graphviz). On
//! macOS the linker is pointed at Homebrew's Graphviz and the build targets macOS 12.

use std::path::PathBuf;
use std::process::Command;

use super::PROGRAMS;
use crate::args::{self, Spec};
use crate::fsutil::{copy, create_dir, write};
use crate::{Result, process, repository_root};

const HELP: &str = "\
Builds guidedog, dsc, dscc, guidedog-pages and graphdog the way the release workflow
does (cargo build --release --locked) and copies them into build/.

Usage: cargo xtask release build [--target TRIPLE] [--dry-run]

  --target TRIPLE   build for this Rust target, such as aarch64-apple-darwin
  --dry-run         print the cargo command and change nothing

Graphviz's development files must be installed (Debian/Ubuntu: libgraphviz-dev; macOS:
brew install graphviz). build/build.command and build/rustc.version record how the
programs were built, for the archives' SOURCE.txt.
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
    let line = process::describe(&cargo);
    println!("{line}");
    if options.dry_run {
        return Ok(());
    }
    prepare_macos(&mut cargo)?;
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
fn target_dir(root: &std::path::Path) -> PathBuf {
    let dir = std::env::var_os("CARGO_TARGET_DIR").filter(|d| !d.is_empty());
    match dir.map(PathBuf::from) {
        Some(dir) if dir.is_absolute() => dir,
        Some(dir) => root.join(dir),
        None => root.join("target"),
    }
}

/// On macOS, targets macOS 12 unless MACOSX_DEPLOYMENT_TARGET says otherwise, and points
/// the linker at Homebrew's Graphviz when brew is installed.
fn prepare_macos(cargo: &mut Command) -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Ok(());
    }
    if std::env::var_os("MACOSX_DEPLOYMENT_TARGET").is_none() {
        cargo.env("MACOSX_DEPLOYMENT_TARGET", "12.0");
    }
    let Ok(prefix) = process::output(Command::new("brew").args(["--prefix", "graphviz"])) else {
        return Ok(());
    };
    let mut library_path = format!("{}/lib", prefix.trim());
    if let Some(existing) = std::env::var_os("LIBRARY_PATH").filter(|p| !p.is_empty()) {
        library_path.push(':');
        library_path.push_str(&existing.to_string_lossy());
    }
    cargo.env("LIBRARY_PATH", library_path);
    Ok(())
}
