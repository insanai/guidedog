//! Lists and list-like blocks: bullet and enumerated lists, definition, field, and
//! option lists, and line blocks. Items are content arguments of one helper call.

use guidedoc::{ListStyle, NodeFlag, NodeId, NodeKind};

use crate::state::State;

/// The Typst numbering letter of each list style.
fn numbering(style: ListStyle) -> &'static str {
    match style {
        ListStyle::Bullet | ListStyle::Arabic => "1",
        ListStyle::LowerAlpha => "a",
        ListStyle::UpperAlpha => "A",
        ListStyle::LowerRoman => "i",
        ListStyle::UpperRoman => "I",
    }
}

impl State<'_, '_> {
    /// Writes `#list` or `#enum` with one content argument per item.
    pub(crate) fn render_list(&mut self, id: NodeId) {
        let info = self.doc.list_info(id);
        let tight = if self.has_flag(id, NodeFlag::Tight) { "true" } else { "false" };
        if info.style == ListStyle::Bullet {
            self.emit("#list(tight: ");
            self.emit(tight);
        } else {
            self.emit("#enum(tight: ");
            self.emit(tight);
            self.emit(", start: ");
            let mut start = info.start;
            if start < 0 || (start == 0 && info.style != ListStyle::Arabic) {
                start = 1;
            }
            self.int(i64::from(start));
            self.emit(", numbering: \"");
            if info.delimiter == b'(' {
                self.byte(b'(');
            }
            self.emit(numbering(info.style));
            self.byte(if matches!(info.delimiter, b')' | b'(') { b')' } else { b'.' });
            self.byte(b'"');
        }
        self.emit(",\n");
        self.emit_item_arguments(id);
        self.emit(")\n");
    }

    /// Writes each child of `list` as a content argument followed by ",\n".
    pub(crate) fn emit_item_arguments(&mut self, list: NodeId) {
        for item in self.doc.children(list) {
            self.map_node(item);
            self.emit_block_argument(item);
            self.emit(",\n");
        }
    }

    /// Writes the block children of `id` as a content argument `[...]`.
    pub(crate) fn emit_block_argument(&mut self, id: NodeId) {
        if !self.enter(id) {
            return;
        }
        // The label goes inside the brackets: the argument is in code mode, where
        // markup such as #metadata cannot stand.
        self.byte(b'[');
        self.emit_anchor(id);
        if self.doc.has_children(id) {
            self.byte(b'\n');
            self.render_blocks(id);
        }
        self.byte(b']');
        self.leave();
    }

    /// Writes a definition list: terms and classifiers, then the definition.
    pub(crate) fn render_definitions(&mut self, id: NodeId) {
        let doc = self.doc;
        self.emit("#gd-terms(\n");
        for item in doc.children(id) {
            self.map_node(item);
            self.emit("terms.item([");
            self.emit_anchor(item); // inside the brackets: the arguments are code.
            let mut terms = 0;
            for part in doc.children(item) {
                let kind = doc.kind(part);
                if kind == NodeKind::Term {
                    if terms > 0 {
                        self.emit(" \\\n");
                    }
                    self.emit_anchor(part);
                    self.render_inlines(part);
                    terms += 1;
                } else if kind == NodeKind::Classifier {
                    self.wrap_inlines("#gd-classifier[", part, "]");
                }
            }
            self.emit("], ");
            let definition = self.child_of_kind(item, NodeKind::Definition);
            self.emit_optional_argument(definition);
            self.emit("),\n");
        }
        self.emit(")\n");
    }

    /// Returns the first child of `id` of `kind`.
    pub(crate) fn child_of_kind(&self, id: NodeId, kind: NodeKind) -> Option<NodeId> {
        self.doc.children(id).find(|&c| self.doc.kind(c) == kind)
    }

    /// Writes `id` as a content argument, or `[]` without one.
    fn emit_optional_argument(&mut self, id: Option<NodeId>) {
        match id {
            Some(id) => self.emit_block_argument(id),
            None => self.emit("[]"),
        }
    }

    /// Writes a field list as alternating name and body cells.
    pub(crate) fn render_fields(&mut self, id: NodeId) {
        self.emit("#gd-fields(\n");
        for field in self.doc.children(id) {
            self.map_node(field);
            self.byte(b'[');
            self.emit_anchor(field); // inside the brackets: the arguments are code.
            if let Some(name) = self.child_of_kind(field, NodeKind::FieldName) {
                self.render_inlines(name);
            }
            self.emit("], ");
            self.emit_optional_argument(self.child_of_kind(field, NodeKind::FieldBody));
            self.emit(",\n");
        }
        self.emit(")\n");
    }

    /// Writes an option list: the option group as raw text, then the description.
    pub(crate) fn render_options(&mut self, id: NodeId) {
        self.emit("#gd-options(\n");
        for item in self.doc.children(id) {
            self.map_node(item);
            self.byte(b'[');
            self.emit_anchor(item); // inside the brackets: the arguments are code.
            for group in self.doc.children(item) {
                if self.doc.kind(group) == NodeKind::OptionGroup {
                    self.emit_option_group(group);
                }
            }
            self.emit("], ");
            let description = self.child_of_kind(item, NodeKind::OptionDescription);
            self.emit_optional_argument(description);
            self.emit(",\n");
        }
        self.emit(")\n");
    }

    fn emit_option_group(&mut self, group: NodeId) {
        let doc = self.doc;
        for (i, option) in doc.children(group).enumerate() {
            if i > 0 {
                self.emit(", ");
            }
            self.emit("#raw(\"");
            self.emit_string_body(doc.node_text(option), false);
            for argument in doc.children(option) {
                let delimiter = doc.node(argument).value;
                self.byte(if delimiter == i32::from(b'=') { b'=' } else { b' ' });
                self.emit_string_body(doc.node_text(argument), false);
            }
            self.emit("\")");
        }
    }

    /// Writes lines separated by forced breaks; a nested line block is indented one
    /// step further.
    pub(crate) fn render_line_block(&mut self, id: NodeId) {
        if !self.enter(id) {
            return;
        }
        self.emit("#gd-lines[\n");
        let mut previous_line = false;
        for child in self.doc.children(id) {
            self.map_node(child);
            if self.doc.kind(child) == NodeKind::LineBlock {
                if previous_line {
                    self.byte(b'\n');
                }
                self.emit("#gd-indent[");
                self.render_line_block(child);
                self.emit("]\n");
                previous_line = false;
                continue;
            }
            if previous_line {
                self.emit(" \\\n");
            }
            self.emit_anchor(child);
            if self.doc.has_children(child) {
                self.render_inlines(child);
            } else {
                self.byte(b'~');
            }
            previous_line = true;
        }
        if previous_line {
            self.byte(b'\n');
        }
        self.byte(b']');
        self.leave();
    }
}
