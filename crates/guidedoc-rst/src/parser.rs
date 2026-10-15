//! The block parser's state and its core: nodes, spans, nesting, classes, inline
//! content, and the dispatch of body elements.

use std::fmt;

use guidedoc::{
    Arg, AttributeKey, Builder, CORE_DEPTH, Fail, LineView, NodeId, NodeKind, ReadContext,
    SourceId, SourceSpan, StackGuard, Text, arg_int, arg_span, diagnostic, recycle, stack_guard,
};
use guidedoc_rst_inline::{InlineBuffers, InlineState, parse_inline};

use crate::extension::{Extension, ExtensionHook};
use crate::fields::{field_marker, option_item_valid};
use crate::host::Host;
use crate::lines::{Line, SplitWork, trim_blank_end};
use crate::lists::{Seq, bullet_width, enum_item_valid, parse_enum};
use crate::settings::{Settings, inline_options};
use crate::tables::{TableExtra, is_grid_border, is_simple_border};
use crate::tables_grid::GridWork;

/// Title adornment styles are single printable ASCII characters, underline-only or
/// over-and-underline, so at most 64 distinct styles and section levels exist.
pub(crate) const MAX_LEVELS: usize = 64;

/// Role directives a document may hold beyond those its main source spells out.
const ROLES_EXTRA: usize = 32;

/// A title adornment: the overline character (0 for none) and the underline character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Style {
    pub over: u8,
    pub under: u8,
}

/// An open section: its node, level, source, and where its content ends so far.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SectionEntry {
    pub id: NodeId,
    pub level: usize,
    pub src: SourceId,
    pub end: u32,
}

/// Classes a class directive gave the next element.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Pending {
    pub classes: Text,
    pub span: SourceSpan,
}

/// Line buffers lent to nested blocks and inline content (`Parser::line_buf`,
/// `Parser::view_buf`), kept empty between uses. Lines borrow one read's sources, so
/// the buffers are kept as `'static` ones and turned back with `guidedoc::recycle`.
#[derive(Debug, Default)]
pub(crate) struct LinePool {
    lines: Vec<Vec<Line<'static>>>,
    views: Vec<Vec<LineView<'static>>>,
}

/// The reader's storage, kept in the workspace's scratch between reads.
#[derive(Debug, Default)]
struct RstWork {
    inline: InlineBuffers,
    styles: Vec<Style>,
    sections: Vec<SectionEntry>,
    pool: LinePool,
    grids: Vec<GridWork>,
}

/// The main source's lines and the storage that splits them, kept in the workspace's
/// scratch between reads. Lines borrow the source, so they are kept empty.
#[derive(Debug, Default)]
pub(crate) struct MainLines {
    pub split: SplitWork,
    pub text: String,
    pub lines: Vec<Line<'static>>,
}

/// The reader's state of one document: the current source, tab stops, section levels,
/// pending classes, the inline state, and nesting depth. `read` keeps one for the call;
/// a host markup that embeds the reader (`embed_begin`) keeps it between calls and
/// views it as a `Parser` with `ReaderState::parser`.
pub struct ReaderState {
    pub(crate) inl: InlineState,
    pub(crate) roles_limit: usize,
    pub(crate) src: SourceId,
    pub(crate) text_len: u32,
    pub(crate) tab: usize,
    pub(crate) depth: usize,
    pub(crate) stack: StackGuard,
    pub(crate) styles: Vec<Style>,
    pub(crate) sections: Vec<SectionEntry>,
    /// Sections open before titled content (`parse_titled`) began.
    pub(crate) section_base: usize,
    /// The level of the last of them: titled content's sections nest in it.
    pub(crate) level_base: usize,
    /// Top-level elements since the current section began.
    pub(crate) top_count: usize,
    /// The last top-level element was a transition.
    pub(crate) top_transition: bool,
    pub(crate) transition_span: SourceSpan,
    pub(crate) pending: Option<Pending>,
    /// The running directive was written in the host markup.
    pub(crate) hosted: bool,
    /// A table directive's title and options for the table the host emits.
    pub(crate) table_extra: Option<TableExtra>,
    /// Buffers lent to nested parses.
    pub(crate) pool: LinePool,
    /// Grid tables' storage, one per table being read.
    pub(crate) grids: Vec<GridWork>,
}

impl fmt::Debug for ReaderState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReaderState")
            .field("src", &self.src)
            .field("tab", &self.tab)
            .field("depth", &self.depth)
            .field("sections", &self.sections.len())
            .finish_non_exhaustive()
    }
}

impl ReaderState {
    /// Prepares the state for the main source of `ctx` with `settings`, with the
    /// storage an earlier read left in the workspace (`end` gives it back).
    pub fn new(ctx: &mut ReadContext<'_>, settings: &Settings) -> ReaderState {
        let tab = if settings.tab_width > 0 {
            settings.tab_width
        } else if ctx.options.tab_width > 0 {
            ctx.options.tab_width
        } else {
            8
        };
        let text = ctx.source_of(ctx.source);
        let mut w: RstWork = ctx.take_scratch();
        w.styles.clear();
        w.sections.clear();
        let mut inl = InlineState::with_buffers(inline_options(settings, tab), w.inline);
        inl.source = ctx.source;
        ReaderState {
            inl,
            roles_limit: count_roles(&text) + ROLES_EXTRA,
            src: ctx.source,
            text_len: u32::try_from(text.len()).unwrap_or(u32::MAX),
            tab,
            depth: 0,
            stack: stack_guard(&ctx.limits),
            styles: w.styles,
            sections: w.sections,
            section_base: 0,
            level_base: 0,
            top_count: 0,
            top_transition: false,
            transition_span: SourceSpan::default(),
            pending: None,
            hosted: false,
            table_extra: None,
            pool: w.pool,
            grids: w.grids,
        }
    }

    /// Gives the state's storage back to the workspace for the next read; call it once
    /// when the read ends (`Parser::embed_end` and `read_inline` do). The state stays
    /// usable, with empty buffers.
    pub fn end(&mut self, ctx: &mut ReadContext<'_>) {
        let w = RstWork {
            inline: self.inl.take_buffers(),
            styles: std::mem::take(&mut self.styles),
            sections: std::mem::take(&mut self.sections),
            pool: std::mem::take(&mut self.pool),
            grids: std::mem::take(&mut self.grids),
        };
        ctx.put_scratch(w);
    }

    /// Views the state as a parser over `ctx`, with a dialect's handlers and a host.
    pub fn parser<'p, 'a>(
        &'p mut self,
        ctx: &'p mut ReadContext<'a>,
        ext: Option<&'p dyn Extension>,
        host: Option<&'p dyn Host>,
    ) -> Parser<'p, 'a> {
        Parser { ctx, st: self, ext, host }
    }
}

/// Bounds the role directives of a source: "role::", and "{role}" in Markdown.
fn count_roles(text: &str) -> usize {
    text.matches("role::").count() + text.matches("{role}").count()
}

/// The block parser: a read context, the reader's state, and the dialect's handlers and
/// host. Handlers receive it; `ctx.b` builds nodes.
pub struct Parser<'p, 'a> {
    /// The read context: builder, options, sources, reports.
    pub ctx: &'p mut ReadContext<'a>,
    pub(crate) st: &'p mut ReaderState,
    pub(crate) ext: Option<&'p dyn Extension>,
    pub(crate) host: Option<&'p dyn Host>,
}

impl fmt::Debug for Parser<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Parser").field("st", &self.st).finish_non_exhaustive()
    }
}

/// What an unindented, non-blank line starts, in Docutils' body order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Construct {
    Text,
    Bullet,
    Enumerated,
    Field,
    Option,
    Doctest,
    LineBlock,
    Grid,
    Simple,
    Explicit,
    Anonymous,
    Adornment,
}

/// Classifies the unindented, non-blank line `i`.
pub(crate) fn detect(lines: &[Line<'_>], i: usize) -> Construct {
    let t = lines[i].text;
    if bullet_width(t) > 0 {
        return Construct::Bullet;
    }
    if parse_enum(t, Seq::None).is_some_and(|e| enum_item_valid(lines, i, &e)) {
        return Construct::Enumerated;
    }
    if field_marker(t).is_some() {
        return Construct::Field;
    }
    if option_item_valid(lines, i) {
        return Construct::Option;
    }
    match () {
        () if marker(t, ">>>") => Construct::Doctest,
        () if marker(t, "|") => Construct::LineBlock,
        () if is_grid_border(t) => Construct::Grid,
        () if is_simple_border(t) => Construct::Simple,
        () if marker(t, "..") => Construct::Explicit,
        () if marker(t, "__") => Construct::Anonymous,
        () if is_adornment(t) => Construct::Adornment,
        () => Construct::Text,
    }
}

/// Tells whether `t` starts with `m` followed by a space or the end of the line.
pub(crate) fn marker(t: &str, m: &str) -> bool {
    t.strip_prefix(m).is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
}

/// Printable 7-bit punctuation, the characters of adornments and quoted literals.
pub(crate) fn is_punct7(c: u8) -> bool {
    matches!(c, b'!'..=b'/' | b':'..=b'@' | b'['..=b'`' | b'{'..=b'~')
}

/// Tells whether `t` is one punctuation character repeated.
pub(crate) fn is_adornment(t: &str) -> bool {
    let b = t.as_bytes();
    !b.is_empty() && is_punct7(b[0]) && b.iter().all(|&c| c == b[0])
}

impl<'p, 'a> Parser<'p, 'a> {
    /// The builder of the document being read.
    pub fn b(&mut self) -> &mut Builder<'a> {
        &mut self.ctx.b
    }

    /// The source the current lines come from.
    pub fn src(&self) -> SourceId {
        self.st.src
    }

    /// The tab stops of the current source.
    pub fn tab(&self) -> usize {
        self.st.tab
    }

    /// Tells whether the running directive was written in the host markup.
    pub fn is_hosted(&self) -> bool {
        self.st.hosted
    }

    /// The inline state: custom roles and the default role.
    pub fn inline_state(&mut self) -> &mut InlineState {
        &mut self.st.inl
    }

    /// The stack guard of the read, for a handler's own recursion (a C++ parser).
    pub fn stack_guard(&self) -> StackGuard {
        self.st.stack
    }

    /// Runs role `name` on interpreted text written in the host markup (MyST's
    /// `{name}` roles), with the dialect's roles first; see
    /// `guidedoc_rst_inline::apply_host_role`. Fails only when the builder fails.
    pub fn apply_host_role(
        &mut self,
        name: &str,
        lines: &[LineView<'_>],
        content: std::ops::Range<usize>,
    ) -> Result<(), Fail> {
        self.st.inl.source = self.st.src;
        let mut hook = ExtensionHook(self.ext);
        guidedoc_rst_inline::apply_host_role(
            self.ctx,
            &mut self.st.inl,
            &mut hook,
            name,
            lines,
            content,
        )
    }

    /// Takes the classes a class directive left for the next element, if any.
    pub fn take_pending_classes(&mut self) -> Option<Text> {
        self.st.pending.take().map(|p| p.classes)
    }

    /// Reports message `m` at `span` with its arguments. Reports never fail the read.
    pub fn report(&mut self, m: guidedoc::Message, span: SourceSpan, a: Arg, b: Arg) {
        self.ctx.report(m, span, a, b);
    }

    /// Reports `m` at `span` with the span quoted as its argument.
    pub(crate) fn report_at(&mut self, m: guidedoc::Message, span: SourceSpan) {
        self.ctx.report(m, span, arg_span(span), Arg::None);
    }

    /// Begins an element and gives it the classes of a preceding class directive.
    pub fn open(&mut self, kind: NodeKind, span: SourceSpan) -> Result<NodeId, Fail> {
        let id = self.ctx.b.begin_node(kind, span)?;
        if let Some(p) = self.st.pending.take() {
            self.ctx.b.add_attribute(id, AttributeKey::Class, p.classes)?;
        }
        Ok(id)
    }

    /// Ends the element `open` returned `id` for.
    pub fn close(&mut self, id: NodeId) -> Result<(), Fail> {
        self.ctx.b.end_node(id)
    }

    /// Adds an element without children, with text `text`.
    pub(crate) fn leaf(
        &mut self,
        kind: NodeKind,
        span: SourceSpan,
        text: Text,
    ) -> Result<(), Fail> {
        let id = self.open(kind, span)?;
        self.ctx.b.set_text(id, text);
        self.close(id)
    }

    /// The source span from the start of `first` to the end of `last`, in the current
    /// source, clamped to its text.
    pub fn span_of(&self, first: &Line<'_>, last: &Line<'_>) -> SourceSpan {
        let limit = self.st.text_len;
        let a = first.start(self.st.tab).min(limit);
        let z = last.end(self.st.tab).max(a).min(limit);
        SourceSpan::new(self.st.src, a, z)
    }

    /// The source span of one line.
    pub fn line_span(&self, l: &Line<'_>) -> SourceSpan {
        self.span_of(l, l)
    }

    /// The source span of `lines` without trailing blank lines; empty lines give an
    /// empty span at the start of the source.
    pub fn lines_span(&self, lines: &[Line<'_>]) -> SourceSpan {
        let Some(first) = lines.first() else {
            return SourceSpan::new(self.st.src, 0, 0);
        };
        let kept = trim_blank_end(lines);
        let last = kept.last().unwrap_or(first);
        self.span_of(first, last)
    }

    /// Runs `f` one nesting level deeper, failing past `Limits::max_depth` or the stack
    /// budget.
    pub(crate) fn nested<R>(
        &mut self,
        span: SourceSpan,
        f: impl FnOnce(&mut Self) -> Result<R, Fail>,
    ) -> Result<R, Fail> {
        let max = self.ctx.limits.max_depth;
        if self.st.depth >= max {
            let d = diagnostic(
                CORE_DEPTH,
                span,
                arg_int(i64::try_from(max).unwrap_or(i64::MAX)),
                Arg::None,
            );
            return Err(self.ctx.b.fail(d));
        }
        if self.st.stack.is_exhausted() {
            let d = self.st.stack.diagnostic(span);
            return Err(self.ctx.b.fail(d));
        }
        self.st.depth += 1;
        let r = f(self);
        self.st.depth -= 1;
        r
    }

    /// Leaves `classes` for the next element; classes already pending are kept.
    pub(crate) fn set_pending(&mut self, classes: Text, span: SourceSpan) -> Result<(), Fail> {
        if classes.len == 0 {
            return Ok(());
        }
        let classes = match self.st.pending {
            Some(p) if p.classes != classes => {
                self.ctx.b.spend(u64::from(p.classes.len + classes.len))?;
                let joined = format!(
                    "{} {}",
                    self.ctx.b.pool_string(p.classes),
                    self.ctx.b.pool_string(classes)
                );
                self.ctx.b.add_text(&joined)?
            }
            _ => classes,
        };
        self.st.pending = Some(Pending { classes, span });
        Ok(())
    }

    /// Lends an empty line buffer from the reader's pool; give it back with
    /// `give_lines` once its lines are used.
    pub(crate) fn line_buf<'s>(&mut self) -> Vec<Line<'s>> {
        recycle(self.st.pool.lines.pop().unwrap_or_default())
    }

    /// Gives a line buffer back to the pool.
    pub(crate) fn give_lines(&mut self, lines: Vec<Line<'_>>) {
        self.st.pool.lines.push(recycle(lines));
    }

    /// Lends an empty view buffer from the reader's pool (see `line_buf`).
    pub(crate) fn view_buf<'s>(&mut self) -> Vec<LineView<'s>> {
        recycle(self.st.pool.views.pop().unwrap_or_default())
    }

    /// Gives a view buffer back to the pool.
    pub(crate) fn give_views(&mut self, views: Vec<LineView<'_>>) {
        self.st.pool.views.push(recycle(views));
    }

    /// Parses `lines` as inline content of the open node, in the markup of the running
    /// directive (see `Host`).
    pub fn inline(&mut self, lines: &[Line<'_>]) -> Result<(), Fail> {
        if lines.is_empty() {
            return Ok(());
        }
        if self.st.hosted
            && let Some(host) = self.host
        {
            return host.inline(self, lines);
        }
        let tab = self.st.tab;
        let mut views = self.view_buf();
        views.extend(lines.iter().map(|l| l.view(tab)));
        let result = self.inline_views(&views);
        self.give_views(views);
        result
    }

    /// Parses source views as reStructuredText inline content of the open node.
    pub fn inline_views(&mut self, views: &[LineView<'_>]) -> Result<(), Fail> {
        self.st.inl.source = self.st.src;
        let mut hook = ExtensionHook(self.ext);
        parse_inline(self.ctx, &mut self.st.inl, views, &mut hook)
    }

    /// Adds a node of `kind` holding `lines` parsed as inline content; nothing for no
    /// lines.
    pub fn inline_node(&mut self, kind: NodeKind, lines: &[Line<'_>]) -> Result<(), Fail> {
        if lines.is_empty() {
            return Ok(());
        }
        let id = self.ctx.b.begin_node(kind, self.lines_span(lines))?;
        self.inline(lines)?;
        self.close(id)
    }

    /// Parses nested body content of the open node in the markup of the running
    /// directive (see `Host`); outside hosted directives it is `parse_blocks`.
    pub fn parse_body(
        &mut self,
        lines: &[Line<'_>],
        sections: bool,
        each: Option<Text>,
    ) -> Result<(), Fail> {
        if self.st.hosted
            && let Some(host) = self.host
        {
            if lines.is_empty() {
                return Ok(());
            }
            return host.body(self, lines, sections, each);
        }
        self.parse_blocks(lines, sections, each)
    }

    /// Parses body elements as reStructuredText. `sections` allows titles and
    /// transitions; `each` gives classes to every element parsed at this level (the
    /// class directive's content).
    pub fn parse_blocks(
        &mut self,
        lines: &[Line<'_>],
        sections: bool,
        each: Option<Text>,
    ) -> Result<(), Fail> {
        let Some(first) = lines.first() else {
            return Ok(());
        };
        let span = self.line_span(first);
        self.nested(span, |p| {
            let hosted = std::mem::replace(&mut p.st.hosted, false);
            let r = p.blocks_loop(lines, sections, each);
            p.st.hosted = hosted;
            r
        })
    }

    fn blocks_loop(
        &mut self,
        lines: &[Line<'_>],
        sections: bool,
        each: Option<Text>,
    ) -> Result<(), Fail> {
        let mut i = 0;
        while i < lines.len() {
            if lines[i].is_blank() {
                i += 1;
                continue;
            }
            self.ctx.b.spend(1)?;
            if let Some(each) = each
                && !self.st.pending.is_some_and(|p| p.classes == each)
            {
                let span = self.line_span(&lines[i]);
                self.set_pending(each, span)?;
            }
            let before = self.ctx.b.snap.nodes.len();
            let n = self.parse_block(lines, i, sections)?.max(1);
            if sections {
                self.note_top(&lines[i..(i + n).min(lines.len())], before);
            }
            i += n;
        }
        Ok(())
    }

    fn parse_block(
        &mut self,
        lines: &[Line<'_>],
        i: usize,
        sections: bool,
    ) -> Result<usize, Fail> {
        if lines[i].is_indented() {
            return self.block_quote(lines, i);
        }
        match detect(lines, i) {
            Construct::Bullet => self.bullet_list(lines, i),
            Construct::Enumerated => self.enumerated_list(lines, i),
            Construct::Field => self.field_list(lines, i),
            Construct::Option => self.option_list(lines, i),
            Construct::Doctest => self.doctest_block(lines, i),
            Construct::LineBlock => self.line_block(lines, i),
            Construct::Grid => self.grid_table(lines, i, None),
            Construct::Simple => self.simple_table(lines, i, None),
            Construct::Explicit => self.explicit(lines, i, sections),
            Construct::Anonymous => self.anonymous_target(lines, i),
            Construct::Adornment => self.adornment(lines, i, sections),
            Construct::Text => self.text_block(lines, i, sections),
        }
    }
}
