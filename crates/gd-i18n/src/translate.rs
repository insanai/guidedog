//! The translation pass, Sphinx's Locale transform: every translatable node whose
//! message the catalog translates gets the translation.

use std::cell::{Cell, RefCell};

use gd_gettext::Lookup;
use guidedoc::{
    Builder, Document, Fail, INVALID_NODE, Limits, LinkKind, NONE, NodeFlag, NodeId, NodeKind,
    Pass, PassOutcome, Policy, ROOT, ReadInlineFn, ReadOptions, Workspace, make_id,
    transform_document,
};

use crate::extract::is_hidden_container;
use crate::figures::ImageLocalization;
use crate::units::{Targets, Unit, literal_message, unit_of, write_message};

/// The name of the translation pass, for diagnostics.
pub const TRANSLATION_PASS: &str = "i18n.translate";

/// translation_progress_classes: which translatable nodes get the class `translated`
/// or `untranslated`, so a stylesheet can show what is left to translate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Progress {
    /// Mark translated nodes.
    pub translated: bool,
    /// Mark untranslated nodes.
    pub untranslated: bool,
}

impl Progress {
    /// Both classes (`translation_progress_classes = True`).
    pub const ALL: Progress = Progress { translated: true, untranslated: true };

    /// Tells whether no class is added.
    pub fn is_empty(self) -> bool {
        !self.translated && !self.untranslated
    }
}

/// The settings of the translation pass. Inline content is re-parsed with
/// `read_inline`, in the markup the document was written in (`read` carries the
/// reader's settings), and keeps the original's references; titles keep their anchors.
/// `Default` translates nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct Translation<'a> {
    /// The translations; `None` translates no text (images may still be localized).
    pub catalog: Option<&'a dyn Lookup>,
    /// Parses a translation as inline content (`guidedoc::Reader::read_inline`); `None`
    /// translates only literal and attribute messages.
    pub read_inline: Option<ReadInlineFn>,
    /// The reader's options, as the document was read.
    pub read: ReadOptions<'a>,
    /// The trust decisions for reading translations.
    pub policy: Policy,
    /// The bounds for reading translations and for the pass.
    pub limits: Limits,
    /// `gettext_additional_targets`.
    pub targets: Targets,
    /// Names the translations where they are parsed, e.g. "de/index.po".
    pub source_name: &'a str,
    /// `translation_progress_classes`.
    pub progress: Progress,
    /// `figure_language_filename`; it applies without a catalog.
    pub images: ImageLocalization<'a>,
}

/// What the last run of a translation pass did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TranslationCounts {
    /// Messages replaced by their translation.
    pub translated: usize,
    /// Translations refused (bad markup or references), each with a warning.
    pub kept: usize,
}

/// The buffers a pass reuses between documents.
#[derive(Debug, Default)]
pub(crate) struct PassState {
    /// Parses translations; one workspace per pass, reset for each translation.
    pub reader: Workspace,
    /// For each node of the document, its copy (`INVALID_NODE` until copied).
    pub moved: Vec<NodeId>,
    /// One message being looked up.
    pub message: String,
    /// One attribute value or localized path being written.
    pub scratch: String,
}

/// The translation pass, with the buffers it reuses and the counts of its last run.
/// Use it with `transform_document` beside other passes:
///
/// ```
/// use gd_i18n::{TRANSLATION_PASS, Translation, TranslationPass};
/// let pass = TranslationPass::new(Translation::default());
/// let run = |doc: guidedoc::Document<'_>, b: &mut guidedoc::Builder<'_>| pass.run(doc, b);
/// let passes = [guidedoc::Pass { name: TRANSLATION_PASS, run: &run }];
/// # let _ = passes;
/// ```
///
/// or run it alone with [`translate_document`].
#[derive(Debug)]
pub struct TranslationPass<'a> {
    translation: Translation<'a>,
    state: RefCell<PassState>,
    counts: Cell<TranslationCounts>,
}

impl<'a> TranslationPass<'a> {
    /// Returns the pass for `translation`.
    pub fn new(translation: Translation<'a>) -> TranslationPass<'a> {
        TranslationPass {
            translation,
            state: RefCell::new(PassState::default()),
            counts: Cell::new(TranslationCounts::default()),
        }
    }

    /// Returns the settings.
    pub fn translation(&self) -> &Translation<'a> {
        &self.translation
    }

    /// Returns what the last run did.
    pub fn counts(&self) -> TranslationCounts {
        self.counts.get()
    }

    /// Translates `doc` into `b` (a `guidedoc::PassFn`). A document the catalog does not
    /// touch costs one scan and is `Unchanged`; a builder failure is `Failed`.
    pub fn run(&self, doc: Document<'_>, b: &mut Builder<'_>) -> PassOutcome {
        self.counts.set(TranslationCounts::default());
        let t = &self.translation;
        if t.catalog.is_none() && !t.images.is_active() {
            return PassOutcome::Unchanged;
        }
        let Ok(mut state) = self.state.try_borrow_mut() else {
            return PassOutcome::Failed; // the pass ran inside itself.
        };
        let mut w = Walk { t, doc, b, st: &mut state, counts: TranslationCounts::default() };
        if !t.progress.untranslated && !w.any_translation() {
            return PassOutcome::Unchanged;
        }
        let outcome = match w.copy_document() {
            Ok(()) => PassOutcome::Rebuilt,
            Err(_) => PassOutcome::Failed,
        };
        self.counts.set(w.counts);
        outcome
    }
}

/// Runs the translation pass alone on the workspace's current document and returns the
/// translated document (or the same one, unchanged) and what the pass did. Fails as
/// `transform_document` does.
pub fn translate_document<'w>(
    ws: &'w mut Workspace,
    translation: Translation<'_>,
) -> Result<(Document<'w>, TranslationCounts), Fail> {
    let limits = translation.limits;
    let pass = TranslationPass::new(translation);
    let run = |doc: Document<'_>, b: &mut Builder<'_>| pass.run(doc, b);
    let passes = [Pass { name: TRANSLATION_PASS, run: &run }];
    let doc = transform_document(ws, &passes, &limits)?;
    Ok((doc, pass.counts()))
}

/// One run of the pass over one document.
pub(crate) struct Walk<'t, 'a, 'd, 'b, 'bb> {
    pub t: &'t Translation<'a>,
    pub doc: Document<'d>,
    pub b: &'b mut Builder<'bb>,
    pub st: &'t mut PassState,
    pub counts: TranslationCounts,
}

impl<'a> Walk<'_, 'a, '_, '_, '_> {
    /// Looks for one translated message (or localized image), so documents the catalog
    /// does not touch cost one scan and no rebuild.
    fn any_translation(&mut self) -> bool {
        let doc = self.doc;
        let mut i = 0;
        while i < doc.node_count() {
            let id = NodeId::from_index(i);
            if is_hidden_container(doc.kind(id)) {
                i = doc.subtree_end(id).index();
                continue;
            }
            i += 1;
            if doc.kind(id) == NodeKind::Image && self.localized_image_of(id) {
                return true;
            }
            let found = match unit_of(doc, id, self.t.targets) {
                Unit::None => false,
                Unit::Inline | Unit::Parsed => self.inline_translation(id).is_some(),
                Unit::Literal => self.lookup(literal_message(doc, id)).is_some(),
                // Attribute units are checked while copying; cheap either way.
                Unit::Alt
                | Unit::Graphviz
                | Unit::Entry
                | Unit::Contents
                | Unit::Caption
                | Unit::Meta
                | Unit::Index => true,
            };
            if found {
                return true;
            }
        }
        false
    }

    /// Tells whether image `id` of the document has a localized file.
    fn localized_image_of(&mut self, id: NodeId) -> bool {
        let Some(link) = self.doc.link_info(id) else {
            return false;
        };
        let uri = self.doc.text(link.destination);
        self.t.images.localize(uri, &mut self.st.scratch)
    }

    /// Returns a translation that differs from the message, as Sphinx applies them.
    pub fn lookup(&self, msgid: &str) -> Option<&'a str> {
        if msgid.is_empty() {
            return None;
        }
        let s = self.t.catalog?.translation(msgid, None)?;
        (s != msgid).then_some(s)
    }

    /// Returns the translation of node `id`'s inline message.
    pub fn inline_translation(&mut self, id: NodeId) -> Option<&'a str> {
        let mut message = std::mem::take(&mut self.st.message);
        let found =
            if write_message(self.doc, id, &mut message) { self.lookup(&message) } else { None };
        self.st.message = message;
        found
    }

    /// Copies the whole document, translating it, and points internal links at the
    /// copies of their targets.
    fn copy_document(&mut self) -> Result<(), Fail> {
        let doc = self.doc;
        self.st.moved.clear();
        self.st.moved.resize(doc.node_count(), INVALID_NODE);
        self.st.moved[0] = ROOT;
        // The root is open already: give it the original's value, flags, and attributes.
        let root = doc.node(ROOT);
        self.b.set_value(ROOT, root.value);
        self.b.add_flags(ROOT, root.flags);
        for a in doc.attributes(ROOT) {
            let value = self.b.add_text(doc.text(a.value))?;
            let name = self.b.add_text(doc.text(a.name))?;
            self.b.add_attribute_named(ROOT, a.key, value, name)?;
        }
        for child in doc.children(ROOT) {
            self.copy_node(child, false)?;
        }
        self.remap_links();
        Ok(())
    }

    /// Copies node `id` and its subtree, translating it when it carries a message.
    /// `hidden` is true inside content that is never translated.
    fn copy_node(&mut self, id: NodeId, hidden: bool) -> Result<(), Fail> {
        let doc = self.doc;
        let n = *doc.node(id);
        let hidden = hidden || is_hidden_container(n.kind);
        let unit = if hidden { Unit::None } else { unit_of(doc, id, self.t.targets) };
        let copy = self.b.begin_node(n.kind, n.span)?;
        self.st.moved[id.index()] = copy;
        self.b.set_value(copy, n.value);
        self.b.add_flags(copy, n.flags);
        let text = self.node_text(id, unit)?;
        self.b.set_text(copy, text);
        self.copy_attributes(id, copy, unit)?;
        if !hidden {
            self.keep_anchor(id, copy)?;
        }
        self.b.copy_payload(doc, id, copy)?;
        if n.kind == NodeKind::Image {
            self.localize_image(copy)?;
        }
        let end = doc.subtree_end(id);
        let mut rest = id + 1;
        let mut translated = false;
        if unit.is_inline() {
            // Words a transform put before the written content (an info field's argument
            // and type) stay in front of the translation, as Sphinx translates the
            // content alone.
            rest = crate::units::written_content(doc, id);
            self.copy_children(id + 1, rest, hidden)?;
            translated = self.translate_inline(id)?;
        }
        if translated {
            self.copy_generated_children(id, copy, rest)?;
        } else {
            self.copy_children(rest, end, hidden)?;
        }
        if unit.is_inline() && !self.t.progress.is_empty() {
            self.mark_progress(id, copy, translated)?;
        }
        self.b.end_node(copy)
    }

    /// Keeps structural children outside the replaced inline text. Glossary entries
    /// follow the translated display text, while their target and category stay fixed.
    fn copy_generated_children(
        &mut self,
        id: NodeId,
        copy: NodeId,
        first: NodeId,
    ) -> Result<(), Fail> {
        let doc = self.doc;
        let mut term = String::new();
        if doc.kind(id) == NodeKind::Term {
            let fresh = self.b.document();
            let mut child = copy + 1;
            while child.index() < fresh.node_count() {
                fresh.plain_text(child, &mut term);
                child = fresh.subtree_end(child);
            }
        }
        for child in doc.children(id) {
            if child < first || !doc.node(child).flags.has(NodeFlag::Generated) {
                continue;
            }
            let new = NodeId::from_index(self.b.snap.nodes.len());
            self.copy_node(child, false)?;
            if doc.kind(id) == NodeKind::Term && doc.kind(child) == NodeKind::Index {
                self.set_index_term(new, &term)?;
            }
        }
        Ok(())
    }

    /// Copies the sibling subtrees from `first` up to `end`.
    fn copy_children(&mut self, first: NodeId, end: NodeId, hidden: bool) -> Result<(), Fail> {
        let mut c = first;
        while c < end {
            self.copy_node(c, hidden)?;
            c = self.doc.subtree_end(c);
        }
        Ok(())
    }

    /// Returns the copy's text: the node's, or a literal node's translation.
    fn node_text(&mut self, id: NodeId, unit: Unit) -> Result<guidedoc::Text, Fail> {
        let original = self.doc.node_text(id);
        let literal =
            unit == Unit::Literal || (unit == Unit::Graphviz && self.t.targets.literal_block);
        if literal && let Some(s) = self.lookup(literal_message(self.doc, id)) {
            self.counts.translated += 1;
            let newline = original.ends_with('\n');
            return self.b.build_text(|b| {
                b.text_push(s)?;
                if newline { b.text_push_byte(b'\n') } else { Ok(()) }
            });
        }
        self.b.add_text(original)
    }

    /// Gives a section whose title is translated, or a heading outside sections, an
    /// explicit id made from the original title, so anchors and the links to them do
    /// not change with the language.
    fn keep_anchor(&mut self, id: NodeId, copy: NodeId) -> Result<(), Fail> {
        let doc = self.doc;
        let heading = match doc.kind(id) {
            NodeKind::Section if doc.has_children(id) && doc.kind(id + 1) == NodeKind::Heading => {
                id + 1
            }
            NodeKind::Heading if id.0 == 0 || doc.kind(id - 1) != NodeKind::Section => id,
            _ => return Ok(()),
        };
        if self.inline_translation(heading).is_none() {
            return Ok(());
        }
        let mut title = String::new();
        doc.plain_text(heading, &mut title);
        let mut ident = std::mem::take(&mut self.st.scratch);
        ident.clear();
        make_id(&mut ident, &title);
        let result = if ident.is_empty() {
            Ok(())
        } else {
            self.b.add_attribute_string(copy, guidedoc::AttributeKey::Id, &ident)
        };
        self.st.scratch = ident;
        result
    }

    /// Points an image's copy at its localized file, when that file exists.
    fn localize_image(&mut self, copy: NodeId) -> Result<(), Fail> {
        let row = self.b.snap.nodes[copy.index()].payload;
        if row == NONE {
            return Ok(());
        }
        let destination = self.b.snap.links[row as usize].destination;
        let uri = self.b.pool_string(destination);
        let mut out = std::mem::take(&mut self.st.scratch);
        let found = self.t.images.localize(uri, &mut out);
        let result = if found {
            self.b.add_text(&out).map(|t| self.b.snap.links[row as usize].destination = t)
        } else {
            Ok(())
        };
        self.st.scratch = out;
        result
    }

    /// Points internal links at the copies of their targets. A link whose target has
    /// no copy points at the document root.
    fn remap_links(&mut self) {
        let moved = &self.st.moved;
        for link in self.b.snap.links.iter_mut().filter(|l| l.kind == LinkKind::Internal) {
            link.resolved = match moved.get(link.resolved.index()) {
                Some(&copy) if copy != INVALID_NODE => copy,
                _ => ROOT,
            };
        }
    }
}
