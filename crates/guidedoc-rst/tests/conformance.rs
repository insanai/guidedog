#![allow(clippy::unwrap_used)]

//! Differential conformance of the reader against Docutils 0.22.
//!
//! `tests/data/docutils/corpus.txt` lists the corpus: Docutils' functional test inputs
//! and the reStructuredText specification documents, each with the settings it is read
//! with. `expected/` holds Docutils' document tree of every source (Docutils XML). This
//! test reads each source, brings both trees into one structural form
//! (`conformance/norm.rs`), and reports the first difference per file and differences
//! in the number of warnings and errors. docs/manual/internals/rst-conformance.rst
//! explains every rule of the common form that hides a deliberate difference.
//!
//! Environment: `RST_ONLY=<source>` checks one source; `RST_DUMP=<dir>` writes both
//! normalized trees of every source there, for diffing.

#[path = "conformance/docutils.rs"]
mod docutils;
#[path = "conformance/guidedoc.rs"]
mod guidedoc_form;
#[path = "conformance/norm.rs"]
mod norm;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use guidedoc::{
    Policy, ReadOptions, Request, Resource, ResourceProvider, ResourceStatus, Severity,
};
use guidedoc::{Workspace, default_read_options, read_document, read_extension, trusted_policy};
use guidedoc_rst::{Settings, SmartQuotes, reader};

use norm::{Counts, norm_string};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/docutils")
}

/// Turns corpus settings into read options, reader settings, and a policy.
fn settings_for(names: &[&str]) -> (ReadOptions<'static>, Settings, Policy) {
    let mut options = default_read_options();
    options.today = "2026-09-29";
    let mut settings = Settings::default();
    let mut policy = trusted_policy();
    for &s in names {
        match s {
            "sectsubtitle" => settings.sectsubtitle = true,
            "smartquotes" => settings.smart_quotes = SmartQuotes::On,
            "smartquotes-alt" => settings.smart_quotes = SmartQuotes::Alt,
            "pep" => settings.pep_references = true,
            "rfc" => settings.rfc_references = true,
            "trim" => settings.trim_footnote_reference_space = true,
            "nodocinfo" => settings.docinfo = false,
            "notitle" => options.promote_title = false,
            "charlevel" => settings.character_level_inline_markup = true,
            "nofile" => policy.includes = false,
            "noraw" => policy.raw = guidedoc::ContentPolicy::Refuse,
            s if s.starts_with("lang=") => settings.language = s[5..].to_owned(),
            s if s.starts_with("tab=") => settings.tab_width = s[4..].parse().unwrap_or(0),
            _ => {}
        }
    }
    (options, settings, policy)
}

/// Resolves include names against the including source's directory; "<name>" names a
/// standard definition file in the corpus's include directory.
struct CorpusFiles;

impl ResourceProvider for CorpusFiles {
    fn provide(&self, from: &str, name: &str) -> Result<Resource, ResourceStatus> {
        let path = match name.strip_prefix('<').and_then(|n| n.strip_suffix('>')) {
            Some(standard) => corpus().join("include").join(standard),
            None => Path::new(from).parent().unwrap_or(Path::new("")).join(name),
        };
        let text = std::fs::read_to_string(&path).map_err(|_| ResourceStatus::Missing)?;
        Ok(Resource { text: Arc::from(text), canonical: path.to_string_lossy().into_owned() })
    }
}

/// Reads a corpus source: its common form, its counts, and the codes it reported.
fn guidedoc_norm(source: &str, names: &[&str]) -> Result<(String, Counts, String), String> {
    let path = corpus().join(source);
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let (options, settings, mut policy) = settings_for(names);
    // Keep a document with errors, as Docutils writes its page anyway.
    policy.keep_documents_with_errors = true;
    let name = path.to_string_lossy().into_owned();
    let mut request =
        Request { reader: "rst", policy, resources: Some(&CorpusFiles), ..Request::default() };
    request.source.name = &name;
    request.source.text = &text;
    request.read = ReadOptions { extension: Some(read_extension(&settings)), ..options };
    let mut ws = Workspace::new();
    let doc = read_document(&mut ws, &reader(), &request)
        .map_err(|f| format!("{:?}: {}", f.status, f.code()))?;
    let mut tree = guidedoc_form::from_guidedoc(doc);
    let mut counts = Counts::default();
    let mut codes = String::new();
    for d in ws.reports().items() {
        if d.severity != Severity::Note {
            codes.push_str(&format!("{}@{} ", d.code, d.span.start));
        }
        match d.severity {
            Severity::Warning => counts.warnings += 1,
            Severity::Error => counts.errors += 1,
            Severity::Note => {}
        }
    }
    Ok((norm_string(&mut tree), counts, codes))
}

/// The first differing line of two normalized trees, with the lines around it.
fn first_difference(want: &str, got: &str) -> String {
    let w: Vec<&str> = want.lines().collect();
    let g: Vec<&str> = got.lines().collect();
    let i = w.iter().zip(&g).take_while(|(a, b)| a == b).count();
    let mut out = format!("first difference at normalized line {}\n  common context:\n", i + 1);
    for line in &w[i.saturating_sub(6)..i] {
        out.push_str(&format!("    {line}\n"));
    }
    out.push_str("  Docutils:\n");
    for line in w.iter().skip(i).take(8) {
        out.push_str(&format!("  - {line}\n"));
    }
    out.push_str("  Guidedoc:\n");
    for line in g.iter().skip(i).take(8) {
        out.push_str(&format!("  + {line}\n"));
    }
    out
}

/// Compares one source; "" when it matches Docutils.
fn check_source(source: &str, names: &[&str]) -> String {
    let expected = corpus().join("expected").join(format!("{source}.xml"));
    let Ok(xml) = std::fs::read_to_string(expected) else {
        return "no expected tree".into();
    };
    let (mut want_tree, want_counts) = docutils::docutils_norm(&xml);
    let want = norm_string(&mut want_tree);
    let (got, got_counts, codes) = match guidedoc_norm(source, names) {
        Ok(r) => r,
        Err(e) => return format!("Guidedoc read failed: {e}"),
    };
    if let Ok(dir) = std::env::var("RST_DUMP") {
        let flat = source.replace('/', "_");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(Path::new(&dir).join(format!("{flat}.docutils")), &want);
        let _ = std::fs::write(Path::new(&dir).join(format!("{flat}.guidedoc")), &got);
    }
    let mut detail = String::new();
    if want != got {
        detail = first_difference(&want, &got);
    }
    let same = if names.contains(&"messages=total") {
        // A documented difference of severity: compare the totals.
        want_counts.warnings + want_counts.errors == got_counts.warnings + got_counts.errors
    } else {
        want_counts == got_counts
    };
    if !same {
        detail.push_str(&format!(
            "messages: Docutils {} warnings, {} errors; Guidedoc {}, {}\n  {}{codes}\n",
            want_counts.warnings,
            want_counts.errors,
            got_counts.warnings,
            got_counts.errors,
            "Guidedoc reports: ",
        ));
    }
    detail
}

#[test]
fn rst_conformance() {
    let manifest = std::fs::read_to_string(corpus().join("corpus.txt")).unwrap();
    let only = std::env::var("RST_ONLY").ok();
    let (mut total, mut failures) = (0, Vec::new());
    for line in manifest.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let Some((&source, names)) = fields.split_first().filter(|(s, _)| !s.starts_with('#'))
        else {
            continue;
        };
        if only.as_deref().is_some_and(|o| o != source) {
            continue;
        }
        total += 1;
        let detail = check_source(source, names);
        if !detail.is_empty() {
            failures.push(format!("{source} differs from Docutils:\n{detail}"));
        }
    }
    println!("rst conformance: {total} sources, {} match Docutils", total - failures.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
