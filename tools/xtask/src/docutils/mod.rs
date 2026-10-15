//! `cargo xtask rstref`: stores the reference document trees of the reStructuredText
//! conformance corpus (crates/guidedoc-rst/tests/data/docutils). For each source listed
//! in corpus.txt it runs a Docutils command line program (the XML writer) in the
//! source's folder, with the options the listed settings name, and writes the output to
//! expected/SOURCE.xml. The trees are committed; tests never run the program.
//!
//! With `--ids` it writes each source as HTML instead (the html5 writer, with Sphinx's
//! `auto_id_prefix = "id"`, as a Sphinx build reads documents) and stores every id of
//! each page in ids.txt: the fragments links can reach. A page that shows system
//! messages has a second line, `SOURCE (system messages): ...`, with the ids of the
//! messages, which Guidedoc does not write.
//!
//! `cargo xtask verify-docutils` ([`verify`]) regenerates everything into a work folder
//! and compares it byte for byte with the committed files.

mod verify;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::args::{self, Spec};
use crate::fsutil::{absolute, create_dir, read_text, write};
use crate::{Result, fail, process};

pub(crate) use verify::main as verify_main;

const HELP: &str = "\
Stores the Docutils reference trees of the reStructuredText conformance corpus: runs a
Docutils command line program (the XML writer) for each source corpus.txt lists, with the
options its settings name, and writes FIXTURES_DIR/expected/SOURCE.xml.

Usage: cargo xtask rstref [--ids] DOCUTILS FIXTURES_DIR [SOURCE...]

  DOCUTILS      the `docutils` program of a pinned Docutils, in a separate verification
                environment (Guidedog never runs Python)
  FIXTURES_DIR  the corpus: crates/guidedoc-rst/tests/data/docutils
  SOURCE        only these sources, as corpus.txt names them (default: all)
  --ids         write FIXTURES_DIR/ids.txt instead: every id of each source's HTML page
                (html5 writer, auto_id_prefix = \"id\") and the ids of its system messages

Regenerate only after changing corpus.txt or a source; tests never run the program.
";

/// The header of ids.txt.
const IDS_HEADER: &str = "\
# Every id of each corpus source's HTML page from Docutils 0.22's html5 writer, with Sphinx's
# auto_id_prefix = \"id\", and the ids of the page's system messages; written by
# cargo xtask rstref --ids.
";

/// What `rstref` stores.
#[derive(Debug, Clone, Default)]
pub(crate) struct Options {
    /// The `docutils` program.
    pub docutils: PathBuf,
    /// The corpus folder, with corpus.txt.
    pub fixtures: PathBuf,
    /// Whether to write ids.txt instead of the XML trees.
    pub ids: bool,
    /// Only these sources; empty means all.
    pub only: Vec<String>,
}

/// What a run stored, and the sources that failed with why.
#[derive(Debug, Default)]
pub(crate) struct Outcome {
    /// How many sources were stored.
    pub written: usize,
    /// One message per source that failed.
    pub failures: Vec<String>,
}

/// Runs `cargo xtask rstref`.
pub(crate) fn main(args: &[String]) -> Result<()> {
    let spec = Spec { command: "rstref", flags: &["--ids"], options: &[], help: HELP };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    let options = Options {
        docutils: crate::site::program_path(parsed.positional(0, "a docutils program", &spec)?),
        fixtures: absolute(Path::new(parsed.positional(1, "the corpus folder", &spec)?)),
        ids: parsed.has("--ids"),
        only: parsed.positionals.iter().skip(2).cloned().collect(),
    };
    let outcome = store_references(&options)?;
    for failure in &outcome.failures {
        eprintln!("{failure}");
    }
    println!("{} trees written, {} failed", outcome.written, outcome.failures.len());
    if outcome.failures.is_empty() {
        return Ok(());
    }
    Err(fail(
        "rstref",
        "REFERENCES NOT STORED",
        &format!("{} of the corpus's sources could not be stored.", outcome.failures.len()),
        "Read the messages above: each names the source and what Docutils said.",
    ))
}

/// Stores the trees (or ids.txt) as [`Options`] says. A source that fails is listed in
/// the [`Outcome`] and the others are still stored; ids.txt is written only when every
/// source succeeded. Fails when corpus.txt cannot be read or a file cannot be written.
pub(crate) fn store_references(options: &Options) -> Result<Outcome> {
    let manifest = read_text(&options.fixtures.join("corpus.txt"))?;
    let mut outcome = Outcome::default();
    let mut ids = String::from(IDS_HEADER);
    for (source, settings) in corpus_entries(&manifest) {
        if !options.only.is_empty() && !options.only.iter().any(|s| s == source) {
            continue;
        }
        match store_source(options, source, &settings, &mut ids) {
            Ok(()) => outcome.written += 1,
            Err(message) => outcome.failures.push(message),
        }
    }
    if options.ids && outcome.failures.is_empty() {
        write(&options.fixtures.join("ids.txt"), ids)?;
    }
    Ok(outcome)
}

/// The sources of corpus.txt with their settings, comment and blank lines left out.
fn corpus_entries(manifest: &str) -> impl Iterator<Item = (&str, Vec<&str>)> {
    manifest.lines().filter_map(|line| {
        let mut fields = line.split_whitespace();
        let source = fields.next().filter(|first| !first.starts_with('#'))?;
        Some((source, fields.collect()))
    })
}

/// The program's options for a source's settings; fails naming a setting it does not
/// know, so a typing mistake in corpus.txt cannot store a tree made with other options.
fn docutils_options(settings: &[&str], ids: bool) -> std::result::Result<Vec<String>, String> {
    let mut out: Vec<String> = if ids {
        vec!["--writer=html5".into(), "--auto-id-prefix=id".into()]
    } else {
        vec!["--writer=xml".into(), "--no-xml-declaration".into(), "--no-doctype".into()]
    };
    out.push("--syntax-highlight=none".into());
    for &setting in settings {
        let option = match setting {
            "sectsubtitle" => "--section-subtitles".to_string(),
            "smartquotes" => "--smart-quotes=yes".into(),
            "smartquotes-alt" => "--smart-quotes=alt".into(),
            "pep" => "--pep-references".into(),
            "rfc" => "--rfc-references".into(),
            "trim" => "--trim-footnote-reference-space".into(),
            "nodocinfo" => "--no-doc-info".into(),
            "notitle" => "--no-doc-title".into(),
            "charlevel" => "--character-level-inline-markup".into(),
            "nofile" => "--no-file-insertion".into(),
            "noraw" => "--no-raw".into(),
            // Read by the conformance test only: it counts the system messages.
            "messages=total" => continue,
            _ => match setting.split_once('=') {
                Some(("lang", tag)) => format!("--language={tag}"),
                Some(("tab", width)) => format!("--tab-width={width}"),
                _ => return Err(format!("unknown setting `{setting}`")),
            },
        };
        out.push(option);
    }
    Ok(out)
}

/// Runs the program for one source in its folder and stores the output: the XML tree,
/// or the source's lines of ids.txt appended to `ids`. Returns a message on failure.
fn store_source(
    options: &Options,
    source: &str,
    settings: &[&str],
    ids: &mut String,
) -> std::result::Result<(), String> {
    let path = options.fixtures.join(source);
    let (Some(dir), Some(file)) = (path.parent(), path.file_name()) else {
        return Err(format!("{source}: not a file path"));
    };
    let arguments =
        docutils_options(settings, options.ids).map_err(|e| format!("{source}: {e}"))?;
    let mut command = Command::new(&options.docutils);
    command.current_dir(dir).args(&arguments).arg(file).stdin(Stdio::null());
    let out = command
        .output()
        .map_err(|err| format!("{source}: cannot run {}: {err}", process::describe(&command)))?;
    if !out.status.success() {
        let errors = String::from_utf8_lossy(&out.stderr);
        return Err(format!("{source}: failed ({}): {}", out.status, errors.trim_end()));
    }
    if options.ids {
        let (all, messages) = document_ids(&String::from_utf8_lossy(&out.stdout));
        ids.push_str(&format!("{source}:{all}\n"));
        if !messages.is_empty() {
            ids.push_str(&format!("{source} (system messages):{messages}\n"));
        }
        return Ok(());
    }
    let target = options.fixtures.join("expected").join(format!("{source}.xml"));
    let stored =
        target.parent().map_or(Ok(()), create_dir).and_then(|()| write(&target, &out.stdout));
    stored.map_err(|_| format!("{source}: cannot write {}", target.display()))
}

/// Every id of an HTML page, and the ids of its system messages (elements of class
/// `system-message`), each after a space, sorted.
fn document_ids(html: &str) -> (String, String) {
    let mut found = Vec::new();
    let mut system = Vec::new();
    let mut at = 0;
    while let Some(i) = html[at..].find(" id=\"") {
        let start = at + i + 5;
        let Some(length) = html[start..].find('"') else { break };
        let end = start + length;
        let open = html[..start].rfind('<').unwrap_or(0);
        let close = html[end..].find('>').map_or(html.len(), |c| end + c);
        let message = html[open..close].contains("class=\"system-message\"");
        for id in html[start..end].split_whitespace() {
            found.push(id);
            if message {
                system.push(id);
            }
        }
        at = end;
    }
    (joined(found), joined(system))
}

fn joined(mut ids: Vec<&str>) -> String {
    ids.sort_unstable();
    ids.iter().map(|id| format!(" {id}")).collect()
}
