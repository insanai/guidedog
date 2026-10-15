//! The Sphinx layer, rendered with Sphinx's element structure and class names, so
//! styles written for Sphinx themes carry over. See AST.md, "Sphinx layer".

use guidedoc::{Content, Label, NodeId, NodeKind, content_model, label, split_label};

use crate::blocks::list_tag;
use crate::state::{Html, char_count};
use crate::walk::{Close, Visit};

/// A kind of version note: its `type`, its class, and its label.
struct VersionChange {
    kind: &'static str,
    class: &'static str,
    label: Label,
}

const VERSION_CHANGES: [VersionChange; 4] = [
    VersionChange { kind: "versionadded", class: "added", label: Label::VersionAdded },
    VersionChange { kind: "versionchanged", class: "changed", label: Label::VersionChanged },
    VersionChange { kind: "deprecated", class: "deprecated", label: Label::VersionDeprecated },
    VersionChange { kind: "versionremoved", class: "removed", label: Label::VersionRemoved },
];

/// The author directives and their labels.
const AUTHOR_LABELS: [(&str, Label); 3] = [
    ("sectionauthor", Label::SectionAuthor),
    ("moduleauthor", Label::ModuleAuthor),
    ("codeauthor", Label::CodeAuthor),
];

/// The element and classes Sphinx gives a generic role's content.
const ROLE_STYLES: [(&str, &str, &str); 19] = [
    ("kbd", "kbd", "kbd docutils literal notranslate"),
    ("guilabel", "span", "guilabel"),
    ("accelerator", "span", "accelerator"),
    ("menuselection", "span", "menuselection"),
    ("command", "strong", "command"),
    ("program", "strong", "program"),
    ("makevar", "strong", "makevar"),
    ("dfn", "em", "dfn"),
    ("mimetype", "em", "mimetype"),
    ("newsgroup", "em", "newsgroup"),
    ("mailheader", "em", "mailheader"),
    ("manpage", "em", "manpage"),
    ("file", "code", "file docutils literal notranslate"),
    ("samp", "code", "samp docutils literal notranslate"),
    ("regexp", "code", "regexp docutils literal notranslate"),
    // :cpp:expr:, :cpp:texpr:, :c:expr:, and :c:texpr:, their names linked (desc_inline).
    ("cpp-expr", "code", "cpp-expr sig sig-inline cpp"),
    ("cpp-texpr", "span", "cpp-texpr sig sig-inline cpp"),
    ("c-expr", "code", "c-expr sig sig-inline c"),
    ("c-texpr", "span", "c-texpr sig sig-inline c"),
];

impl Html<'_, '_> {
    fn version_change(&self, id: NodeId) -> &'static VersionChange {
        let kind = self.custom(id, "type");
        VERSION_CHANGES.iter().find(|c| c.kind == kind).unwrap_or(&VERSION_CHANGES[1])
    }

    /// Writes "Added in version X: " before the explanation, or the whole sentence
    /// "Added in version X." when there is none.
    pub fn version_label(&mut self, id: NodeId, explained: bool) {
        let change = self.version_change(id);
        self.w.put("<span class=\"versionmodified ");
        self.w.put(change.class);
        self.w.put("\">");
        let (before, after) = split_label(label(self.options, change.label));
        self.w.esc(before);
        self.w.esc(self.custom(id, "version"));
        self.w.esc(after);
        self.w.put(if explained { ": " } else { "." });
        self.w.put("</span>");
    }

    /// Writes an author directive's line when the project shows authors.
    pub fn enter_author(&mut self, id: NodeId, name: &str) -> Visit {
        if self.custom(id, "show") != "1" {
            return Visit::Skip;
        }
        for (directive, l) in AUTHOR_LABELS {
            if directive != name {
                continue;
            }
            self.w.cr();
            self.tag("p", id, "");
            self.w.put("<span class=\"");
            self.w.put(directive);
            self.w.put("\">");
            self.w.esc(label(self.options, l));
            self.w.put("</span>");
            self.w.esc(self.custom(id, "author"));
            self.line_close("p");
        }
        Visit::Skip
    }

    /// Starts a caption's Sphinx parts: its number label, then its text span.
    pub fn caption_open(&mut self, owner: Option<NodeId>) {
        let Some(owner) = owner.filter(|&o| self.sphinx_caption(o)) else {
            return;
        };
        let number = self.custom(owner, "number-label");
        if !number.is_empty() {
            self.w.put("<span class=\"caption-number\">");
            self.w.esc(number);
            self.w.put(" </span>");
        }
        self.w.put("<span class=\"caption-text\">");
    }

    pub fn caption_close(&mut self, owner: Option<NodeId>, title: &str) {
        let Some(owner) = owner.filter(|&o| self.sphinx_caption(o)) else {
            return;
        };
        self.w.put("</span>");
        self.headerlink(Some(owner), title);
    }

    fn sphinx_caption(&self, owner: NodeId) -> bool {
        !self.w.reference
            && (self.options.permalinks || !self.custom(owner, "number-label").is_empty())
    }

    pub fn enter_index(&mut self, id: NodeId) -> Visit {
        let parent = self.parent_kind(self.parent());
        if parent == NodeKind::Desc {
            return Visit::Skip; // written before the <dl>.
        }
        let block = content_model(parent) == Content::Blocks;
        if block {
            self.w.cr();
        }
        self.write_all_ids(id);
        if block {
            self.w.cr();
        }
        Visit::Skip
    }

    pub fn enter_glossary(&mut self, id: NodeId) -> Visit {
        self.w.cr();
        self.write_all_ids(id);
        Visit::Descend(Close::Nothing)
    }

    pub fn enter_version(&mut self, id: NodeId) -> Visit {
        self.block_open("div", id, self.version_change(id).kind);
        let explained =
            self.doc.first_child(id).is_some_and(|f| self.doc.kind(f) == NodeKind::Paragraph);
        if !explained {
            self.w.put("<p>");
            self.version_label(id, false);
            self.w.put("</p>");
            self.w.cr();
        }
        Visit::Descend(Close::Block("div"))
    }

    /// Opens a domain object's `<dl class="domain objtype">`; its index entries' ids go
    /// before it, since a `<dl>` holds only terms and details.
    pub fn enter_desc(&mut self, id: NodeId) -> Visit {
        self.w.cr();
        for child in self.doc.children(id) {
            if self.doc.kind(child) == NodeKind::Index {
                self.write_all_ids(child);
            }
        }
        self.write_aliases(id);
        self.w.put("<dl");
        self.write_id_attribute(id);
        self.w.put(" class=\"");
        let (domain, objtype) = (self.custom(id, "domain"), self.custom(id, "objtype"));
        self.w.esc(domain);
        if !domain.is_empty() && !objtype.is_empty() {
            self.w.put_byte(b' ');
        }
        self.w.esc(objtype);
        self.write_more_classes(id);
        self.w.put("\">");
        self.w.cr();
        Visit::Descend(Close::Block("dl"))
    }

    pub fn enter_signature(&mut self, id: NodeId) -> Visit {
        self.w.cr();
        self.w.put("<dt");
        self.write_id_attribute(id);
        self.w.put(" class=\"sig sig-object");
        let domain = self.parent().map_or("", |p| self.custom(p, "domain"));
        if !domain.is_empty() {
            self.w.put_byte(b' ');
            self.w.esc(domain);
        }
        self.write_more_classes(id);
        self.w.put("\">");
        self.write_aliases(id);
        Visit::Descend(Close::Signature)
    }

    pub fn leave_signature(&mut self, id: NodeId) {
        if self.custom(id, "nopermalink").is_empty() {
            // A redeclaration has none.
            self.headerlink(Some(id), label(self.options, Label::LinkDefinition));
        }
        self.line_close("dt");
    }

    /// Tells whether a term is one of a glossary's: Glossary > DefinitionList >
    /// DefinitionItem > Term.
    pub fn in_glossary(&self, term: NodeId) -> bool {
        let mut node = Some(term);
        for _ in 0..3 {
            node = node.and_then(|n| self.doc.parent_of(n));
        }
        node.is_some_and(|n| self.doc.kind(n) == NodeKind::Glossary)
    }

    /// Splits the list's items into columns of a one-row table, as Sphinx does: the
    /// first columns take one item more when the items do not divide evenly.
    pub fn enter_hlist(&mut self, id: NodeId) -> Visit {
        let doc = self.doc;
        let columns = usize::try_from(doc.node(id).value).ok().filter(|&c| c > 0).unwrap_or(2);
        self.w.cr();
        self.tag("table", id, "hlist");
        self.w.put("<tr>");
        if let Some(list) = doc.first_child(id).filter(|&l| doc.kind(l) == NodeKind::List) {
            let items = doc.child_count(list);
            let (per, more) = (items / columns, items % columns);
            let mut children = doc.children(list);
            let mut index = 0;
            for column in 0..columns {
                let count = per + usize::from(column < more);
                if count == 0 {
                    continue;
                }
                self.w.put("<td>");
                self.open_list(list, index as i64, index == 0);
                for item in children.by_ref().take(count) {
                    self.walk(item, doc.subtree_end(item), Some(list));
                }
                self.block_close(list_tag(doc.list_info(list).style));
                self.w.put("</td>");
                index += count;
            }
        }
        self.w.put("</tr>");
        self.w.cr();
        self.w.close("table");
        self.w.cr();
        Visit::Skip
    }

    pub fn enter_production_list(&mut self, id: NodeId) -> Visit {
        let doc = self.doc;
        self.productions =
            doc.children(id).map(|c| char_count(doc.node_text(c))).max().unwrap_or(0);
        self.w.cr();
        self.tag("pre", id, "productionlist");
        Visit::Descend(Close::Line("pre"))
    }

    /// Writes "token ::= " padded to the widest token; a continuation line (no token)
    /// is indented to the definitions.
    pub fn enter_production(&mut self, id: NodeId) -> Visit {
        let token = self.doc.node_text(id);
        if token.is_empty() {
            for _ in 0..self.productions + 5 {
                self.w.put_byte(b' ');
            }
            return Visit::Descend(Close::Text("\n"));
        }
        self.tag("strong", id, "");
        self.w.esc(token);
        for _ in char_count(token)..self.productions {
            self.w.put_byte(b' ');
        }
        self.w.put("</strong> ::= ");
        Visit::Descend(Close::Text("\n"))
    }

    pub fn enter_role(&mut self, id: NodeId) -> Visit {
        let name = self.doc.node_text(id);
        match ROLE_STYLES.iter().find(|r| r.0 == name) {
            Some(&(_, element, class)) => self.inline(element, id, class),
            None => {
                self.tag("span", id, name);
                Visit::Descend(Close::Tag("span"))
            }
        }
    }

    pub fn enter_xref(&mut self, id: NodeId) -> Visit {
        if self.custom(id, "unstyled") == "1" {
            return Visit::Descend(Close::Nothing); // an info field's: the display alone.
        }
        self.w.put("<span");
        self.write_id_attribute(id);
        self.w.put(" class=\"xref ");
        self.write_xref_classes(self.custom(id, "reftype"));
        self.write_more_classes(id);
        self.w.put("\">");
        self.write_aliases(id);
        Visit::Descend(Close::Tag("span"))
    }

    /// Turns a reftype into Sphinx's classes: "ref" gives "std std-ref", "py:func"
    /// gives "py py-func".
    fn write_xref_classes(&mut self, reftype: &str) {
        let (domain, role) = reftype.split_once(':').unwrap_or(("std", reftype));
        self.w.esc(domain);
        if role.is_empty() {
            return;
        }
        self.w.put_byte(b' ');
        self.w.esc(domain);
        self.w.put_byte(b'-');
        self.w.esc(role);
    }

    pub fn enter_parameter_list(&mut self) -> Visit {
        self.w.put("<span class=\"sig-paren\">(</span>");
        Visit::Descend(Close::Text("<span class=\"sig-paren\">)</span>"))
    }

    pub fn enter_parameter(&mut self, id: NodeId) -> Visit {
        if self.parameter_follows(id) {
            self.w.put(", ");
        }
        self.inline("em", id, "sig-param")
    }

    pub fn enter_optional(&mut self) -> Visit {
        self.w.put("<span class=\"optional\">[</span>");
        Visit::Descend(Close::Text("<span class=\"optional\">]</span>"))
    }

    pub fn enter_returns(&mut self, id: NodeId) -> Visit {
        self.w.put(" <span class=\"sig-return\"><span class=\"sig-return-icon\">&#x2192;</span> ");
        self.tag("span", id, "sig-return-typehint");
        Visit::Descend(Close::Text("</span></span>"))
    }

    /// Tells whether a parameter comes after another one, so a comma separates them; a
    /// first parameter inside an optional group counts from the group.
    fn parameter_follows(&self, id: NodeId) -> bool {
        let mut n = id;
        let ancestors = self.open_ancestors().rev().map(Some).chain([self.outer]);
        for p in ancestors {
            let Some(p) = p else {
                return false;
            };
            if p + 1 != n {
                return true;
            }
            if self.doc.kind(p) != NodeKind::DescOptional {
                return false;
            }
            n = p;
        }
        false
    }
}
