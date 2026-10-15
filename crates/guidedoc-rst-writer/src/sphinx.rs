//! The Sphinx layer's block directives: toctree, only and ifconfig, glossary, version
//! notes, hlist, productionlist, and index.

use guidedoc::{AttributeKey, NodeId, NodeKind};

use crate::blocks::Opt;
use crate::state::Writer;

/// Toctree flags kept as Custom "1".
const TOCTREE_FLAGS: [&str; 5] = ["hidden", "includehidden", "titlesonly", "glob", "reversed"];

impl Writer<'_, '_> {
    /// `.. toctree::` with its options and entries.
    pub fn toctree(&mut self, id: NodeId) {
        let mut options: Vec<Opt> = Vec::new();
        let depth = self.doc.node(id).value;
        if depth > 0 {
            options.push(("maxdepth", depth.to_string()));
        }
        for key in ["caption", "name"] {
            if let Some(v) = self.custom(id, key) {
                options.push((key, v.to_owned()));
            }
        }
        for flag in TOCTREE_FLAGS {
            if self.custom(id, flag).is_some() {
                options.push((flag, String::new()));
            }
        }
        if let Some(n) = self.custom(id, "numbered") {
            options.push(("numbered", if n == "999" { String::new() } else { n.to_owned() }));
        }
        let classes = self.classes(id, &[]);
        if !classes.is_empty() {
            options.push(("class", classes));
        }
        self.directive_head("toctree", "", &options);
        self.push_frame(3, "");
        self.block_start();
        for entry in self.doc.children(id) {
            let target = self.doc.node_text(entry);
            let line = match self.attr(entry, AttributeKey::Title) {
                Some(title) => format!("{title} <{target}>"),
                None => target.to_owned(),
            };
            self.line(&line);
        }
        self.pop_frame();
        self.block_end();
    }

    /// `.. only:: expression`, or `.. ifconfig:: expression`.
    pub fn only(&mut self, id: NodeId) {
        let (name, expression) = match self.custom(id, "ifconfig") {
            Some(e) => ("ifconfig", e),
            None => ("only", self.custom(id, "expression").unwrap_or("")),
        };
        self.directive_head(name, expression, &[]);
        self.directive_body(id, 0);
    }

    /// `.. glossary::` and its terms.
    pub fn glossary(&mut self, id: NodeId) {
        let mut options: Vec<Opt> = Vec::new();
        if self.custom(id, "sorted").is_some() {
            options.push(("sorted", String::new()));
        }
        let classes = self.classes(id, &["glossary"]);
        if !classes.is_empty() {
            options.push(("class", classes));
        }
        self.directive_head("glossary", "", &options);
        self.push_frame(3, "");
        for list in self.doc.children(id) {
            for item in self.doc.children(list) {
                self.definition_item(item, true);
            }
        }
        self.pop_frame();
        self.block_end();
    }

    /// `.. versionadded:: version` and the other version notes.
    pub fn version_modified(&mut self, id: NodeId) {
        let kind = self.custom(id, "type").unwrap_or("versionchanged").to_owned();
        let version = self.custom(id, "version").unwrap_or("").to_owned();
        self.directive_head(&kind, &version, &[]);
        self.directive_body(id, 0);
    }

    /// `.. hlist::`: its column lists joined into one list again.
    pub fn hlist(&mut self, id: NodeId) {
        let columns = self.doc.node(id).value;
        let options = vec![("columns", columns.to_string())];
        self.directive_head("hlist", "", &options);
        self.push_frame(3, "");
        self.block_start();
        for (column, list) in self.doc.children(id).enumerate() {
            let info = self.doc.list_info(list);
            for (n, item) in self.doc.children(list).enumerate() {
                if column + n > 0 {
                    self.blank = false;
                    self.block_start();
                }
                let marker = crate::lists::marker(info, n);
                self.push_frame(marker.chars().count(), &marker);
                self.blocks(item);
                self.pop_frame();
            }
        }
        self.pop_frame();
        self.block_end();
    }

    /// `.. productionlist:: group` with a line per production; token references are
    /// backquoted again.
    pub fn production_list(&mut self, id: NodeId) {
        let group = self.custom(id, "group").unwrap_or("").to_owned();
        self.directive_head("productionlist", &group, &[]);
        self.push_frame(3, "");
        self.block_start();
        for production in self.doc.children(id) {
            let mut line = String::new();
            line.push_str(self.doc.node_text(production));
            line.push_str(": ");
            for part in self.doc.children(production) {
                let mut text = String::new();
                self.doc.plain_text(part, &mut text);
                if self.kind(part) == NodeKind::Xref {
                    line.push('`');
                    line.push_str(&text);
                    line.push('`');
                } else {
                    line.push_str(&text);
                }
            }
            self.line(line.trim_end());
        }
        self.pop_frame();
        self.block_end();
    }

    /// `.. index::` with its entries; inside text, the `:index:` role, which also
    /// writes the text after it.
    pub fn index(&mut self, id: NodeId) {
        let entries = self.custom(id, "entries").unwrap_or("").to_owned();
        if self.in_inline {
            return self.index_role(id, &entries);
        }
        let mut lines = entries.lines();
        let first = lines.next().unwrap_or("").to_owned();
        let rest: Vec<String> = lines.map(str::to_owned).collect();
        if rest.is_empty() {
            self.directive_head("index", &first, &[]);
            return;
        }
        self.directive_head("index", "", &[]);
        self.push_frame(3, "");
        self.block_start();
        self.line(&first);
        for line in &rest {
            self.line(line);
        }
        self.pop_frame();
        self.block_end();
    }

    fn index_role(&mut self, id: NodeId, entries: &str) {
        let next = self.doc.subtree_end(id);
        let follows = (next.index() < self.doc.node_count())
            .then_some(next)
            .filter(|&n| self.kind(n) == NodeKind::Text)
            .map(|n| self.doc.node_text(n));
        let generated = (next.index() < self.doc.node_count())
            && matches!(self.kind(next), NodeKind::Xref | NodeKind::Link | NodeKind::Reference);
        let in_term = self.doc.parent_of(id).is_some_and(|p| self.kind(p) == NodeKind::Term);
        if generated || in_term {
            // A role that makes its own index entry (envvar, pep, rfc) writes it again,
            // and so does a glossary for its terms.
            return;
        }
        let term = entries.strip_prefix("single: ").filter(|t| !t.contains('\n'));
        if let (Some(term), Some(text)) = (term, follows)
            && text.starts_with(term)
        {
            self.inline.markup(&format!(":index:`{}`", term.replace('`', "\\`")));
            self.skip_text = term.len();
            return;
        }
        self.lossy(id, "an index entry inside text", "an index directive's entry as a role");
        self.inline.markup(&format!(":index:`<{}>`", entries.replace('\n', "; ")));
    }
}
