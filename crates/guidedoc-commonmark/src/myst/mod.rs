//! The MyST syntax (myst-parser 0.16.1): headings make sections, as in
//! reStructuredText; footnotes collect at the end of the document; directives and roles
//! run through the extension (`extension.rs`). Shapes follow AST.md, "Markdown (MyST)
//! mapping".

mod blocks;
mod directive;
mod emit;
mod eval;
mod front;
mod html;
mod inline;
mod math;
mod notes;
mod paths;
mod resolve;
mod subs;
mod table;
pub(crate) mod yaml;

use guidedoc::{Arg, Fail, NameMap, NodeId, NodeKind, ROOT, SourceSpan, Text, arg_int, arg_text};

use crate::blocks::{Skeleton, parse_lines};
use crate::emit::open_block;
use crate::extension::Line;
use crate::host::Host;
use crate::messages::{HEADING_SKIP, SLUG_FUNC};

pub(crate) use directive::{is_directive_fence, run_fence_directive};
pub(crate) use emit::{
    block_substitution, emit_definition_list, emit_field, emit_heading, emit_leaf,
    prescan_headings, task_checkbox, task_list, task_state,
};
pub(crate) use front::skip_front_matter;
pub(crate) use html::emit_html_extension;
pub(crate) use inline::inline_lines;
pub(crate) use paths::relocate;

/// A section open in the builder, and the Markdown heading level that opened it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OpenSection {
    pub node: NodeId,
    pub level: usize,
}

/// A footnote definition. Its lines are kept (copied) until the footnotes are emitted
/// at the end of the document.
#[derive(Debug, Clone, Default)]
pub(crate) struct Note {
    /// As written, without "^".
    pub label: String,
    pub source: guidedoc::SourceId,
    /// Its lines: ranges of `MystState::note_text` and their source offsets.
    pub lines: Vec<(usize, usize, u32)>,
    pub span: SourceSpan,
    /// 0 until referenced, then its place in reference order.
    pub order: usize,
}

/// A heading anchor: the k-th section heading of the document and its slug.
#[derive(Debug, Clone, Default)]
pub(crate) struct Slug {
    pub slug: String,
    /// The heading's plain text.
    pub title: String,
    pub ordinal: usize,
    /// The section, once emitted.
    pub node: Option<NodeId>,
}

/// A substitution the `{sub-ref}` role reaches and the reader provides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SubRef {
    Words,
    Minutes,
    Today,
    Version,
    Release,
}

impl SubRef {
    pub(crate) const ALL: [SubRef; 5] =
        [SubRef::Words, SubRef::Minutes, SubRef::Today, SubRef::Version, SubRef::Release];

    pub(crate) fn name(self) -> &'static str {
        match self {
            SubRef::Words => "wordcount-words",
            SubRef::Minutes => "wordcount-minutes",
            SubRef::Today => "today",
            SubRef::Version => "version",
            SubRef::Release => "release",
        }
    }
}

/// The most sections open at once that heading levels track.
const MAX_SECTIONS: usize = 64;

/// The MyST layer's document-wide state.
#[derive(Debug, Default)]
pub(crate) struct MystState {
    /// Classes for every top-level element (parse_body).
    pub each: Option<Text>,
    /// Classes for the next element (a class directive).
    pub pending: Option<Text>,
    pub open: Vec<OpenSection>,
    /// Section headings seen, for heading anchors.
    pub headings: usize,
    pub notes: Vec<Note>,
    /// Normalized label to index in `notes`.
    pub note_names: NameMap<usize>,
    pub note_text: String,
    /// Referenced notes in reference order.
    pub ordered: Vec<usize>,
    pub slugs: Vec<Slug>,
    /// Slug to index in `slugs`.
    pub slug_index: NameMap<usize>,
    /// Base slug to the next suffix to try.
    pub slug_bases: NameMap<usize>,
    /// The first slug no section has taken yet; slugs are in order.
    pub slug_next: usize,
    /// Links to heading anchors: link row and slug index.
    pub anchors: Vec<(u32, usize)>,
    /// amsmath equations numbered so far.
    pub equations: usize,
    /// Words of text, for the wordcount substitutions.
    pub words: usize,
    /// The reader's substitutions that `{sub-ref}` uses.
    pub sub_refs: Vec<SubRef>,
    /// The body is being read: its words count.
    pub counting: bool,
    /// The task state of the item being emitted (`task_state`).
    pub task: u8,
    pub subs: subs::Substitutions,
}

impl MystState {
    /// Forgets the last read, keeping the storage for the next one.
    pub(crate) fn clear(&mut self) {
        self.each = None;
        self.pending = None;
        self.open.clear();
        self.headings = 0;
        self.notes.clear();
        self.note_names.clear();
        self.note_text.clear();
        self.ordered.clear();
        self.slugs.clear();
        self.slug_index.clear();
        self.slug_bases.clear();
        self.slug_next = 0;
        self.anchors.clear();
        self.equations = 0;
        self.words = 0;
        self.sub_refs.clear();
        self.counting = false;
        self.task = 0;
        self.subs.clear();
    }
}

/// Reads the main source: front matter, the body with sections, then the footnotes,
/// and resolution.
pub(crate) fn myst_document(h: &mut Host<'_, '_>, lines: &[Line<'_>]) -> Result<(), Fail> {
    let source = h.ctx.source;
    let slug_func = &h.settings.heading_slug_func;
    if !slug_func.is_empty() {
        h.report(SLUG_FUNC, SourceSpan::new(source, 0, 0), arg_text(slug_func), Arg::None);
    }
    prepare(h);
    let body = front::front_matter(h, lines)?;
    if let Some(ext) = h.ext() {
        ext.begin(h)?;
    }
    h.ms.counting = true;
    parse_lines(h, body, source, true, true)?;
    // myst-parser appends the footnotes to the section open at the end.
    notes::emit_notes(h)?;
    close_sections(h, 0)?;
    if !h.ms.sub_refs.is_empty() {
        emit_sub_refs(h)?;
    }
    if let Some(ext) = h.ext() {
        ext.end(h)?;
    }
    resolve::resolve_anchors(h);
    let resolved = match h.ext() {
        Some(ext) => ext.resolve(h)?,
        None => false,
    };
    if !resolved {
        resolve::resolve_document(h)?;
    }
    Ok(())
}

/// Prepares the state every MyST read needs: the configured substitutions.
pub(crate) fn prepare(h: &mut Host<'_, '_>) {
    let settings = h.settings;
    for s in &settings.substitutions {
        h.ms.subs.add(&s.name, &s.value, None, 0);
    }
}

/// Closes the open sections whose Markdown level is at least `level`; 0 closes all.
pub(crate) fn close_sections(h: &mut Host<'_, '_>, level: usize) -> Result<(), Fail> {
    while let Some(top) = h.ms.open.last().copied().filter(|t| t.level >= level) {
        if h.b().current() != Some(top.node) {
            break;
        }
        h.b().end_node(top.node)?;
        h.ms.open.pop();
    }
    Ok(())
}

/// Tells whether headings of the running parse may open sections: the builder's open
/// node is the innermost section, or the document.
pub(crate) fn sections_here(h: &mut Host<'_, '_>, sk: &Skeleton<'_>) -> bool {
    if !sk.sections {
        return false;
    }
    let want = h.ms.open.last().map_or(ROOT, |s| s.node);
    h.b().current() == Some(want)
}

/// Closes sections of the same or a deeper level and opens a section for a heading of
/// Markdown `level`, as myst-parser nests them; returns it and its depth.
pub(crate) fn open_section(
    h: &mut Host<'_, '_>,
    level: usize,
    span: SourceSpan,
) -> Result<(NodeId, usize), Fail> {
    close_sections(h, level)?;
    if let Some(top) = h.ms.open.last().copied().filter(|t| level > t.level + 1) {
        h.report(HEADING_SKIP, span, arg_int(top.level as i64), arg_int(level as i64));
    }
    let depth = h.ms.open.len() + 1;
    let id = open_block(h, NodeKind::Section, span)?;
    h.b().set_value(id, depth as i32);
    if h.ms.open.len() < MAX_SECTIONS {
        h.ms.open.push(OpenSection { node: id, level });
    }
    Ok((id, depth))
}

/// Defines the substitutions that `{sub-ref}` reaches and the reader provides: the
/// wordcount (`words_per_minute`), today's date (`Settings::today`, else
/// `ReadOptions::today`), and env.config version and release (`env_config`).
fn emit_sub_refs(h: &mut Host<'_, '_>) -> Result<(), Fail> {
    let per = h.settings.words_per_minute.max(1);
    let span = SourceSpan::new(h.ctx.source, 0, 0);
    let refs: Vec<SubRef> =
        SubRef::ALL.into_iter().filter(|k| h.ms.sub_refs.contains(k)).collect();
    for k in refs {
        let value = match k {
            SubRef::Words => h.ms.words.to_string(),
            SubRef::Minutes => ((h.ms.words + per / 2) / per).to_string(),
            SubRef::Today if !h.settings.today.is_empty() => h.settings.today.clone(),
            SubRef::Today => h.ctx.options.today.to_owned(),
            SubRef::Version | SubRef::Release => h
                .settings
                .env_config
                .iter()
                .filter(|c| c.name == k.name())
                .map(|c| c.value.as_str())
                .collect(),
        };
        if value.is_empty() {
            continue;
        }
        let b = h.b();
        let id = b.begin_node(NodeKind::SubstitutionDefinition, span)?;
        let name = b.add_text(k.name())?;
        b.set_text(id, name);
        b.add_text_leaf(NodeKind::Text, span, &value)?;
        b.end_node(id)?;
    }
    Ok(())
}

/// Trims spaces and tabs at the start and ASCII white space at the end.
pub(crate) fn trim_space(s: &str) -> &str {
    trim_right(s.trim_start_matches([' ', '\t']))
}

/// Trims ASCII white space at the end.
pub(crate) fn trim_right(s: &str) -> &str {
    s.trim_end_matches([' ', '\t', '\n', '\r', '\x0b', '\x0c'])
}
