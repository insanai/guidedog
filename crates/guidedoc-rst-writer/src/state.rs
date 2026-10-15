//! The writer's state: the output, indentation frames, blank-line bookkeeping,
//! recursion guards, and reporting.

use guidedoc::{
    Arg, AttributeKey, CORE_STACK, Document, DocumentReading, NodeId, NodeKind, ROOT,
    RenderContext, Severity, StackGuard, Status, arg_int, arg_static, arg_text, kept_message,
    stack_guard,
};

use std::fmt;

use crate::inline::{Inline, Lines};
use crate::roles::RoleTable;
use crate::tables::TableWork;
use crate::{DEPTH_LIMIT, LOSSY, Settings};

/// One level of indentation. A marker ("- ", "1. ", ":name: ") takes the place of the
/// indentation on the first line written inside the frame; it may be wider than the
/// indentation of the lines after it.
#[derive(Debug, Default)]
struct Frame {
    indent: usize,
    marker: String,
    /// Whether the marker is still to be written.
    pending: bool,
}

/// The writer's storage, kept in the workspace's scratch between renders.
#[derive(Debug, Default)]
pub(crate) struct WriterWork {
    out: String,
    frames: Vec<Frame>,
    inline: Inline,
    inlines: Vec<Inline>,
    lines: Vec<Lines>,
    strings: Vec<String>,
    line_buf: String,
    tables: Vec<TableWork>,
    pub text: String,
}

/// The state of one render. Blocks write whole lines; inline content collects in
/// `inline` until the block that owns it wraps and writes it.
pub(crate) struct Writer<'c, 'a> {
    ctx: &'c mut RenderContext<'a>,
    pub doc: Document<'a>,
    pub settings: Settings,
    pub out: String,
    frames: Vec<Frame>,
    /// The first frame of the current output (a captured table cell starts its own).
    frame_base: usize,
    /// Frames in use; frames past it are kept for reuse.
    depth: usize,
    /// A blank line is due before the next block.
    pub blank: bool,
    /// The inline content being collected, when a block collects one.
    pub inline: Inline,
    /// Whether inline nodes go to `inline` (true) or are blocks (false).
    pub in_inline: bool,
    /// The roles the document uses that the reader must be told about.
    pub roles: RoleTable,
    /// Read by the Sphinx layer: Sphinx's directive names apply.
    pub sphinx: bool,
    /// Section levels start at this adornment (a promoted title takes the first).
    pub adornment_base: usize,
    /// The module each domain's objects are written in (`py:currentmodule`).
    pub modules: Vec<(String, String)>,
    /// The block written before the one being written, in the same container.
    pub prev_block: Option<NodeId>,
    /// The end of the container whose children are being written.
    pub container_end: u32,
    /// The paragraph just written announced a literal block with "::".
    pub literal_announced: bool,
    /// Images met inside text, whose substitution definitions follow the block.
    pub pending_images: Vec<(String, NodeId, Option<NodeId>)>,
    /// Bytes of the next Text node an `:index:` role already wrote.
    pub skip_text: usize,
    /// Comments and targets the reader moved after the docinfo from before the title,
    /// written back before it.
    pub moved: Vec<NodeId>,
    /// Comments and targets already written out of order.
    pub done: Vec<NodeId>,
    /// Lines may break inside markup (a table cell of fixed width).
    pub tight_wrap: bool,
    /// Inline buffers for nested collections, kept for reuse.
    inlines: Vec<Inline>,
    /// Wrapped lines, kept for reuse (`collect`, `give_lines`).
    lines: Vec<Lines>,
    /// Strings, kept for reuse (`buffer`, `recycle`).
    strings: Vec<String>,
    /// Room for a formatted line (`line_fmt`).
    line_buf: String,
    /// Tables' storage, one per table being written.
    pub tables: Vec<TableWork>,
    nesting: usize,
    nesting_limit: usize,
    guard: StackGuard,
    pub status: Status,
}

impl<'c, 'a> Writer<'c, 'a> {
    /// Starts a render with the storage in `work`; `finish` gives it back.
    pub fn new(ctx: &'c mut RenderContext<'a>, settings: Settings, work: WriterWork) -> Self {
        let (doc, limits) = (ctx.doc, ctx.limits);
        let sphinx = doc.node(ROOT).value == DocumentReading::Sphinx as i32;
        let WriterWork {
            mut out,
            frames,
            mut inline,
            inlines,
            lines,
            strings,
            line_buf,
            tables,
            ..
        } = work;
        out.clear();
        inline.clear(false);
        Writer {
            ctx,
            doc,
            settings,
            out,
            frames,
            frame_base: 0,
            depth: 0,
            blank: false,
            inline,
            inlines,
            lines,
            strings,
            line_buf,
            tables,
            in_inline: false,
            roles: RoleTable::default(),
            sphinx,
            adornment_base: 0,
            modules: Vec::new(),
            prev_block: None,
            container_end: u32::MAX,
            literal_announced: false,
            pending_images: Vec::new(),
            skip_text: 0,
            tight_wrap: false,
            moved: Vec::new(),
            done: Vec::new(),
            nesting: 0,
            nesting_limit: limits.max_depth.max(32) + 64,
            guard: stack_guard(&limits),
            status: Status::Ok,
        }
    }

    /// Ends the render: returns the storage, with `text` for the final output.
    pub fn finish(self, text: String) -> WriterWork {
        let Writer { out, frames, inline, inlines, lines, strings, line_buf, tables, .. } = self;
        WriterWork { out, frames, inline, inlines, lines, strings, line_buf, tables, text }
    }

    /// Returns an empty string, reused from earlier ones; give it back with `recycle`.
    pub fn buffer(&mut self) -> String {
        self.strings.pop().unwrap_or_default()
    }

    /// Gives a string back for reuse.
    pub fn recycle(&mut self, mut s: String) {
        s.clear();
        self.strings.push(s);
    }

    /// Returns empty lines, reused from earlier ones; give them back with `give_lines`.
    pub fn take_lines(&mut self) -> Lines {
        let mut lines = self.lines.pop().unwrap_or_default();
        lines.clear();
        lines
    }

    /// Gives lines back for reuse.
    pub fn give_lines(&mut self, lines: Lines) {
        self.lines.push(lines);
    }

    /// Puts an empty inline buffer in place of the current one and returns the
    /// current one; `end_inline` puts it back.
    pub fn begin_inline(&mut self, preserve: bool) -> Inline {
        let mut fresh = self.inlines.pop().unwrap_or_default();
        fresh.clear(preserve);
        std::mem::replace(&mut self.inline, fresh)
    }

    /// Puts `saved` back as the current inline buffer and returns the one used since
    /// `begin_inline`; give it back with `give_inline` once read.
    pub fn end_inline(&mut self, saved: Inline) -> Inline {
        std::mem::replace(&mut self.inline, saved)
    }

    /// Gives an inline buffer back for reuse.
    pub fn give_inline(&mut self, inline: Inline) {
        self.inlines.push(inline);
    }

    /// Writes one line formatted from `args` (see `line`).
    pub fn line_fmt(&mut self, args: fmt::Arguments<'_>) {
        let mut text = std::mem::take(&mut self.line_buf);
        text.clear();
        let _ = fmt::write(&mut text, args);
        self.line(&text);
        self.line_buf = text;
    }

    /// Reports that `what` could not be written exactly (a Note); `how` says what was
    /// written instead.
    pub fn lossy(&mut self, id: NodeId, what: &'static str, how: &str) {
        let span = self.doc.node(id).span;
        self.ctx.report(LOSSY, span, arg_static(what), arg_text(how));
    }

    /// Reports an error that fails the render, or a warning under
    /// `keep_documents_with_errors`.
    fn fail(&mut self, m: guidedoc::Message, id: NodeId, a: Arg) {
        let kept = kept_message(&self.ctx.policy, m);
        let span = self.doc.node(id).span;
        self.ctx.report(kept, span, a, Arg::None);
        if kept.severity == Severity::Error && self.status == Status::Ok {
            self.status = kept.status;
        }
    }

    /// Guards recursion; every nested block and inline passes through it. Pair a true
    /// result with `leave`.
    pub fn enter(&mut self, id: NodeId) -> bool {
        if self.status != Status::Ok {
            return false;
        }
        if self.nesting >= self.nesting_limit {
            let limit = arg_int(i64::try_from(self.nesting_limit).unwrap_or(i64::MAX));
            self.fail(DEPTH_LIMIT, id, limit);
            return false;
        }
        if self.guard.is_exhausted() {
            let limit = arg_int(i64::try_from(self.guard.limit()).unwrap_or(i64::MAX));
            self.fail(CORE_STACK, id, limit);
            return false;
        }
        self.nesting += 1;
        true
    }

    pub fn leave(&mut self) {
        self.nesting -= 1;
    }

    // Attribute shorthands.

    pub fn attr(&self, id: NodeId, key: AttributeKey) -> Option<&'a str> {
        self.doc.attribute(id, key)
    }

    pub fn custom(&self, id: NodeId, name: &str) -> Option<&'a str> {
        self.doc.custom_attribute(id, name)
    }

    pub fn kind(&self, id: NodeId) -> NodeKind {
        self.doc.kind(id)
    }

    // Lines and frames.

    /// Separates the next block from the one before it by a blank line, unless it
    /// starts on a pending marker's line.
    pub fn block_start(&mut self) {
        let on_marker = self.frames[self.frame_base..self.depth].iter().any(|f| f.pending);
        if self.blank && !on_marker && !self.out.is_empty() {
            self.out.push('\n');
        }
        self.blank = false;
    }

    /// Asks for a blank line before the next block.
    pub fn block_end(&mut self) {
        self.blank = true;
    }

    /// The width of the indentation of continuation lines.
    pub fn indentation(&self) -> usize {
        self.frames[self.frame_base..self.depth].iter().map(|f| f.indent).sum()
    }

    /// The width of the first line's indentation, markers included.
    pub fn first_indentation(&self) -> usize {
        self.frames[self.frame_base..self.depth]
            .iter()
            .map(|f| if f.pending { f.indent.max(f.marker.chars().count()) } else { f.indent })
            .sum()
    }

    /// The width wrapped text may use on lines after the first.
    pub fn available(&self) -> usize {
        self.settings.width.saturating_sub(self.indentation()).max(self.settings.width / 3)
    }

    /// Writes one line: the indentation (or pending markers) and `text`. An empty
    /// text writes an empty line without indentation, unless a marker is pending.
    pub fn line(&mut self, text: &str) {
        let pending = self.frames[self.frame_base..self.depth].iter().any(|f| f.pending);
        if text.is_empty() && !pending {
            self.out.push('\n');
            return;
        }
        for f in &mut self.frames[self.frame_base..self.depth] {
            if f.pending {
                self.out.push_str(&f.marker);
                f.pending = false;
                let pad = f.indent.saturating_sub(f.marker.chars().count());
                self.out.extend(std::iter::repeat_n(' ', pad));
            } else {
                self.out.extend(std::iter::repeat_n(' ', f.indent));
            }
        }
        if text.is_empty() {
            // A marker alone ends with its own trailing space trimmed.
            let trimmed = self.out.trim_end_matches(' ').len();
            self.out.truncate(trimmed);
        } else {
            self.out.push_str(text);
        }
        self.out.push('\n');
    }

    /// Indents what follows by `indent`; `marker` leads its first line. Close it with
    /// `pop_frame`.
    pub fn push_frame(&mut self, indent: usize, marker: &str) {
        if self.depth == self.frames.len() {
            self.frames.push(Frame::default());
        }
        let f = &mut self.frames[self.depth];
        f.marker.clear();
        f.marker.push_str(marker);
        f.pending = !marker.is_empty();
        f.indent = indent;
        self.depth += 1;
    }

    /// Closes a frame; a marker nothing followed still gets its own line.
    pub fn pop_frame(&mut self) {
        if self.frames[self.depth - 1].pending {
            self.blank = false;
            self.line("");
            self.blank = true;
        }
        self.depth -= 1;
    }

    /// Writes a pending marker on a line of its own, for content that cannot start on
    /// the marker's line.
    pub fn detach_marker(&mut self) {
        if self.frames[self.frame_base..self.depth].iter().any(|f| f.pending) {
            self.line("");
            self.blank = false;
        }
    }

    /// Runs `f` with output going to a fresh buffer at `width`, outside every frame,
    /// and returns what it wrote (a table cell's content); give it back with
    /// `recycle` when done.
    pub fn capture(&mut self, width: usize, f: impl FnOnce(&mut Self)) -> String {
        let fresh = self.buffer();
        let out = std::mem::replace(&mut self.out, fresh);
        let saved_width = self.settings.width;
        let saved_depth = self.depth;
        let saved_blank = self.blank;
        let saved_inline = self.begin_inline(false);
        let saved_in_inline = self.in_inline;
        let saved_tight = std::mem::replace(&mut self.tight_wrap, true);
        // Frames below the capture stay in the vector; the capture starts past them.
        let saved_base = std::mem::replace(&mut self.frame_base, self.depth);
        self.settings.width = width;
        self.blank = false;
        self.in_inline = false;
        f(self);
        let captured = std::mem::replace(&mut self.out, out);
        self.frame_base = saved_base;
        self.depth = saved_depth;
        self.settings.width = saved_width;
        self.blank = saved_blank;
        let used = self.end_inline(saved_inline);
        self.give_inline(used);
        self.in_inline = saved_in_inline;
        self.tight_wrap = saved_tight;
        captured
    }
}
