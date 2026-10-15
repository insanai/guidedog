//! Differential conformance of the "myst" reader with the Sphinx layer's directives and
//! roles against myst-parser 0.16.1 (a copy of guidedoc-commonmark's harness whose
//! `extension()` is the Sphinx layer's, so every difference must be documented).
//!
//! `tests/data/myst/fixtures` holds myst-parser's own test fixtures: Markdown and the
//! Docutils tree myst-parser builds for it, in pseudo-XML; `tests/data/myst/sphinx`
//! holds its Sphinx build tests. Each case is read with Guidedoc, both trees are brought
//! into one structural form (`norm.rs`), and the cases that differ are reported.
//! docs/manual/internals/commonmark-conformance.rst explains the rules of the common
//! form and every documented difference (`DIFFERENCES`).
//!
//! Most fixtures use directives and roles, which the Sphinx layer's `Extension`
//! provides. This crate cannot depend on the Sphinx layer (it depends on this crate), so
//! `extension()` is the seam: without handlers the harness reads every case, prints
//! how many match, and checks only that each case reads; with the Sphinx layer's
//! extension (its own copy of this harness) every difference must be documented.
//!
//! `MYST_ONLY=<file.md>` (or `<directory>/<document>`) checks one fixture;
//! `MYST_DUMP=<dir>` writes both normalized trees of every case there, for diffing.

mod guidedoc_tree;
mod norm;
mod pseudo_xml;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use guidedoc::{
    Document, Request, Resource, ResourceProvider, ResourceStatus, Severity, Workspace,
    default_request, read_document, read_extension, trusted_policy,
};
use guidedoc_commonmark::{Extension, MYST_READER, Meta, Settings, Substitution, SyntaxExtension};
use norm::{Counts, Norm, first_difference, norm_string, strip_code_space};

/// The cases whose trees differ from myst-parser's by design.
const DIFFERENCES: [(&str, &str); 5] = [
    // In Sphinx, `class` is the Python domain's class directive (hence rst-class); the
    // fixture reads it with plain Docutils.
    ("docutil_directives.md", "class (`docutils.parsers.rst.directives.misc.Class`):"),
    // Docutils reports a Markdown link to an unknown target when reading; with Sphinx
    // it is a cross-reference, which the project host reports when it cannot resolve it.
    ("reporter_warnings.md", "Missing Reference:"),
    // The fixture's Sphinx added "()" to :js:class:, as Sphinx 9.1 no longer does.
    ("sphinx_roles.md", "js:class (`sphinx.domains.javascript.JSConstructor`):"),
    // The wordcount is 52 words where myst-parser counts 53.
    ("sphinx", "basic/content"),
    // A substitution whose value is a directive, used inline, stays inline text.
    ("sphinx", "substitutions/index"),
];

/// The directive and role handlers the cases are read with: the Sphinx layer's, for the
/// document `docname`. The fixtures show myst-parser's trees before Docutils' and
/// Sphinx's transforms, so they are read without smart quotes and docinfo.
fn extension(docname: &str, fixture: bool) -> Option<Arc<dyn Extension>> {
    let mut config = guidedoc_sphinx::Config { docname: docname.to_owned(), ..Default::default() };
    if fixture {
        config.settings.smart_quotes = guidedoc_rst::SmartQuotes::Off;
        config.settings.docinfo = false;
    }
    guidedoc_sphinx::myst_settings(config, Settings::default()).extension
}

fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/myst")
}

fn documented(file: &str, title: &str) -> bool {
    DIFFERENCES.iter().any(|(f, t)| *f == file && *t == title)
}

/// One case of a fixture file.
struct Case {
    title: String,
    input: String,
    expected: String,
    line: usize,
}

/// Splits a markdown-it fixture file: a title line, then the input and the expected
/// output, each ended by a line ".". Cases myst-parser skips are left out.
fn cases(text: &str) -> Vec<Case> {
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
            out.push(Case {
                input: format!("{}\n", lines[a..b.min(lines.len())].join("\n")),
                expected: lines[(b + 1).min(lines.len())..e].join("\n"),
                title,
                line: i,
            });
        }
        i = e + 1;
    }
    out
}

/// Serves the files that myst-parser's include fixtures write, or the files of a
/// Sphinx build's source directory.
struct Provider {
    missing_other: bool,
    root: Option<PathBuf>,
}

impl ResourceProvider for Provider {
    fn provide(&self, from: &str, name: &str) -> Result<Resource, ResourceStatus> {
        let resource = |text: &str, canonical: &str| Resource {
            text: Arc::from(text),
            canonical: canonical.to_owned(),
        };
        if let Some(root) = &self.root {
            let path = match name.strip_prefix('/') {
                Some(abs) => root.join(abs),
                None => Path::new(from).parent().unwrap_or(Path::new("")).join(name),
            };
            let text = std::fs::read_to_string(&path).map_err(|_| ResourceStatus::Missing)?;
            return Ok(resource(&text, &path.to_string_lossy()));
        }
        match name {
            "other.md" if !self.missing_other => Ok(resource("a\nb\nc", "other.md")),
            "fmatter.md" => Ok(resource("---\na: 1\n---\nb", "fmatter.md")),
            "bad.md" => Ok(resource("{a}`b`", "bad.md")),
            _ => Err(ResourceStatus::Missing),
        }
    }
}

/// Reads `text` as a project reads Markdown and returns the common form and counts.
fn read(
    name: &str,
    text: &str,
    settings: &Settings,
    provider: &Provider,
) -> (Option<Norm>, Counts) {
    let mut ws = Workspace::new();
    let mut request: Request<'_> = default_request(name, text, "myst", "html");
    request.policy = trusted_policy();
    request.policy.keep_documents_with_errors = true;
    request.read.promote_title = false;
    request.read.extension = Some(read_extension(settings));
    request.resources = Some(provider);
    let doc = read_document(&mut ws, &MYST_READER, &request).ok();
    let norm = doc.map(|d: Document<'_>| guidedoc_tree::from_guidedoc(d));
    let mut counts = Counts::default();
    for d in ws.reports().items() {
        match d.severity {
            Severity::Warning => counts.warnings += 1,
            Severity::Error => counts.errors += 1,
            Severity::Note => {}
        }
    }
    (norm, counts)
}

/// Compares two trees; "" when they match.
fn compare(name: &str, line: usize, want: &mut Norm, got: &mut Norm) -> String {
    let (w, g) = (norm_string(want), norm_string(got));
    if let Ok(dir) = std::env::var("MYST_DUMP") {
        let base = Path::new(&dir).join(format!("{}-{line:04}", name.replace('/', "_")));
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(base.with_extension("myst"), &w);
        let _ = std::fs::write(base.with_extension("guidedoc"), &g);
    }
    if w == g { String::new() } else { first_difference(&w, &g) }
}

/// The result of one part of the harness.
#[derive(Debug, Default)]
struct Tally {
    total: usize,
    matched: usize,
    documented: usize,
    undocumented: Vec<String>,
}

impl Tally {
    fn add(&mut self, file: &str, title: &str, detail: String) {
        self.total += 1;
        if detail.is_empty() {
            self.matched += 1;
        } else if documented(file, title) {
            self.documented += 1;
        } else {
            self.undocumented.push(format!("{file} {title:?} differs:\n{detail}"));
        }
    }

    /// Prints the result; with handlers, every difference must be documented.
    fn finish(&self, part: &str) {
        eprintln!(
            "{part}: {} cases, {} match myst-parser 0.16.1, {} documented differences, {} other",
            self.total,
            self.matched,
            self.documented,
            self.undocumented.len()
        );
        if std::env::var("MYST_ONLY").is_ok() {
            eprintln!("{}", self.undocumented.join("\n"));
        }
        if extension("", false).is_some() {
            assert!(self.undocumented.is_empty(), "{}", self.undocumented.join("\n"));
        }
    }
}

fn fixture_case(file: &str, how: &str, settings: &Settings, case: &Case) -> String {
    let provider = Provider { missing_other: file == "mock_include_errors.md", root: None };
    let (got, counts) = read("index.md", &case.input, settings, &provider);
    if how == "warnings" {
        let mut want = Counts::default();
        for line in case.expected.lines() {
            if line.contains("(WARNING/2)") {
                want.warnings += 1;
            } else if line.contains("(ERROR/3)") || line.contains("(SEVERE/4)") {
                want.errors += 1;
            }
        }
        return if want == counts { String::new() } else { format!("{want:?} != {counts:?}") };
    }
    let (mut want, want_counts) = pseudo_xml::pseudo_xml_norm(&case.expected);
    let Some(mut got) = got else { return "Guidedoc read failed".into() };
    let mut detail = compare(file, case.line, &mut want, &mut got);
    if want_counts.errors > 0 && counts.errors == 0 {
        detail.push_str(&format!(
            "myst-parser reports {} errors, Guidedog none\n",
            want_counts.errors
        ));
    }
    detail
}

#[test]
fn myst_conformance() {
    let only = std::env::var("MYST_ONLY").unwrap_or_default();
    let manifest = std::fs::read_to_string(data().join("fixtures.txt")).expect("manifest");
    let mut tally = Tally::default();
    for line in manifest.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 3
            || fields[0].starts_with('#')
            || (!only.is_empty() && fields[0] != only)
        {
            continue;
        }
        let mut settings =
            Settings { extension: extension("mock_docname", true), ..Settings::default() };
        if fields[2] != "-" {
            for name in fields[2].split(',') {
                if let Some(e) = SyntaxExtension::named(name) {
                    settings.enable_extensions.insert(e);
                }
            }
        }
        let path = data().join("fixtures").join(fields[0]);
        let text = std::fs::read_to_string(path).expect("fixture");
        for case in cases(&text) {
            let detail = fixture_case(fields[0], fields[1], &settings, &case);
            tally.add(fields[0], &case.title, detail);
        }
    }
    tally.finish("myst conformance");
}

/// Turns a manifest entry into reader settings.
fn build_settings(entry: &[&str]) -> Settings {
    let mut settings = Settings::default();
    settings.docname = Path::new(entry[0])
        .file_name()
        .map_or(String::new(), |n| n.to_string_lossy().into_owned());
    settings.extension = extension(&settings.docname, false);
    for s in &entry[1..] {
        let (key, value) = s.split_once('=').unwrap_or((s, ""));
        let value = value.replace("%20", " ");
        match key {
            "commonmark_only" => settings.commonmark_only = true,
            "heading_anchors" => settings.heading_anchors = value.parse().unwrap_or(0),
            "slug_func" => settings.heading_slug_func = value,
            "dmath_allow_space" => settings.dmath_allow_space = value == "true",
            "dmath_double_inline" => settings.dmath_double_inline = value == "true",
            "disable" => settings.disable_syntax.push(value),
            "enable" => {
                settings.enable_extensions = Default::default();
                for e in value.split(',').filter_map(SyntaxExtension::named) {
                    settings.enable_extensions.insert(e);
                }
            }
            "sub" | "meta" => {
                let (k, v) = value.split_once('|').unwrap_or((&value, ""));
                if key == "sub" {
                    settings.substitutions.push(Substitution::new(k, v));
                } else {
                    settings.html_meta.push(Meta { key: k.into(), content: v.into() });
                }
            }
            _ => {}
        }
    }
    settings
}

fn build_case(entry: &[&str]) -> String {
    let base = data().join("sphinx").join(entry[0]);
    let Ok(xml) = std::fs::read_to_string(base.with_extension("xml")) else {
        return "no expected tree".into();
    };
    let path = base.with_extension("md");
    let Ok(text) = std::fs::read_to_string(&path) else { return "no source".into() };
    let settings = build_settings(entry);
    let root = path.parent().map(Path::to_path_buf);
    let provider = Provider { missing_other: false, root };
    let name = path.to_string_lossy().into_owned();
    let (got, _) = read(&name, &text, &settings, &provider);
    let Some(mut got) = got else { return "Guidedog read failed".into() };
    let (mut want, _) = pseudo_xml::pseudo_xml_norm(&xml);
    strip_code_space(&mut want);
    strip_code_space(&mut got);
    compare(entry[0], 0, &mut want, &mut got)
}

#[test]
fn myst_conformance_sphinx() {
    let only = std::env::var("MYST_ONLY").unwrap_or_default();
    let manifest = std::fs::read_to_string(data().join("sphinx.txt")).expect("manifest");
    let mut tally = Tally::default();
    for line in manifest.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 2
            || fields[0].starts_with('#')
            || (!only.is_empty() && fields[0] != only)
        {
            continue;
        }
        tally.add("sphinx", fields[0], build_case(&fields));
    }
    tally.finish("myst sphinx builds");
}
