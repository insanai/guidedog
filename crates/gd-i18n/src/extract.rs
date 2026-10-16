//! Extraction: the translatable messages of documents collected into a POT template,
//! as Sphinx's gettext builder writes it.

use std::collections::HashMap;
use std::fmt::Write as _;

use gd_gettext::{Catalog, Message, ReferenceStyle, WriteOptions, write_po};
use guidedoc::{AttributeKey, Document, NodeId, NodeKind, SourceId};

use crate::units::{
    Targets, Unit, index_parts, inline_span, literal_message, unit_of, write_message,
};

/// The gettext builder's settings for one document. Borrowed strings; `Default` is no
/// extra targets, no locations, and no uuids.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExtractOptions<'a> {
    /// `gettext_additional_targets`.
    pub targets: Targets,
    /// `gettext_location`: a `#: path:line` line for every occurrence.
    pub location: bool,
    /// `gettext_uuid`: a `#: uuid` line for every occurrence.
    pub uuid: bool,
    /// How locations name the main source, e.g. "../../index.rst"; "" keeps its name.
    pub path: &'a str,
    /// The document's name, e.g. "guide/usage"; it seeds the uuids.
    pub docname: &'a str,
    /// Included files are named relative to this directory, when set.
    pub base: &'a str,
}

/// The values of a template's header, as Sphinx fills them.
#[derive(Debug, Clone, Copy, Default)]
pub struct PotHeader<'a> {
    /// The project's name.
    pub project: &'a str,
    /// The project's version.
    pub version: &'a str,
    /// The copyright line.
    pub copyright: &'a str,
    /// `gettext_last_translator`.
    pub last_translator: &'a str,
    /// `gettext_language_team`.
    pub language_team: &'a str,
    /// The creation date, e.g. "2026-09-29 12:00+0000" (see `format_date`).
    pub creation_date: &'a str,
}

/// gettext_compact: which text domain (catalog) a document's messages belong to.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Compact {
    /// One catalog per top-level directory (Sphinx's default).
    #[default]
    PerDirectory,
    /// One catalog per document.
    PerDocument,
    /// One catalog of the given name for every document.
    Single(String),
}

/// Returns the text domain of a document's messages, as Sphinx's `docname_to_domain`
/// does: "guide/usage" is "guide" per directory, else "guide/usage"; "index" is "index"
/// either way. The result borrows `docname` or `compact`.
pub fn domain_for<'a>(docname: &'a str, compact: &'a Compact) -> &'a str {
    match compact {
        Compact::Single(name) => name,
        Compact::PerDocument => docname,
        Compact::PerDirectory => docname.split_once('/').map_or(docname, |(dir, _)| dir),
    }
}

/// One message of a template: its msgid, its locations, and its uuids.
#[derive(Debug, Clone)]
struct Entry {
    id: String,
    locations: Vec<String>,
    uuids: Vec<String>,
}

/// The messages of one or more documents, collected as Sphinx's gettext builder does: a
/// message found again gains a location instead of a second entry, and messages keep
/// the order of their first occurrence. The template owns copies of everything added.
#[derive(Debug, Clone, Default)]
pub struct Template {
    name: String,
    entries: Vec<Entry>,
    /// msgid -> index in `entries`.
    seen: HashMap<String, usize>,
    /// Occurrences so far of each document and message, for the uuids.
    occurrences: HashMap<String, u64>,
    header: Option<Message>,
    /// Scratch for one inline message.
    message: String,
}

impl Template {
    /// Returns an empty template for the text domain `name`.
    pub fn new(name: &str) -> Template {
        Template { name: name.to_owned(), ..Template::default() }
    }

    /// Returns the number of distinct messages.
    pub fn message_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns the msgids in the order of their first occurrence, borrowed.
    pub fn message_ids(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|e| e.id.as_str())
    }

    /// Adds one occurrence of a message: its location ("path:line", or "" for none) and
    /// uuid ("" for none). The same msgid gains the location once.
    pub fn add(&mut self, msgid: &str, location: &str, uuid: &str) {
        let index = match self.seen.get(msgid) {
            Some(&index) => index,
            None => {
                self.entries.push(Entry {
                    id: msgid.to_owned(),
                    locations: Vec::new(),
                    uuids: Vec::new(),
                });
                self.seen.insert(msgid.to_owned(), self.entries.len() - 1);
                self.entries.len() - 1
            }
        };
        let entry = &mut self.entries[index];
        if !location.is_empty() && !entry.locations.iter().any(|l| l == location) {
            entry.locations.push(location.to_owned());
        }
        if !uuid.is_empty() {
            entry.uuids.push(uuid.to_owned());
        }
    }

    /// Adds every translatable message of `doc` and returns how many occurrences it
    /// found. Content inside substitution definitions, comments, and targets is skipped,
    /// as in Sphinx.
    pub fn extract(&mut self, doc: Document<'_>, options: &ExtractOptions<'_>) -> usize {
        let mut found = 0;
        let mut lines = LineCursor::default();
        let mut message = std::mem::take(&mut self.message);
        let mut i = 0;
        while i < doc.node_count() {
            let id = NodeId::from_index(i);
            if is_untranslated(doc, id) {
                i = doc.subtree_end(id).index();
                continue;
            }
            i += 1;
            let unit = unit_of(doc, id, options.targets);
            if unit == Unit::None {
                continue;
            }
            let location = if options.location {
                locate(doc, id, options, &mut lines)
            } else {
                String::new()
            };
            for_each_message(doc, id, unit, options.targets, &mut message, |msgid| {
                let uuid = if options.uuid {
                    self.next_uuid(options.docname, msgid)
                } else {
                    String::new()
                };
                self.add(msgid, &location, &uuid);
                found += 1;
            });
        }
        self.message = message;
        found
    }

    /// Gives an occurrence a stable identifier: the same document, message, and
    /// occurrence number give the same uuid in every build, so templates diff cleanly
    /// (Sphinx draws random ones and matches them against the previous build's).
    fn next_uuid(&mut self, docname: &str, msgid: &str) -> String {
        let key = format!("{docname}\0{msgid}");
        let n = *self.occurrences.get(&key).unwrap_or(&0);
        let uuid = stable_uuid(&key, n);
        self.occurrences.insert(key, n + 1);
        uuid
    }

    /// Gives the template the header Sphinx's gettext builder writes.
    pub fn set_header(&mut self, h: &PotHeader<'_>) {
        let mut header = Message::new("");
        header.comments = vec![
            "SOME DESCRIPTIVE TITLE.".to_owned(),
            format!("Copyright (C) {}", h.copyright),
            format!(
                "This file is distributed under the same license as the {} package.",
                h.project
            ),
            "FIRST AUTHOR <EMAIL@ADDRESS>, YEAR.".to_owned(),
            String::new(),
        ];
        header.fuzzy = true;
        header.strs = vec![format!(
            "Project-Id-Version: {} {}\nReport-Msgid-Bugs-To: \nPOT-Creation-Date: {}\n\
             PO-Revision-Date: YEAR-MO-DA HO:MI+ZONE\nLast-Translator: {}\n\
             Language-Team: {}\nMIME-Version: 1.0\nContent-Type: text/plain; charset=UTF-8\n\
             Content-Transfer-Encoding: 8bit\n",
            h.project, h.version, h.creation_date, h.last_translator, h.language_team
        )];
        self.header = Some(header);
    }

    /// Returns the template as a gettext catalog: every location in order (sorted by
    /// file, then line), followed by the uuids, as `#:` references.
    pub fn to_catalog(&self) -> Catalog {
        let mut cat = Catalog::new(&self.name);
        cat.header = self.header.clone();
        for e in &self.entries {
            let mut m = Message::new(&e.id);
            m.references = e.locations.clone();
            m.references.sort_by(|a, b| location_order(a, b));
            m.references.extend(e.uuids.iter().cloned());
            cat.add_message(m);
        }
        cat
    }

    /// Returns the template as POT text in Sphinx's layout: every location and uuid on
    /// its own `#:` line, and strings broken only after newlines.
    pub fn write(&self) -> String {
        let options = WriteOptions {
            width: 0,
            references: ReferenceStyle::OnePerLine,
            ..Default::default()
        };
        write_po(&self.to_catalog(), &options)
    }
}

/// Tells whether a kind's content is never translated (nor extracted).
pub(crate) fn is_untranslated(doc: Document<'_>, id: NodeId) -> bool {
    if matches!(
        doc.kind(id),
        NodeKind::SubstitutionDefinition | NodeKind::Comment | NodeKind::Target
    ) {
        return true;
    }
    if doc.kind(id) != NodeKind::Field {
        return false;
    }
    let Some(list) = doc.parent_of(id) else { return false };
    if doc.kind(list) != NodeKind::FieldList || doc.first_child(guidedoc::ROOT) != Some(list) {
        return false;
    }
    // Publication metadata is source configuration. Translating a key can erase a draft
    // flag, and translating its value can change a release boundary.
    let mut name = String::new();
    doc.plain_text(id + 1, &mut name);
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "template"
            | "date"
            | "updated"
            | "publish-at"
            | "expires-at"
            | "draft"
            | "author"
            | "authors"
            | "tags"
            | "category"
            | "series"
            | "summary"
            | "pdf"
            | "nosidebar"
    )
}

/// Calls `f` with each msgid node `id` carries; `scratch` holds an inline message.
pub(crate) fn for_each_message(
    doc: Document<'_>,
    id: NodeId,
    unit: Unit,
    targets: Targets,
    scratch: &mut String,
    mut f: impl FnMut(&str),
) {
    let mut give = |s: &str| {
        if !s.is_empty() {
            f(s);
        }
    };
    match unit {
        Unit::None => {}
        Unit::Inline | Unit::Parsed => {
            if write_message(doc, id, scratch) {
                give(scratch);
            }
        }
        Unit::Literal => give(literal_message(doc, id)),
        Unit::Graphviz => {
            give(doc.custom_attribute(id, "alt").unwrap_or(""));
            if targets.literal_block {
                give(literal_message(doc, id));
            }
        }
        Unit::Alt => give(doc.attribute(id, AttributeKey::Alt).unwrap_or("")),
        Unit::Entry | Unit::Contents => give(doc.attribute(id, AttributeKey::Title).unwrap_or("")),
        Unit::Caption => give(doc.custom_attribute(id, "caption").unwrap_or("")),
        Unit::Meta => {
            for a in doc.attributes(id).iter().filter(|a| a.key == AttributeKey::Custom) {
                give(doc.text(a.value));
            }
        }
        Unit::Index => {
            let entries = doc.custom_attribute(id, "entries").unwrap_or("");
            for part in entries.lines().flat_map(index_parts) {
                give(part.text);
            }
        }
    }
}

/// Turns byte offsets into line numbers without rescanning from the start for offsets
/// that increase, as a document's do.
#[derive(Debug, Default)]
struct LineCursor {
    source: Option<SourceId>,
    offset: u32,
    line: usize,
}

/// Returns the "path:line" location of node `id`'s message.
fn locate(
    doc: Document<'_>,
    id: NodeId,
    options: &ExtractOptions<'_>,
    cursor: &mut LineCursor,
) -> String {
    let span = inline_span(doc, id).unwrap_or(doc.node(id).span);
    if cursor.source != Some(span.source) || cursor.offset > span.start {
        *cursor = LineCursor { source: Some(span.source), offset: 0, line: 1 };
    }
    let text = doc.source_str(span.source).as_bytes();
    let from = (cursor.offset as usize).min(text.len());
    let to = (span.start as usize).min(text.len());
    cursor.line += text[from..to].iter().filter(|&&c| c == b'\n').count();
    cursor.offset = span.start;
    let name = doc.source_entry(span.source).map_or("", |e| e.name.as_str());
    let mut out = String::new();
    if span.source == SourceId::MAIN && !options.path.is_empty() {
        out.push_str(options.path);
    } else if !options.base.is_empty() {
        out.push_str(&relative_path(options.base, name));
    } else {
        out.push_str(name);
    }
    let _ = write!(out, ":{}", cursor.line);
    out
}

/// Returns `name` relative to the directory `base`, both cleaned lexically, with "/"
/// separators; `name` cleaned when either is empty or `base` climbs out of its root.
fn relative_path(base: &str, name: &str) -> String {
    let base = clean_components(base);
    let name = clean_components(name);
    if base.is_empty() || name.is_empty() || base.first().is_some_and(|b| b == "..") {
        return name.join("/");
    }
    let common = base.iter().zip(&name).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<&str> = vec![".."; base.len() - common];
    parts.extend(name[common..].iter().map(String::as_str));
    parts.join("/")
}

/// Splits a path into components, dropping "." and resolving ".." lexically; a leading
/// "/" is kept as an empty first component, so absolute and relative paths differ.
fn clean_components(path: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if path.starts_with(['/', '\\']) {
        out.push(String::new());
    }
    for piece in path.split(['/', '\\']) {
        match piece {
            "" | "." => {}
            ".." if out.last().is_some_and(|last| last != ".." && !last.is_empty()) => {
                out.pop();
            }
            _ => out.push(piece.to_owned()),
        }
    }
    out
}

/// Orders locations by file, then by line number.
fn location_order(a: &str, b: &str) -> std::cmp::Ordering {
    match (a.rsplit_once(':'), b.rsplit_once(':')) {
        (Some((fa, la)), Some((fb, lb))) => fa.cmp(fb).then_with(|| {
            let la: u64 = la.parse().unwrap_or(0);
            let lb: u64 = lb.parse().unwrap_or(0);
            la.cmp(&lb)
        }),
        _ => a.cmp(b),
    }
}

/// Hashes a document-and-message key and its occurrence number into 32 hex digits
/// with two FNV-style streams.
fn stable_uuid(key: &str, n: u64) -> String {
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let (mut h1, mut h2): (u64, u64) = (0xcbf2_9ce4_8422_2325, 0x8422_2325_cbf2_9ce4);
    for &c in key.as_bytes() {
        h1 = (h1 ^ u64::from(c)).wrapping_mul(PRIME);
        h2 = (h2 ^ u64::from(c)).wrapping_mul(PRIME).wrapping_add(0x9e37_79b9_7f4a_7c15);
    }
    h1 = (h1 ^ n).wrapping_mul(PRIME);
    h2 = (h2 ^ n).wrapping_mul(PRIME);
    format!("{h1:016x}{h2:016x}")
}
