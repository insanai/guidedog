//! `cargo xtask verify-docutils`: checks the committed conformance trees against a
//! pinned Python Docutils, run in a separate verification environment (Guidedog never
//! runs Python). It copies the corpus to WORK_DIR, regenerates every expected tree and
//! ids.txt there with [`store_references`], compares them byte for byte with the
//! committed files, and writes a record: versions, command, date, the SHA-256 of every
//! regenerated file, and the result.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::{Options, store_references};
use crate::args::{self, Spec};
use crate::fsutil::{absolute, copy_tree, create_dir, remove, sha256_file, write};
use crate::{Result, evidence, fail, repository_root};

const HELP: &str = "\
Checks the committed reStructuredText conformance trees and ids.txt against a pinned
Python Docutils: regenerates them into WORK_DIR with `cargo xtask rstref` and compares
them byte for byte with crates/guidedoc-rst/tests/data/docutils.

Usage: cargo xtask verify-docutils DOCUTILS WORK_DIR [RECORD]

  DOCUTILS  the `docutils` program of a pinned Docutils, in a separate verification
            environment (Guidedog never runs Python)
  WORK_DIR  emptied, then filled with the regenerated corpus and its logs
  RECORD    the record to write, such as the corpus's VERIFICATION.txt (default: print)

GUIDEDOG_REVISION names the revision when the tree is not a git checkout. The task fails
unless every tree and ids.txt are identical.
";

/// The corpus, relative to the repository.
const CORPUS: &str = "crates/guidedoc-rst/tests/data/docutils";

/// What `verify-docutils` runs and where it writes.
#[derive(Debug, Clone, Default)]
pub(crate) struct VerifyOptions {
    /// The `docutils` program.
    pub docutils: PathBuf,
    /// The work folder, emptied first.
    pub work: PathBuf,
    /// The record to write; `None` prints it.
    pub record: Option<PathBuf>,
}

/// What the comparison found.
#[derive(Debug, Default)]
struct Verdict {
    regenerated: Vec<String>,
    committed: usize,
    tree_failures: usize,
    ids_failures: usize,
    differing: Vec<String>,
    ids_same: bool,
}

impl Verdict {
    fn identical(&self) -> bool {
        self.tree_failures == 0
            && self.ids_failures == 0
            && self.differing.is_empty()
            && self.regenerated.len() == self.committed
            && self.ids_same
    }
}

/// Runs `cargo xtask verify-docutils`.
pub(crate) fn main(args: &[String]) -> Result<()> {
    let spec = Spec { command: "verify-docutils", flags: &[], options: &[], help: HELP };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(3, &spec)?;
    let options = VerifyOptions {
        docutils: crate::site::program_path(parsed.positional(0, "a docutils program", &spec)?),
        work: absolute(Path::new(parsed.positional(1, "a work folder", &spec)?)),
        record: parsed.positionals.get(2).map(|r| absolute(Path::new(r))),
    };
    verify_docutils(&options)
}

/// Regenerates and compares as [`VerifyOptions`] says, writes the record, and fails
/// unless everything is identical.
pub(crate) fn verify_docutils(options: &VerifyOptions) -> Result<()> {
    let root = repository_root();
    let corpus = root.join(CORPUS);
    let copy = options.work.join("corpus");
    if root.starts_with(&options.work) {
        return Err(fail(
            "verify-docutils",
            "WORK FOLDER HOLDS THE REPOSITORY",
            &format!(
                "{} is emptied first, and the repository is inside it.",
                options.work.display()
            ),
            "Name a new folder for the work, such as /tmp/dc.",
        ));
    }
    remove(&options.work)?;
    create_dir(&options.work)?;
    copy_tree(&corpus, &copy)?;
    remove(&copy.join("expected"))?;
    remove(&copy.join("ids.txt"))?;
    let mut run = Options {
        docutils: options.docutils.clone(),
        fixtures: copy.clone(),
        ..Default::default()
    };
    let trees = store_references(&run)?;
    write(&options.work.join("trees.log"), trees.failures.join("\n"))?;
    run.ids = true;
    let ids = store_references(&run)?;
    write(&options.work.join("ids.log"), ids.failures.join("\n"))?;
    let regenerated = xml_files(&copy.join("expected"));
    let differing = regenerated
        .iter()
        .filter(|tree| {
            !same_bytes(&copy.join("expected").join(tree), &corpus.join("expected").join(tree))
        })
        .map(|tree| format!("expected/{tree}"))
        .collect();
    let verdict = Verdict {
        committed: xml_files(&corpus.join("expected")).len(),
        regenerated,
        tree_failures: trees.failures.len(),
        ids_failures: ids.failures.len(),
        differing,
        ids_same: same_bytes(&copy.join("ids.txt"), &corpus.join("ids.txt")),
    };
    let record = record(options, &root, &copy, &verdict)?;
    match &options.record {
        Some(path) => write(path, record)?,
        None => print!("{record}"),
    }
    let result = if verdict.identical() { "identical" } else { "different" };
    println!(
        "docutils corpus: {} trees, {} differ, ids.txt identical: {} -> {result}",
        verdict.regenerated.len(),
        verdict.differing.len(),
        yes_no(verdict.ids_same)
    );
    if verdict.identical() {
        return Ok(());
    }
    Err(fail(
        "verify-docutils",
        "CORPUS DIFFERS",
        "The trees Docutils writes now differ from the committed ones.",
        &format!(
            "Compare the files under {} with the committed corpus; if Docutils is right, \
             store them with `cargo xtask rstref`.",
            copy.display()
        ),
    ))
}

/// The record of a run, in the shape of the corpus's VERIFICATION.txt.
fn record(options: &VerifyOptions, root: &Path, copy: &Path, verdict: &Verdict) -> Result<String> {
    let mut text = String::from(
        "# Python Docutils run over the reStructuredText conformance corpus, written by\n\
         # cargo xtask verify-docutils. Guidedog never runs Python; this is evidence only.\n",
    );
    let _ = writeln!(text, "date: {}", evidence::utc_timestamp());
    let _ = writeln!(text, "host: {}", evidence::host());
    let _ = writeln!(text, "docutils: {}", evidence::version(&options.docutils));
    let _ = writeln!(text, "guidedog revision: {}", evidence::revision(root));
    let docutils = options.docutils.display();
    let _ = writeln!(
        text,
        "command: cargo xtask verify-docutils {docutils} {}",
        options.work.display()
    );
    text.push_str(
        "rstref options: see cargo xtask rstref (--writer=xml --no-xml-declaration --no-doctype \
         --syntax-highlight=none plus each source's settings from corpus.txt)\n",
    );
    let (total, failed) = (verdict.regenerated.len(), verdict.tree_failures);
    let _ = writeln!(
        text,
        "trees regenerated: {total} ({failed} failed); committed: {}",
        verdict.committed
    );
    let _ = writeln!(text, "trees differing from committed: {}", verdict.differing.len());
    let (same, failed) = (yes_no(verdict.ids_same), verdict.ids_failures);
    let _ = writeln!(text, "ids.txt identical: {same} (rstref --ids: {failed} failed)");
    let result = if verdict.identical() { "identical" } else { "different" };
    let _ = writeln!(text, "result: {result}");
    for tree in &verdict.differing {
        let _ = writeln!(text, "  differs: {tree}");
    }
    text.push_str("sha256 of regenerated files:\n");
    let files = std::iter::once(String::from("ids.txt"))
        .chain(verdict.regenerated.iter().map(|tree| format!("expected/{tree}")));
    for file in files {
        let path = copy.join(&file);
        let sum = if path.is_file() { sha256_file(&path)? } else { String::from("missing") };
        let _ = writeln!(text, "{sum}  {file}");
    }
    Ok(text)
}

/// The .xml files below `dir`, relative with `/`, sorted.
fn xml_files(dir: &Path) -> Vec<String> {
    let mut files = Vec::new();
    collect_xml(dir, "", &mut files);
    files.sort();
    files
}

fn collect_xml(dir: &Path, prefix: &str, files: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let relative = format!("{prefix}{name}");
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            collect_xml(&entry.path(), &format!("{relative}/"), files);
        } else if name.ends_with(".xml") {
            files.push(relative);
        }
    }
}

/// Whether two files exist and hold the same bytes.
fn same_bytes(a: &Path, b: &Path) -> bool {
    matches!((std::fs::read(a), std::fs::read(b)), (Ok(a), Ok(b)) if a == b)
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
