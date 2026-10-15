//! The renderer's state: indentation frames, wrapping, inline layout, and reporting.

use guidedoc::{
    Arg, AttributeKey, CORE_STACK, Document, Limits, Message, NodeId, NodeKind, Policy, ROOT,
    RenderContext, RenderOptions, Severity, StackGuard, Status, arg_int, kept_message,
    stack_guard,
};

use std::fmt::Write as _;

use crate::DEPTH_LIMIT;
use crate::tables::Table;

/// Sphinx's text builder MAXWIDTH.
pub(crate) const WIDTH: usize = 70;
/// Sphinx's STDINDENT: definitions, block quotes, and literal blocks.
pub(crate) const INDENT: usize = 3;
/// Wrapped text keeps at least this width however deep the indentation.
const NARROW: usize = 20;

/// One level of indentation. A marker ("* ", "1. ", "[1] ", "Note: ") takes the place
/// of the indentation on the first line written inside the frame.
#[derive(Debug, Default)]
struct Frame {
    indent: usize,
    marker: String,
    /// Whether the marker is still to be written.
    pending: bool,
}

/// The state of sectnum numbering.
#[derive(Debug, Default)]
pub(crate) struct Numbers<'a> {
    pub enabled: bool,
    /// 0 means unlimited.
    pub depth: usize,
    pub start: i64,
    pub prefix: &'a str,
    pub suffix: &'a str,
    pub counters: [i64; 16],
}

impl Numbers<'_> {
    pub fn numbered(&self, d: usize) -> bool {
        self.enabled && d >= 1 && d <= self.counters.len() && (self.depth == 0 || d <= self.depth)
    }

    /// Writes "prefix1.2suffix " for `counters`.
    pub fn write(&self, out: &mut String, counters: &[i64]) {
        out.push_str(self.prefix);
        for (i, c) in counters.iter().enumerate() {
            if i > 0 {
                out.push('.');
            }
            let _ = write!(out, "{c}");
        }
        out.push_str(self.suffix);
        out.push(' ');
    }
}

/// Counts a section at depth `d` and resets the deeper counters.
pub(crate) fn count_section(counters: &mut [i64], d: usize, start: i64) {
    if d < 1 || d > counters.len() {
        return;
    }
    counters[d - 1] = match counters[d - 1] {
        0 if d == 1 => start,
        0 => 1,
        n => n + 1,
    };
    counters[d..].fill(0);
}

/// What writes a node's inline content when it is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fill {
    /// Its inline children.
    Inlines,
    /// An image's stand-in text.
    Image,
    /// A rubric: `-[ text ]-`.
    Rubric,
    /// A term and its classifiers.
    Term,
    /// A field name and its colon.
    FieldName,
    /// The options of an option group.
    OptionGroup,
    /// A topic's or sidebar's title, and a subtitle.
    FrameTitle,
    /// A note's label.
    NoteLabel,
}

/// The renderer's storage, kept in the workspace's scratch between renders.
#[derive(Debug, Default)]
pub(crate) struct TextWork {
    out: String,
    spare: Vec<String>,
    frames: Vec<Frame>,
    tables: Vec<Table>,
}

pub(crate) struct State<'c, 'a> {
    ctx: &'c mut RenderContext<'a>,
    pub doc: Document<'a>,
    pub options: &'a RenderOptions<'a>,
    pub policy: Policy,
    pub limits: Limits,
    /// Where text goes: the document, or a layout or cell being measured.
    pub out: String,
    /// Buffers for layouts, kept for reuse.
    spare: Vec<String>,
    frames: Vec<Frame>,
    /// Table layouts, kept for reuse (nested tables take one each).
    pub tables: Vec<Table>,
    /// Frames in use.
    depth: usize,
    /// The first frame of the current margin (a table cell starts its own).
    base: usize,
    /// The line width, indentation included.
    pub width: usize,
    /// The least width wrapped text keeps.
    pub narrow: usize,
    /// A blank line is due before the next block.
    pub blank: bool,
    /// Lines written to `out`.
    pub lines: usize,
    /// A trial layout, whose diagnostics the final one reports.
    pub quiet: bool,
    nesting: usize,
    nesting_limit: usize,
    guard: StackGuard,
    pub expansion: usize,
    /// Enclosing sections.
    pub sections: usize,
    pub has_title: bool,
    pub numbers: Numbers<'a>,
    pub status: Status,
}

impl<'c, 'a> State<'c, 'a> {
    /// Starts a render with the storage in `work`; `finish` gives it back.
    pub fn new(ctx: &'c mut RenderContext<'a>, work: TextWork) -> Self {
        let (doc, options, policy, limits) = (ctx.doc, ctx.options, ctx.policy, ctx.limits);
        let TextWork { mut out, spare, frames, tables } = work;
        out.clear();
        State {
            ctx,
            doc,
            options,
            policy,
            limits,
            out,
            spare,
            frames,
            tables,
            depth: 0,
            base: 0,
            width: WIDTH,
            narrow: NARROW,
            blank: false,
            lines: 0,
            quiet: false,
            nesting: 0,
            nesting_limit: limits.max_depth.max(32) + 64,
            guard: stack_guard(&limits),
            expansion: 0,
            sections: 0,
            has_title: false,
            numbers: Numbers::default(),
            status: Status::Ok,
        }
    }

    /// Ends the render: writes the text to the context's output and returns the
    /// render's status and the storage, for the next render.
    pub fn finish(self) -> (Status, TextWork) {
        let State { ctx, status, out, spare, frames, tables, .. } = self;
        ctx.out.emit(&out);
        (status, TextWork { out, spare, frames, tables })
    }

    /// Returns an empty string buffer, reused from earlier layouts; give it back with
    /// `recycle`.
    pub fn buffer(&mut self) -> String {
        self.spare.pop().unwrap_or_default()
    }

    /// Finds a promoted title and sectnum settings.
    pub fn scan_document(&mut self) {
        let doc = self.doc;
        self.has_title = doc.first_child(ROOT).is_some_and(|f| doc.kind(f) == NodeKind::Title);
        self.numbers.enabled = self.options.section_numbers;
        self.numbers.start = 1;
        let sectnum = (0..doc.node_count()).map(NodeId::from_index).find(|&id| {
            doc.kind(id) == NodeKind::Directive && self.directive_name(id) == "sectnum"
        });
        if let Some(id) = sectnum {
            self.numbers.enabled = true;
            self.numbers.depth =
                usize::try_from(parse_int(self.custom(id, "depth"), 0)).unwrap_or(0);
            self.numbers.start = parse_int(self.custom(id, "start"), 1);
            self.numbers.prefix = self.custom(id, "prefix");
            self.numbers.suffix = self.custom(id, "suffix");
        }
    }

    /// Reports `m`, except during a trial layout; an error fails the render.
    pub fn fail(&mut self, m: Message, id: NodeId, a: Arg, b: Arg) {
        if self.quiet {
            return;
        }
        let kept = kept_message(&self.policy, m);
        let span = self.doc.node(id).span;
        self.ctx.report(kept, span, a, b);
        if kept.severity == Severity::Error && self.status == Status::Ok {
            self.status = kept.status;
        }
    }

    /// Guards recursion; every nested block, inline, and expansion passes through it.
    /// Pair a true result with `leave`.
    pub fn enter(&mut self, id: NodeId) -> bool {
        let reporting = self.status == Status::Ok && !self.quiet;
        if self.nesting >= self.nesting_limit {
            if reporting {
                let limit = arg_int(i64::try_from(self.nesting_limit).unwrap_or(i64::MAX));
                self.fail(DEPTH_LIMIT, id, limit, Arg::None);
            }
            return false;
        }
        if self.guard.is_exhausted() {
            if reporting {
                let limit = arg_int(i64::try_from(self.guard.limit()).unwrap_or(i64::MAX));
                self.fail(CORE_STACK, id, limit, Arg::None);
            }
            return false;
        }
        self.nesting += 1;
        true
    }

    pub fn leave(&mut self) {
        self.nesting -= 1;
    }

    pub fn put(&mut self, s: &str) {
        self.out.push_str(s);
    }

    pub fn put_char(&mut self, c: char) {
        self.out.push(c);
    }

    pub fn put_repeat(&mut self, c: char, n: usize) {
        self.out.extend(std::iter::repeat_n(c, n));
    }

    /// Separates a block from the one before it by a blank line.
    pub fn block_start(&mut self) {
        if self.blank && self.lines > 0 {
            self.out.push('\n');
            self.lines += 1;
        }
        self.blank = false;
    }

    pub fn block_end(&mut self) {
        self.blank = true;
    }

    pub fn indentation(&self) -> usize {
        self.frames[self.base..self.depth].iter().map(|f| f.indent).sum()
    }

    /// Writes the indentation of a line, and the markers of the frames whose first
    /// line it is.
    pub fn line_begin(&mut self) {
        for f in &mut self.frames[self.base..self.depth] {
            if f.pending {
                self.out.push_str(&f.marker);
                let used = f.marker.chars().count();
                self.out.extend(std::iter::repeat_n(' ', f.indent.saturating_sub(used)));
                f.pending = false;
            } else {
                self.out.extend(std::iter::repeat_n(' ', f.indent));
            }
        }
    }

    pub fn line_end(&mut self) {
        self.out.push('\n');
        self.lines += 1;
    }

    /// Indents what follows; a marker leads its first line and sets the indent when it
    /// is wider. Close it with `pop_frame`.
    pub fn push_frame(&mut self, indent: usize, marker: &str) {
        if self.depth == self.frames.len() {
            self.frames.push(Frame::default());
        }
        let f = &mut self.frames[self.depth];
        f.marker.clear();
        f.marker.push_str(marker);
        f.pending = !marker.is_empty();
        f.indent = indent.max(marker.chars().count());
        self.depth += 1;
    }

    /// Closes a frame; a marker nothing followed still gets its own line.
    pub fn pop_frame(&mut self) {
        let f = &mut self.frames[self.depth - 1];
        if f.pending {
            let trimmed = f.marker.trim_end_matches(' ').len();
            f.marker.truncate(trimmed);
            f.indent = f.marker.chars().count();
            self.block_start();
            self.line_begin();
            self.line_end();
            self.block_end();
        }
        self.depth -= 1;
    }

    /// Writes laid-out inline text, wrapping it at the line width; '\n' in the text is
    /// a forced line break. Runs of spaces are break opportunities.
    pub fn write_wrapped(&mut self, text: &str) {
        let avail = self.width.saturating_sub(self.indentation()).max(self.narrow);
        let mut column = 0;
        let mut open = false;
        let mut position = 0;
        for (i, line) in text.split('\n').enumerate() {
            if i > 0 {
                // A forced break: it ends the open line, or makes an empty one unless
                // the text starts with it.
                if open || position > 1 {
                    self.line_end();
                }
                open = false;
                column = 0;
            }
            position += line.len() + 1;
            for word in line.split(' ').filter(|w| !w.is_empty()) {
                let length = word.chars().count();
                if open && column + 1 + length > avail {
                    self.line_end();
                    open = false;
                }
                if open {
                    self.out.push(' ');
                    column += 1;
                } else {
                    self.line_begin();
                    open = true;
                    column = 0;
                }
                self.out.push_str(word);
                column += length;
            }
        }
        if open {
            self.line_end();
        }
    }

    /// Writes text line by line without wrapping, as literal blocks need.
    pub fn write_lines(&mut self, text: &str) {
        for line in text.split_inclusive('\n') {
            let line = line.trim_end_matches('\n').trim_end_matches([' ', '\r']);
            if !line.is_empty() {
                self.line_begin();
                self.out.push_str(line);
            }
            self.line_end();
        }
    }

    /// Lays out a node's inline content, led by `lead`, as one string.
    pub fn lay_out(&mut self, id: NodeId, lead: &str, fill: Fill) -> String {
        let buffer = self.spare.pop().unwrap_or_default();
        let saved = std::mem::replace(&mut self.out, buffer);
        self.out.push_str(lead);
        self.fill(fill, id);
        std::mem::replace(&mut self.out, saved)
    }

    fn fill(&mut self, fill: Fill, id: NodeId) {
        match fill {
            Fill::Inlines => self.render_inlines(id),
            Fill::Image => self.image_text(id),
            Fill::Rubric => self.rubric_text(id),
            Fill::Term => self.term_text(id),
            Fill::FieldName => self.field_name(id),
            Fill::OptionGroup => self.option_group(id),
            Fill::FrameTitle => self.frame_title(id),
            Fill::NoteLabel => self.write_note_label(id),
        }
    }

    /// Returns a layout's buffer for reuse.
    pub fn recycle(&mut self, mut buffer: String) {
        buffer.clear();
        self.spare.push(buffer);
    }

    /// Writes a node's inlines as a wrapped paragraph.
    pub fn paragraph(&mut self, id: NodeId, lead: &str, fill: Fill) {
        let text = self.lay_out(id, lead, fill);
        self.block_start();
        self.write_wrapped(&text);
        self.block_end();
        self.recycle(text);
    }

    /// Lays out a cell's blocks into a string at `width`, from a fresh left margin;
    /// a quiet layout reports nothing.
    pub fn cell_layout(&mut self, id: NodeId, width: usize, quiet: bool) -> String {
        let buffer = self.spare.pop().unwrap_or_default();
        let out = std::mem::replace(&mut self.out, buffer);
        let saved = (self.width, self.narrow, self.blank, self.lines, self.base, self.quiet);
        self.width = width;
        self.narrow = 1;
        self.blank = false;
        self.lines = 0;
        self.base = self.depth;
        self.quiet = self.quiet || quiet;
        self.render_blocks(id);
        (self.width, self.narrow, self.blank, self.lines, self.base, self.quiet) = saved;
        std::mem::replace(&mut self.out, out)
    }

    pub fn directive_name(&self, id: NodeId) -> &'a str {
        self.doc.attribute(id, AttributeKey::Name).unwrap_or("")
    }

    pub fn custom(&self, id: NodeId, name: &str) -> &'a str {
        self.doc.custom_attribute(id, name).unwrap_or("")
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
