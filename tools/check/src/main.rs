//! Source-limit checker.
//!
//! Every implementation file (Rust, CSS, JavaScript, HTML and Typst templates, shell,
//! TOML) keeps lines within 99 characters (soft) and 108 (hard) and stays within 1,408
//! lines. Rust functions keep at most 70 logical lines. Documentation (anything under a
//! `docs` directory) and third-party data (`data` directories) are exempt.
//!
//! Usage: `cargo run -p check [PATH...]`. Without paths it checks `crates`, `cmd`, `tools`
//! and `examples`. `--soft` also fails on soft-width violations.

mod rust;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const SOFT_WIDTH: usize = 99;
const HARD_WIDTH: usize = 108;
const MAX_FILE_LINES: usize = 1408;
const MAX_FN_LINES: usize = 70;

const SOURCE_EXTENSIONS: &[&str] = &["rs", "css", "js", "html", "typ", "sh", "toml", "c", "h"];
const SKIPPED_DIRECTORIES: &[&str] = &["data", "target", "build", "node_modules", "work", "out"];

#[derive(Debug, Default)]
struct Tally {
    files: usize,
    hard: usize,
    soft: usize,
}

fn main() -> ExitCode {
    let mut strict_soft = false;
    let mut roots: Vec<PathBuf> = Vec::new();
    for arg in std::env::args().skip(1) {
        if arg == "--soft" {
            strict_soft = true;
        } else {
            roots.push(PathBuf::from(arg));
        }
    }
    if roots.is_empty() {
        roots = ["crates", "cmd", "tools", "examples"]
            .iter()
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .collect();
    }
    let mut tally = Tally::default();
    for root in &roots {
        if let Err(err) = visit(root, &mut tally) {
            eprintln!("-- CANNOT READ ----------------------------------------------------------");
            eprintln!("\nI could not read {}: {err}\n", root.display());
            eprintln!("Hint: run the checker from the repository root, or pass existing paths.");
            return ExitCode::FAILURE;
        }
    }
    println!(
        "Checked {} source files: {} limit violations, {} soft-width warnings.",
        tally.files, tally.hard, tally.soft
    );
    let failed = tally.hard > 0 || (strict_soft && tally.soft > 0);
    if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

fn visit(path: &Path, tally: &mut Tally) -> std::io::Result<()> {
    if path.is_file() {
        if is_source(path) {
            check_file(path, tally)?;
        }
        return Ok(());
    }
    let mut entries: Vec<_> = std::fs::read_dir(path)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let child = entry.path();
        if child.is_dir() {
            let skipped = name.starts_with('.') || SKIPPED_DIRECTORIES.contains(&&*name);
            if !skipped {
                visit(&child, tally)?;
            }
        } else if is_source(&child) {
            check_file(&child, tally)?;
        }
    }
    Ok(())
}

/// Tells whether the limits apply: code by extension, unless it is prose under `docs`.
fn is_source(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else { return false };
    if !SOURCE_EXTENSIONS.contains(&ext) {
        return false;
    }
    let docs = path.components().any(|c| c.as_os_str() == "docs");
    !(ext == "typ" && docs)
}

fn check_file(path: &Path, tally: &mut Tally) -> std::io::Result<()> {
    tally.files += 1;
    let source = std::fs::read_to_string(path)?;
    let lines: Vec<&str> = source.lines().collect();
    if lines.len() > MAX_FILE_LINES {
        eprintln!("{}: file has {} lines (limit {MAX_FILE_LINES})", path.display(), lines.len());
        tally.hard += 1;
    }
    for (i, line) in lines.iter().enumerate() {
        let width = line.chars().count();
        if width > HARD_WIDTH {
            eprintln!("{}:{}: hard width limit ({width} characters)", path.display(), i + 1);
            tally.hard += 1;
        } else if width > SOFT_WIDTH {
            eprintln!("{}:{}: soft width limit ({width} characters)", path.display(), i + 1);
            tally.soft += 1;
        }
    }
    if path.extension().is_some_and(|e| e == "rs") {
        for f in rust::functions(&source) {
            if f.logic_lines > MAX_FN_LINES {
                eprintln!(
                    "{}:{}: function `{}` has {} logic lines (limit {MAX_FN_LINES})",
                    path.display(),
                    f.line,
                    f.name,
                    f.logic_lines
                );
                tally.hard += 1;
            }
        }
    }
    Ok(())
}
