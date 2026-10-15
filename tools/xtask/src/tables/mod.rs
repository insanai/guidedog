//! `cargo xtask tables`: regenerates the committed Rust data tables from the upstream
//! files in `tools/data` (see its README.md), or checks that they match.

mod entities;
mod pack;
mod python;
mod styles;
mod unicode;

use std::path::{Path, PathBuf};

use crate::args::{self, Spec};
use crate::fsutil::{read_text, sha256_file, write};
use crate::{Result, fail, repository_root};

const HELP: &str = "\
Regenerates the Rust data tables from the upstream data in tools/data.

Usage: cargo xtask tables [--check]

  --check   write nothing; fail when a committed table differs from what the data
            gives (CI runs this)

Tables:
  crates/guidedoc/src/tables/*.rs        Unicode case folding, identifier folding,
                                         decomposition (NFD), general categories
  crates/guidedoc-rst-inline/src/punctuation_table.rs
                                         Unicode punctuation and white space
  crates/guidedoc-commonmark/src/inline/entity_table.rs
                                         HTML5 named character references
  crates/gd-highlight/src/style_tables.rs
                                         Pygments' and Sphinx's highlighting styles
";

/// The longest file a table may be, the source-limit checker's.
const MAX_LINES: usize = 1408;

/// One generated file: its path from the repository root, and its contents.
#[derive(Debug)]
pub(crate) struct Table {
    path: String,
    text: String,
}

impl Table {
    fn new(path: String, text: String) -> Table {
        Table { path, text }
    }
}

/// Runs `cargo xtask tables`.
pub(crate) fn main(args: &[String]) -> Result<()> {
    let spec = Spec { command: "tables", flags: &["--check"], options: &[], help: HELP };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(0, &spec)?;
    let root = repository_root();
    let data = root.join("tools/data");
    verify_data(&data)?;
    let mut tables = unicode::generate(&data)?;
    tables.extend(entities::generate(&data)?);
    tables.extend(styles::generate(&root)?);
    if let Some(long) = tables.iter().find(|t| t.text.lines().count() > MAX_LINES) {
        return Err(fail(
            "tables-long",
            "TABLE TOO LONG",
            &format!("{} would have more than {MAX_LINES} lines.", long.path),
            "Split the table into two files in the generator (tools/xtask/src/tables).",
        ));
    }
    let mut stale: Vec<&Table> = Vec::new();
    for table in &tables {
        let path: PathBuf = root.join(&table.path);
        let current = read_text(&path).unwrap_or_default();
        if current != table.text {
            stale.push(table);
        }
    }
    if parsed.has("--check") {
        return report_check(&tables, &stale);
    }
    for table in &stale {
        write(&root.join(&table.path), &table.text)?;
        println!("wrote {}", table.path);
    }
    println!("{} tables, {} rewritten.", tables.len(), stale.len());
    Ok(())
}

fn report_check(tables: &[Table], stale: &[&Table]) -> Result<()> {
    if stale.is_empty() {
        println!("{} tables match tools/data.", tables.len());
        return Ok(());
    }
    let names: Vec<&str> = stale.iter().map(|t| t.path.as_str()).collect();
    Err(fail(
        "tables-stale",
        "STALE TABLES",
        &format!(
            "These tables differ from what tools/data gives:\n\n    {}",
            names.join("\n    ")
        ),
        "Run `cargo xtask tables` and commit the result. Tables are generated: change the \
         data or the generator in tools/xtask/src/tables, never the table by hand.",
    ))
}

/// Checks every file `tools/data/SHA256SUMS` lists against its sum, so the tables are
/// made from the bytes the README records.
fn verify_data(data: &Path) -> Result<()> {
    let sums = read_text(&data.join("SHA256SUMS"))?;
    for line in sums.lines().filter(|line| !line.trim().is_empty()) {
        let Some((expected, name)) = line.split_once("  ") else {
            return Err(fail(
                "tables-sums",
                "UNREADABLE SUMS",
                &format!("tools/data/SHA256SUMS has the line `{line}`."),
                "Write each line as a SHA-256, two spaces and a path, as sha256sum does.",
            ));
        };
        let got = sha256_file(&data.join(name))?;
        if got != expected {
            return Err(fail(
                "tables-sums",
                "CHANGED DATA",
                &format!(
                    "tools/data/{name} has SHA-256 {got}, but SHA256SUMS records {expected}."
                ),
                "Restore the file from its source (tools/data/README.md names it), or, for a \
                 new upstream version, record its sum in SHA256SUMS.",
            ));
        }
    }
    Ok(())
}
