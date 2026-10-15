//! Literal content: literal and code blocks, doctest blocks, math, and raw blocks.

use guidedoc::{AttributeKey, NodeId};

use crate::blocks::Opt;
use crate::state::Writer;

/// Custom attributes of a code block that the directive's options do not set: they
/// come from the reader, the project host, or the attributes written separately.
const DERIVED: &[&str] = &[
    "linenos",
    "caption",
    "name",
    "graphviz",
    "testnodetype",
    "groups",
    "number",
    "number-label",
    "info",
    "layout",
    "alt",
    "pinned-ids",
];

impl Writer<'_, '_> {
    /// Tells whether `id` is a code block a paragraph may announce with "::": no
    /// language and no options.
    pub fn is_literal_block(&self, id: NodeId) -> bool {
        self.kind(id) == guidedoc::NodeKind::CodeBlock
            && self.doc.attributes(id).is_empty()
            && self.doc.node(id).value == 0
            && !self.doc.has_children(id)
            && !self.doc.node_text(id).is_empty()
    }

    /// A literal block (after "::") or a code-block directive with its options.
    pub fn code_block(&mut self, id: NodeId) {
        let announced = std::mem::take(&mut self.literal_announced);
        if self.custom(id, "graphviz").is_some() {
            self.graphviz(id);
            return;
        }
        if let Some(kind) = self.custom(id, "testnodetype") {
            let groups = self.custom(id, "groups").unwrap_or("").to_owned();
            let kind = kind.to_owned();
            self.directive_head(&kind, &groups, &[]);
            self.code_lines(id);
            return;
        }
        if self.is_literal_block(id) {
            if !announced {
                self.block_start();
                self.line("::");
                self.block_end();
            }
            self.code_lines(id);
            return;
        }
        let language = self.attr(id, AttributeKey::Language).unwrap_or("").to_owned();
        let options = self.code_options(id);
        let name = if self.sphinx { "code-block" } else { "code" };
        if self.custom(id, "info").is_some() {
            self.lossy(id, "the words after a fence's language", "the language alone");
        }
        self.directive_head(name, &language, &options);
        self.code_lines(id);
    }

    fn code_options(&self, id: NodeId) -> Vec<Opt> {
        let mut options: Vec<Opt> = Vec::new();
        let start = self.doc.node(id).value;
        if self.custom(id, "linenos").is_some() || (self.sphinx && start > 0) {
            if start > 1 {
                options.push(("lineno-start", start.to_string()));
            } else {
                options.push(("linenos", String::new()));
            }
        } else if start > 0 {
            let value = if start == 1 { String::new() } else { start.to_string() };
            options.push(("number-lines", value));
        }
        if let Some(caption) = self.custom(id, "caption") {
            options.push(("caption", caption.to_owned()));
        }
        if let Some(name) = self.custom(id, "name") {
            options.push(("name", name.to_owned()));
        }
        self.common_options(id, &[], &mut options);
        self.custom_options(id, DERIVED, &mut options);
        options
    }

    /// Adds every Custom attribute not in `derived` as an option of its name.
    pub fn custom_options(&self, id: NodeId, derived: &[&str], options: &mut Vec<Opt>) {
        for a in self.doc.attributes(id) {
            if a.key != AttributeKey::Custom {
                continue;
            }
            let key = self.doc.text(a.name);
            if derived.contains(&key) || options.iter().any(|(k, _)| *k == key) {
                continue;
            }
            // Option names are a small vocabulary; leak-free static names cover them.
            if let Some(&known) = OPTION_NAMES.iter().find(|n| **n == key) {
                options.push((known, self.doc.text(a.value).to_owned()));
            }
        }
    }

    /// Writes a code block's text as indented lines.
    fn code_lines(&mut self, id: NodeId) {
        let text = self.doc.node_text(id);
        let body = text.strip_suffix('\n').unwrap_or(text);
        if body.is_empty() {
            self.lossy(id, "an empty code block", "a directive without content");
            return;
        }
        let indented = body.lines().filter(|l| !l.trim().is_empty()).all(|l| l.starts_with(' '));
        if indented {
            self.lossy(id, "code whose every line is indented", "it, which loses that indent");
        }
        self.push_frame(3, "");
        self.block_start();
        for line in body.split('\n') {
            if line.trim().is_empty() {
                self.line("");
            } else {
                self.line(line);
            }
        }
        self.pop_frame();
        self.block_end();
    }

    /// A graphviz block: its DOT source under `.. graphviz::`.
    fn graphviz(&mut self, id: NodeId) {
        let mut options: Vec<Opt> = Vec::new();
        for key in ["layout", "caption"] {
            if let Some(v) = self.custom(id, key) {
                options.push((if key == "layout" { "layout" } else { "caption" }, v.to_owned()));
            }
        }
        if let Some(alt) = self.attr(id, AttributeKey::Alt).or_else(|| self.custom(id, "alt")) {
            options.push(("alt", alt.to_owned()));
        }
        self.common_options(id, &["graphviz"], &mut options);
        self.custom_options(id, DERIVED, &mut options);
        self.directive_head("graphviz", "", &options);
        self.code_lines(id);
    }

    /// A doctest block, written as it was read.
    pub fn doctest_block(&mut self, id: NodeId) {
        self.separate(Some(id));
        self.class_prefix(id, &[]);
        let text = self.doc.node_text(id);
        self.block_start();
        for line in text.trim_end_matches('\n').split('\n') {
            self.line(line);
        }
        self.block_end();
    }

    /// `.. math::` with its label.
    pub fn math_block(&mut self, id: NodeId) {
        let mut options: Vec<Opt> = Vec::new();
        let label = self.custom(id, "label").map(str::to_owned).or_else(|| {
            let id_attr = self.attr(id, AttributeKey::Id)?;
            id_attr.strip_prefix("equation-").map(str::to_owned)
        });
        if let Some(label) = label {
            options.push(("label", label));
        }
        if self.custom(id, "nowrap").is_some() {
            options.push(("nowrap", String::new()));
        }
        self.common_options(id, &[], &mut options);
        self.directive_head("math", "", &options);
        self.text_lines(id);
    }

    /// `.. raw:: format`.
    pub fn raw_block(&mut self, id: NodeId) {
        let format = self.attr(id, AttributeKey::Format).unwrap_or("html").to_owned();
        let mut options = Vec::new();
        self.common_options(id, &[], &mut options);
        self.directive_head("raw", &format, &options);
        self.text_lines(id);
    }

    /// Writes a node's text as directive content.
    fn text_lines(&mut self, id: NodeId) {
        let text = self.doc.node_text(id).trim_end_matches('\n');
        if text.trim().is_empty() {
            return;
        }
        self.push_frame(3, "");
        self.block_start();
        for line in text.split('\n') {
            self.line(line.trim_end());
        }
        self.pop_frame();
        self.block_end();
    }
}

/// Directive options the writer passes through from Custom attributes of the same
/// name.
const OPTION_NAMES: &[&str] = &[
    "emphasize-lines",
    "force",
    "dedent",
    "lines",
    "start-after",
    "end-before",
    "prepend",
    "append",
    "backlinks",
    "loading",
    "figwidth",
    "author",
    "depth",
    "prefix",
    "suffix",
    "start",
    "noindex",
    "no-contents-entry",
    "no-index-entry",
    "no-typesetting",
    "annotation",
    "canonical",
    "async",
    "abstractmethod",
    "classmethod",
    "staticmethod",
    "final",
    "type",
    "value",
    "platform",
    "synopsis",
    "deprecated",
];
