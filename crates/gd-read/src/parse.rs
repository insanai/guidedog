//! Reading one document: its source (once), whether the previous build's reading still
//! holds, and otherwise the parse, the object, the symbols and the dependencies.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use gd_conf::Digest;
use gd_env::{DocInfo, Environment, SourceFile, SourceKind};
use gd_host::{Ledger, Problem, Session};
use gd_report::{Report, Severity};
use guidedoc::{
    Builder, Document, Pass, PassOutcome, ReadOptions, Reader, Request, SourceId, SourceUse,
    default_request, read_extension,
};
use guidedoc_commonmark::Settings as MystSettings;
use guidedoc_sphinx::Autodoc;

use crate::autodoc::{PythonHost, tag_autodoc_reports};
use crate::context::ReadContext;
use crate::odin::OdinHost;
use crate::translate::{image_exists, translation};

/// What reading every document shares, made once per build: the readers, their base
/// settings, and the folders includes may come from.
pub(crate) struct Prepared {
    rst: Option<Reader>,
    markdown: Option<Reader>,
    sphinx: guidedoc_sphinx::Config,
    myst: MystSettings,
    roots: Vec<PathBuf>,
    digests: FileDigests,
}

/// The digests of the files documents depend on, each computed once per build: every
/// page of a crate's API depends on all of the crate's files, and hashing them again
/// for each page was most of the time spent checking for changes.
#[derive(Debug)]
struct FileDigests(Mutex<HashMap<PathBuf, Option<Digest>>>);

impl FileDigests {
    /// Knows already the digests the generated pages of `sources` were made with: they
    /// were taken in this build, as the pages were made or their record checked.
    fn with_generated(sources: &[SourceFile]) -> FileDigests {
        let generated = sources.iter().filter_map(|f| f.generated.as_ref());
        let deps = generated.flat_map(|g| &g.deps);
        let map = deps.map(|dep| (dep.path.clone(), Some(dep.digest))).collect();
        FileDigests(Mutex::new(map))
    }

    /// The digest of the bytes of the file at `path` (`gd_env::file_digest`), computed
    /// at its first use in this build; `None` when it cannot be read.
    fn digest(&self, path: &Path) -> Option<Digest> {
        let known = self.0.lock().unwrap_or_else(PoisonError::into_inner).get(path).copied();
        if let Some(digest) = known {
            return digest;
        }
        // Hashed without the lock, so threads hash different files at once; two that
        // hash the same file find the same digest.
        let digest = gd_env::file_digest(path);
        let mut map = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        map.insert(path.to_path_buf(), digest);
        digest
    }

    /// Whether a file `doc` depended on changed, as `gd_env::dependencies_changed`
    /// decides it: a file that cannot be read counts as changed.
    fn changed(&self, doc: &DocInfo) -> bool {
        doc.deps.iter().any(|dep| self.digest(&dep.path) != Some(dep.digest))
    }
}

/// The reader ids a project reads its documents with: reStructuredText with the Sphinx
/// layer, Markdown as MyST.
const RST_READER: &str = "sphinx";
const MARKDOWN_READER: &str = "myst";

impl Prepared {
    /// Prepares reading `sources`. Fails with `build.reader` when the registry lacks the
    /// reader of a kind of document the project has: plain Docutils reStructuredText has
    /// none of Sphinx's directives, and its pages would be silently wrong.
    pub(crate) fn new(ctx: &ReadContext<'_>, sources: &[SourceFile]) -> Result<Prepared, Problem> {
        let find = |kind: SourceKind| -> Result<Option<Reader>, Problem> {
            let Some(source) = sources.iter().find(|s| s.kind == kind) else { return Ok(None) };
            let id = match kind {
                SourceKind::Rst => RST_READER,
                SourceKind::Markdown => MARKDOWN_READER,
            };
            ctx.registry.find_reader(id).copied().map(Some).ok_or_else(|| no_reader(id, source))
        };
        let c = ctx.config;
        Ok(Prepared {
            rst: find(SourceKind::Rst)?,
            markdown: find(SourceKind::Markdown)?,
            sphinx: crate::config::sphinx_config(c, &ctx.intl.labels, ctx.today),
            myst: crate::config::myst_settings(c, ctx.today),
            roots: gd_conf::readable_roots(c),
            digests: FileDigests::with_generated(sources),
        })
    }

    fn reader(&self, kind: SourceKind) -> Option<&Reader> {
        match kind {
            SourceKind::Rst => self.rst.as_ref(),
            SourceKind::Markdown => self.markdown.as_ref(),
        }
    }
}

fn no_reader(id: &str, source: &SourceFile) -> Problem {
    let what = match source.kind {
        SourceKind::Rst => "Sphinx's reStructuredText",
        SourceKind::Markdown => "Markdown",
    };
    Problem::new(
        "build.reader",
        "NO READER FOR THE DOCUMENTS",
        &source.relative,
        format!("The format registry has no \"{id}\" reader, which reads {what}."),
        "Build with a registry that has it, such as guidedoc_defaults::registry().",
        2,
    )
}

/// What became of one source: the previous build's reading still holds (its index in
/// the previous environment), or the document was read now.
#[derive(Debug)]
pub(crate) enum Slot {
    Reused(usize),
    Read(Box<DocInfo>),
}

/// A reading thread: the context, what every read shares, and the thread's own
/// workspace, reused from one document to the next.
pub(crate) struct Worker<'a> {
    ctx: &'a ReadContext<'a>,
    prepared: &'a Prepared,
    session: Session,
}

impl<'a> Worker<'a> {
    pub(crate) fn new(ctx: &'a ReadContext<'a>, prepared: &'a Prepared) -> Worker<'a> {
        Worker { ctx, prepared, session: Session::new() }
    }

    /// Reads `f`'s source once: keeps the previous reading when it still holds,
    /// otherwise parses the text read.
    pub(crate) fn read(&mut self, f: &SourceFile, previous: &Environment) -> Slot {
        let mut d = DocInfo {
            docname: f.docname.clone(),
            relative: f.relative.clone(),
            kind: f.kind,
            generated: f.generated.is_some(),
            ..DocInfo::default()
        };
        if gd_host::was_interrupted() {
            return Slot::Read(Box::new(failed(d, gd_host::interrupted_problem().report())));
        }
        let ledger = Ledger::new(self.ctx.budget);
        let text = match source_text(f, &ledger) {
            Ok((text, digest)) => {
                d.digest = digest;
                text
            }
            Err(p) => return Slot::Read(Box::new(failed(d, p.report()))),
        };
        if let Some(i) = previous.id_of(&f.docname)
            && reusable(self.ctx, self.prepared, &previous.docs[i.index()], &d)
        {
            return Slot::Reused(i.index());
        }
        self.parse(&mut d, f, &text, &ledger);
        Slot::Read(Box::new(d))
    }

    /// Parses `text`, `f`'s source, into `d`, after the build plugins' source hooks. The
    /// files it includes are charged to `ledger`.
    pub(crate) fn parse(
        &mut self,
        d: &mut DocInfo,
        f: &SourceFile,
        text: &str,
        ledger: &Ledger<'_>,
    ) {
        let processed = match self.ctx.plugins.process_source(f, &self.ctx.config.language, text) {
            Ok(processed) => processed,
            Err(p) => {
                d.failed = true;
                d.reports = vec![p.report()];
                return;
            }
        };
        self.parse_source(d, f, &processed, ledger);
        if let Cow::Owned(_) = processed {
            mark_expanded(&mut d.reports, f);
        }
    }

    fn parse_source(&mut self, d: &mut DocInfo, f: &SourceFile, text: &str, ledger: &Ledger<'_>) {
        let (ctx, prepared) = (self.ctx, self.prepared);
        let Some(reader) = prepared.reader(f.kind) else {
            // Prepared::new found a reader for every kind of the sources it was given.
            d.failed = true;
            d.reports = vec![no_reader(RST_READER, f).report()];
            return;
        };
        let c = ctx.config;
        let python = PythonHost::new(c).map(Arc::new);
        let odin = Arc::new(OdinHost::new(c));
        let mut sphinx = prepared.sphinx.clone();
        sphinx.docname.clone_from(&f.docname);
        if let Some(python) = &python {
            sphinx.autodoc.push(Autodoc { domain: "py".into(), provider: python.clone() });
        }
        sphinx.autodoc.push(Autodoc { domain: "odin".into(), provider: odin.clone() });
        let myst;
        let extension = match f.kind {
            SourceKind::Rst => read_extension(&sphinx),
            SourceKind::Markdown => {
                myst = guidedoc_sphinx::myst_settings(sphinx.clone(), prepared.myst.clone());
                read_extension(&myst)
            }
        };
        let read = ReadOptions {
            // As Sphinx, whose document titles stay sections.
            promote_title: false,
            today: ctx.date,
            extension: Some(extension),
            ..ReadOptions::default()
        };
        let files =
            gd_host::Files::new(&prepared.roots).with_ledger(ledger).with_project(&c.source_dir);
        let source_name = ctx.intl.source_name(&f.docname);
        let exists = |docname: &str, path: &str| image_exists(&c.source_dir, docname, path);
        let pass = translation(ctx, &f.docname, reader, read, &source_name, &exists)
            .map(gd_i18n::TranslationPass::new);
        let run = |doc: Document<'_>, b: &mut Builder<'_>| match &pass {
            Some(pass) => pass.run(doc, b),
            None => PassOutcome::Unchanged,
        };
        let passes = [Pass { name: gd_i18n::TRANSLATION_PASS, run: &run }];
        let name = f.path.display().to_string();
        let request = Request {
            read,
            policy: ctx.policy,
            limits: ctx.limits,
            passes: if pass.is_some() { &passes } else { &[] },
            resources: Some(&files),
            ..default_request(&name, text, reader.id, "")
        };
        self.run_read(d, reader, &request, text);
        d.deps.extend_from_slice(ctx.intl.files_of(&f.docname));
        if let Some(python) = &python {
            python.dependencies(&mut d.deps);
        }
        odin.dependencies(&mut d.deps);
        if let Some(generated) = &f.generated {
            d.deps.extend_from_slice(&generated.deps);
        }
    }

    /// Reads `request` and, when it succeeds, writes the object and the search words,
    /// collects the symbols, and notes the files included.
    fn run_read(&mut self, d: &mut DocInfo, reader: &Reader, request: &Request<'_>, text: &str) {
        let read = self.session.read(reader, request).map(|_| ()).map_err(|fail| fail.status);
        let mut reports = self.session.reports();
        tag_autodoc_reports(&mut reports);
        let document = self.session.document().filter(|_| read.is_ok());
        let Some(doc) = document else {
            if !reports.iter().any(|r| r.severity == Severity::Error) {
                // A read that failed must say so, even when the reader reported nothing.
                reports.push(not_read(&d.relative, read.err()));
            }
            d.reports = reports;
            d.failed = true;
            return;
        };
        content_warnings(&mut reports);
        let store = self.ctx.store;
        match store.save_object(&d.docname, doc) {
            Ok(object) => d.object = Some(object),
            Err(p) => reports.push(p.report()),
        }
        let words = crate::collect::collect(d, doc);
        if !words.is_empty() {
            match store.save_search(&d.docname, &words) {
                Ok(search) => d.search = Some(search),
                Err(p) => reports.push(p.report()),
            }
        }
        for todo in &mut d.todos {
            let offset = (todo.line as usize).min(text.len());
            todo.line = u32::try_from(
                1 + text.as_bytes()[..offset].iter().filter(|&&b| b == b'\n').count(),
            )
            .unwrap_or(u32::MAX);
        }
        d.failed = d.object.is_none();
        d.written = !d.failed;
        d.reports = reports;
        self.note_included(d);
    }

    /// The files the document included or quoted, each a dependency; generated sources
    /// (`<autodoc ...>`) are not files.
    fn note_included(&self, d: &mut DocInfo) {
        let ws = self.session.workspace();
        for (i, entry) in ws.sources().iter().enumerate().skip(1) {
            if entry.name.is_empty() || entry.name.starts_with('<') {
                continue;
            }
            let path = Path::new(&entry.name);
            // The digest `gd_env::dependencies_changed` compares: the file's bytes.
            let id = SourceId(u16::try_from(i).unwrap_or(u16::MAX));
            let digest = self
                .prepared
                .digests
                .digest(path)
                .unwrap_or_else(|| Digest::of_parts(&[ws.source_str(id).as_bytes()]));
            let included = entry.usage == SourceUse::Parsed;
            d.deps.push(gd_env::Dependency { path: path.to_path_buf(), digest, included });
        }
    }
}

/// A source's text and the digest of what it was made from: a file's bytes (read within
/// the budget, UTF-8 checked, without a byte order mark), or a generated page's text.
fn source_text<'f>(
    f: &'f SourceFile,
    ledger: &Ledger<'_>,
) -> Result<(Cow<'f, str>, Digest), Problem> {
    if let Some(generated) = &f.generated {
        let digest = Digest::of_parts(&[generated.text.as_bytes()]);
        return Ok((Cow::Borrowed(&generated.text), digest));
    }
    let bytes = gd_host::read_file(&f.path, Some(ledger), gd_host::MAX_SOURCE_BYTES)?;
    let digest = Digest::of_parts(&[&bytes]);
    let mut text = String::from_utf8(bytes).map_err(|e| utf8_problem(f, e.utf8_error()))?;
    if text.starts_with('\u{feff}') {
        text.drain(..'\u{feff}'.len_utf8());
    }
    Ok((Cow::Owned(text), digest))
}

fn utf8_problem(f: &SourceFile, e: std::str::Utf8Error) -> Problem {
    Problem::new(
        "host.utf8",
        "SOURCE IS NOT UTF-8",
        &f.relative,
        format!("Byte {} of the document is not valid UTF-8.", e.valid_up_to()),
        "Save the file as UTF-8; most editors offer it under \"Save as\" or \"Encoding\".",
        1,
    )
}

/// Whether the previous build's reading of a document still holds: the same file and
/// markup, the same source digest, read without failing, no dependency changed, and its
/// object still in the doctrees folder.
fn reusable(ctx: &ReadContext<'_>, prepared: &Prepared, old: &DocInfo, now: &DocInfo) -> bool {
    if old.digest != now.digest
        || old.failed
        || old.kind != now.kind
        || old.relative != now.relative
    {
        return false;
    }
    if old.generated != now.generated || prepared.digests.changed(old) {
        return false;
    }
    old.object.as_ref().is_some_and(|object| ctx.store.path_of(object).is_file())
}

/// Makes the errors a reader found in a document it read anyway warnings, as Sphinx
/// logs Docutils' ERROR and SEVERE messages: the build succeeds with them, and `-W`
/// fails it. Only a document that could not be read is an error of the build.
pub(crate) fn content_warnings(reports: &mut [Report]) {
    for report in reports.iter_mut().filter(|r| r.severity == Severity::Error) {
        report.severity = Severity::Warning;
    }
}

/// `d` failed with `report`.
fn failed(mut d: DocInfo, report: Report) -> DocInfo {
    d.failed = true;
    d.reports = vec![report];
    d
}

fn not_read(relative: &str, status: Option<guidedoc::Status>) -> Report {
    let status = status.map_or_else(|| "no document".to_owned(), |s| format!("{s:?}"));
    let mut report = Report::error(
        "build.read",
        "DOCUMENT NOT READ",
        &format!("Reading stopped with status {status}."),
    )
    .hint("Report this with the document to the Guidedog developers.");
    report.path = relative.to_owned();
    report
}

/// Says, on each report about the document itself, that its location is in the text the
/// build plugins produced.
fn mark_expanded(reports: &mut [Report], f: &SourceFile) {
    let path = f.path.display().to_string();
    // Session reports name the source as display_path shows it: relative to the working
    // directory, which is the project's folder only when the build runs from there.
    let shown = gd_host::display_path(&path);
    let about_document = |r: &Report| r.path == path || r.path == shown || r.path == f.relative;
    for report in reports.iter_mut().filter(|r| about_document(r)) {
        report.path = format!("{path} (plugin-expanded input)");
        report
            .message
            .push_str(" This location is in the text produced by the selected build plugins.");
        report.hints.push(
            "Correct the source or its plugin configuration to change the expanded text.".into(),
        );
    }
}
