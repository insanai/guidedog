#![allow(clippy::unwrap_used, missing_docs)]

//! Round trips: a source is read, written as reStructuredText, and read again; the
//! two trees must be equal (compared as `dump_tree` outlines, which hold no source
//! spans, with soft line breaks as spaces, since the writer wraps paragraphs anew).
//!
//! Three corpora: the reStructuredText conformance sources of guidedoc-rst (read
//! with their corpus settings), myst-parser's fixtures (read as MyST with the Sphinx
//! layer, read back with the Sphinx reader, compared after the normalization in
//! `myst_form`), and every page of docs/manual (Sphinx reader).
//!
//! `RSTW_ONLY=<substring>` limits the sources; `RSTW_DUMP=<dir>` writes the output
//! and both outlines of every source that differs.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use guidedoc::{
    Document, Emitter, Policy, ReadOptions, Reader, RenderContext, RenderOptions, Reports,
    Request, Resource, ResourceProvider, ResourceStatus, Severity, Workspace,
    default_read_options, dump_to_string, read_document, read_extension, trusted_policy,
};
use guidedoc_rst::{Settings, SmartQuotes};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Serves included files relative to the including source; `<name>` from the Docutils
/// corpus's standard include directory.
struct Files;

impl ResourceProvider for Files {
    fn provide(&self, from: &str, name: &str) -> Result<Resource, ResourceStatus> {
        let include = root().join("crates/guidedoc-rst/tests/data/docutils/include");
        let path = match name.strip_prefix('<').and_then(|n| n.strip_suffix('>')) {
            Some(standard) => include.join(standard),
            None => Path::new(from).parent().unwrap_or(Path::new("")).join(name),
        };
        let text = std::fs::read_to_string(&path).map_err(|_| ResourceStatus::Missing)?;
        Ok(Resource { text: Arc::from(text), canonical: path.to_string_lossy().into_owned() })
    }
}

/// Turns corpus settings into read options, reader settings, and a policy.
fn corpus_settings(names: &[&str]) -> (ReadOptions<'static>, Settings, Policy) {
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
            s if s.starts_with("lang=") => settings.language = s[5..].to_owned(),
            s if s.starts_with("tab=") => settings.tab_width = s[4..].parse().unwrap_or(0),
            _ => {}
        }
    }
    policy.keep_documents_with_errors = true;
    (options, settings, policy)
}

/// What one read needs besides the text.
struct ReadWith<'a> {
    reader: Reader,
    id: &'static str,
    read: ReadOptions<'a>,
    policy: Policy,
}

/// Reads `text` and returns its outline (or why it failed).
fn read(name: &str, text: &str, how: &ReadWith<'_>, ws: &mut Workspace) -> Result<String, String> {
    let mut request = Request {
        reader: how.id,
        policy: how.policy,
        resources: Some(&Files),
        read: how.read,
        ..Request::default()
    };
    request.source.name = name;
    request.source.text = text;
    let doc = read_document(ws, &how.reader, &request).map_err(|f| f.code().to_owned())?;
    Ok(dump_to_string(doc))
}

/// Reads `text`, writes it, and returns the reStructuredText and the notes count.
fn write(name: &str, text: &str, how: &ReadWith<'_>) -> Result<(String, String, usize), String> {
    let mut ws = Workspace::new();
    let mut request = Request {
        reader: how.id,
        policy: how.policy,
        resources: Some(&Files),
        read: how.read,
        ..Request::default()
    };
    request.source.name = name;
    request.source.text = text;
    let doc: Document<'_> =
        read_document(&mut ws, &how.reader, &request).map_err(|f| f.code().to_owned())?;
    let outline = dump_to_string(doc);
    let mut bytes = Vec::new();
    let options = RenderOptions::default();
    let mut reports = Reports::default();
    reports.clear(100_000);
    let mut ctx = RenderContext::new(doc, Emitter::new(&mut bytes), &options, &mut reports);
    guidedoc_rst_writer::render(&mut ctx).map_err(|f| format!("write: {}", f.code()))?;
    let notes = reports.items().iter().filter(|d| d.severity == Severity::Note).count();
    Ok((String::from_utf8(bytes).unwrap(), outline, notes))
}

/// An outline in comparable form: soft breaks become spaces in the text around them
/// (unless `soft` keeps them, to check that the author's lines survive), and internal
/// link targets are named by kind instead of node number.
fn comparable(outline: &str, myst: bool, soft: bool) -> String {
    let lines: Vec<&str> = outline.lines().collect();
    let kinds: Vec<&str> =
        lines.iter().map(|l| l.trim_start().split(' ').next().unwrap_or("")).collect();
    let mut out: Vec<String> = Vec::new();
    for line in &lines {
        let indent = line.len() - line.trim_start().len();
        let body = line.trim_start();
        let mut body =
            if body == "soft-break" && !soft { "text \" \"".to_owned() } else { body.to_owned() };
        if let Some(at) = body.find(" ->").filter(|&a| {
            body[a + 3..].starts_with(|c: char| c.is_ascii_digit()) && !body.starts_with("code")
        }) {
            let n: usize = body[at + 3..].split(' ').next().unwrap_or("").parse().unwrap_or(0);
            let kind = kinds.get(n).copied().unwrap_or("?");
            let end = body[at + 3..].find(' ').map_or(body.len(), |e| at + 3 + e);
            body.replace_range(at..end, &format!(" ->{kind}"));
        }
        if myst {
            body = myst_form(&body);
            if body.is_empty() {
                continue;
            }
        }
        let text = body.strip_prefix("text \"").and_then(|t| t.strip_suffix('"'));
        if let (Some(t), Some(prev)) = (text, out.last_mut())
            && prev.len() - prev.trim_start().len() == indent
            && prev.trim_start().starts_with("text \"")
            && !prev.contains("\" @")
            && !prev.contains("+generated")
        {
            prev.pop();
            prev.push_str(t);
            prev.push('"');
            continue;
        }
        out.push(format!("{}{body}", " ".repeat(indent)));
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

/// The MyST normalization: what the Markdown reader records that reStructuredText
/// has no source for (tightness, MyST's classes, slugs, list delimiters of Markdown).
fn myst_form(body: &str) -> String {
    let mut b = body.replace(" +tight", "");
    if b.starts_with("list list=bullet,") && b.contains(",1") {
        b = b.replacen(",1", ",0", 1);
    }
    if b.starts_with("cell") {
        b = b.replace(" +header", "");
        if let Some(at) = b.find(" @class=") {
            b.truncate(at);
        }
    }
    for drop in [" @class=\"block_break\"", " @class=\"footnotes\"", " @class=\"amsmath\""] {
        b = b.replace(drop, "");
    }
    if let Some(at) = b.find(" @cpp:name=") {
        let end = b[at + 12..].find('"').map_or(b.len(), |e| at + 13 + e);
        b.replace_range(at..end, "");
    }
    for drop in [" @class=\"simple myst\"", " @class=\"myst\"", " @class=\"colwidths-auto\""] {
        b = b.replace(drop, "");
    }
    for attr in ["slug", "widths", "class"] {
        while let Some(at) = b.find(&format!(" @{attr}=\"")) {
            let open = at + attr.len() + 4;
            let end = b[open..].find('"').map_or(b.len(), |e| open + e + 1);
            if attr == "class" && !b[at..end].contains("text-") {
                break;
            }
            b.replace_range(at..end, "");
        }
    }
    if b == "comment" || b == "document" || b == "document value=1" {
        return if b == "comment" { String::new() } else { "document".into() };
    }
    b
}

/// The result of a corpus.
#[derive(Default)]
struct Tally {
    total: usize,
    passed: usize,
    /// Round trips that also keep every soft line break where the source had it.
    same_lines: usize,
    notes: usize,
    failures: Vec<String>,
}

impl Tally {
    fn check(
        &mut self,
        name: &str,
        text: &str,
        first: &ReadWith<'_>,
        second: &ReadWith<'_>,
        myst: bool,
    ) {
        self.total += 1;
        let (rst, outline, notes) = match write(name, text, first) {
            Ok(w) => w,
            Err(e) => {
                self.failures.push(format!("{name}: {e}"));
                return;
            }
        };
        self.notes += notes;
        let mut ws = Workspace::new();
        let again =
            read(name, &rst, second, &mut ws).unwrap_or_else(|e| format!("read failed: {e}"));
        let (a, b) = (comparable(&outline, myst, false), comparable(&again, myst, false));
        if a == b {
            self.passed += 1;
            let (a, b) = (comparable(&outline, myst, true), comparable(&again, myst, true));
            if a == b {
                self.same_lines += 1;
            } else if std::env::var("RSTW_LINES").is_ok() {
                // Soft breaks inside markup and table cells are expected to move.
                eprintln!("LINES {name}: {}", first_difference(&a, &b));
            }
            return;
        }
        self.failures.push(format!("{name}: {}", first_difference(&a, &b)));
        if let Ok(dir) = std::env::var("RSTW_DUMP") {
            let base = Path::new(&dir).join(name.replace(['/', '.'], "_"));
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(base.with_extension("rst"), &rst);
            let _ = std::fs::write(base.with_extension("a"), &a);
            let _ = std::fs::write(base.with_extension("b"), &b);
        }
    }

    fn report(&self, part: &str) {
        eprintln!(
            "{part}: {}/{} round-trip, {} with the same lines ({} notes)",
            self.passed, self.total, self.same_lines, self.notes
        );
        for f in &self.failures {
            eprintln!("  {f}");
        }
    }
}

fn first_difference(a: &str, b: &str) -> String {
    let (x, y): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let i = x.iter().zip(&y).take_while(|(p, q)| p == q).count();
    let get = |v: &Vec<&str>| v.get(i).map_or("<end>".to_owned(), |l| l.trim().to_owned());
    format!("line {}: {} | {}", i + 1, get(&x), get(&y))
}

fn only() -> String {
    std::env::var("RSTW_ONLY").unwrap_or_default()
}

#[test]
fn rst_conformance_sources_round_trip() {
    let corpus = root().join("crates/guidedoc-rst/tests/data/docutils");
    let manifest = std::fs::read_to_string(corpus.join("corpus.txt")).unwrap();
    let mut tally = Tally::default();
    for line in manifest.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let Some((&source, names)) = fields.split_first().filter(|(s, _)| !s.starts_with('#'))
        else {
            continue;
        };
        if !source.contains(&only()) {
            continue;
        }
        let (options, settings, policy) = corpus_settings(names);
        let path = corpus.join(source);
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let how = ReadWith {
            reader: guidedoc_rst::reader(),
            id: "rst",
            read: ReadOptions { extension: Some(read_extension(&settings)), ..options },
            policy,
        };
        tally.check(&path.to_string_lossy(), &text, &how, &how, false);
    }
    tally.report("reStructuredText corpus");
    // Raise this when the writer improves; the README lists what still differs.
    assert!(only().is_empty() || tally.passed > 0);
    assert!(!only().is_empty() || tally.passed >= 77, "fewer round trips than before");
    assert!(!only().is_empty() || tally.same_lines >= 65, "fewer kept lines than before");
}

#[test]
fn manual_pages_round_trip() {
    let mut pages = Vec::new();
    collect_pages(&root().join("docs/manual"), &mut pages);
    pages.sort();
    let config = guidedoc_sphinx::Config::default();
    let mut policy = trusted_policy();
    policy.keep_documents_with_errors = true;
    let how = ReadWith {
        reader: guidedoc_sphinx::reader(),
        id: "sphinx",
        read: ReadOptions { extension: Some(read_extension(&config)), ..default_read_options() },
        policy,
    };
    let mut tally = Tally::default();
    for page in pages.iter().filter(|p| p.to_string_lossy().contains(&only())) {
        let text = std::fs::read_to_string(page).unwrap();
        tally.check(&page.to_string_lossy(), &text, &how, &how, false);
    }
    tally.report("docs/manual");
    // Raise this when the writer improves; the README lists what still differs.
    assert!(only().is_empty() || tally.passed > 0);
    assert!(!only().is_empty() || tally.passed >= 69, "fewer round trips than before");
    assert!(!only().is_empty() || tally.same_lines >= 56, "fewer kept lines than before");
}

fn collect_pages(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('_') || name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            collect_pages(&p, out);
        } else if name.ends_with(".rst") {
            out.push(p);
        }
    }
}

/// Splits a markdown-it fixture file into its inputs.
fn fixture_inputs(text: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = Vec::new();
    let mut i = 1;
    while i < lines.len() {
        if lines[i] != "." {
            i += 1;
            continue;
        }
        let a = i + 1;
        let b = a + lines[a..].iter().take_while(|l| **l != ".").count();
        let e = (b
            + 1
            + lines.get(b + 1..).map_or(0, |r| r.iter().take_while(|l| **l != ".").count()))
        .min(lines.len());
        let title = lines[i - 1].to_owned();
        if !title.starts_with("SKIP") {
            out.push((title, format!("{}\n", lines[a..b.min(lines.len())].join("\n"))));
        }
        i = e + 1;
    }
    out
}

#[test]
fn myst_fixtures_round_trip() {
    use guidedoc_commonmark::{Settings as Myst, SyntaxExtension};
    let data = root().join("crates/guidedoc-commonmark/tests/data/myst");
    let manifest = std::fs::read_to_string(data.join("fixtures.txt")).unwrap();
    let mut config = guidedoc_sphinx::Config::default();
    config.settings.smart_quotes = SmartQuotes::Off;
    let mut policy = trusted_policy();
    policy.keep_documents_with_errors = true;
    let second = ReadWith {
        reader: guidedoc_sphinx::reader(),
        id: "sphinx",
        read: ReadOptions { extension: Some(read_extension(&config)), ..default_read_options() },
        policy,
    };
    let mut tally = Tally::default();
    for line in manifest.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 3 || fields[0].starts_with('#') || fields[1] == "warnings" {
            continue;
        }
        let mut myst = Myst::default();
        if fields[2] != "-" {
            for name in fields[2].split(',') {
                if let Some(e) = SyntaxExtension::named(name) {
                    myst.enable_extensions.insert(e);
                }
            }
        }
        let myst = guidedoc_sphinx::myst_settings(config.clone(), myst);
        let first = ReadWith {
            reader: guidedoc_commonmark::MYST_READER,
            id: "myst",
            read: ReadOptions { extension: Some(read_extension(&myst)), ..default_read_options() },
            policy,
        };
        let text = std::fs::read_to_string(data.join("fixtures").join(fields[0])).unwrap();
        for (title, input) in fixture_inputs(&text) {
            let name = format!("{}: {title}", fields[0]);
            if name.contains(&only()) {
                tally.check(&name, &input, &first, &second, true);
            }
        }
    }
    tally.report("MyST fixtures");
    // Raise this when the writer improves; the README lists what still differs.
    assert!(only().is_empty() || tally.passed > 0);
    assert!(!only().is_empty() || tally.passed >= 185, "fewer round trips than before");
    assert!(!only().is_empty() || tally.same_lines >= 185, "fewer kept lines than before");
}
