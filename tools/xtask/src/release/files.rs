//! `release checksums` and `release formula`: the SHA256SUMS of a release, and its
//! Homebrew formula.

use std::path::Path;

use crate::args::{self, Spec};
use crate::fsutil::{read_text, sha256_file, sorted_entries, write};
use crate::{Result, fail, repository_root};

const CHECKSUMS_HELP: &str = "\
Writes DIR/SHA256SUMS for every release file in DIR (*.tar.gz, *.zip, *.rb), sorted by
name, in the format `sha256sum -c` reads.

Usage: cargo xtask release checksums DIR
";

const FORMULA_HELP: &str = "\
Renders the Homebrew formula DIR/guidedog.rb from tools/release/guidedog.rb.in, with the
version and the SHA-256 of the four macOS and Linux archives in DIR. The formula installs
the release binary for the machine, so every archive it names must be in DIR.

Usage: cargo xtask release formula DIR
";

/// The packages the Homebrew formula names, each with its placeholder.
const FORMULA_PACKAGES: [(&str, &str); 4] = [
    ("macos-arm64", "@MACOS_ARM64@"),
    ("macos-amd64", "@MACOS_AMD64@"),
    ("linux-arm64", "@LINUX_ARM64@"),
    ("linux-amd64", "@LINUX_AMD64@"),
];

/// Runs `release checksums`.
pub(super) fn checksums_main(args: &[String]) -> Result<()> {
    let spec =
        Spec { command: "release checksums", flags: &[], options: &[], help: CHECKSUMS_HELP };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(1, &spec)?;
    let dir = super::given_path(parsed.positional(0, "the folder of release files", &spec)?);
    let sums = write_checksums(&dir)?;
    print!("{sums}");
    Ok(())
}

/// Writes `dir/SHA256SUMS` and returns its text.
pub(crate) fn write_checksums(dir: &Path) -> Result<String> {
    let mut sums = String::new();
    for path in sorted_entries(dir)? {
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let release_file = [".tar.gz", ".zip", ".rb"].iter().any(|ext| name.ends_with(ext));
        if path.is_file() && release_file {
            sums.push_str(&format!("{}  {name}\n", sha256_file(&path)?));
        }
    }
    if sums.is_empty() {
        return Err(fail(
            "release-files",
            "NO RELEASE FILES",
            &format!("{} holds no release files (*.tar.gz, *.zip, *.rb).", dir.display()),
            "Put the archives there with `cargo xtask release package` first.",
        ));
    }
    write(&dir.join("SHA256SUMS"), &sums)?;
    Ok(sums)
}

/// Checks the files `dir/SHA256SUMS` lists against their sums, like `sha256sum -c`.
pub(crate) fn verify_checksums(dir: &Path) -> Result<()> {
    let sums = read_text(&dir.join("SHA256SUMS"))?;
    for line in sums.lines().filter(|line| !line.is_empty()) {
        let (expected, name) = line.split_once("  ").unwrap_or((line, ""));
        let got = sha256_file(&dir.join(name.trim_start_matches('*')))?;
        if got != expected {
            return Err(fail(
                "release-sums",
                "CHECKSUM MISMATCH",
                &format!("{name} in {} does not match its SHA256SUMS line.", dir.display()),
                "Rebuild the release files, then write SHA256SUMS again with `cargo xtask \
                 release checksums`.",
            ));
        }
    }
    Ok(())
}

/// Runs `release formula`.
pub(super) fn formula_main(args: &[String]) -> Result<()> {
    let spec = Spec { command: "release formula", flags: &[], options: &[], help: FORMULA_HELP };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(1, &spec)?;
    let dir = super::given_path(parsed.positional(0, "the folder of release archives", &spec)?);
    let root = repository_root();
    let version = super::version::release_version(&root)?;
    let mut formula =
        read_text(&root.join("tools/release/guidedog.rb.in"))?.replace("@VERSION@", &version);
    for (package, placeholder) in FORMULA_PACKAGES {
        let archive = dir.join(super::archive_name("guidedog", package));
        if !archive.is_file() {
            return Err(fail(
                "release-formula",
                "MISSING ARCHIVE",
                &format!(
                    "{} is missing; the formula needs all four macOS and Linux archives.",
                    archive.display()
                ),
                "Download every platform's archive into the folder first.",
            ));
        }
        formula = formula.replace(placeholder, &sha256_file(&archive)?);
    }
    if has_placeholder(&formula) {
        return Err(fail(
            "release-formula",
            "UNFILLED PLACEHOLDER",
            "guidedog.rb still holds a placeholder such as @NAME@.",
            "Add the placeholder's value to FORMULA_PACKAGES in tools/xtask/src/release/files.rs, \
             or remove it from tools/release/guidedog.rb.in.",
        ));
    }
    let target = dir.join("guidedog.rb");
    write(&target, &formula)?;
    println!("{}", target.display());
    Ok(())
}

/// Whether the text holds `@NAME@` with capitals, digits or underscores.
fn has_placeholder(text: &str) -> bool {
    let pieces: Vec<&str> = text.split('@').collect();
    // Each piece between two @ signs could be a placeholder's name.
    pieces.get(1..pieces.len().saturating_sub(1)).unwrap_or_default().iter().any(|name| {
        name.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_placeholders() {
        assert!(has_placeholder("sha256 \"@MACOS_ARM64@\""));
        assert!(!has_placeholder("noreply@example.com and a@b"));
    }
}
