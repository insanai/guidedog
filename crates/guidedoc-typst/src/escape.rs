//! Encoders that keep document text inert in Typst source. Markup text escapes every
//! ASCII punctuation character; code, URLs, labels, and paths are string literals whose
//! encoder escapes the quote, the backslash, and every control character.

use guidedoc::{NodeId, NodeKind, TextSink};

use crate::state::{Mark, State};

/// Writes `text` as inert Typst markup. Every ASCII punctuation character is escaped
/// with a backslash, which Typst accepts for all of them, so no markup (`#`, `$`, `*`,
/// `_`, `` ` ``, `<`, `@`, `[`, `=`, `-`, `+`, `/`, `~`, quotes, `//` comments, list and
/// heading markers, shorthands such as `--` or `...`, URL autolinks) can arise from text.
/// Line breaks are white space; control characters are dropped. It never fails.
pub fn write_markup_text(out: &mut (impl TextSink + ?Sized), text: &str) {
    write_markup(out, text, None);
}

/// Writes `text` as markup; with `pre` (the current column), spaces become non-breaking,
/// tabs expand to the next multiple of eight, and line breaks are kept.
pub(crate) fn write_markup(
    out: &mut (impl TextSink + ?Sized),
    text: &str,
    mut pre: Option<&mut usize>,
) {
    let bytes = text.as_bytes();
    let mut start = 0;
    for (i, &c) in bytes.iter().enumerate() {
        let plain = c >= 0x80 || c.is_ascii_alphanumeric() || (c == b' ' && pre.is_none());
        if plain {
            // A UTF-8 lead byte or an ASCII character starts one column.
            if let Some(column) = pre.as_deref_mut()
                && !(0x80..0xc0).contains(&c)
            {
                *column += 1;
            }
            continue;
        }
        out.put_str(&text[start..i]);
        start = i + 1;
        write_special(out, c, pre.as_deref_mut());
    }
    out.put_str(&text[start..]);
}

/// Writes one ASCII byte that is not a letter or digit (or a space in pre mode).
fn write_special(out: &mut (impl TextSink + ?Sized), c: u8, pre: Option<&mut usize>) {
    match (c, pre) {
        (b' ', Some(column)) => {
            out.put_char('~');
            *column += 1;
        }
        (b'\n', Some(column)) => {
            out.put_str("\\\n");
            *column = 0;
        }
        (b'\n', None) | (b'\t', None) => out.put_char(' '),
        (b'\t', Some(column)) => loop {
            out.put_char('~');
            *column += 1;
            if (*column).is_multiple_of(8) {
                break;
            }
        },
        // Control characters have no printable form; carriage returns end lines.
        (c, _) if c < 0x20 || c == 0x7f => {}
        (c, pre) => {
            out.put_char('\\');
            out.put_char(char::from(c));
            if let Some(column) = pre {
                *column += 1;
            }
        }
    }
}

/// Writes `s` as a Typst string literal, quotes included. It never fails.
pub fn write_string_literal(out: &mut (impl TextSink + ?Sized), s: &str) {
    out.put_char('"');
    write_string_body(out, s, false);
    out.put_char('"');
}

/// Writes the inside of a string literal: backslash, quote, and every control character
/// are escaped, so the literal always ends where the encoder says. With `spaces`, line
/// breaks and tabs become spaces (titles, alt text, text in math).
pub(crate) fn write_string_body(out: &mut (impl TextSink + ?Sized), s: &str, spaces: bool) {
    let mut start = 0;
    for (i, &c) in s.as_bytes().iter().enumerate() {
        if c >= 0x20 && c != b'"' && c != b'\\' && c != 0x7f {
            continue;
        }
        out.put_str(&s[start..i]);
        start = i + 1;
        match c {
            b'"' => out.put_str("\\\""),
            b'\\' => out.put_str("\\\\"),
            b'\n' | b'\t' | b'\r' if spaces => out.put_char(' '),
            b'\n' => out.put_str("\\n"),
            b'\t' => out.put_str("\\t"),
            b'\r' => out.put_str("\\r"),
            _ => {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                out.put_str("\\u{");
                out.put_char(char::from(HEX[usize::from(c >> 4)]));
                out.put_char(char::from(HEX[usize::from(c & 15)]));
                out.put_char('}');
            }
        }
    }
    out.put_str(&s[start..]);
}

/// Tells whether `name` can be written as a Typst label literal `<name>`: non-empty,
/// made of letters, digits, non-ASCII characters, and `-`, `_`, `.`, `:`.
pub fn is_label_safe(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|c| {
            c >= 0x80 || c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.' | b':')
        })
}

impl State<'_, '_> {
    /// Writes document text as markup, in the current preformatted mode.
    pub(crate) fn emit_text(&mut self, text: &str) {
        let pre = if self.pre { Some(&mut self.column) } else { None };
        write_markup(&mut self.ctx.out, text, pre);
    }

    /// Writes `s` as a string literal.
    pub(crate) fn emit_string(&mut self, s: &str) {
        write_string_literal(&mut self.ctx.out, s);
    }

    /// Writes the inside of a string literal.
    pub(crate) fn emit_string_body(&mut self, s: &str, spaces: bool) {
        write_string_body(&mut self.ctx.out, s, spaces);
    }

    /// Writes the text of a subtree as one string literal, for the document title and
    /// alt text. Line breaks become spaces.
    pub(crate) fn emit_plain_string(&mut self, id: NodeId) {
        let doc = self.doc;
        self.byte(b'"');
        for i in id.0..doc.subtree_end(id).0 {
            let node = NodeId(i);
            let kind = doc.kind(node);
            if matches!(kind, NodeKind::Text | NodeKind::Code | NodeKind::Math) {
                self.emit_string_body(doc.node_text(node), true);
            } else if matches!(kind, NodeKind::SoftBreak | NodeKind::HardBreak) {
                self.byte(b' ');
            }
        }
        self.byte(b'"');
    }

    /// Attaches the node's anchor as a label to the element just written, once. Page
    /// furniture, repeated on every page, and heading content, repeated in the outline,
    /// get none: Typst rejects a label that occurs twice. Returns whether it wrote one.
    pub(crate) fn emit_label(&mut self, id: NodeId) -> bool {
        if self.quiet || self.copied || self.has_mark(id, Mark::Labelled) {
            return false;
        }
        let Some(name) = self.anchors.anchor_of(id).filter(|n| is_label_safe(n)) else {
            return false;
        };
        let out = &mut self.ctx.out;
        out.emit_byte(b'<');
        out.emit(name);
        out.emit_byte(b'>');
        self.set_mark(id, Mark::Labelled);
        self.emit_aliases(id);
        true
    }

    /// Marks a position with the node's label through an invisible metadata element,
    /// for nodes whose Typst form cannot carry a label itself.
    pub(crate) fn emit_anchor(&mut self, id: NodeId) {
        if self.quiet || self.copied || self.has_mark(id, Mark::Labelled) {
            return;
        }
        if self.anchors.anchor_of(id).is_some_and(is_label_safe) {
            self.emit("#metadata(none)");
            self.emit_label(id);
        }
    }

    /// Writes a node's ids after its primary one as labelled metadata.
    fn emit_aliases(&mut self, id: NodeId) {
        let out = &mut self.ctx.out;
        for name in self.anchors.anchor_ids(id).skip(1).filter(|n| is_label_safe(n)) {
            out.emit("#metadata(none)<");
            out.emit(name);
            out.emit_byte(b'>');
        }
    }
}
