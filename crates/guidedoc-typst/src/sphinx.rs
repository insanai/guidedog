//! The Sphinx layer (AST.md, "Sphinx layer") in Typst: labels for every id, numbered
//! captions and headings as the host numbered them, and styled blocks for version notes,
//! signatures, horizontal lists, and grammars. The helpers are defined in the preamble.

use guidedoc::{Label, ListStyle, NodeFlag, NodeId, NodeKind, label, split_label};

use crate::state::State;

/// How a version note's `type` reads.
struct VersionLabel {
    type_name: &'static str,
    kind: &'static str,
    label: Label,
}

const VERSION_LABELS: [VersionLabel; 4] = [
    VersionLabel { type_name: "versionadded", kind: "added", label: Label::VersionAdded },
    VersionLabel { type_name: "versionchanged", kind: "changed", label: Label::VersionChanged },
    VersionLabel { type_name: "deprecated", kind: "deprecated", label: Label::VersionDeprecated },
    VersionLabel { type_name: "versionremoved", kind: "removed", label: Label::VersionRemoved },
];

/// The markup around the content of the roles Sphinx defines.
const ROLE_STYLES: [(&str, &str, &str); 16] = [
    ("kbd", "#gd-kbd[", "]"),
    ("guilabel", "#emph[", "]"),
    ("menuselection", "#emph[", "]"),
    ("accelerator", "#underline[", "]"),
    ("command", "#strong[#gd-code-text[", "]]"),
    ("program", "#strong[#gd-code-text[", "]]"),
    ("makevar", "#strong[#gd-code-text[", "]]"),
    ("dfn", "#emph[", "]"),
    ("manpage", "#emph[", "]"),
    ("mimetype", "#emph[#gd-code-text[", "]]"),
    ("newsgroup", "#emph[#gd-code-text[", "]]"),
    ("mailheader", "#emph[#gd-code-text[", "]]"),
    ("file", "#gd-code-text[", "]"),
    ("samp", "#gd-code-text[", "]"),
    ("regexp", "#gd-code-text[", "]"),
    ("xref", "#emph[", "]"),
];

/// The lead-in label of each author directive.
const AUTHOR_LABELS: [(&str, Label); 3] = [
    ("sectionauthor", Label::SectionAuthor),
    ("moduleauthor", Label::ModuleAuthor),
    ("codeauthor", Label::CodeAuthor),
];

impl<'a> State<'_, 'a> {
    /// Writes `#gd-version("added", [Added in version 2.1:])[explanation]`.
    pub(crate) fn render_version(&mut self, id: NodeId) {
        let type_name = self.custom(id, "type");
        let v =
            VERSION_LABELS.iter().find(|l| l.type_name == type_name).unwrap_or(&VERSION_LABELS[1]);
        let explained = self.doc.has_children(id);
        self.emit("#gd-version(\"");
        self.emit(v.kind);
        self.emit("\", [");
        let (before, after) = split_label(label(self.options, v.label));
        self.emit_text(before);
        self.emit_text(self.custom(id, "version"));
        self.emit_text(after);
        self.emit_text(if explained { ":" } else { "." });
        self.emit("])[\n");
        self.render_blocks(id);
        self.emit("]\n");
    }

    /// Writes each signature as a labelled `#gd-signature`, then the description.
    pub(crate) fn render_desc(&mut self, id: NodeId) {
        let doc = self.doc;
        let attached =
            doc.children(id).any(|c| doc.kind(c) == NodeKind::DescContent && doc.has_children(c));
        for child in doc.children(id) {
            self.map_node(child);
            let kind = doc.kind(child);
            if kind == NodeKind::Index {
                self.emit_anchor(child);
            } else if kind == NodeKind::DescSignature {
                self.emit_anchor(child);
                let open =
                    if attached { "#gd-signature[" } else { "#gd-signature(attached: false)[" };
                self.wrap_inlines(open, child, "]\n");
            } else if kind == NodeKind::DescContent {
                self.emit_anchor(child);
                if doc.has_children(child) {
                    self.emit("#gd-description[\n");
                    self.render_blocks(child);
                    self.emit("]\n");
                }
            }
        }
    }

    /// Writes a parameter list or optional group between `open` and `close`.
    pub(crate) fn render_parameter_list(&mut self, id: NodeId, open: &str, close: &str) {
        self.emit(open);
        self.render_parameters(id, false);
        self.emit(close);
    }

    /// Writes a parameter list's contents; an optional group is shown in brackets, and
    /// parameters are separated by commas across groups, as in `f(a[, b])`. Returns
    /// whether a parameter has been written.
    fn render_parameters(&mut self, id: NodeId, follows: bool) -> bool {
        if !self.enter(id) {
            return follows;
        }
        let mut after = follows;
        for child in self.doc.children(id) {
            if self.doc.kind(child) == NodeKind::DescOptional {
                self.emit("\\[");
                after = self.render_parameters(child, after);
                self.emit("\\]");
                continue;
            }
            if after {
                self.emit(", ");
            }
            self.render_inline(child);
            after = true;
        }
        self.leave();
        after
    }

    /// Styles the content of the roles Sphinx defines; others show it plainly.
    pub(crate) fn render_role(&mut self, id: NodeId) {
        let name = self.doc.node_text(id);
        match ROLE_STYLES.iter().find(|r| r.0 == name) {
            Some(&(_, open, close)) => self.wrap_inlines(open, id, close),
            None => self.render_inlines(id),
        }
    }

    /// Writes `#gd-hlist`, which splits the items into columns for PDF.
    pub(crate) fn render_hlist(&mut self, id: NodeId) {
        let doc = self.doc;
        let Some(list) = doc.first_child(id).filter(|&l| doc.kind(l) == NodeKind::List) else {
            return;
        };
        let columns = doc.node(id).value;
        let info = doc.list_info(list);
        self.emit("#gd-hlist(columns: ");
        self.int(i64::from(if columns > 0 { columns } else { 2 }));
        self.emit(", tight: ");
        self.emit(if self.has_flag(list, NodeFlag::Tight) { "true" } else { "false" });
        if info.style != ListStyle::Bullet {
            self.emit(", start: ");
            self.int(i64::from(info.start.max(1)));
        }
        self.emit(",\n");
        self.emit_item_arguments(list);
        self.emit(")\n");
    }

    /// Writes a grammar as rows of token, `::=`, and definition; each token carries its
    /// label, and a continuation row has neither.
    pub(crate) fn render_productions(&mut self, id: NodeId) {
        self.emit("#gd-productions(\n");
        for p in self.doc.children(id) {
            self.map_node(p);
            let token = self.doc.node_text(p);
            self.byte(b'[');
            self.emit_anchor(p);
            self.emit_text(token);
            self.emit(if token.is_empty() { "], [], [" } else { "], [\\:\\:\\=], [" });
            self.render_inlines(p);
            self.emit("],\n");
        }
        self.emit(")\n");
    }

    /// Writes a paragraph; the centered directive centers it in bold, and a toctree
    /// caption becomes a rubric.
    pub(crate) fn render_paragraph(&mut self, id: NodeId) {
        if self.doc.has_class(id, "centered") {
            self.wrap_inlines("#gd-align(center)[#strong[", id, "]]\n");
        } else if self.doc.has_class(id, "caption") && self.is_toctree_caption(id) {
            self.wrap_inlines("#gd-rubric[", id, "]\n");
        } else {
            self.render_inlines(id);
            self.byte(b'\n');
        }
    }

    fn is_toctree_caption(&self, id: NodeId) -> bool {
        self.doc.parent_of(id).is_some_and(|p| {
            self.doc.kind(p) == NodeKind::Container && self.doc.has_class(p, "toctree-wrapper")
        })
    }

    /// Returns the lead-in of an author directive, or `None` for other directives.
    pub(crate) fn author_label(&self, name: &str) -> Option<&'a str> {
        let (_, l) = AUTHOR_LABELS.iter().find(|a| a.0 == name)?;
        Some(label(self.options, *l))
    }

    pub(crate) fn render_author(&mut self, id: NodeId, lead: &str) {
        self.emit_text(lead);
        self.emit_text(self.custom(id, "author"));
        self.byte(b'\n');
    }

    /// Writes a host-assigned number label before a caption's text.
    pub(crate) fn emit_caption_number(&mut self, id: NodeId) {
        let number = self.custom(id, "number-label");
        if !number.is_empty() {
            self.wrap_text("#strong[", number, "] ");
        }
    }

    /// Writes the emphasized lines as a Typst array, such as `(1, 3, 4, 5, )`.
    pub(crate) fn emit_line_list(&mut self, spec: &str, code: &str) {
        let lines = code.lines().count();
        self.byte(b'(');
        for line in 1..=lines {
            if in_line_ranges(spec, line) {
                self.int(line as i64);
                self.emit(", ");
            }
        }
        self.byte(b')');
    }
}

/// Tells whether 1-based `line` is in a list such as "1,3-5"; "7-" runs to the end.
pub(crate) fn in_line_ranges(spec: &str, line: usize) -> bool {
    spec.split(',').any(|range| {
        let (lo, hi) = match range.split_once('-') {
            Some((lo, hi)) => {
                let hi = read_number(hi);
                (read_number(lo), if hi == 0 { usize::MAX } else { hi })
            }
            None => {
                let lo = read_number(range);
                (lo, lo)
            }
        };
        lo > 0 && (lo..=hi).contains(&line)
    })
}

/// Reads the number at the start of `s` after spaces; 0 without one.
fn read_number(s: &str) -> usize {
    s.trim_start_matches(' ')
        .bytes()
        .take_while(u8::is_ascii_digit)
        .fold(0, |v, d| if v < 1_000_000 { v * 10 + usize::from(d - b'0') } else { v })
}
