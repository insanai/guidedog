//! Anchors give nodes stable, unique identifiers: HTML ids and Typst labels. Every
//! renderer and the project host build the same table, so internal links agree across
//! targets and the labels a project records are the ids its pages publish. The ids are
//! the ones Docutils gives a document read as Sphinx reads it (auto_id_prefix "id"), so
//! a project that moves from Sphinx keeps its links and bookmarks:
//!
//! - An explicit Id attribute is used verbatim (terms, equations, domain objects).
//! - Every element Docutils names gets one id when Docutils' parser or transforms name
//!   it, in the same order (`anchors_order`): the id made from its name by `make_id`,
//!   or, when the name gives none or only an id already used, the next "idN". Elements
//!   without a name (footnote references, anonymous targets) take the next "idN".
//! - Targets and names Docutils does not write still take their id, so later ids are
//!   numbered as Docutils numbers them, but no node carries it.
//! - A label (an internal block Target) gives its id to the element it names, after the
//!   element's own ids.
//! - Elements a renderer generates have ids too (`anchor_extras`): the entries of a table
//!   of contents and the footnotes of target-notes with their references.
//!
//! A node may have several ids: `anchor_of` returns the primary one, and `anchor_ids`
//! iterates all of them for renderers that emit the others as aliases.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::document::Document;
use crate::options::{Label, RenderOptions, label};
use crate::tables::idfold::ID_FOLDS;
use crate::text::{TextSink, is_decimal_digit, is_letter, to_lower};
use crate::types::{DocumentReading, INVALID_NODE, NONE, NodeId, ROOT};

/// The id of an element a renderer generates for a node:
///
/// - an entry of a table of contents: `owner` is the Contents, `item` the Section;
/// - a footnote of target-notes: `owner` is the directive, `item` the first target (or
///   anonymous reference) with the URI it lists, `number` its footnote number;
/// - the footnote reference target-notes adds after a reference: `owner` is the
///   reference, `item` the directive, `note` the item of its footnote, `number` the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnchorExtra {
    /// The node the element is generated for.
    pub owner: NodeId,
    /// The node it lists or notes.
    pub item: NodeId,
    /// For a target-notes reference: the item of its footnote.
    pub note: NodeId,
    /// Its id's row in the table (`extra_text`).
    pub span: u32,
    /// Its footnote number.
    pub number: i32,
}

/// The anchor table of one document. Build it with `build_anchors`.
#[derive(Debug, Clone, Default)]
pub struct Anchors {
    /// Per node: its first id row, or NONE.
    pub(crate) index: Vec<u32>,
    /// Per id row: its byte range in `bytes`.
    spans: Vec<(u32, u32)>,
    /// Per id row: the node's next id row, or NONE.
    pub(crate) next: Vec<u32>,
    /// Every id, one after another.
    bytes: String,
    prefix: String,
    /// The bytes the prefix takes before an id.
    pub(crate) prefix_length: usize,
    /// Every id given: the node that carries it, or NONE.
    names: HashMap<String, u32>,
    counter: u64,
    /// Per node: the first id labels give it, until they follow its own.
    pub(crate) labels: Vec<u32>,
    /// Sorted by owner, then in the order the ids were given.
    pub(crate) extras: Vec<AnchorExtra>,
}

/// Who carries a new id: the node itself, the node a label names (after the node's own
/// ids), or nobody.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Chain {
    Own,
    Label,
    Hidden,
}

/// Builds the anchor table of `doc` with the options' anchor prefix. `headings` gives
/// ids to headings outside sections (every profile but CommonMark's reference one).
pub fn build_anchors(doc: Document<'_>, options: &RenderOptions<'_>, headings: bool) -> Anchors {
    let n = doc.node_count();
    let mut a = Anchors {
        index: vec![NONE; n],
        labels: vec![NONE; n],
        prefix: options.anchor_prefix.to_owned(),
        ..Anchors::default()
    };
    let sphinx = n > 0 && doc.node(ROOT).value == DocumentReading::Sphinx as i32;
    let contents = label(options, Label::Contents);
    let mut order = crate::anchors_order::Order::new(&mut a, doc, sphinx, headings, contents);
    order.parse_order();
    order.transform_order();
    for i in 0..n {
        a.follow_with_labels(NodeId::from_index(i));
    }
    if sphinx {
        a.sort_section_ids(doc);
    }
    // Stable, so each owner's extras keep the order their ids were given in.
    a.extras.sort_by_key(|e| e.owner);
    a
}

impl Anchors {
    /// Returns the primary id of node `id`, borrowed from the table.
    pub fn anchor_of(&self, id: NodeId) -> Option<&str> {
        match self.index.get(id.index()) {
            Some(&span) if span != NONE => Some(self.span_text(span)),
            _ => None,
        }
    }

    /// Iterates every id of node `id`, the primary one first.
    pub fn anchor_ids(&self, id: NodeId) -> AnchorIds<'_> {
        AnchorIds { anchors: self, span: self.index.get(id.index()).copied().unwrap_or(NONE) }
    }

    /// Returns the ids generated for `owner`'s sake, in the order they were given.
    pub fn anchor_extras(&self, owner: NodeId) -> &[AnchorExtra] {
        let lo = self.extras.partition_point(|e| e.owner < owner);
        let hi = self.extras.partition_point(|e| e.owner <= owner);
        &self.extras[lo..hi]
    }

    /// Returns the id of the element generated for `owner` and `item`.
    pub fn extra_anchor(&self, owner: NodeId, item: NodeId) -> Option<&str> {
        let e = self.anchor_extras(owner).iter().find(|e| e.item == item)?;
        Some(self.span_text(e.span))
    }

    /// Returns the id of a generated element.
    pub fn extra_text(&self, e: &AnchorExtra) -> &str {
        self.span_text(e.span)
    }

    pub(crate) fn span_text(&self, span: u32) -> &str {
        let Some(&(start, len)) = self.spans.get(span as usize) else {
            return "";
        };
        self.bytes.get(start as usize..(start + len) as usize).unwrap_or("")
    }

    /// Gives an id as Docutils' `document.set_id` does: the id made from `name` when it
    /// is not used yet, otherwise the next unused "idN". Returns the id's row.
    pub(crate) fn assign(&mut self, node: NodeId, name: &str, chain: Chain) -> u32 {
        let start = self.bytes.len();
        self.write_prefix();
        let body = self.bytes.len();
        if !name.is_empty() {
            make_id(&mut self.bytes, name);
            if self.bytes.len() > body
                && let Some(span) = self.register(node, start, chain)
            {
                return span;
            }
        }
        loop {
            self.bytes.truncate(body);
            self.counter += 1;
            let _ = write!(self.bytes, "id{}", self.counter);
            if let Some(span) = self.register(node, start, chain) {
                return span;
            }
        }
    }

    /// Keeps an explicit id. One already used adds nothing, unless a label gave it to
    /// this node: it then takes its place among the node's own ids, in the order its
    /// explicit ids give (a pinned section's label comes before the section).
    pub(crate) fn assign_verbatim(&mut self, node: NodeId, id: &str) {
        let start = self.bytes.len();
        self.write_prefix();
        let body = self.bytes.len();
        write_verbatim_id(&mut self.bytes, id);
        if self.bytes.len() == body {
            self.bytes.truncate(start);
            return;
        }
        if self.register(node, start, Chain::Own).is_some() {
            return;
        }
        let key = self.bytes.split_off(start);
        self.take_label(node, &key);
    }

    /// Moves the label id `key` of `node` from its label chain to its own ids.
    fn take_label(&mut self, node: NodeId, key: &str) {
        let Some(&head) = self.labels.get(node.index()) else {
            return;
        };
        let mut previous = NONE;
        let mut span = head;
        while span != NONE {
            let after = self.next[span as usize];
            if self.span_text(span) == key {
                if previous == NONE {
                    self.labels[node.index()] = after;
                } else {
                    self.next[previous as usize] = after;
                }
                self.next[span as usize] = NONE;
                self.index[node.index()] = self.append_span(self.index[node.index()], span);
                return;
            }
            previous = span;
            span = after;
        }
    }

    fn write_prefix(&mut self) {
        if self.prefix.is_empty() {
            return;
        }
        let start = self.bytes.len();
        write_id(&mut self.bytes, &self.prefix);
        self.bytes.push('-');
        self.prefix_length = self.bytes.len() - start;
    }

    /// Records the id written from `start`; `None` when it is used already.
    fn register(&mut self, node: NodeId, start: usize, chain: Chain) -> Option<u32> {
        let key = &self.bytes[start..];
        if self.names.contains_key(key) {
            return None;
        }
        let value = if chain == Chain::Hidden { NONE } else { node.0 };
        self.names.insert(key.to_owned(), value);
        let span = u32::try_from(self.spans.len()).unwrap_or(NONE);
        let len = u32::try_from(self.bytes.len() - start).unwrap_or(0);
        self.spans.push((u32::try_from(start).unwrap_or(0), len));
        self.next.push(NONE);
        match chain {
            Chain::Own => {
                self.index[node.index()] = self.append_span(self.index[node.index()], span)
            }
            Chain::Label => {
                self.labels[node.index()] = self.append_span(self.labels[node.index()], span)
            }
            Chain::Hidden => {}
        }
        Some(span)
    }

    /// Appends `span` to the chain starting at `head` and returns the chain's head.
    pub(crate) fn append_span(&mut self, head: u32, span: u32) -> u32 {
        if head == NONE {
            return span;
        }
        let mut s = head;
        while self.next[s as usize] != NONE {
            s = self.next[s as usize];
        }
        self.next[s as usize] = span;
        head
    }

    /// Puts the ids labels gave a node after its own ids.
    fn follow_with_labels(&mut self, id: NodeId) {
        let i = id.index();
        if self.labels[i] == NONE {
            return;
        }
        self.index[i] = self.append_span(self.index[i], self.labels[i]);
        self.labels[i] = NONE;
    }

    /// Gives a generated element its id.
    pub(crate) fn add_extra(&mut self, e: AnchorExtra) {
        let span = self.assign(INVALID_NODE, "", Chain::Hidden);
        self.extras.push(AnchorExtra { span, ..e });
    }
}

/// The ids of one node, primary first; `Anchors::anchor_ids` makes it.
#[derive(Debug, Clone)]
pub struct AnchorIds<'a> {
    anchors: &'a Anchors,
    span: u32,
}

impl<'a> Iterator for AnchorIds<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.span == NONE {
            return None;
        }
        let s = self.anchors.span_text(self.span);
        self.span = self.anchors.next.get(self.span as usize).copied().unwrap_or(NONE);
        Some(s)
    }
}

/// Turns a name into an identifier matching `[a-z](-?[a-z0-9]+)*`: letters are
/// lowercased and transliterated to ASCII (`id_fold`), every other run of characters
/// becomes one hyphen, and leading digits and hyphens are dropped, as published Sphinx
/// URLs expect. It may write nothing.
pub fn make_id(out: &mut (impl TextSink + ?Sized), name: &str) {
    let mut wrote = false;
    let mut hyphen = false;
    for r in name.chars() {
        let lower = to_lower(r);
        let mut single = [0u8; 4];
        let folded =
            if lower.is_ascii() { &*lower.encode_utf8(&mut single) } else { id_fold(lower) };
        if folded.is_empty() {
            hyphen = true;
        }
        for c in folded.bytes() {
            if !(c.is_ascii_lowercase() || c.is_ascii_digit()) {
                hyphen = true;
                continue;
            }
            if !wrote && c.is_ascii_digit() {
                continue;
            }
            if hyphen && wrote {
                out.put_char('-');
            }
            hyphen = false;
            out.put_char(char::from(c));
            wrote = true;
        }
    }
}

/// Returns the ASCII transliteration `make_id` gives a lowercase character ("é" gives
/// "e", "²" gives "2"), or "" when it has none.
pub fn id_fold(r: char) -> &'static str {
    match ID_FOLDS.binary_search_by_key(&(r as u32), |row| row.0) {
        Ok(i) => ID_FOLDS[i].1,
        Err(_) => "",
    }
}

/// Keeps an explicit id, replacing only characters no HTML id or Typst label may hold.
fn write_verbatim_id(out: &mut String, id: &str) {
    for r in id.chars() {
        let bad = r <= ' ' || matches!(r, '"' | '<' | '>' | '#' | '\'');
        out.push(if bad { '-' } else { r });
    }
}

/// Lowercases letters, keeps letters and digits, and joins every other run of
/// characters with one hyphen. It names documents and chapters, whose names may start
/// with digits (they get "id-" before them); `make_id` names anchors. A name without a
/// letter or digit gives "section".
pub fn write_id(out: &mut (impl TextSink + ?Sized), name: &str) {
    let kept = |r: char| is_letter(r) || is_decimal_digit(r);
    match name.chars().find(|&r| kept(r)) {
        None => {
            out.put_str("section");
            return;
        }
        Some(first) if first.is_ascii_digit() => out.put_str("id-"),
        Some(_) => {}
    }
    let mut wrote = false;
    let mut hyphen = false;
    for r in name.chars() {
        if !kept(r) {
            hyphen = true;
            continue;
        }
        if hyphen && wrote {
            out.put_char('-');
        }
        hyphen = false;
        out.put_char(to_lower(r));
        wrote = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_follow_docutils() {
        let mut s = String::new();
        make_id(&mut s, "2. Ünïcode — Straße!");
        assert_eq!(s, "unicode-strasze");
        s.clear();
        write_id(&mut s, "2024 Notes");
        assert_eq!(s, "id-2024-notes");
        s.clear();
        write_id(&mut s, "--");
        assert_eq!(s, "section");
        assert_eq!(id_fold('é'), "e");
    }
}
