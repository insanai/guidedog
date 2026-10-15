//! Toctrees resolved into project documents, generated pages and URLs (the toctree part
//! of resolve.odin and odin_toctree_entry of odin.odin).

use gd_env::{DocId, DocInfo, ToctreeInfo};

use crate::tables::{Pass, TreeEntry, doc_warning};
use crate::view::{dir_of, doc_target, file_target, generated_page, generated_title};

impl Pass<'_> {
    /// Expands each toctree of a document into project documents and URLs: relative
    /// names resolve against the document's folder, `/name` against the source folder,
    /// and glob patterns against every docname.
    pub(crate) fn resolve_toctrees(&mut self, id: DocId) -> Vec<Vec<TreeEntry>> {
        let env = self.env;
        let d = env.doc(id);
        let mut trees: Vec<Vec<TreeEntry>> =
            d.toctrees.iter().map(|tree| self.resolve_toctree(id, d, tree)).collect();
        let config = self.config;
        let odin = &config.odin_api;
        if odin.add_toctree_entry {
            self.api_toctree_entry(id, &mut trees, &odin.root, "odin");
        }
        let rust = &config.rust_api;
        if rust.add_toctree_entry {
            self.api_toctree_entry(id, &mut trees, &rust.root, "rust");
        }
        trees
    }

    fn resolve_toctree(&mut self, id: DocId, d: &DocInfo, tree: &ToctreeInfo) -> Vec<TreeEntry> {
        let base = dir_of(&d.docname);
        let mut entries = Vec::with_capacity(tree.entries.len());
        for e in &tree.entries {
            let name = doc_target(base, &e.target);
            if e.target == "self" {
                entries.push(TreeEntry {
                    doc: Some(id),
                    title: e.title.clone(),
                    ..Default::default()
                });
            } else if let Some(page) = generated_page(&name) {
                let title = if e.title.is_empty() {
                    self.ui.gettext(generated_title(&name)).into_owned()
                } else {
                    e.title.clone()
                };
                entries.push(TreeEntry { doc: None, url: page.into(), title, generated: true });
            } else if e.target.contains("://") || e.target.starts_with("mailto:") {
                let url = e.target.clone();
                entries.push(TreeEntry { url, title: e.title.clone(), ..Default::default() });
            } else if tree.glob && e.target.contains(['*', '?', '[']) {
                self.expand_glob(id, &name, tree.reversed, &mut entries);
            } else {
                let found = self.tables.doc_named(&file_target(base, &e.target));
                match found.or_else(|| self.tables.doc_named(&name)) {
                    Some(target) => {
                        if entries.iter().any(|x| x.doc == Some(target)) {
                            self.warn_entry(d, tree, &e.target, Problem::Duplicate);
                        }
                        entries.push(TreeEntry {
                            doc: Some(target),
                            title: e.title.clone(),
                            ..Default::default()
                        });
                        self.warn_untitled(d, target, tree.line);
                    }
                    None => self.warn_entry(d, tree, &e.target, Problem::Missing),
                }
            }
        }
        entries
    }

    /// Lists the documents a glob pattern matches, sorted, leaving out the document
    /// itself and those the toctree already lists, as Sphinx's toctree directive does.
    fn expand_glob(&self, me: DocId, pattern: &str, reversed: bool, entries: &mut Vec<TreeEntry>) {
        // The environment is sorted by docname, so the matches already are.
        let mut matches: Vec<DocId> = self
            .env
            .iter()
            .filter(|&(id, d)| {
                id != me
                    && !entries.iter().any(|e| e.doc == Some(id))
                    && gd_env::glob_match(pattern, &d.docname)
            })
            .map(|(id, _)| id)
            .collect();
        if reversed {
            matches.reverse();
        }
        entries
            .extend(matches.into_iter().map(|m| TreeEntry { doc: Some(m), ..Default::default() }));
    }

    fn warn_entry(&mut self, d: &DocInfo, tree: &ToctreeInfo, target: &str, problem: Problem) {
        let r = match problem {
            Problem::Duplicate => doc_warning(
                d,
                tree.line,
                [
                    "build.toctree.duplicate",
                    "DUPLICATE TOCTREE ENTRY",
                    &format!("The toctree lists {target} more than once."),
                    "Remove the repeated entry.",
                    "toc.duplicate_entry",
                ],
            ),
            Problem::Missing => {
                let known = self.tables.by_name.keys().map(String::as_str);
                let mut hint = "Fix the name, or check exclude_patterns in conf.toml.".to_string();
                if let Some(near) = gd_report::suggest(target.trim_start_matches('/'), known) {
                    hint.push_str(&format!(" Did you mean {near}?"));
                }
                doc_warning(
                    d,
                    tree.line,
                    [
                        "build.toctree.missing",
                        "TOCTREE ENTRY NOT FOUND",
                        &format!(
                            "The toctree names {target}, which is not a document of this project."
                        ),
                        &hint,
                        "toc.not_readable",
                    ],
                )
            }
        };
        self.reports.push(r);
    }

    /// Warns, as Sphinx does, when a toctree names a document whose contents give it no
    /// entry: no title, no object described, and no toctree.
    fn warn_untitled(&mut self, d: &DocInfo, target: DocId, line: u32) {
        let t = self.env.doc(target);
        if !t.sections.is_empty()
            || !t.title.is_empty()
            || !t.toc_objects.is_empty()
            || !t.toctrees.is_empty()
        {
            return;
        }
        let message =
            format!("The toctree names {}, which has no title, so it gets no link.", t.docname);
        self.reports.push(doc_warning(
            d,
            line,
            [
                "build.toctree.untitled",
                "TOCTREE ENTRY WITHOUT TITLE",
                &message,
                "Give the document a title.",
                "toc.no_title",
            ],
        ));
    }

    /// Adds a generated API index (`<root>/index`) to the root document's first toctree
    /// (`odin_autoapi_add_toctree_entry`, `rust_autoapi_add_toctree_entry`), or warns
    /// that the root has none.
    fn api_toctree_entry(
        &mut self,
        id: DocId,
        trees: &mut [Vec<TreeEntry>],
        api_root: &str,
        language: &str,
    ) {
        let d = self.env.doc(id);
        if d.docname != self.config.root_doc {
            return;
        }
        let Some(index) = self.tables.doc_named(&format!("{api_root}/index")) else { return };
        if !self.env.doc(index).generated {
            return;
        }
        let Some(first) = trees.first_mut() else {
            let setting = format!("{language}_autoapi_add_toctree_entry");
            let message = format!(
                "{setting} adds the API index to the root document's first toctree, and \
                 the root document has none."
            );
            let hint = format!(
                "Add a toctree to the root document, or set {setting} = false and list the \
                 index yourself."
            );
            let code = format!("{language}.autoapi.toctree");
            let r = doc_warning(d, 0, [&code, "NO TOCTREE FOR THE API", &message, &hint, ""]);
            self.reports.push(r);
            return;
        };
        if !first.iter().any(|e| e.doc == Some(index)) {
            first.push(TreeEntry { doc: Some(index), ..Default::default() });
        }
    }
}

/// What is wrong with a toctree entry.
#[derive(Debug, Clone, Copy)]
enum Problem {
    Duplicate,
    Missing,
}
