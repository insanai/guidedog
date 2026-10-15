//! `release check-version`: a release names one version everywhere. The VERSION file
//! holds it (0.7.0), `[workspace.package]` in Cargo.toml holds the same version, which
//! every crate inherits, a tag build (GITHUB_REF=refs/tags/...) must be tagged
//! v<VERSION>, and `guidedog --version` must begin "guidedog <VERSION>".

use std::path::Path;
use std::process::Command;

use crate::args::{self, Spec};
use crate::fsutil::read_text;
use crate::{Result, fail, process, repository_root};

const HELP: &str = "\
Checks that a release names one version everywhere, and prints it.

Usage: cargo xtask release check-version [PROGRAM]

  VERSION, Cargo.toml's [workspace.package] version, and the tag in GITHUB_REF (on a
  tag build) must agree. With PROGRAM (build/guidedog, build/dsc, ...), its --version
  must also begin with its own name and the version.
";

/// Runs `release check-version`.
pub(super) fn main(args: &[String]) -> Result<()> {
    let spec = Spec { command: "release check-version", flags: &[], options: &[], help: HELP };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(1, &spec)?;
    let version = release_version(&repository_root())?;
    if let Some(program) = parsed.positionals.first() {
        check_program(&super::given_path(program), &version)?;
    }
    println!("{version}");
    Ok(())
}

/// The release version, once VERSION, Cargo.toml and the tag being built agree.
pub(super) fn release_version(root: &Path) -> Result<String> {
    let path = root.join("VERSION");
    if !path.is_file() {
        return Err(fail(
            "version-missing",
            "NO VERSION FILE",
            "There is no VERSION file at the repository root.",
            "Create it with the release version on one line, such as 0.7.0.",
        ));
    }
    let version: String = read_text(&path)?.split_whitespace().collect();
    if !is_version(&version) {
        return Err(fail(
            "version-format",
            "NOT A VERSION",
            &format!("VERSION holds \"{version}\", which is not a version such as 0.7.0."),
            "Write three numbers joined by dots, optionally with a suffix: 0.7.0-rc.1.",
        ));
    }
    let cargo = workspace_version(&read_text(&root.join("Cargo.toml"))?);
    if cargo.as_deref() != Some(version.as_str()) {
        let cargo = cargo.unwrap_or_default();
        return Err(fail(
            "version-cargo",
            "VERSIONS DIFFER",
            &format!(
                "Cargo.toml's [workspace.package] version is \"{cargo}\", but VERSION is \
                 {version}."
            ),
            "Change the version in VERSION and Cargo.toml together.",
        ));
    }
    let reference = std::env::var("GITHUB_REF").unwrap_or_default();
    if let Some(tag) = reference.strip_prefix("refs/tags/")
        && tag != format!("v{version}")
    {
        return Err(fail(
            "version-tag",
            "TAG DIFFERS",
            &format!("The tag {tag} does not match VERSION {version}."),
            &format!("Tag the commit v{version}, or correct VERSION and tag again."),
        ));
    }
    Ok(version)
}

/// Whether the text is a version such as 0.7.0 or 0.7.0-rc.1.
fn is_version(text: &str) -> bool {
    let (core, suffix) = text.split_once('-').unwrap_or((text, ""));
    let numbers: Vec<&str> = core.split('.').collect();
    let numeric = |part: &&str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    let suffix_ok = suffix.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.');
    numbers.len() == 3
        && numbers.iter().all(numeric)
        && suffix_ok
        && (suffix.is_empty() == !text.contains('-'))
}

/// The first `version` key after `[workspace.package]` in a Cargo.toml.
fn workspace_version(manifest: &str) -> Option<String> {
    let mut inside = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == "[workspace.package]";
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        if inside && key.trim() == "version" {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// Fails unless `program --version` begins with its own name and the version.
pub(super) fn check_program(program: &Path, version: &str) -> Result<()> {
    let output = process::output(Command::new(program).arg("--version"))?;
    let reported = output.lines().next().unwrap_or_default().trim_end_matches('\r');
    let name = program.file_stem().unwrap_or_default().to_string_lossy();
    let expected = format!("{name} {version}");
    if reported == expected || reported.starts_with(&format!("{expected} ")) {
        return Ok(());
    }
    Err(fail(
        "version-program",
        "PROGRAM VERSION DIFFERS",
        &format!("{} reports \"{reported}\", but VERSION is {version}.", program.display()),
        "Rebuild after bumping VERSION and Cargo.toml together.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_versions() {
        assert!(is_version("0.7.0") && is_version("1.2.3-rc.1"));
        assert!(!is_version("0.7") && !is_version("0.7.0-") && !is_version("v0.7.0"));
        let manifest = "[package]\nversion = \"9\"\n[workspace.package]\nversion = \"0.7.0\"\n";
        assert_eq!(workspace_version(manifest).as_deref(), Some("0.7.0"));
    }
}
