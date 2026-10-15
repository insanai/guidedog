//! The document and page set rules of a standalone file: title, language, heading
//! numbering, and page furniture.

use guidedoc::{AttributeKey, NodeId, SourceSpan, arg_text};

use crate::messages::LANGUAGE_INVALID;
use crate::state::{Scan, State};

impl State<'_, '_> {
    /// Writes the document and page set rules.
    pub(crate) fn emit_settings(&mut self, scan: &Scan) {
        self.emit("#set document(title: ");
        self.emit_title(scan);
        self.emit(")\n#set text(");
        self.emit_language();
        self.emit(", font: (\"Libertinus Serif\", \"New Computer Modern\"), size: 11pt)\n");
        self.emit("#set par(justify: true)\n");
        self.emit("#set page(paper: \"a4\", margin: (x: 2.2cm, y: 2.5cm), numbering: \"1\")\n");
        self.emit_numbering(scan);
        self.emit_furniture("header", scan.header);
        self.emit_furniture("footer", scan.footer);
        self.byte(b'\n');
    }

    /// Writes the title string: the caller's override, a title directive, the document
    /// title, or the first heading, in that order; `none` without any.
    fn emit_title(&mut self, scan: &Scan) {
        if !self.options.title.is_empty() {
            self.emit_string(self.options.title);
            return;
        }
        let directive_title =
            scan.title_directive.and_then(|d| self.doc.attribute(d, AttributeKey::Title));
        if let Some(title) = directive_title {
            self.emit_string(title);
        } else if let Some(node) = scan.title.or(scan.heading) {
            self.emit_plain_string(node);
        } else {
            self.emit("none");
        }
    }

    /// Writes `lang` (and `region`) from a BCP 47 tag such as "pt-BR".
    fn emit_language(&mut self) {
        let given = self.options.language;
        let tag = if given.is_empty() { "en" } else { given }.as_bytes();
        let n = tag.iter().take_while(|c| c.is_ascii_alphabetic()).count();
        if !(2..=3).contains(&n) || (n < tag.len() && tag[n] != b'-' && tag[n] != b'_') {
            self.fail(LANGUAGE_INVALID, SourceSpan::default(), arg_text(given));
            self.emit("lang: \"en\"");
            return;
        }
        self.emit("lang: \"");
        for &c in &tag[..n] {
            self.byte(c.to_ascii_lowercase());
        }
        self.byte(b'"');
        let rest = tag.get(n + 1..).unwrap_or(&[]);
        let region = rest.len() >= 2
            && rest[0].is_ascii_alphabetic()
            && rest[1].is_ascii_alphabetic()
            && (rest.len() == 2 || rest[2] == b'-' || rest[2] == b'_');
        if region {
            self.emit(", region: \"");
            self.byte(rest[0].to_ascii_uppercase());
            self.byte(rest[1].to_ascii_uppercase());
            self.byte(b'"');
        }
    }

    /// Numbers headings for `options.section_numbers` or a sectnum directive, with the
    /// directive's depth, prefix, suffix, and start.
    fn emit_numbering(&mut self, scan: &Scan) {
        if !self.options.section_numbers && scan.sectnum.is_none() {
            return;
        }
        let (mut depth, mut start) = (99, 1);
        let (mut prefix, mut suffix) = ("", "");
        if let Some(id) = scan.sectnum {
            if let Some(n) = parse_count(self.custom(id, "depth")).filter(|&n| n > 0) {
                depth = n;
            }
            if let Some(n) = parse_count(self.custom(id, "start")) {
                start = n;
            }
            prefix = self.custom(id, "prefix");
            suffix = self.custom(id, "suffix");
        }
        self.emit("#set heading(numbering: gd-sectnum(");
        self.int(depth);
        self.emit(", ");
        self.emit_string(prefix);
        self.emit(", ");
        self.emit_string(suffix);
        self.emit(", ");
        self.int(start);
        self.emit("))\n");
    }

    /// Renders a header or footer directive's blocks as page furniture.
    fn emit_furniture(&mut self, which: &str, id: Option<NodeId>) {
        let Some(id) = id else {
            return;
        };
        self.map_node(id);
        self.emit("#set page(");
        self.emit(which);
        self.emit(": [\n#set text(size: 0.85em)\n");
        self.quiet = true;
        self.render_blocks(id);
        self.quiet = false;
        self.emit("])\n");
    }
}

/// Reads a small non-negative decimal integer (at most six digits).
pub(crate) fn parse_count(t: &str) -> Option<i64> {
    if t.is_empty() || t.len() > 6 || !t.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    t.parse().ok()
}
