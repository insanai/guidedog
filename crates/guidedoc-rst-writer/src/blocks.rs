//! Shared block machinery (inline collection, directive heads and bodies, classes,
//! separators) and the plain blocks: paragraphs, rubrics, parsed literals, block
//! quotes, and transitions.

use guidedoc::{AttributeKey, NodeId, NodeKind};

use crate::inline::Lines;
use crate::state::Writer;

/// Classes the Markdown reader adds to mirror myst-parser's HTML, which a
/// reStructuredText source does not write.
const MARKDOWN_CLASSES: &[&str] = &[
    "myst",
    "contains-task-list",
    "task-list-item",
    "block_break",
    "footnotes",
    "amsmath",
    "docinfo",
];

/// A directive option: its name and value ("" for a flag).
pub(crate) type Opt = (&'static str, String);

impl Writer<'_, '_> {
    /// Collects the inline children of `id` and wraps them into lines; `preserve` keeps
    /// white space and line breaks. `finish` may adjust the buffer before wrapping.
    pub fn collect(
        &mut self,
        id: NodeId,
        preserve: bool,
        finish: impl FnOnce(&mut crate::inline::Inline),
    ) -> Lines {
        let saved = self.begin_inline(preserve);
        let saved_mode = self.in_inline;
        self.in_inline = true;
        self.inline_children(id);
        self.in_inline = saved_mode;
        finish(&mut self.inline);
        let lines = self.inline_lines();
        let used = self.end_inline(saved);
        self.give_inline(used);
        lines
    }

    /// Breaks the inline buffer into lines: at the source's soft line breaks, or
    /// rewrapped at the width when the settings (or a table cell) ask for it.
    pub fn inline_lines(&mut self) -> Lines {
        let mut lines = self.take_lines();
        if !self.reflow {
            self.inline.keep_lines(&mut lines);
            return lines;
        }
        let first = self.width.saturating_sub(self.first_indentation()).max(self.width / 3);
        let (rest, tight) = (self.available(), self.tight_wrap);
        self.inline.wrap(first, rest, tight, &mut lines);
        lines
    }

    /// Collects the inline children of `id` as one line (for titles, directive
    /// arguments, and terms). Give the line back with `recycle` when done.
    pub fn inline_line(&mut self, id: NodeId) -> String {
        let saved = self.begin_inline(false);
        let saved_mode = self.in_inline;
        self.in_inline = true;
        self.inline_children(id);
        self.in_inline = saved_mode;
        let mut used = self.end_inline(saved);
        let spare = self.buffer();
        let mut line = std::mem::replace(&mut used.buf, spare);
        self.give_inline(used);
        let end = line.trim_end().len();
        line.truncate(end);
        line
    }

    /// Writes lines as one block.
    pub fn write_block_lines(&mut self, lines: &[String]) {
        self.block_start();
        for line in lines {
            self.line(line);
        }
        self.block_end();
    }

    /// Returns the class attribute of `id` without the classes the Markdown reader
    /// adds and those in `implied` (which the construct's directive adds itself).
    pub fn classes(&self, id: NodeId, implied: &[&str]) -> String {
        let mut out = String::new();
        let doc = self.doc;
        for a in doc.attributes(id).iter().filter(|a| a.key == AttributeKey::Class) {
            let value = doc.text(a.value);
            let myst = value.split(' ').any(|c| c == "myst");
            for class in value.split(' ').filter(|c| !c.is_empty()) {
                let dropped = MARKDOWN_CLASSES.contains(&class)
                    || implied.contains(&class)
                    || (myst && class == "simple")
                    || class.starts_with("text-") && doc.kind(id) == NodeKind::Cell;
                if !dropped {
                    if !out.is_empty() {
                        out.push(' ');
                    }
                    out.push_str(class);
                }
            }
        }
        out
    }

    /// Writes the classes of an element that has no `:class:` option with the class
    /// directive before it (`rst-class` for Sphinx).
    pub fn class_prefix(&mut self, id: NodeId, implied: &[&str]) {
        let classes = self.classes(id, implied);
        if classes.is_empty() {
            return;
        }
        let name = if self.sphinx { "rst-class" } else { "class" };
        self.block_start();
        self.line_fmt(format_args!(".. {name}:: {classes}"));
        self.block_end();
    }

    /// Writes a directive's first line and options.
    pub fn directive_head(&mut self, name: &str, argument: &str, options: &[Opt]) {
        self.separate(None);
        self.block_start();
        if argument.is_empty() {
            self.line_fmt(format_args!(".. {name}::"));
        } else {
            self.line_fmt(format_args!(".. {name}:: {argument}"));
        }
        self.push_frame(3, "");
        for (key, value) in options {
            if value.is_empty() {
                self.line_fmt(format_args!(":{key}:"));
            } else {
                let value = OneLine(value);
                self.line_fmt(format_args!(":{key}: {value}"));
            }
        }
        self.pop_frame();
        self.block_end();
    }

    /// Writes children of `id` from `first` on as a directive's content.
    pub fn directive_body(&mut self, id: NodeId, skip: usize) {
        self.push_frame(3, "");
        let saved = self.prev_block;
        let saved_end = self.container_end;
        self.prev_block = None;
        self.container_end = self.doc.subtree_end(id).0;
        for child in self.doc.children(id).skip(skip) {
            self.node(child);
            self.prev_block = Some(child);
        }
        self.prev_block = saved;
        self.container_end = saved_end;
        self.pop_frame();
        self.block_end();
    }

    /// The common options of a directive's node: `:class:` and `:name:`.
    pub fn common_options(&self, id: NodeId, implied: &[&str], options: &mut Vec<Opt>) {
        let classes = self.classes(id, implied);
        if !classes.is_empty() {
            options.push(("class", classes));
        }
        if let Some(name) = self.attr(id, AttributeKey::Name) {
            options.push(("name", name.to_owned()));
        }
    }

    /// Writes an empty comment when the block about to be written would otherwise
    /// continue the one before it (an indented block after a list or directive, or a
    /// list continuing a list). `next` is the kind about to be written, or `None` for
    /// explicit markup, which never needs one.
    pub fn separate(&mut self, next: Option<NodeId>) {
        let (Some(prev), Some(next)) = (self.prev_block, next) else { return };
        let prev_kind = self.kind(prev);
        let needed = match self.kind(next) {
            NodeKind::BlockQuote => {
                !matches!(
                    prev_kind,
                    NodeKind::Paragraph | NodeKind::ThematicBreak | NodeKind::Title
                ) && !(prev_kind == NodeKind::Comment && self.doc.node_text(prev).is_empty())
            }
            NodeKind::List => prev_kind == NodeKind::List && self.lists_merge(prev, next),
            NodeKind::DefinitionList => prev_kind == NodeKind::DefinitionList,
            NodeKind::FieldList => prev_kind == NodeKind::FieldList,
            NodeKind::OptionList => prev_kind == NodeKind::OptionList,
            _ => false,
        };
        if needed {
            self.block_start();
            self.line("..");
            self.block_end();
        }
    }

    /// Returns the next sibling of `id` in the container being written.
    pub fn next_block(&self, id: NodeId) -> Option<NodeId> {
        let next = self.doc.subtree_end(id);
        (next.0 < self.container_end).then_some(next)
    }

    /// Writes a paragraph: the class directive, then its wrapped inline content. A
    /// paragraph followed by a literal block announces it with "::".
    pub fn paragraph(&mut self, id: NodeId) {
        let classes = self.classes(id, &[]);
        if self.sphinx && classes == "centered" {
            let text = self.inline_line(id);
            self.directive_head("centered", &text, &[]);
            self.recycle(text);
            return;
        }
        if let Some(only) = self.lone_image(id) {
            self.class_prefix(id, &[]);
            match self.image_in(only) {
                Some(image) if image != only => self.image_directive("image", image, Some(only)),
                _ => self.block_image(only),
            }
            return;
        }
        self.class_prefix(id, &[]);
        let announce = self.next_block(id).is_some_and(|n| self.is_literal_block(n));
        let lines = self.collect(id, false, |inline| {
            inline.guard_literal_marker();
            if announce {
                let colon = inline.buf.ends_with(':') && !inline.buf.ends_with("\\:");
                inline.raw(if colon { ":" } else { " ::" });
            }
        });
        self.literal_announced = announce;
        if lines.iter().all(String::is_empty) {
            // An escaped space is a paragraph with no text.
            self.write_block_lines(&["\\ ".to_owned()]);
        } else {
            self.write_block_lines(&lines);
            self.flush_substitutions();
        }
        self.give_lines(lines);
    }

    /// Returns the image a paragraph holds alone (perhaps inside a link), which is
    /// written as an image directive.
    fn lone_image(&self, id: NodeId) -> Option<NodeId> {
        let mut children = self.doc.children(id);
        let only = children.next()?;
        if children.next().is_some() {
            return None;
        }
        self.image_in(only).map(|_| only)
    }

    /// Writes inline content as a block of its own (a caption, term, or line met
    /// outside its parent).
    pub fn plain_block(&mut self, id: NodeId) {
        let lines = self.collect(id, false, |inline| inline.guard_literal_marker());
        self.write_block_lines(&lines);
        self.give_lines(lines);
    }

    /// `.. rubric:: text`.
    pub fn rubric(&mut self, id: NodeId) {
        let text = self.inline_line(id);
        let mut options = Vec::new();
        self.common_options(id, &[], &mut options);
        self.directive_head("rubric", &text, &options);
        self.recycle(text);
    }

    /// `.. parsed-literal::` with its inline content, white space kept.
    pub fn parsed_literal(&mut self, id: NodeId) {
        let mut options = Vec::new();
        self.common_options(id, &[], &mut options);
        self.directive_head("parsed-literal", "", &options);
        self.push_frame(3, "");
        let collected = self.collect(id, true, |_| {});
        let lines = trim_blank_ends(&collected);
        if lines.iter().all(|l| l.is_empty() || l.starts_with(' ')) {
            self.lossy(id, "a parsed literal whose every line is indented", "its lines dedented");
        }
        self.write_block_lines(lines);
        self.give_lines(collected);
        self.pop_frame();
        self.block_end();
    }

    /// A block quote: indented, or an epigraph, highlights, or pull-quote directive. The
    /// attribution follows as "-- text".
    pub fn block_quote(&mut self, id: NodeId) {
        const STYLES: [&str; 3] = ["epigraph", "highlights", "pull-quote"];
        let classes = self.classes(id, &[]);
        if let Some(style) = STYLES.iter().find(|s| classes.split(' ').any(|c| c == **s)) {
            let rest = self.classes(id, &[style]);
            let options = if rest.is_empty() { Vec::new() } else { vec![("class", rest)] };
            self.directive_head(style, "", &options);
            self.quote_body(id);
            return;
        }
        let after_comment = self.prev_block.is_some_and(|p| {
            self.kind(p) == NodeKind::Comment && self.doc.node_text(p).is_empty()
        });
        if !classes.is_empty() && !after_comment {
            // The class directive would take an indented quote for its content: an
            // empty comment between them keeps the quote a quote.
            self.lossy(id, "a block quote with a class", "the class, a comment, and the quote");
            self.class_prefix(id, &[]);
            self.block_start();
            self.line("..");
            self.block_end();
        }
        self.separate(Some(id));
        self.quote_body(id);
    }

    fn quote_body(&mut self, id: NodeId) {
        self.push_frame(4, "");
        let saved = self.prev_block;
        let saved_end = self.container_end;
        self.prev_block = None;
        self.container_end = self.doc.subtree_end(id).0;
        for child in self.doc.children(id) {
            if self.kind(child) == NodeKind::Attribution {
                self.block_start();
                self.push_frame(3, "-- ");
                let lines = self.collect(child, false, |_| {});
                for line in &lines {
                    self.line(line);
                }
                self.give_lines(lines);
                self.pop_frame();
                self.block_end();
            } else {
                self.node(child);
            }
            self.prev_block = Some(child);
        }
        self.prev_block = saved;
        self.container_end = saved_end;
        self.pop_frame();
        self.block_end();
    }

    /// A transition.
    pub fn transition(&mut self, _id: NodeId) {
        self.block_start();
        self.line("----");
        self.block_end();
    }

    /// A hard line break, which reStructuredText paragraphs cannot hold.
    pub fn hard_break(&mut self, id: NodeId) {
        self.lossy(id, "a hard line break", "a soft one; use a line block for verse");
        self.inline.soft_break();
    }
}

/// Drops empty lines at both ends.
pub(crate) fn trim_blank_ends(lines: &[String]) -> &[String] {
    let end = lines.iter().rposition(|l| !l.trim().is_empty()).map_or(0, |i| i + 1);
    let lead = lines[..end].iter().take_while(|l| l.trim().is_empty()).count();
    &lines[lead..end]
}

/// Text written on one line: its line breaks become spaces as it is formatted.
struct OneLine<'s>(&'s str);

impl std::fmt::Display for OneLine<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (k, piece) in self.0.split('\n').enumerate() {
            if k > 0 {
                f.write_str(" ")?;
            }
            f.write_str(piece)?;
        }
        Ok(())
    }
}
