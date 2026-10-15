//! A small project for gd-site's end-to-end tests. The reading layer (gd-read) is not
//! used: each document is parsed with the Sphinx layer's readers (reStructuredText,
//! MyST), its object saved to the store, and its `DocInfo` filled by hand (title,
//! sections, toctrees, index entries, modules, search words); gd-resolve then makes the
//! tables, and `Project::build` runs `SiteBuilder` through a real `Generation`.

#![allow(dead_code, clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use gd_conf::{Config, Digest};
use gd_env::{
    BuildOptions, BuilderKind, DocInfo, EnglishUi, Environment, ObjectStore, PluginRun,
    SectionInfo, SourceKind, TocEntry, ToctreeInfo,
};
use gd_host::{Budget, Problem};
use gd_outputs::{CommitRecords, Generation, GenerationOptions, output_paths};
use gd_report::{Report, Severity};
use gd_resolve::{BuildView, Dates, Intersphinx, OutputBuilder, Tables};
use gd_site::SiteBuilder;
use guidedoc::{Limits, Policy, Registry, Renderer, Request, Source, Workspace, read_extension};

/// A temporary folder removed on drop.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("gd-site-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir.canonicalize().unwrap())
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A project and everything a `BuildView` borrows.
pub struct Project {
    pub dir: TempDir,
    pub config: Config,
    pub options: BuildOptions,
    pub env: Environment,
    pub tables: Tables,
    pub intersphinx: Intersphinx,
    pub objects: ObjectStore,
    pub registry: Registry,
    pub policy: Policy,
    pub limits: Limits,
    pub dates: Dates,
    pub plugins: PluginRun,
    pub budget: Budget,
    pub roots: Vec<PathBuf>,
    /// What resolving reported.
    pub reports: Vec<Report>,
}

impl Project {
    /// An empty project named Demo in a new folder, rendering with `renderer`.
    pub fn with_renderer(name: &str, renderer: Renderer) -> Project {
        let dir = TempDir::new(name);
        let mut config = Config::new(dir.0.clone(), dir.0.join("_build"));
        config.project = "Demo".into();
        config.html.title = "Demo documentation".into();
        config.html.short_title = "Demo".into();
        let objects = ObjectStore::new(gd_env::doctrees_dir(&config));
        Project {
            roots: gd_host::canonical_roots(&gd_conf::readable_roots(&config)),
            dir,
            config,
            options: BuildOptions::default(),
            env: Environment::default(),
            tables: Tables::default(),
            intersphinx: Intersphinx::default(),
            objects,
            registry: Registry { readers: Vec::new(), renderers: vec![renderer] },
            policy: Policy::default(),
            limits: Limits::default(),
            dates: Dates::default(),
            plugins: PluginRun::default(),
            budget: Budget::new(1 << 30),
            reports: Vec::new(),
        }
    }

    /// An empty project rendering with guidedoc-html.
    pub fn new(name: &str) -> Project {
        Project::with_renderer(name, guidedoc_html::RENDERER)
    }

    /// Writes a file below the source folder.
    pub fn write(&self, relative: &str, text: &str) {
        let path = self.dir.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
    }

    /// The path of a file below the source folder.
    pub fn path(&self, relative: &str) -> PathBuf {
        self.dir.0.join(relative)
    }

    /// Writes a document (`.rst` or `.md`), parses it, saves its object, and records it
    /// with `title`; the caller fills in the rest of its `DocInfo`.
    pub fn add(&mut self, relative: &str, title: &str, text: &str) -> &mut DocInfo {
        self.write(relative, text);
        let (docname, kind) = match relative.strip_suffix(".md") {
            Some(stem) => (stem, SourceKind::Markdown),
            None => (relative.strip_suffix(".rst").unwrap(), SourceKind::Rst),
        };
        let object = self.save_object(docname, relative, kind, text);
        self.env.docs.retain(|d| d.docname != docname);
        self.env.docs.push(DocInfo {
            docname: docname.to_string(),
            relative: relative.to_string(),
            kind,
            digest: Digest::of_text(text),
            object: Some(object),
            title: title.to_string(),
            ..DocInfo::default()
        });
        self.env.docs.last_mut().unwrap()
    }

    fn save_object(
        &self,
        docname: &str,
        relative: &str,
        kind: SourceKind,
        text: &str,
    ) -> gd_env::CachedFile {
        let mut ws = Workspace::new();
        let sphinx = guidedoc_sphinx::Config { docname: docname.into(), ..Default::default() };
        let mut request =
            Request { source: Source { name: relative, text }, ..Request::default() };
        let doc = if kind == SourceKind::Markdown {
            let settings = guidedoc_sphinx::myst_settings(sphinx, Default::default());
            request.read.extension = Some(read_extension(&settings));
            guidedoc::read_document(&mut ws, &guidedoc_commonmark::MYST_READER, &request)
        } else {
            request.read.extension = Some(read_extension(&sphinx));
            guidedoc::read_document(&mut ws, &guidedoc_sphinx::READER, &request)
        };
        self.objects.save_object(docname, doc.unwrap()).unwrap()
    }

    /// The document named `docname`.
    pub fn doc(&mut self, docname: &str) -> &mut DocInfo {
        self.env.docs.iter_mut().find(|d| d.docname == docname).unwrap()
    }

    /// Keeps a document's search words: lines of `anchor\ttitle\ttext`.
    pub fn words(&mut self, docname: &str, words: &str) {
        let file = self.objects.save_search(docname, words).unwrap();
        self.doc(docname).search = Some(file);
    }

    /// Sorts the documents and makes the tables.
    pub fn resolve(&mut self) {
        self.env.docs.sort_by(|a, b| a.docname.cmp(&b.docname));
        self.reports.clear();
        self.tables = gd_resolve::resolve(&self.config, &self.env, &EnglishUi, &mut self.reports);
    }

    /// What a builder reads.
    pub fn view(&self, kind: BuilderKind) -> BuildView<'_> {
        BuildView {
            config: &self.config,
            options: &self.options,
            kind,
            env: &self.env,
            tables: &self.tables,
            intersphinx: &self.intersphinx,
            objects: &self.objects,
            registry: &self.registry,
            policy: &self.policy,
            limits: &self.limits,
            ui: &EnglishUi,
            dates: &self.dates,
            plugins: &self.plugins,
            budget: &self.budget,
            roots: &self.roots,
            settings: Digest::of_text(kind.name()),
        }
    }

    /// Builds the site with `kind` and publishes it; the build's reports, or its problem.
    pub fn build(&self, kind: BuilderKind) -> Result<Vec<Report>, Problem> {
        let paths = output_paths(&self.config, kind, None)?;
        let mut reports = Vec::new();
        let options = GenerationOptions { all: false, budget: &self.budget };
        let mut generation = Generation::begin(paths, options, &mut reports)?;
        let builder = SiteBuilder { kind, ..SiteBuilder::default() };
        builder.write(&self.view(kind), &mut generation, &mut reports)?;
        let outcome =
            generation.commit(CommitRecords { environment: &self.env, store: &self.objects });
        if let Some(problem) = outcome.problem {
            return Err(problem);
        }
        reports.extend(outcome.reports);
        Ok(reports)
    }

    /// The output folder of a builder.
    pub fn out(&self, kind: BuilderKind) -> PathBuf {
        self.dir.0.join("_build").join(kind.name())
    }

    /// A file of a builder's output, as text.
    pub fn read(&self, kind: BuilderKind, relative: &str) -> String {
        let path = self.out(kind).join(relative);
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }
}

/// A section.
pub fn section(level: u32, title: &str, anchor: &str, parent: Option<u32>) -> SectionInfo {
    SectionInfo {
        level,
        title: title.into(),
        anchor: anchor.into(),
        parent,
        ids: vec![anchor.into()],
        ..SectionInfo::default()
    }
}

/// A toctree in the section `section`, after `position` sections, listing `entries`.
pub fn toctree(section: Option<u32>, position: u32, entries: &[&str]) -> ToctreeInfo {
    ToctreeInfo {
        section,
        position,
        entries: entries
            .iter()
            .map(|e| TocEntry { target: (*e).into(), title: String::new() })
            .collect(),
        ..ToctreeInfo::default()
    }
}

/// The warnings and errors among `reports`.
pub fn problems(reports: &[Report]) -> Vec<&Report> {
    reports.iter().filter(|r| r.severity != Severity::Note).collect()
}

/// Whether `path` is a file.
pub fn is_file(path: &Path) -> bool {
    path.is_file()
}
