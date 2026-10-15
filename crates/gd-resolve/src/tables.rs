//! The project tables: every document's symbols joined, its toctrees resolved, its
//! sections and items numbered (resolve.odin, numbering.odin, cpp_symbols.odin).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use gd_conf::Config;
use gd_env::{DocId, DocInfo, Environment, LabelKind, ObjectInfo, UiText};
use gd_report::{Report, Severity};

use crate::cpp::CppTree;
use crate::view::chapter_key;

/// Where a name (label, term, object) points.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TargetRef {
    /// The document it is in.
    pub doc: DocId,
    /// Its id on the page; empty for the document itself.
    pub anchor: String,
    /// The section title or caption a reference shows; may be empty.
    pub title: String,
    /// What a label names.
    pub kind: LabelKind,
    /// An object's type; empty for labels and terms.
    pub objtype: String,
    /// An object's canonical name (`:canonical:`), not the object's own.
    pub aliased: bool,
}

/// A resolved toctree entry: a document of the project, a generated page, or a URL.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TreeEntry {
    /// The document; `None` for a URL or a generated page.
    pub doc: Option<DocId>,
    /// A URL, or a generated page's file name relative to the output root.
    pub url: String,
    /// The explicit title; empty for none.
    pub title: String,
    /// The general index, the module index, or the search page.
    pub generated: bool,
}

/// The project-wide tables. Per-document tables are indexed by `DocId::index`; names
/// are the only keyed lookups.
#[derive(Debug, Clone, Default)]
pub struct Tables {
    /// Document ids by docname.
    pub by_name: HashMap<String, DocId>,
    /// Reading (toctree) order from the root document.
    pub order: Vec<DocId>,
    /// Per document: the document whose toctree reached it first; `None` for the root
    /// and unreached ones.
    pub parent: Vec<Option<DocId>>,
    /// Per document, per toctree: its resolved entries.
    pub trees: Vec<Vec<Vec<TreeEntry>>>,
    /// Per document, per section: its number (`"1.2"`) or empty.
    pub secnumbers: Vec<Vec<String>>,
    /// Per document, per numbered item: its number (`"3"`, `"1.2"`) or empty.
    pub fignumbers: Vec<Vec<String>>,
    /// Labels by normalized name.
    pub labels: HashMap<String, TargetRef>,
    /// Glossary terms by lowercased name.
    pub terms: HashMap<String, TargetRef>,
    /// Objects by key (`object_key`): `domain:fullname`, `std:<type>:<name>`.
    pub objects: HashMap<String, TargetRef>,
    /// The C++ domain's symbols.
    pub cpp: CppTree,
    /// Per document: its anchor namespace in the book and singlehtml
    /// (`install-guide` for `install/guide`, numbered when two names make one key).
    pub keys: Vec<String>,
}

impl Tables {
    /// The id of the document named `docname`.
    pub fn doc_named(&self, docname: &str) -> Option<DocId> {
        self.by_name.get(docname).copied()
    }
}

/// Joins every document's symbols into the project tables: resolves each toctree
/// (relative names against the document's folder, `/name` against the source folder,
/// globs against every docname, `genindex`/`modindex`/`search` as generated pages
/// titled with `ui`), walks them from the root document (order, parents, cycles),
/// registers labels, terms and objects as Sphinx's domains do (duplicates warned about,
/// the later one winning), builds the C++ symbol tree, warns about documents no
/// toctree reaches, and numbers sections and figures. The API index joins the root's
/// first toctree when `odin_autoapi_add_toctree_entry` or `rust_autoapi_add_toctree_entry`
/// asks. Warnings go to `reports`; it never fails.
pub fn resolve(
    config: &Config,
    env: &Environment,
    ui: &dyn UiText,
    reports: &mut Vec<Report>,
) -> Tables {
    let tables = Tables {
        by_name: env.iter().map(|(id, d)| (d.docname.clone(), id)).collect(),
        parent: vec![None; env.docs.len()],
        trees: Vec::with_capacity(env.docs.len()),
        secnumbers: env.docs.iter().map(|d| vec![String::new(); d.sections.len()]).collect(),
        fignumbers: env.docs.iter().map(|d| vec![String::new(); d.numbered.len()]).collect(),
        keys: document_keys(env),
        ..Tables::default()
    };
    let mut pass = Pass { config, env, ui, reports, tables };
    for (id, _) in env.iter() {
        let trees = pass.resolve_toctrees(id);
        pass.tables.trees.push(trees);
    }
    pass.visit_from_root();
    for (id, _) in env.iter() {
        pass.register_symbols(id);
    }
    pass.tables.cpp = CppTree::build(env, pass.reports);
    pass.warn_unreached();
    crate::numbering::number_sections(&mut pass);
    crate::numbering::number_figures(&mut pass);
    pass.tables
}

/// Files an object for lookup. Standard objects other than environment variables and
/// configuration values are filed by type, as Sphinx keys them by (objtype, name):
/// `std:option:rm -r`, `std:token:python-grammar:expr`, declared types; every other
/// object is `domain:fullname`.
pub fn object_key(domain: &str, objtype: &str, fullname: &str) -> String {
    if domain == "std" && objtype != "envvar" && objtype != "confval" {
        return format!("std:{objtype}:{fullname}");
    }
    format!("{domain}:{fullname}")
}

/// Lowercases a label or term and collapses white space, as Sphinx does.
pub fn normalize_name(name: &str) -> String {
    let lower = name.to_lowercase();
    lower.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// One run of `resolve`: what it reads, what it fills, where its warnings go.
pub(crate) struct Pass<'a> {
    pub(crate) config: &'a Config,
    pub(crate) env: &'a Environment,
    pub(crate) ui: &'a dyn UiText,
    pub(crate) reports: &'a mut Vec<Report>,
    pub(crate) tables: Tables,
}

/// A warning about a document, at `line` (0: the document itself).
pub(crate) fn doc_warning(d: &DocInfo, line: u32, parts: [&str; 5]) -> Report {
    let [code, title, message, hint, category] = parts;
    Report {
        severity: Severity::Warning,
        path: d.relative.clone(),
        line: line as usize,
        category: category.to_string(),
        ..Report::error(code, title, message).hint(hint)
    }
}

impl Pass<'_> {
    pub(crate) fn doc(&self, id: DocId) -> &DocInfo {
        self.env.doc(id)
    }

    /// Walks the toctrees depth-first from the root document, recording the reading
    /// order and each document's parent, and warning about cycles. The walk keeps its
    /// own stack, so a long chain of documents cannot overflow the thread's.
    fn visit_from_root(&mut self) {
        let Some(root) = self.tables.doc_named(&self.config.root_doc) else {
            let message =
                format!("root_doc is {}, but there is no such document.", self.config.root_doc);
            let report = Report::error("build.root.missing", "ROOT DOCUMENT MISSING", &message)
                .hint("Create it, or set root_doc in conf.toml.");
            let path = self.config.source_dir.to_string_lossy().into_owned();
            self.reports.push(Report { path, ..report });
            return;
        };
        let mut on_path = vec![false; self.env.docs.len()];
        // Per document on the path: (document, toctree, next entry).
        let mut stack: Vec<(DocId, usize, usize)> = vec![(root, 0, 0)];
        self.tables.order.push(root);
        on_path[root.index()] = true;
        while let Some(top) = stack.last_mut() {
            let (id, tree, entry) = *top;
            let trees = &self.tables.trees[id.index()];
            if tree >= trees.len() {
                on_path[id.index()] = false;
                stack.pop();
                continue;
            }
            if entry >= trees[tree].len() {
                *top = (id, tree + 1, 0);
                continue;
            }
            top.2 += 1;
            let Some(child) = trees[tree][entry].doc.filter(|&c| c != id) else { continue };
            if on_path[child.index()] {
                self.warn_cycle(id, child, tree);
                continue;
            }
            if self.tables.parent[child.index()].is_some() {
                continue;
            }
            self.tables.parent[child.index()] = Some(id);
            self.tables.order.push(child);
            on_path[child.index()] = true;
            stack.push((child, 0, 0));
        }
    }

    fn warn_cycle(&mut self, id: DocId, child: DocId, tree: usize) {
        let d = self.doc(id);
        let line = d.toctrees.get(tree).map_or(0, |t| t.line);
        let message =
            format!("{} includes {}, which includes it.", d.docname, self.doc(child).docname);
        let r = doc_warning(
            d,
            line,
            [
                "build.toctree.cycle",
                "CIRCULAR TOCTREE",
                &message,
                "Remove one of the two toctree entries.",
                "toc.circular",
            ],
        );
        self.reports.push(r);
    }

    /// Warns about documents no toctree reaches, as Sphinx's `check_consistency` does.
    /// As there, a document another document includes is part of that one's page.
    fn warn_unreached(&mut self) {
        let root = self.tables.doc_named(&self.config.root_doc);
        let included: HashSet<&PathBuf> = self
            .env
            .docs
            .iter()
            .flat_map(|d| d.deps.iter().filter(|dep| dep.included).map(|dep| &dep.path))
            .collect();
        for (id, d) in self.env.iter() {
            if Some(id) == root || self.tables.parent[id.index()].is_some() {
                continue;
            }
            if d.orphan || d.failed || included.contains(&self.config.source_dir.join(&d.relative))
            {
                continue;
            }
            self.reports.push(doc_warning(
                d,
                0,
                [
                    "build.toctree.orphan",
                    "DOCUMENT NOT IN ANY TOCTREE",
                    "No toctree includes this document, so readers cannot navigate to it.",
                    "Add it to a toctree, or put :orphan: at the top of the document.",
                    "toc.not_included",
                ],
            ));
        }
    }

    /// Adds a document's labels, terms and objects to the project tables.
    fn register_symbols(&mut self, id: DocId) {
        let env = self.env;
        let d = env.doc(id);
        self.register_labels(id, d);
        if self.config.has_extension("sphinx.ext.autosectionlabel") {
            self.section_labels(id, d);
        }
        for term in &d.terms {
            let target = TargetRef {
                doc: id,
                anchor: term.anchor.clone(),
                title: term.name.clone(),
                ..TargetRef::default()
            };
            self.tables.terms.insert(term.name.clone(), target);
        }
        self.register_objects(id, d);
    }

    /// As Sphinx's standard domain: a name already given to a titled label (a section,
    /// a captioned figure, table or code block) is warned about, and the later label
    /// takes it; an anonymous label (before an uncaptioned block, only for `:ref:` with
    /// a title) is replaced silently, and never replaces a titled one.
    fn register_labels(&mut self, id: DocId, d: &DocInfo) {
        for l in &d.labels {
            if let Some(other) = self.tables.labels.get(&l.name)
                && other.doc != id
                && !other.title.is_empty()
            {
                let message = format!(
                    "The label {} is also defined in {}.",
                    l.name,
                    self.doc(other.doc).docname
                );
                self.reports.push(doc_warning(
                    d,
                    0,
                    [
                        "build.label.duplicate",
                        "DUPLICATE LABEL",
                        &message,
                        "Rename one of the labels; references go to the one read last.",
                        "",
                    ],
                ));
                if l.title.is_empty() {
                    continue;
                }
            }
            let target = TargetRef {
                doc: id,
                anchor: l.anchor.clone(),
                title: l.title.clone(),
                kind: l.kind,
                ..TargetRef::default()
            };
            self.tables.labels.insert(l.name.clone(), target);
        }
    }

    /// As `PythonDomain.note_object`: an alias does not take the name of an object, and
    /// an object takes the name of an alias. Sphinx's C lookup also finds a member of
    /// an anonymous entity through its parent (`A.a` for `A.@data.a`) when the parent
    /// has no member of that name.
    fn register_objects(&mut self, id: DocId, d: &DocInfo) {
        for o in &d.objects {
            let key = object_key(&o.domain, &o.objtype, &o.fullname);
            if o.aliased && self.tables.objects.get(&key).is_some_and(|other| !other.aliased) {
                continue;
            }
            self.warn_duplicate_object(id, d, o, &key);
            self.tables.objects.insert(key, object_target(id, o, o.aliased));
        }
        for o in d.objects.iter().filter(|o| o.domain == "c" && o.fullname.contains('@')) {
            let key = object_key(&o.domain, &o.objtype, &without_anonymous(&o.fullname));
            self.tables.objects.entry(key).or_insert_with(|| object_target(id, o, false));
        }
    }

    /// Warns, as the `note_object` of Sphinx's Python, JavaScript and standard domains
    /// does, when an object is described again: a Python name an alias holds is not a
    /// duplicate, nor is an option, which Sphinx notes per program without a warning.
    fn warn_duplicate_object(&mut self, id: DocId, d: &DocInfo, o: &ObjectInfo, key: &str) {
        let checked = match o.domain.as_str() {
            "py" | "js" | "rust" => true,
            "std" => o.objtype != "cmdoption",
            _ => false,
        };
        let Some(other) = self.tables.objects.get(key) else { return };
        if !checked || other.aliased || o.aliased || (other.doc == id && other.anchor == o.anchor)
        {
            return;
        }
        let message = format!(
            "{} is also described in {}; references go to this description.",
            o.fullname,
            self.doc(other.doc).docname
        );
        self.reports.push(doc_warning(
            d,
            o.line,
            [
                "build.object.duplicate",
                "DUPLICATE OBJECT DESCRIPTION",
                &message,
                "Add :no-index: to one of the descriptions.",
                "duplicate_object",
            ],
        ));
    }

    /// `sphinx.ext.autosectionlabel`: every section is a label named by its title, or
    /// `docname:title` with `autosectionlabel_prefix_document`; sections at
    /// `autosectionlabel_maxdepth` or deeper (the document title is depth 0) get none.
    /// A name already taken is warned about, and the later section takes it.
    fn section_labels(&mut self, id: DocId, d: &DocInfo) {
        let c = self.config;
        for s in &d.sections {
            let mut depth = 0i64;
            let mut parent = s.parent;
            while let Some(p) = parent {
                depth += 1;
                parent = d.sections.get(p as usize).and_then(|x| x.parent);
            }
            if c.autosectionlabel_maxdepth > 0 && depth >= c.autosectionlabel_maxdepth {
                continue;
            }
            // As Sphinx, by the title as written (its rawsource), not as it reads.
            let title = if s.raw_title.is_empty() { &s.title } else { &s.raw_title };
            let name = if c.autosectionlabel_prefix_document {
                normalize_name(&format!("{}:{title}", d.docname))
            } else {
                normalize_name(title)
            };
            if let Some(other) = self.tables.labels.get(&name) {
                let message = format!(
                    "The section label {name} is also defined in {}.",
                    self.doc(other.doc).docname
                );
                let category = format!("autosectionlabel.{}", d.docname);
                self.reports.push(doc_warning(
                    d,
                    0,
                    [
                        "build.label.duplicate",
                        "DUPLICATE LABEL",
                        &message,
                        "Set autosectionlabel_prefix_document = true, or retitle one of the \
                     sections.",
                        &category,
                    ],
                ));
            }
            let target = TargetRef {
                doc: id,
                anchor: s.anchor.clone(),
                title: s.title.clone(),
                kind: LabelKind::Section,
                ..TargetRef::default()
            };
            self.tables.labels.insert(name, target);
        }
    }
}

fn object_target(id: DocId, o: &ObjectInfo, aliased: bool) -> TargetRef {
    TargetRef {
        doc: id,
        anchor: o.anchor.clone(),
        title: o.fullname.clone(),
        kind: LabelKind::Other,
        objtype: o.objtype.clone(),
        aliased,
    }
}

/// A C name with its anonymous parts left out (`A.@data.a` is `A.a`).
pub(crate) fn without_anonymous(name: &str) -> String {
    name.split('.').filter(|p| !p.starts_with('@')).collect::<Vec<_>>().join(".")
}

/// Gives every document an anchor namespace (`chapter_key`); names that make the same
/// key are told apart by a number.
fn document_keys(env: &Environment) -> Vec<String> {
    let mut taken = HashSet::with_capacity(env.docs.len());
    let mut keys = Vec::with_capacity(env.docs.len());
    for d in &env.docs {
        let base = chapter_key(&d.docname);
        let mut key = base.clone();
        let mut n = 2;
        while taken.contains(&key) {
            key = format!("{base}-{n}");
            n += 1;
        }
        taken.insert(key.clone());
        keys.push(key);
    }
    keys
}
