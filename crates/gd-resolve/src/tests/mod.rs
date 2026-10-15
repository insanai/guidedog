//! Tests with hand-built environments: no reader is needed, as resolution reads only
//! what reading recorded (`gd_env::DocInfo`).

mod index;
mod intersphinx;
mod rust;
mod tables;
mod xref;

use std::path::PathBuf;

use gd_conf::Config;
use gd_env::{
    BuildOptions, BuilderKind, DocId, DocInfo, EnglishUi, Environment, LabelInfo, LabelKind,
    ObjectInfo, ObjectStore, PluginRun, SectionInfo, TocEntry, ToctreeInfo, Trace,
};
use gd_host::Budget;
use gd_report::Report;
use guidedoc::{Limits, Policy, Registry};

use crate::{BuildView, Dates, Intersphinx, Resolved, Tables, Unresolved, XrefQuery};

/// A temporary folder, removed when dropped.
pub(crate) struct TempDir(pub(crate) PathBuf);

impl TempDir {
    pub(crate) fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("gd-resolve-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    pub(crate) fn put(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A document titled `title`, with one section of that title.
pub(crate) fn doc(docname: &str, title: &str) -> DocInfo {
    DocInfo {
        docname: docname.into(),
        relative: format!("{docname}.rst"),
        title: title.into(),
        sections: vec![section(title, &slug(title), None)],
        ..DocInfo::default()
    }
}

pub(crate) fn slug(title: &str) -> String {
    title.to_lowercase().replace(' ', "-")
}

pub(crate) fn section(title: &str, anchor: &str, parent: Option<u32>) -> SectionInfo {
    SectionInfo {
        level: if parent.is_some() { 2 } else { 1 },
        title: title.into(),
        anchor: anchor.into(),
        parent,
        ids: vec![anchor.into()],
        ..SectionInfo::default()
    }
}

pub(crate) fn toctree(entries: &[&str]) -> ToctreeInfo {
    ToctreeInfo {
        position: 1,
        entries: entries
            .iter()
            .map(|e| TocEntry { target: (*e).into(), title: String::new() })
            .collect(),
        ..ToctreeInfo::default()
    }
}

pub(crate) fn label(name: &str, anchor: &str, title: &str, kind: LabelKind) -> LabelInfo {
    LabelInfo { name: name.into(), anchor: anchor.into(), title: title.into(), kind }
}

pub(crate) fn object(domain: &str, objtype: &str, fullname: &str) -> ObjectInfo {
    let anchor = format!("{domain}-{}", fullname.replace([':', '.'], "-"));
    ObjectInfo {
        domain: domain.into(),
        objtype: objtype.into(),
        fullname: fullname.into(),
        anchor,
        ..ObjectInfo::default()
    }
}

/// Everything a `BuildView` borrows, resolved from `docs`.
pub(crate) struct Fixture {
    pub(crate) config: Config,
    pub(crate) options: BuildOptions,
    pub(crate) kind: BuilderKind,
    pub(crate) env: Environment,
    pub(crate) tables: Tables,
    pub(crate) intersphinx: Intersphinx,
    pub(crate) reports: Vec<Report>,
    objects: ObjectStore,
    registry: Registry,
    policy: Policy,
    limits: Limits,
    dates: Dates,
    plugins: PluginRun,
    budget: Budget,
    roots: Vec<PathBuf>,
}

impl Fixture {
    pub(crate) fn new(docs: Vec<DocInfo>) -> Fixture {
        Fixture::with_config(Config::new("/project", "/project/_build"), docs)
    }

    pub(crate) fn with_config(config: Config, mut docs: Vec<DocInfo>) -> Fixture {
        docs.sort_by(|a, b| a.docname.cmp(&b.docname));
        let env = Environment { docs, ..Environment::default() };
        let mut reports = Vec::new();
        let tables = crate::resolve(&config, &env, &EnglishUi, &mut reports);
        let roots = gd_host::canonical_roots(&gd_conf::readable_roots(&config));
        Fixture {
            options: BuildOptions::default(),
            kind: BuilderKind::Html,
            env,
            tables,
            intersphinx: Intersphinx::default(),
            reports,
            objects: ObjectStore::new(config.build_dir.join(".doctrees")),
            registry: Registry::default(),
            policy: Policy::default(),
            limits: Limits::default(),
            dates: Dates::default(),
            plugins: PluginRun::default(),
            budget: Budget::new(1 << 20),
            roots,
            config,
        }
    }

    /// Resolves the tables again, as a new build would with changed documents.
    pub(crate) fn rebuild(&mut self, change: impl FnOnce(&mut Vec<DocInfo>)) {
        change(&mut self.env.docs);
        self.env.docs.sort_by(|a, b| a.docname.cmp(&b.docname));
        self.reports.clear();
        self.tables = crate::resolve(&self.config, &self.env, &EnglishUi, &mut self.reports);
    }

    pub(crate) fn view(&self) -> BuildView<'_> {
        BuildView {
            config: &self.config,
            options: &self.options,
            kind: self.kind,
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
            settings: gd_env::Digest::default(),
        }
    }

    pub(crate) fn id(&self, docname: &str) -> DocId {
        self.tables.doc_named(docname).unwrap()
    }

    /// Resolves `reftype`:`target` written in `from`, with `edit` filling the rest.
    pub(crate) fn resolve(
        &self,
        from: &str,
        reftype: &str,
        target: &str,
        edit: impl FnOnce(&mut XrefQuery),
    ) -> (Result<Resolved, Box<Unresolved>>, Trace) {
        let mut query = XrefQuery {
            from: self.id(from),
            reftype: reftype.into(),
            target: target.into(),
            ..XrefQuery::default()
        };
        edit(&mut query);
        let mut trace = Trace::new();
        let result = crate::resolve_xref(&self.view(), &query, &mut trace);
        (result, trace)
    }

    /// Where `reftype`:`target` written in `from` points: (docname, anchor).
    pub(crate) fn target(
        &self,
        from: &str,
        reftype: &str,
        target: &str,
    ) -> Option<(String, String)> {
        let (result, _) = self.resolve(from, reftype, target, |_| {});
        let r = result.ok()?;
        Some((self.env.doc(r.doc?).docname.clone(), r.anchor))
    }

    pub(crate) fn codes(&self) -> Vec<&str> {
        self.reports.iter().map(|r| r.code.as_str()).collect()
    }
}
