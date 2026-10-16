//! The repository wrapper delegates website assembly to Guidedog's generic SSG mounts.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::args::{self, Spec};
use crate::fsutil::{self, absolute};
use crate::{Result, fail, process, repository_root};

const HELP: &str = "\
Builds the website declared in docs/conf.toml with guidedog ssg build.

Usage: cargo xtask build-site [--guidedog PATH] [--out DIR] [--no-pdf]

  --guidedog PATH   executable to use; otherwise build target/release/guidedog
  --out DIR         final website directory (default: build/site)
  --no-pdf          skip PDF mounts for a local preview

Fetch the CJK print fonts with cargo xtask fetch-fonts before building the PDF editions.
Every project must succeed before the complete website replaces its previous output.
";

/// Runs the generic site command with this repository's declarative configuration.
pub(crate) fn main(args: &[String]) -> Result<()> {
    let spec = Spec {
        command: "build-site",
        flags: &["--no-pdf"],
        options: &["--guidedog", "--out"],
        help: HELP,
    };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(0, &spec)?;
    let root = repository_root();
    let executable = parsed
        .option("--guidedog")
        .map(String::from)
        .or_else(|| std::env::var("GUIDEDOG").ok())
        .filter(|g| !g.is_empty());
    let guidedog = match executable {
        Some(name) => program_path(&name),
        None => build_guidedog(&root, Instant::now())?,
    };
    if !fsutil::is_executable(&guidedog) {
        return Err(fail(
            "site-guidedog",
            "NO GUIDEDOG",
            &format!("{} is not executable.", guidedog.display()),
            "Build guidedog or pass --guidedog PATH.",
        ));
    }
    let output = parsed
        .option("--out")
        .map(|p| absolute(Path::new(p)))
        .unwrap_or_else(|| root.join("build/site"));
    let mut command = Command::new(&guidedog);
    command.current_dir(&root).args(["ssg", "build", "docs", "--out"]).arg(output);
    if parsed.has("--no-pdf") {
        command.arg("--no-pdf");
    }
    process::run(&mut command)
}

/// Finds a named program on PATH, or makes an explicit path absolute.
pub(crate) fn program_path(name: &str) -> PathBuf {
    if name.contains(std::path::MAIN_SEPARATOR) || name.contains('/') {
        return absolute(Path::new(name));
    }
    let found = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path).map(|dir| dir.join(name)).find(|p| fsutil::is_executable(p))
    });
    found.unwrap_or_else(|| PathBuf::from(name))
}

/// Builds the release executable, honoring CARGO_TARGET_DIR; fails when Cargo fails.
pub(crate) fn build_guidedog(root: &Path, started: Instant) -> Result<PathBuf> {
    println!("== building guidedog ({} s)", started.elapsed().as_secs());
    let mut cargo = Command::new("cargo");
    cargo.current_dir(root).args(["build", "--release", "-p", "guidedog-cmd"]);
    process::run(&mut cargo)?;
    let target =
        std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), PathBuf::from);
    let target = if target.is_absolute() { target } else { root.join(target) };
    Ok(target.join("release").join(format!("guidedog{}", std::env::consts::EXE_SUFFIX)))
}
