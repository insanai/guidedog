//! Inline content as Sphinx's text builder writes it: emphasis as `*x*`, strong as
//! `**x**`, literals in double quotes, a hard break as a line break, links as their text.

use std::fmt::Write as _;

use guidedoc::{
    Arg, AttributeKey, LinkKind, MediaKind, NodeFlag, NodeId, NodeKind, arg_int, arg_span,
    media_kind,
};

use crate::blocks::Context;
use crate::state::State;
use crate::{EXPANSION, UNRESOLVED};

/// The markup Sphinx's text builder gives a role's content.
const ROLE_STYLES: [(&str, &str, &str); 14] = [
    ("command", "**", "**"),
    ("program", "**", "**"),
    ("makevar", "**", "**"),
    ("dfn", "*", "*"),
    ("mimetype", "*", "*"),
    ("newsgroup", "*", "*"),
    ("mailheader", "*", "*"),
    ("manpage", "*", "*"),
    ("file", "\"", "\""),
    ("samp", "\"", "\""),
    ("regexp", "\"", "\""),
    ("kbd", "", ""),
    ("guilabel", "", ""),
    ("menuselection", "", ""),
];

/// The footnote symbols of Docutils, in order.
const SYMBOLS: [&str; 10] = ["*", "†", "‡", "§", "¶", "#", "♠", "♥", "♦", "♣"];

impl State<'_, '_> {
    pub fn render_inlines(&mut self, parent: NodeId) {
        for child in self.doc.children(parent) {
            self.render_inline(child);
        }
    }

    /// Writes one inline node (see `render_node`).
    pub fn render_inline(&mut self, id: NodeId) {
        self.render_node(id, Context::Inline);
    }

    /// Writes a code span in double quotes.
    pub fn render_code(&mut self, id: NodeId) {
        self.put_char('"');
        self.write_text(self.doc.node_text(id));
        self.put_char('"');
    }

    pub fn render_raw_inline(&mut self, id: NodeId) {
        if self.raw_allowed(id) {
            self.put(self.doc.node_text(id));
        }
    }

    pub fn render_note_reference(&mut self, id: NodeId) {
        self.put_char('[');
        self.write_note_label(id);
        self.put_char(']');
    }

    pub fn wrap(&mut self, open: &str, id: NodeId, close: &str) {
        self.put(open);
        self.render_inlines(id);
        self.put(close);
    }

    /// Writes text with line breaks and tabs as spaces; wrapping decides lines.
    pub fn write_text(&mut self, text: &str) {
        for c in text.chars() {
            self.put_char(if matches!(c, '\n' | '\t' | '\r') { ' ' } else { c });
        }
    }

    pub fn render_link(&mut self, id: NodeId) {
        let unresolved = self.doc.link_info(id).is_none_or(|l| l.kind == LinkKind::Unresolved);
        if unresolved {
            let span = self.doc.node(id).span;
            self.fail(UNRESOLVED, id, arg_span(span), Arg::None);
        }
        self.render_inlines(id);
    }

    pub fn render_abbreviation(&mut self, id: NodeId) {
        self.render_inlines(id);
        if let Some(title) = self.doc.attribute(id, AttributeKey::Title).filter(|t| !t.is_empty())
        {
            self.put(" (");
            self.write_text(title);
            self.put_char(')');
        }
    }

    pub fn render_role(&mut self, id: NodeId) {
        let name = self.doc.node_text(id);
        match ROLE_STYLES.iter().find(|r| r.0 == name) {
            Some(&(_, open, close)) => self.wrap(open, id, close),
            None => self.render_inlines(id),
        }
    }

    pub fn parameters_in(&mut self, open: &str, id: NodeId, close: &str) {
        self.put(open);
        self.render_parameters(id, false);
        self.put(close);
    }

    /// Writes signature parameters separated by commas; an optional group is
    /// bracketed, as in f(a[, b]). It returns whether a parameter was written.
    fn render_parameters(&mut self, id: NodeId, follows: bool) -> bool {
        let mut after = follows;
        for child in self.doc.children(id) {
            if self.doc.kind(child) == NodeKind::DescOptional {
                self.put_char('[');
                after = self.render_parameters(child, after);
                self.put_char(']');
                continue;
            }
            if after {
                self.put(", ");
            }
            self.render_inline(child);
            after = true;
        }
        after
    }

    /// Writes the resolved definition's inlines in place.
    pub fn render_substitution(&mut self, id: NodeId) {
        let doc = self.doc;
        let span = doc.node(id).span;
        let definition = doc.link_info(id).filter(|l| {
            l.kind == LinkKind::Internal
                && l.resolved.index() < doc.node_count()
                && doc.kind(l.resolved) == NodeKind::SubstitutionDefinition
        });
        let Some(l) = definition else {
            self.fail(UNRESOLVED, id, arg_span(span), Arg::None);
            return;
        };
        let limit = if self.limits.max_expansion > 0 { self.limits.max_expansion } else { 16 };
        if self.expansion >= limit {
            let max = arg_int(i64::try_from(limit).unwrap_or(i64::MAX));
            self.fail(EXPANSION, id, arg_span(span), max);
            return;
        }
        self.expansion += 1;
        self.render_inlines(l.resolved);
        self.expansion -= 1;
    }

    /// Writes a note's number, symbol, or label.
    pub fn write_note_label(&mut self, id: NodeId) {
        let node = self.doc.node(id);
        let citation = matches!(node.kind, NodeKind::Citation | NodeKind::CitationReference);
        if node.flags.has(NodeFlag::Symbol) {
            let index = usize::try_from(node.value.saturating_sub(1)).unwrap_or(0);
            for _ in 0..=(index / SYMBOLS.len()).min(7) {
                self.put(SYMBOLS[index % SYMBOLS.len()]);
            }
        } else if node.value > 0 && !citation {
            let _ = write!(self.out, "{}", node.value);
        } else {
            self.put(self.doc.text(node.text));
        }
    }

    /// Stands for an image: "[image: alt]", or "[image]" without alt text; video and
    /// audio files say "[video: ...]" and "[audio: ...]".
    pub fn image_text(&mut self, id: NodeId) {
        let doc = self.doc;
        let dest = doc.link_info(id).map_or("", |l| doc.text(l.destination));
        let word = match media_kind(dest) {
            MediaKind::Video => "video",
            MediaKind::Audio => "audio",
            MediaKind::Image => "image",
        };
        let alt = doc.attribute(id, AttributeKey::Alt);
        self.put_char('[');
        self.put(word);
        match alt {
            None if doc.has_children(id) => {
                self.put(": ");
                self.render_inlines(id);
            }
            Some(alt) if !alt.is_empty() => {
                self.put(": ");
                self.put(alt);
            }
            _ => {}
        }
        self.put_char(']');
    }
}
