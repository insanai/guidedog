//! The renderer's state: the output writer, the document-wide settings found before
//! rendering, and the helpers that write tags, ids, and classes.

use guidedoc::{
    Anchors, Arg, AttributeKey, Document, Limits, Message, NodeId, NodeKind, Policy, ROOT,
    RenderContext, RenderOptions, Severity, SourceSpan, StackGuard, Status, kept_message,
    stack_guard,
};

use crate::notes::Notes;
use crate::walk::Close;

/// Writes the output and records problems. It remembers the last byte written, so
/// block elements can start on a line of their own without doubling newlines.
pub(crate) struct Writer<'c, 'a> {
    ctx: &'c mut RenderContext<'a>,
    /// The last byte written; 0 after a highlighter wrote an unknown amount.
    pub last: u8,
    /// Writing the CommonMark reference renderer's bytes.
    pub reference: bool,
    /// `Ok`, or the status of the first error reported.
    pub status: Status,
}

impl<'c, 'a> Writer<'c, 'a> {
    /// Writes `s`.
    pub fn put(&mut self, s: &str) {
        if let Some(&c) = s.as_bytes().last() {
            self.ctx.out.emit(s);
            self.last = c;
        }
    }

    /// Writes one byte.
    pub fn put_byte(&mut self, c: u8) {
        self.ctx.out.emit_byte(c);
        self.last = c;
    }

    /// Writes a number in decimal.
    pub fn put_int(&mut self, value: i64) {
        self.ctx.out.emit_int(value);
        self.last = b'0';
    }

    /// Starts a new line unless the output already ends with one.
    pub fn cr(&mut self) {
        if self.last != b'\n' {
            self.put_byte(b'\n');
        }
    }

    /// Writes `</name>`.
    pub fn close(&mut self, name: &str) {
        self.put("</");
        self.put(name);
        self.put_byte(b'>');
    }

    /// Lets the next element start on the current line, as Docutils writes a field
    /// body's or a list item's first child.
    pub fn continue_line(&mut self) {
        self.last = b'\n';
    }

    /// Runs the caller's highlighter on `code`; false means it wrote nothing.
    pub fn highlight(&mut self, language: &str, code: &str) -> bool {
        let Some(highlighter) = self.ctx.options.highlighter else {
            return false;
        };
        if language.is_empty() || code.is_empty() {
            return false;
        }
        if !highlighter.highlight(language, code, &mut self.ctx.out) {
            return false;
        }
        self.last = 0;
        true
    }

    /// Reports `m`, as a warning when the policy keeps documents with content errors;
    /// an error also fails the render with its status.
    pub fn fail(&mut self, m: Message, span: SourceSpan, a: Arg, b: Arg) {
        let kept = kept_message(&self.ctx.policy, m);
        self.ctx.report(kept, span, a, b);
        if kept.severity == Severity::Error && self.status == Status::Ok {
            self.status = kept.status;
        }
    }
}

/// The state of sectnum numbering while rendering.
#[derive(Debug, Default)]
pub(crate) struct SectionNumbers<'a> {
    pub enabled: bool,
    /// 0 means unlimited.
    pub depth: usize,
    pub start: i64,
    pub prefix: &'a str,
    pub suffix: &'a str,
    pub counters: [i64; 16],
}

impl SectionNumbers<'_> {
    /// Tells whether sections at depth `d` (1 = top) are numbered.
    pub fn numbered(&self, d: usize) -> bool {
        self.enabled && d >= 1 && d <= self.counters.len() && (self.depth == 0 || d <= self.depth)
    }
}

/// Counts a section at depth `d` and resets the deeper counters.
pub(crate) fn count_section(counters: &mut [i64], d: usize, start: i64) {
    if d < 1 || d > counters.len() {
        return;
    }
    counters[d - 1] =
        if counters[d - 1] == 0 { if d == 1 { start } else { 1 } } else { counters[d - 1] + 1 };
    counters[d..].fill(0);
}

/// The whole renderer: the writer, the document, and what the walk needs.
pub(crate) struct Html<'c, 'a> {
    pub w: Writer<'c, 'a>,
    pub doc: Document<'a>,
    pub options: &'a RenderOptions<'a>,
    pub policy: Policy,
    pub limits: Limits,
    pub anchors: Anchors,
    pub notes: Notes,
    pub numbers: SectionNumbers<'a>,
    /// The document has a Title node, so sections start at h2.
    pub has_title: bool,
    /// Substitutions being expanded.
    pub expansion: usize,
    /// The widest token of the production list being rendered.
    pub productions: usize,
    /// The first and last tables of contents, for backlinks.
    pub contents: Option<(NodeId, NodeId)>,
    /// Bounds the machine stack of nested walks (substitutions).
    pub guard: StackGuard,
    /// The open ancestors of the walks in progress; each walk uses the part from its
    /// `base` on (see walk.rs).
    pub stack: Vec<(NodeId, Close)>,
    pub base: usize,
    /// The parent of the current walk's top-level nodes, when known.
    pub outer: Option<NodeId>,
}

impl<'c, 'a> Html<'c, 'a> {
    /// Makes the renderer for one render of `ctx`.
    pub fn new(ctx: &'c mut RenderContext<'a>, anchors: Anchors) -> Self {
        let doc = ctx.doc;
        let options = ctx.options;
        let policy = ctx.policy;
        let limits = ctx.limits;
        let reference = options.reference_profile;
        Html {
            w: Writer { ctx, last: b'\n', reference, status: Status::Ok },
            doc,
            options,
            policy,
            limits,
            anchors,
            notes: Notes::default(),
            numbers: SectionNumbers::default(),
            has_title: false,
            expansion: 0,
            productions: 0,
            contents: None,
            guard: stack_guard(&limits),
            stack: Vec::new(),
            base: 0,
            outer: None,
        }
    }

    /// Finds document-wide settings: a promoted title, sectnum, and where tables of
    /// contents are.
    pub fn scan_document(&mut self) {
        let doc = self.doc;
        let first = doc.first_child(ROOT);
        self.has_title =
            !self.w.reference && first.is_some_and(|f| doc.kind(f) == NodeKind::Title);
        self.numbers.enabled = self.options.section_numbers;
        self.numbers.start = 1;
        let mut sectnum = false;
        for i in 0..doc.node_count() {
            let id = NodeId::from_index(i);
            if doc.kind(id) == NodeKind::Contents {
                let first = self.contents.map_or(id, |(first, _)| first);
                self.contents = Some((first, id));
            }
            if doc.kind(id) != NodeKind::Directive || self.directive_name(id) != "sectnum" {
                continue;
            }
            if sectnum {
                continue; // the first one counts.
            }
            sectnum = true;
            self.numbers.enabled = true;
            self.numbers.depth = parse_int(self.custom(id, "depth"), 0) as usize;
            self.numbers.start = parse_int(self.custom(id, "start"), 1);
            self.numbers.prefix = self.custom(id, "prefix");
            self.numbers.suffix = self.custom(id, "suffix");
        }
    }

    /// Returns a directive's name, or "".
    pub fn directive_name(&self, id: NodeId) -> &'a str {
        self.doc.attribute(id, AttributeKey::Name).unwrap_or("")
    }

    /// Returns the Custom attribute `name` of a node, or "".
    pub fn custom(&self, id: NodeId, name: &str) -> &'a str {
        self.doc.custom_attribute(id, name).unwrap_or("")
    }

    /// Returns the kind of a parent; the top of the tree counts as the document.
    pub fn parent_kind(&self, parent: Option<NodeId>) -> NodeKind {
        parent.map_or(NodeKind::Document, |p| self.doc.kind(p))
    }

    /// Tells whether a node has an anchor the output may show.
    pub fn has_anchor(&self, id: NodeId) -> bool {
        !self.w.reference && self.anchors.anchor_of(id).is_some()
    }

    /// Writes `<name` with the node's anchor id and classes, without the closing ">".
    /// A node's other ids become empty spans: before list-like, tabular, and empty
    /// elements, as Docutils places them, and otherwise as the first content (`tag`).
    pub fn open_tag(&mut self, name: &str, id: NodeId, base_class: &str) {
        self.open_tag_with(name, id, base_class, true);
    }

    /// `open_tag`, with or without the node's ids.
    pub fn open_tag_with(&mut self, name: &str, id: NodeId, base_class: &str, with_id: bool) {
        if with_id && self.aliases_before(id) {
            self.write_aliases(id);
        }
        self.w.put_byte(b'<');
        self.w.put(name);
        if with_id {
            self.write_id_attribute(id);
        }
        self.write_classes(id, base_class);
    }

    /// Writes ` class="base own..."` when there are classes; a node's class already in
    /// `base_class` is not repeated.
    pub fn write_classes(&mut self, id: NodeId, base_class: &str) {
        let mut wrote = false;
        if !base_class.is_empty() {
            self.w.put(" class=\"");
            self.w.esc(base_class);
            wrote = true;
        }
        for a in self.doc.attributes(id) {
            if a.key != AttributeKey::Class || a.value.is_empty() {
                continue;
            }
            let class = self.doc.text(a.value);
            if base_class.split(' ').any(|c| c == class) {
                continue;
            }
            self.w.put(if wrote { " " } else { " class=\"" });
            self.w.esc(class);
            wrote = true;
        }
        if wrote {
            self.w.put_byte(b'"');
        }
    }

    /// Continues an open class attribute with the node's classes.
    pub fn write_more_classes(&mut self, id: NodeId) {
        for a in self.doc.attributes(id) {
            if a.key != AttributeKey::Class || a.value.is_empty() {
                continue;
            }
            self.w.put_byte(b' ');
            self.w.esc(self.doc.text(a.value));
        }
    }

    /// Writes a node's classes, space-separated, and tells whether there were any.
    pub fn written_classes(&mut self, id: NodeId) -> bool {
        let mut wrote = false;
        for a in self.doc.attributes(id) {
            if a.key != AttributeKey::Class || a.value.is_empty() {
                continue;
            }
            if wrote {
                self.w.put_byte(b' ');
            }
            self.w.esc(self.doc.text(a.value));
            wrote = true;
        }
        wrote
    }

    /// Writes ` id="primary"` when the node has an anchor.
    pub fn write_id_attribute(&mut self, id: NodeId) {
        if self.w.reference {
            return;
        }
        if let Some(a) = self.anchors.anchor_of(id) {
            self.w.put(" id=\"");
            self.w.esc(a);
            self.w.put_byte(b'"');
        }
    }

    /// Writes a complete start tag with the node's anchor and classes.
    pub fn tag(&mut self, name: &str, id: NodeId, base_class: &str) {
        self.open_tag(name, id, base_class);
        self.w.put_byte(b'>');
        if !self.aliases_before(id) {
            self.write_aliases(id);
        }
    }

    /// Tells whether a node's extra ids go before its element: lists, tables, and
    /// empty elements cannot hold a span as their first child.
    pub fn aliases_before(&self, id: NodeId) -> bool {
        use NodeKind as K;
        matches!(
            self.doc.kind(id),
            K::List
                | K::DefinitionList
                | K::FieldList
                | K::OptionList
                | K::Table
                | K::ThematicBreak
                | K::Image
                | K::Hlist
                | K::Glossary
                | K::Desc
                | K::Index
        )
    }

    /// Writes every id of a node after its primary one as an empty span.
    pub fn write_aliases(&mut self, id: NodeId) {
        if self.w.reference {
            return;
        }
        for name in self.anchors.anchor_ids(id).skip(1) {
            self.w.put("<span id=\"");
            self.w.esc(name);
            self.w.put("\"></span>");
        }
    }

    /// Writes every id of a node as an empty span, for nodes with no element.
    pub fn write_all_ids(&mut self, id: NodeId) {
        if self.w.reference {
            return;
        }
        for name in self.anchors.anchor_ids(id) {
            self.w.put("<span id=\"");
            self.w.esc(name);
            self.w.put("\"></span>");
        }
    }

    /// Writes the permalink Sphinx puts after headings, captions, and signatures.
    pub fn headerlink(&mut self, target: Option<NodeId>, title: &str) {
        if self.w.reference || !self.options.permalinks {
            return;
        }
        let Some(a) = target.and_then(|t| self.anchors.anchor_of(t)) else {
            return;
        };
        self.w.put("<a class=\"headerlink\" href=\"#");
        self.w.esc_code(a); // Sphinx writes the permalink without encoding it.
        self.w.put("\" title=\"");
        self.w.esc_code(title);
        self.w.put("\">");
        let icon = self.options.permalink_icon;
        self.w.put(if icon.is_empty() { "¶" } else { icon });
        self.w.put("</a>");
    }

    /// Block elements holding blocks put their tags on lines of their own.
    pub fn block_open(&mut self, name: &str, id: NodeId, class: &str) {
        self.w.cr();
        self.tag(name, id, class);
        self.w.cr();
    }

    /// Closes what `block_open` opened.
    pub fn block_close(&mut self, name: &str) {
        self.w.cr();
        self.w.close(name);
        self.w.cr();
    }

    /// Elements holding inline content start on a new line.
    pub fn line_open(&mut self, name: &str, id: NodeId, class: &str) {
        self.w.cr();
        self.tag(name, id, class);
    }

    /// Closes what `line_open` opened and ends the line.
    pub fn line_close(&mut self, name: &str) {
        self.w.close(name);
        self.w.cr();
    }
}

/// Parses a small decimal number; `fallback` for anything else.
pub(crate) fn parse_int(s: &str, fallback: i64) -> i64 {
    if s.is_empty() || s.len() > 9 || !s.bytes().all(|c| c.is_ascii_digit()) {
        return fallback;
    }
    s.parse().unwrap_or(fallback)
}

/// Counts the characters of `s`.
pub(crate) fn char_count(s: &str) -> usize {
    s.chars().count()
}
