//! Bullet and enumerated lists, definition lists, field lists, option lists, and line
//! blocks.

use guidedoc::{ListInfo, ListStyle, NodeFlag, NodeId, NodeKind};

use crate::state::Writer;

/// Returns the marker of item `n` (from 0) of a list: "- " or "3. ".
pub(crate) fn marker(info: ListInfo, n: usize) -> String {
    if info.style == ListStyle::Bullet {
        let bullet = match info.delimiter {
            b'*' | b'+' | b'-' => char::from(info.delimiter),
            _ => '•',
        };
        return format!("{bullet} ");
    }
    let number = i64::from(info.start) + i64::try_from(n).unwrap_or(0);
    let label = enumerator(info.style, number);
    match info.delimiter {
        b')' => format!("{label}) "),
        b'(' => format!("({label}) "),
        _ => format!("{label}. "),
    }
}

/// The enumerator of `n` in a style.
fn enumerator(style: ListStyle, n: i64) -> String {
    match style {
        ListStyle::Bullet | ListStyle::Arabic => n.to_string(),
        ListStyle::LowerAlpha => alpha(n).to_ascii_lowercase(),
        ListStyle::UpperAlpha => alpha(n),
        ListStyle::LowerRoman => roman(n).to_ascii_lowercase(),
        ListStyle::UpperRoman => roman(n),
    }
}

fn alpha(n: i64) -> String {
    let n = u8::try_from(n.clamp(1, 26)).unwrap_or(1);
    char::from(b'A' + n - 1).to_string()
}

fn roman(n: i64) -> String {
    const NUMERALS: [(i64, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut n = n.clamp(1, 4999);
    let mut out = String::new();
    for (value, numeral) in NUMERALS {
        while n >= value {
            out.push_str(numeral);
            n -= value;
        }
    }
    out
}

impl Writer<'_, '_> {
    /// Tells whether a list written after `prev` would continue it.
    pub fn lists_merge(&self, prev: NodeId, next: NodeId) -> bool {
        let (a, b) = (self.doc.list_info(prev), self.doc.list_info(next));
        if a.style != b.style || a.delimiter != b.delimiter {
            return false;
        }
        if a.style == ListStyle::Bullet {
            return true;
        }
        let count = i32::try_from(self.doc.child_count(prev)).unwrap_or(i32::MAX);
        a.start.saturating_add(count) == b.start
    }

    /// Tells whether a list item holds at most one paragraph.
    fn is_simple_item(&self, item: NodeId) -> bool {
        let mut children = self.doc.children(item);
        match children.next() {
            None => true,
            Some(c) => self.kind(c) == NodeKind::Paragraph && children.next().is_none(),
        }
    }

    /// A bullet or enumerated list; a tight list's simple items need no blank lines.
    pub fn list(&mut self, id: NodeId) {
        self.separate(Some(id));
        self.class_prefix(id, &[]);
        let info = self.doc.list_info(id);
        let tight = self.doc.node(id).flags.has(NodeFlag::Tight);
        let mut previous_simple = true;
        for (n, item) in self.doc.children(id).enumerate() {
            if n == 0 {
                self.block_start();
            } else {
                self.blank = !(tight && previous_simple);
                self.block_start();
            }
            let marker = marker(info, n);
            self.push_frame(marker.chars().count(), &marker);
            self.blocks(item);
            self.pop_frame();
            previous_simple = self.is_simple_item(item);
        }
        if self.doc.child_count(id) == 0 {
            self.lossy(id, "an empty list", "nothing");
        }
        self.block_end();
    }

    /// A definition list: each term (with its classifiers) on a line, the definition
    /// indented below it.
    pub fn definition_list(&mut self, id: NodeId) {
        self.separate(Some(id));
        self.class_prefix(id, &[]);
        for item in self.doc.children(id) {
            self.definition_item(item, false);
        }
        self.block_end();
    }

    /// One item; a glossary writes several terms of one item on consecutive lines.
    pub fn definition_item(&mut self, item: NodeId, glossary: bool) {
        let mut term_line = String::new();
        let mut definitions = 0;
        self.block_start();
        for part in self.doc.children(item) {
            match self.kind(part) {
                NodeKind::Term => {
                    if !term_line.is_empty() {
                        self.term_line(&term_line, item, glossary);
                    }
                    term_line = guard_classifier(&self.inline_line(part));
                }
                NodeKind::Classifier => {
                    term_line.push_str(" : ");
                    term_line.push_str(&self.inline_line(part));
                }
                _ => {
                    if !term_line.is_empty() {
                        self.line(&std::mem::take(&mut term_line));
                    }
                    definitions += 1;
                    if definitions > 1 {
                        self.lossy(part, "a second definition of a term", "one definition");
                        self.blank = true;
                    }
                    self.push_frame(3, "");
                    self.blocks(part);
                    self.pop_frame();
                }
            }
        }
        if !term_line.is_empty() {
            self.line(&term_line);
            if definitions == 0 {
                self.lossy(item, "a term without a definition", "the term alone");
            }
        }
        self.block_end();
    }

    fn term_line(&mut self, line: &str, item: NodeId, glossary: bool) {
        if !glossary {
            self.lossy(item, "several terms of one definition", "each term on its line");
        }
        self.line(line);
    }

    /// A field list: `:name: body`, the body indented under the name.
    pub fn field_list(&mut self, id: NodeId) {
        self.separate(Some(id));
        self.class_prefix(id, &[]);
        let compact = self.doc.children(id).all(|f| self.is_simple_field(f));
        for (n, field) in self.doc.children(id).enumerate() {
            if n > 0 {
                self.blank = !compact;
            }
            if self.custom(field, "docfield").is_some() {
                self.doc_field(field);
            } else {
                self.field(field);
            }
        }
        if self.doc.children(id).any(|f| self.custom(f, "docinfo").is_some()) {
            self.docinfo_topics(id, compact);
        }
        self.block_end();
    }

    /// The dedication and abstract the reader made topics of, written back as the
    /// bibliographic fields they were.
    fn docinfo_topics(&mut self, list: NodeId, compact: bool) {
        let mut next = self.doc.subtree_end(list);
        while next.index() < self.doc.node_count() && self.kind(next) == NodeKind::Topic {
            let topic = next;
            next = self.doc.subtree_end(topic);
            let special =
                self.doc.has_class(topic, "dedication") || self.doc.has_class(topic, "abstract");
            let Some(title) = self.doc.first_child(topic).filter(|_| special) else { break };
            let name = self.inline_line(title);
            self.blank = !compact;
            self.block_start();
            self.push_frame(3, &format!(":{name}: "));
            let saved = self.prev_block.take();
            for block in self.doc.children(topic).skip(1) {
                self.node(block);
                self.prev_block = Some(block);
            }
            self.prev_block = saved;
            self.pop_frame();
            self.done.push(topic);
        }
    }

    fn is_simple_field(&self, field: NodeId) -> bool {
        self.doc
            .children(field)
            .filter(|&c| self.kind(c) == NodeKind::FieldBody)
            .all(|body| self.is_simple_item(body))
    }

    /// One field with the name `name` (escaped) and the blocks of `body`.
    pub fn field_with(&mut self, name: &str, body: Option<NodeId>) {
        self.block_start();
        let marker = format!(":{name}: ");
        let first = body.and_then(|b| self.doc.first_child(b));
        let same_line = first.is_some_and(|f| self.kind(f) == NodeKind::Paragraph);
        self.push_frame(3, &marker);
        if !same_line {
            self.detach_marker();
        }
        if let Some(body) = body {
            self.blocks(body);
        }
        self.pop_frame();
        self.block_end();
    }

    fn field(&mut self, field: NodeId) {
        let mut name = String::new();
        let mut body = None;
        for part in self.doc.children(field) {
            match self.kind(part) {
                NodeKind::FieldName => name = self.field_name_line(part),
                _ => body = Some(part),
            }
        }
        let address = body
            .and_then(|b| self.doc.first_child(b))
            .filter(|&p| self.kind(p) == NodeKind::Preformatted)
            .filter(|_| self.custom(field, "docinfo") == Some("address"));
        if let Some(lines) = address {
            // The address field keeps its line breaks, written as they are.
            self.block_start();
            self.push_frame(3, &format!(":{name}: "));
            for line in self.collect(lines, true, |_| {}) {
                self.line(&line);
            }
            self.pop_frame();
            self.block_end();
            return;
        }
        self.field_with(&name, body);
    }

    /// An option list: the options of an item, then its description.
    pub fn option_list(&mut self, id: NodeId) {
        self.separate(Some(id));
        self.class_prefix(id, &[]);
        let compact = self.doc.children(id).all(|item| {
            self.doc
                .children(item)
                .filter(|&c| self.kind(c) == NodeKind::OptionDescription)
                .all(|d| self.is_simple_item(d))
        });
        for (n, item) in self.doc.children(id).enumerate() {
            if n > 0 {
                self.blank = !compact;
            }
            self.option_item(item);
        }
        self.block_end();
    }

    fn option_item(&mut self, item: NodeId) {
        let mut options = String::new();
        let mut description = None;
        for part in self.doc.children(item) {
            if self.kind(part) == NodeKind::OptionGroup {
                options = self.option_group(part);
            } else {
                description = Some(part);
            }
        }
        self.block_start();
        let marker = format!("{options}  ");
        self.push_frame(4, &marker);
        if let Some(d) = description {
            let first = self.doc.first_child(d);
            if !first.is_some_and(|f| self.kind(f) == NodeKind::Paragraph) {
                self.lossy(item, "an option whose description does not start with text", "it");
            }
            self.blocks(d);
        }
        self.pop_frame();
        self.block_end();
    }

    fn option_group(&self, group: NodeId) -> String {
        let mut out = String::new();
        for option in self.doc.children(group) {
            if !out.is_empty() {
                out.push_str(", ");
            }
            out.push_str(self.doc.node_text(option));
            for argument in self.doc.children(option) {
                let delimiter = u8::try_from(self.doc.node(argument).value).unwrap_or(b' ');
                out.push(if delimiter == b'=' { '=' } else { ' ' });
                out.push_str(self.doc.node_text(argument));
            }
        }
        out
    }

    /// A line block: "| " before each line, nested blocks indented after the bar.
    pub fn line_block(&mut self, id: NodeId) {
        self.separate(Some(id));
        self.class_prefix(id, &[]);
        self.block_start();
        self.line_block_lines(id, 0);
        self.block_end();
    }

    fn line_block_lines(&mut self, id: NodeId, depth: usize) {
        for child in self.doc.children(id) {
            if self.kind(child) == NodeKind::LineBlock {
                if !self.enter(child) {
                    return;
                }
                self.line_block_lines(child, depth + 4);
                self.leave();
                continue;
            }
            let marker = format!("| {}", " ".repeat(depth));
            self.push_frame(marker.len(), &marker);
            // Long lines continue on indented continuation lines.
            let lines = self.collect(child, false, |_| {});
            for line in &lines {
                self.line(line);
            }
            self.pop_frame();
        }
    }
}

impl Writer<'_, '_> {
    /// A field name's inline content on one line, its colons escaped.
    fn field_name_line(&mut self, id: NodeId) -> String {
        let saved = std::mem::take(&mut self.inline);
        let saved_mode = self.in_inline;
        self.inline.clear(false);
        self.inline.colons = true;
        self.in_inline = true;
        self.inline_children(id);
        self.in_inline = saved_mode;
        let line = std::mem::replace(&mut self.inline, saved).buf;
        line.trim_end().to_owned()
    }
}

/// Escapes " : " in a term, which would start a classifier.
fn guard_classifier(term: &str) -> String {
    term.replace(" : ", " \\: ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers() {
        let info = |style, delimiter, start| ListInfo { style, delimiter, start };
        assert_eq!(marker(info(ListStyle::Bullet, b'*', 0), 3), "* ");
        assert_eq!(marker(info(ListStyle::Arabic, b'.', 3), 1), "4. ");
        assert_eq!(marker(info(ListStyle::LowerAlpha, b')', 1), 1), "b) ");
        assert_eq!(marker(info(ListStyle::UpperRoman, b'(', 1), 3), "(IV) ");
    }
}
