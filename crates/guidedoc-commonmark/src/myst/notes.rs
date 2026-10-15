//! Footnotes (myst-parser 0.16.1): definitions are collected wherever they are written;
//! the referenced ones are emitted at the end of the document in the order of their
//! first reference, after a transition of class "footnotes" (`footnote_transition`).
//! Unreferenced definitions are not shown. A numeric label is a manual number; other
//! labels are numbered automatically. Shapes are reStructuredText's: `[^1]` is `[1]_`,
//! `[^a]` is `[#a]_`.

use guidedoc::{AttributeKey, Fail, LinkInfo, NodeFlags, NodeKind, SourceSpan, normalize_label};

use crate::blocks::{BlockParser, parse_lines};
use crate::extension::Line;
use crate::host::Host;
use crate::messages::FOOTNOTE_DUPLICATE;

use super::Note;

/// A footnote label case-folded, as labels compare.
fn note_key(label: &str) -> String {
    let mut key = String::with_capacity(label.len());
    normalize_label(&mut key, label);
    key
}

fn numeric_label(label: &str) -> bool {
    !label.is_empty() && label.bytes().all(|c| c.is_ascii_digit())
}

/// The text of a footnote or its reference: the label, with "#" before an auto label.
fn label_text(label: &str) -> String {
    if numeric_label(label) { label.to_owned() } else { format!("#{label}") }
}

impl BlockParser<'_, '_, '_, '_> {
    /// Registers a definition and returns its index; `None` for a duplicate label.
    pub(crate) fn add_note(&mut self, label: &str, span: SourceSpan) -> Option<usize> {
        let key = note_key(label);
        let ms = &mut self.h.ms;
        if ms.note_names.contains_key(&key) {
            self.h.report_span(FOOTNOTE_DUPLICATE, span);
            return None;
        }
        let index = ms.notes.len();
        ms.note_names.insert(&key, index);
        ms.notes.push(Note {
            label: label.to_owned(),
            source: self.sk.source,
            lines: Vec::new(),
            span,
            order: 0,
        });
        Some(index)
    }
}

/// Records the first reference to a footnote label.
fn note_referenced(h: &mut Host<'_, '_>, label: &str) {
    let ms = &mut h.ms;
    let Some(&i) = ms.note_names.get(&note_key(label)) else { return };
    if ms.notes[i].order == 0 {
        ms.ordered.push(i);
        ms.notes[i].order = ms.ordered.len();
    }
}

/// Emits `[^label]` as a FootnoteReference.
pub(crate) fn emit_footnote_reference(
    h: &mut Host<'_, '_>,
    label: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    note_referenced(h, label);
    let name = h.normalized_name(label)?;
    let b = h.b();
    let id = b.add_text_leaf(NodeKind::FootnoteReference, span, &label_text(label))?;
    if !numeric_label(label) {
        b.add_flags(id, NodeFlags::AUTO);
    }
    let info = LinkInfo { name, ..LinkInfo::UNRESOLVED };
    b.set_link(id, info)?;
    Ok(())
}

/// Emits the referenced footnotes at the end of the document.
pub(crate) fn emit_notes(h: &mut Host<'_, '_>) -> Result<(), Fail> {
    let Some(&first) = h.ms.ordered.first() else { return Ok(()) };
    if h.settings.footnote_transition {
        let span = h.ms.notes[first].span;
        let b = h.b();
        let id = b.add_leaf(NodeKind::ThematicBreak, span, guidedoc::Text::default())?;
        b.add_attribute_string(id, AttributeKey::Class, "footnotes")?;
    }
    // Footnotes may reference footnotes, which then follow in reference order.
    let mut k = 0;
    while let Some(&i) = h.ms.ordered.get(k) {
        let n = h.ms.notes[i].clone();
        let owned: Vec<(String, u32)> = n
            .lines
            .iter()
            .map(|&(a, z, offset)| (h.ms.note_text[a..z].to_owned(), offset))
            .collect();
        emit_note(h, &n, &owned)?;
        k += 1;
    }
    Ok(())
}

fn emit_note(h: &mut Host<'_, '_>, n: &Note, owned: &[(String, u32)]) -> Result<(), Fail> {
    let manual = numeric_label(&n.label);
    let b = h.b();
    let id = b.begin_node(NodeKind::Footnote, n.span)?;
    let t = b.add_text(&label_text(&n.label))?;
    b.set_text(id, t);
    if manual {
        let v = n.label.bytes().fold(0i64, |v, c| (v * 10 + i64::from(c - b'0')).min(1 << 30));
        b.set_value(id, v as i32);
    } else {
        b.add_flags(id, NodeFlags::AUTO);
    }
    let lines: Vec<Line<'_>> =
        owned.iter().map(|(text, offset)| Line { text, offset: *offset }).collect();
    parse_lines(h, &lines, n.source, false, false)?;
    h.b().end_node(id)
}
