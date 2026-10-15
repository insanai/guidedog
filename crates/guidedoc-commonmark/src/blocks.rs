//! The block parser: lines go into a skeleton of blocks (CommonMark's two-phase
//! algorithm, as the reference implementation runs it), leaf content is copied into an
//! arena, and link reference definitions are collected. The finished skeleton is then
//! emitted in preorder (`emit.rs`).

use guidedoc::{Fail, SourceId, SourceSpan, arg_int};

use crate::emit::emit_document;
use crate::extension::{Line, ListItems};
use crate::host::Host;
use crate::messages::NESTING;

/// Columns of indentation that make indented code.
pub(crate) const CODE_INDENT: usize = 4;

/// A block's index in its skeleton.
pub(crate) type Bid = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum BlockKind {
    #[default]
    Document,
    BlockQuote,
    List,
    Item,
    Paragraph,
    Heading,
    ThematicBreak,
    CodeBlock,
    HtmlBlock,
    // MyST blocks (myst/blocks.rs).
    Target,
    Comment,
    BlockBreak,
    MathBlock,
    FootnoteDef,
    Table,
    DefinitionList,
    DefinitionTerm,
    Definition,
    FieldList,
    Field,
}

/// One skeleton record. Siblings form a doubly linked list under `parent`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Block {
    pub kind: BlockKind,
    pub open: bool,
    pub last_line_blank: bool,
    pub fenced: bool,
    pub ordered: bool,
    pub tight: bool,
    pub html_type: u8,
    /// Heading level; for math blocks 1 ($$) or 2 (amsmath).
    pub level: u8,
    pub fence_char: u8,
    /// Bullet character, or '.' / ')' for ordered lists.
    pub marker: u8,
    pub parent: Option<Bid>,
    pub first: Option<Bid>,
    pub last: Option<Bid>,
    pub prev: Option<Bid>,
    pub next: Option<Bid>,
    pub depth: usize,
    pub start_line: usize,
    pub fence_length: usize,
    pub fence_offset: usize,
    pub marker_offset: usize,
    pub padding: usize,
    pub list_start: i32,
    /// A table's columns.
    pub columns: usize,
    /// Source byte offsets.
    pub start: u32,
    pub end: u32,
    /// Arena byte offsets of the content and of a fence's info string.
    pub content_start: usize,
    pub content_end: usize,
    pub info_start: usize,
    pub info_end: usize,
    /// The block's content lines in `Skeleton::segs`.
    pub seg_first: usize,
    pub seg_end: usize,
    /// Source range of a footnote label or field name.
    pub name_start: u32,
    pub name_end: u32,
    /// A footnote definition's index in the host's notes; `None` when ignored.
    pub note: Option<usize>,
}

/// Maps a content line in the arena back to its source offset.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Seg {
    pub content: usize,
    pub source: u32,
}

/// A finished block parse over lines of one source: the blocks, their content, and the
/// source the spans point into. Emission only reads it.
#[derive(Debug)]
pub(crate) struct Skeleton<'s> {
    pub source: SourceId,
    /// The source's text, for spans and for lines handed to directives.
    pub src: &'s str,
    pub blocks: Vec<Block>,
    pub arena: String,
    pub segs: Vec<Seg>,
    /// Headings open sections (MyST, top level).
    pub sections: bool,
    /// The parse of the main source's body.
    pub main: bool,
}

/// A skeleton's buffers, kept in the workspace's scratch between parses.
#[derive(Debug, Default)]
pub(crate) struct SkeletonParts {
    blocks: Vec<Block>,
    arena: String,
    segs: Vec<Seg>,
}

impl<'s> Skeleton<'s> {
    /// Makes an empty skeleton over `src`, with the buffers an earlier parse gave back
    /// to `h`'s scratch (`give_back`).
    pub(crate) fn new(
        h: &mut Host<'_, '_>,
        source: SourceId,
        src: &'s str,
        sections: bool,
        main: bool,
    ) -> Self {
        let SkeletonParts { mut blocks, mut arena, mut segs } = h.ctx.take_scratch();
        blocks.clear();
        arena.clear();
        segs.clear();
        Skeleton { source, src, blocks, arena, segs, sections, main }
    }

    /// Gives the skeleton's buffers back to `h`'s scratch for the next parse.
    pub(crate) fn give_back(self, h: &mut Host<'_, '_>) {
        let Skeleton { blocks, arena, segs, .. } = self;
        h.ctx.put_scratch(SkeletonParts { blocks, arena, segs });
    }

    /// The span of `[start, end)`, clamped to the source.
    pub fn span(&self, start: usize, end: usize) -> SourceSpan {
        let s = start.min(self.src.len());
        let e = end.clamp(s, self.src.len());
        SourceSpan::new(self.source, s as u32, e as u32)
    }

    pub fn block_span(&self, id: Bid) -> SourceSpan {
        let b = &self.blocks[id];
        self.span(b.start as usize, b.end as usize)
    }

    /// The arena text of `[start, end)`; "" for a range off a character boundary.
    pub fn arena_str(&self, start: usize, end: usize) -> &str {
        self.arena.get(start..end).unwrap_or("")
    }

    /// The content of a leaf.
    pub fn content(&self, id: Bid) -> &str {
        let b = &self.blocks[id];
        self.arena_str(b.content_start, b.content_end)
    }

    /// Maps an arena offset inside a leaf's content to a source offset.
    pub fn source_offset(&self, id: Bid, at: usize) -> usize {
        let blk = &self.blocks[id];
        let (mut lo, mut hi) = (blk.seg_first, blk.seg_end);
        if lo >= hi {
            return blk.start as usize;
        }
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if self.segs[mid].content <= at {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let seg = self.segs[lo];
        if at < seg.content {
            return seg.source as usize;
        }
        seg.source as usize + (at - seg.content)
    }

    /// The text of content line `k` of a block, without its line break.
    pub fn seg_text(&self, id: Bid, k: usize) -> &str {
        let blk = &self.blocks[id];
        let i = blk.seg_first + k;
        let a = self.segs[i].content;
        let e = if i + 1 >= blk.seg_end { blk.content_end } else { self.segs[i + 1].content };
        let s = self.arena_str(a, e);
        s.strip_suffix('\n').unwrap_or(s)
    }

    /// The span of content line `k` of a block.
    pub fn seg_span(&self, id: Bid, k: usize) -> SourceSpan {
        let blk = &self.blocks[id];
        let i = blk.seg_first + k;
        if i >= blk.seg_end {
            return self.block_span(id);
        }
        let at = self.segs[i].source as usize;
        self.span(at, at + self.seg_text(id, k).len())
    }

    /// The number of content lines of a block.
    pub fn seg_count(&self, id: Bid) -> usize {
        let b = &self.blocks[id];
        b.seg_end - b.seg_first
    }

    /// Content lines of a block from line `from` on, as views of the source where the
    /// arena holds the same bytes (so handlers see source text).
    pub fn block_lines(&self, id: Bid, from: usize) -> Vec<Line<'_>> {
        let n = self.seg_count(id).saturating_sub(from);
        let mut lines = Vec::with_capacity(n);
        for k in from..from + n {
            let mut t = self.seg_text(id, k);
            let at = self.segs[self.blocks[id].seg_first + k].source as usize;
            if let Some(src) = self.src.get(at..at + t.len()).filter(|s| *s == t) {
                t = src;
            }
            lines.push(Line { text: t, offset: at as u32 });
        }
        lines
    }

    /// Maps text inside the arena to its source offset; text elsewhere maps to 0.
    pub fn arena_source(&self, s: &str) -> usize {
        let Some(at) = guidedoc::offset_in(&self.arena, s) else {
            return 0;
        };
        if self.segs.is_empty() {
            return 0;
        }
        let (mut lo, mut hi) = (0, self.segs.len());
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if self.segs[mid].content <= at {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let seg = self.segs[lo];
        seg.source as usize + at.saturating_sub(seg.content)
    }

    /// The children of block `id`, in order.
    pub fn children(&self, id: Bid) -> impl Iterator<Item = Bid> + '_ {
        std::iter::successors(self.blocks[id].first, move |&c| self.blocks[c].next)
    }
}

/// What continuing an open block found on the current line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Continue {
    Matched,
    Failed,
    /// The line was consumed, as by a closing code fence.
    Done,
}

/// What a block start found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Start {
    None,
    Container,
    Leaf,
}

/// One block parse over lines of one source, sharing the host with nested parses.
pub(crate) struct BlockParser<'p, 'h, 'a, 's> {
    pub h: &'p mut Host<'h, 'a>,
    pub sk: Skeleton<'s>,
    /// The open footnote definition whose lines are recorded.
    pub note: Option<Bid>,
    /// Where the current line's footnote content starts.
    pub note_at: Option<usize>,
    // The current line and the column state of the block parser.
    pub line: &'s str,
    pub line_start: usize,
    pub line_number: usize,
    pub prev_end: usize,
    pub offset: usize,
    pub column: usize,
    pub next_nonspace: usize,
    pub next_column: usize,
    pub indent: usize,
    pub blank: bool,
    pub indented: bool,
    pub partial_tab: bool,
    pub all_closed: bool,
    pub tip: Option<Bid>,
    pub oldtip: Option<Bid>,
    pub last_matched: Bid,
}

/// Parses `lines` of `source` as blocks and emits them into the builder's open node.
pub(crate) fn parse_lines(
    h: &mut Host<'_, '_>,
    lines: &[Line<'_>],
    source: SourceId,
    sections: bool,
    main: bool,
) -> Result<(), Fail> {
    if lines.is_empty() {
        return Ok(());
    }
    let text = h.ctx.source_of(source);
    let sk = parse_skeleton(h, lines, source, &text, sections, main)?;
    let result = emit_document(h, &sk);
    sk.give_back(h);
    result
}

/// Runs the block phase over `lines` and returns the finished skeleton.
pub(crate) fn parse_skeleton<'s>(
    h: &mut Host<'_, '_>,
    lines: &[Line<'s>],
    source: SourceId,
    src: &'s str,
    sections: bool,
    main: bool,
) -> Result<Skeleton<'s>, Fail> {
    let sk = Skeleton::new(h, source, src, sections, main);
    let mut p = BlockParser::new(h, sk);
    p.parse_blocks(lines)?;
    Ok(p.sk)
}

/// Splits `text`, which starts at source offset `base`, at "\n", "\r\n", and "\r"; a
/// final break starts no line. A byte order mark is skipped.
pub(crate) fn split_source(text: &str, base: u32) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    split_source_into(text, base, &mut lines);
    lines
}

/// `split_source` into `lines`, which it clears first, reusing its storage.
pub(crate) fn split_source_into<'t>(text: &'t str, base: u32, lines: &mut Vec<Line<'t>>) {
    let (t, at) = match text.strip_prefix('\u{feff}') {
        Some(rest) => (rest, base + 3),
        None => (text, base),
    };
    let bytes = t.as_bytes();
    lines.clear();
    lines.reserve(bytes.iter().filter(|&&c| c == b'\n').count() + 1);
    let mut start = 0;
    while start < bytes.len() {
        let mut end = start;
        while end < bytes.len() && bytes[end] != b'\n' && bytes[end] != b'\r' {
            end += 1;
        }
        lines.push(Line { text: &t[start..end], offset: at + start as u32 });
        start = end + 1;
        if end < bytes.len() && bytes[end] == b'\r' && bytes.get(start) == Some(&b'\n') {
            start += 1;
        }
    }
}

pub(crate) fn is_space_or_tab(c: u8) -> bool {
    c == b' ' || c == b'\t'
}

/// Tells whether `s` holds only ASCII white space.
pub(crate) fn is_blank(s: &str) -> bool {
    s.bytes().all(guidedoc::is_ascii_space)
}

impl<'p, 'h, 'a, 's> BlockParser<'p, 'h, 'a, 's> {
    pub(crate) fn new(h: &'p mut Host<'h, 'a>, sk: Skeleton<'s>) -> Self {
        BlockParser {
            h,
            sk,
            note: None,
            note_at: None,
            line: "",
            line_start: 0,
            line_number: 0,
            prev_end: 0,
            offset: 0,
            column: 0,
            next_nonspace: 0,
            next_column: 0,
            indent: 0,
            blank: false,
            indented: false,
            partial_tab: false,
            all_closed: false,
            tip: None,
            oldtip: None,
            last_matched: 0,
        }
    }

    pub(crate) fn parse_blocks(&mut self, input: &[Line<'s>]) -> Result<(), Fail> {
        let (Some(first), Some(last)) = (input.first(), input.last()) else {
            return Ok(());
        };
        self.sk.blocks.clear();
        self.sk.blocks.push(Block {
            kind: BlockKind::Document,
            open: true,
            start: first.offset,
            end: last.offset + last.text.len() as u32,
            ..Block::default()
        });
        let bytes: usize = input.iter().map(|l| l.text.len() + 1).sum();
        self.sk.arena.reserve(bytes);
        self.sk.segs.reserve(input.len());
        self.tip = Some(0);
        for l in input {
            self.incorporate_line(*l)?;
            self.prev_end = l.offset as usize + l.text.len();
        }
        while let Some(tip) = self.tip {
            self.finalize(tip, self.prev_end)?;
        }
        Ok(())
    }

    pub(crate) fn blk(&self, id: Bid) -> &Block {
        &self.sk.blocks[id]
    }

    pub(crate) fn blk_mut(&mut self, id: Bid) -> &mut Block {
        &mut self.sk.blocks[id]
    }

    pub(crate) fn kind(&self, id: Bid) -> BlockKind {
        self.sk.blocks[id].kind
    }

    pub(crate) fn tip_kind(&self) -> BlockKind {
        self.tip.map_or(BlockKind::Document, |t| self.kind(t))
    }

    pub(crate) fn peek(&self, i: usize) -> u8 {
        self.line.as_bytes().get(i).copied().unwrap_or(0)
    }

    /// The current line from `i` on; "" past its end.
    pub(crate) fn rest_from(&self, i: usize) -> &'s str {
        self.line.get(i..).unwrap_or("")
    }

    /// The current line from its first non-space character.
    pub(crate) fn nonspace_rest(&self) -> &'s str {
        self.rest_from(self.next_nonspace)
    }

    pub(crate) fn find_next_nonspace(&mut self) {
        let bytes = self.line.as_bytes();
        let (mut i, mut cols) = (self.offset, self.column);
        while i < bytes.len() {
            match bytes[i] {
                b' ' => cols += 1,
                b'\t' => cols += 4 - cols % 4,
                _ => break,
            }
            i += 1;
        }
        self.blank = i >= bytes.len();
        self.next_nonspace = i;
        self.next_column = cols;
        self.indent = cols - self.column;
        self.indented = self.indent >= CODE_INDENT;
    }

    pub(crate) fn advance_next_nonspace(&mut self) {
        self.offset = self.next_nonspace;
        self.column = self.next_column;
        self.partial_tab = false;
    }

    /// Moves `count` characters, or `count` columns when `columns` is set, in which
    /// case a tab may be consumed partially.
    pub(crate) fn advance_offset(&mut self, count: usize, columns: bool) {
        let bytes = self.line.as_bytes();
        let mut n = count;
        while n > 0 && self.offset < bytes.len() {
            if bytes[self.offset] == b'\t' {
                let to_tab = 4 - self.column % 4;
                if columns {
                    self.partial_tab = to_tab > n;
                    let step = to_tab.min(n);
                    self.column += step;
                    if !self.partial_tab {
                        self.offset += 1;
                    }
                    n -= step;
                } else {
                    self.partial_tab = false;
                    self.column += to_tab;
                    self.offset += 1;
                    n -= 1;
                }
            } else {
                self.partial_tab = false;
                self.offset += 1;
                self.column += 1;
                n -= 1;
            }
        }
    }

    fn incorporate_line(&mut self, l: Line<'s>) -> Result<(), Fail> {
        self.line = l.text;
        self.line_start = l.offset as usize;
        self.line_number += 1;
        self.offset = 0;
        self.column = 0;
        self.blank = false;
        self.partial_tab = false;
        self.note_at = None;
        self.oldtip = self.tip;
        let mut container = 0;
        while let Some(last) = self.blk(container).last.filter(|&b| self.blk(b).open) {
            self.h.spend(1)?;
            container = last;
            self.find_next_nonspace();
            match self.continue_block(container)? {
                Continue::Done => return Ok(()),
                Continue::Failed => {
                    container = self.blk(container).parent.unwrap_or(0);
                    break;
                }
                Continue::Matched => {}
            }
        }
        self.all_closed = Some(container) == self.oldtip;
        self.last_matched = container;
        let k = self.kind(container);
        let mut matched_leaf =
            k != BlockKind::Paragraph && k != BlockKind::Table && accepts_lines(k);
        while !matched_leaf {
            self.find_next_nonspace();
            if !self.indented && !self.maybe_special(self.peek(self.next_nonspace)) {
                self.advance_next_nonspace();
                break;
            }
            let r = self.try_starts(container)?;
            if r == Start::None {
                self.advance_next_nonspace();
                break;
            }
            container = self.tip.unwrap_or(0);
            matched_leaf = r == Start::Leaf;
        }
        self.add_text_to_container(container)?;
        if self.note.is_some() {
            self.record_note_line();
        }
        Ok(())
    }

    fn maybe_special(&self, c: u8) -> bool {
        match c {
            b'#' | b'`' | b'~' | b'*' | b'+' | b'_' | b'=' | b'<' | b'>' | b'-' => true,
            b'0'..=b'9' => true,
            b'%' | b'(' | b'$' | b'\\' | b':' | b'[' | b'|' => self.h.myst,
            _ => false,
        }
    }

    fn continue_block(&mut self, id: Bid) -> Result<Continue, Fail> {
        let blk = *self.blk(id);
        let r = match blk.kind {
            BlockKind::Document | BlockKind::List => Continue::Matched,
            BlockKind::Heading | BlockKind::ThematicBreak => Continue::Failed,
            BlockKind::Paragraph => {
                if self.blank {
                    Continue::Failed
                } else {
                    Continue::Matched
                }
            }
            BlockKind::HtmlBlock => {
                if self.blank && (blk.html_type == 6 || blk.html_type == 7) {
                    Continue::Failed
                } else {
                    Continue::Matched
                }
            }
            BlockKind::BlockQuote => {
                if self.indented || self.peek(self.next_nonspace) != b'>' {
                    return Ok(Continue::Failed);
                }
                self.advance_next_nonspace();
                self.advance_offset(1, false);
                if is_space_or_tab(self.peek(self.offset)) {
                    self.advance_offset(1, true);
                }
                Continue::Matched
            }
            BlockKind::Item => self.continue_padded(id),
            BlockKind::CodeBlock => return self.continue_code(id),
            _ => self.continue_myst(id),
        };
        Ok(r)
    }

    /// Continues a container whose content is indented by its padding (list items,
    /// definitions, fields, footnote definitions).
    pub(crate) fn continue_padded(&mut self, id: Bid) -> Continue {
        let blk = *self.blk(id);
        let need = blk.marker_offset + blk.padding;
        if self.indent >= need {
            self.advance_offset(need, true);
        } else if self.blank && blk.first.is_some() {
            self.advance_next_nonspace();
        } else {
            return Continue::Failed;
        }
        Continue::Matched
    }

    fn continue_code(&mut self, id: Bid) -> Result<Continue, Fail> {
        let blk = *self.blk(id);
        if !blk.fenced {
            if self.indent >= CODE_INDENT {
                self.advance_offset(CODE_INDENT, true);
            } else if self.blank {
                self.advance_next_nonspace();
            } else {
                return Ok(Continue::Failed);
            }
            return Ok(Continue::Matched);
        }
        if self.indent <= 3 && self.peek(self.next_nonspace) == blk.fence_char {
            let s = self.nonspace_rest().as_bytes();
            let n = s.iter().take_while(|&&c| c == blk.fence_char).count();
            let rest = n + s[n..].iter().take_while(|&&c| is_space_or_tab(c)).count();
            if n >= blk.fence_length && rest == s.len() {
                self.finalize(id, self.line_start + self.line.len())?;
                return Ok(Continue::Done);
            }
        }
        let mut i = blk.fence_offset;
        while i > 0 && is_space_or_tab(self.peek(self.offset)) {
            self.advance_offset(1, true);
            i -= 1;
        }
        Ok(Continue::Matched)
    }

    fn add_text_to_container(&mut self, matched: Bid) -> Result<(), Fail> {
        self.find_next_nonspace();
        let c = *self.blk(matched);
        if self.blank
            && let Some(last) = c.last
        {
            self.blk_mut(last).last_line_blank = true;
        }
        let k = c.kind;
        let blank_line = self.blank
            && k != BlockKind::BlockQuote
            && k != BlockKind::Heading
            && k != BlockKind::ThematicBreak
            && !(k == BlockKind::CodeBlock && c.fenced)
            && !(is_item_like(k) && c.first.is_none() && c.start_line == self.line_number);
        self.blk_mut(matched).last_line_blank = blank_line;
        let mut t = c.parent;
        while let Some(id) = t {
            self.blk_mut(id).last_line_blank = false;
            t = self.blk(id).parent;
        }
        if !self.all_closed && !self.blank && self.tip_kind() == BlockKind::Paragraph {
            let note_open = self.note.is_some_and(|n| self.blk(n).open);
            if note_open && self.note_at.is_none() {
                self.note_at = Some(self.offset);
            }
            return self.add_line();
        }
        self.close_unmatched()?;
        if accepts_lines(k) {
            self.add_line()?;
            let t = c.html_type;
            let rest = self.rest_from(self.offset);
            let html_ends =
                k == BlockKind::HtmlBlock && (1..=5).contains(&t) && html_block_end(t, rest);
            if html_ends || (k == BlockKind::MathBlock && self.math_closes(matched)) {
                self.finalize(matched, self.line_start + self.line.len())?;
            }
        } else if self.offset < self.line.len() && !self.blank {
            self.add_child(BlockKind::Paragraph, self.next_nonspace)?;
            self.advance_next_nonspace();
            self.add_line()?;
        }
        Ok(())
    }

    /// Finalizes the blocks the current line did not continue.
    pub(crate) fn close_unmatched(&mut self) -> Result<(), Fail> {
        if self.all_closed {
            return Ok(());
        }
        while let Some(old) = self.oldtip.filter(|&o| o != self.last_matched) {
            let parent = self.blk(old).parent;
            self.finalize(old, self.prev_end)?;
            self.oldtip = parent;
        }
        self.all_closed = true;
        Ok(())
    }

    /// Finalizes blocks that cannot hold the new one, then appends it to the tip.
    pub(crate) fn add_child(&mut self, kind: BlockKind, offset: usize) -> Result<Bid, Fail> {
        while let Some(tip) = self.tip.filter(|&t| !can_contain(self.kind(t), kind)) {
            self.finalize(tip, self.prev_end)?;
        }
        // The document stays open until the parse ends, so there is always a tip.
        let parent = self.tip.unwrap_or(0);
        let depth = self.blk(parent).depth + 1;
        if depth > self.h.ctx.limits.max_depth {
            let pos = self.line_start + offset;
            let max = i64::try_from(self.h.ctx.limits.max_depth).unwrap_or(i64::MAX);
            let d = guidedoc::diagnostic(
                NESTING,
                self.sk.span(pos, pos + 1),
                arg_int(max),
                guidedoc::Arg::None,
            );
            return Err(self.h.b().fail(d));
        }
        let id = self.sk.blocks.len();
        let at = (self.line_start + offset) as u32;
        let (content, seg) = (self.sk.arena.len(), self.sk.segs.len());
        self.sk.blocks.push(Block {
            kind,
            open: true,
            parent: Some(parent),
            prev: self.blk(parent).last,
            depth,
            start_line: self.line_number,
            start: at,
            end: at,
            content_start: content,
            content_end: content,
            seg_first: seg,
            seg_end: seg,
            ..Block::default()
        });
        match self.blk(parent).last {
            Some(last) => self.blk_mut(last).next = Some(id),
            None => self.blk_mut(parent).first = Some(id),
        }
        self.blk_mut(parent).last = Some(id);
        self.tip = Some(id);
        Ok(id)
    }

    /// Removes a block that is its parent's last child.
    pub(crate) fn unlink_last(&mut self, id: Bid) {
        let blk = *self.blk(id);
        let Some(parent) = blk.parent else { return };
        if self.blk(parent).last != Some(id) {
            return;
        }
        self.blk_mut(parent).last = blk.prev;
        match blk.prev {
            Some(prev) => self.blk_mut(prev).next = None,
            None => self.blk_mut(parent).first = None,
        }
    }

    /// Starts a new content line of block `id` at source offset `source`.
    pub(crate) fn begin_seg(&mut self, id: Bid, source: usize) {
        self.sk.segs.push(Seg { content: self.sk.arena.len(), source: source as u32 });
        self.blk_mut(id).seg_end = self.sk.segs.len();
    }

    /// Copies source text into the arena, replacing U+0000 with U+FFFD.
    pub(crate) fn arena_write(&mut self, s: &str) {
        let mut parts = s.split('\0');
        if let Some(first) = parts.next() {
            self.sk.arena.push_str(first);
        }
        for part in parts {
            self.sk.arena.push('\u{fffd}');
            self.sk.arena.push_str(part);
        }
    }

    /// Adds the rest of the current line to the tip's content.
    pub(crate) fn add_line(&mut self) -> Result<(), Fail> {
        let Some(tip) = self.tip else { return Ok(()) };
        self.begin_seg(tip, self.line_start + self.offset);
        if self.partial_tab {
            self.offset += 1;
            for _ in 0..4 - self.column % 4 {
                self.sk.arena.push(' ');
            }
        }
        let rest = self.rest_from(self.offset.min(self.line.len()));
        self.arena_write(rest);
        self.sk.arena.push('\n');
        self.blk_mut(tip).content_end = self.sk.arena.len();
        Ok(())
    }

    /// Closes block `id` at source offset `end`, completing it as its kind requires.
    pub(crate) fn finalize(&mut self, id: Bid, end: usize) -> Result<(), Fail> {
        let above = self.blk(id).parent;
        let start = self.blk(id).start as usize;
        {
            let blk = self.blk_mut(id);
            blk.open = false;
            blk.end = end.max(start) as u32;
        }
        match self.kind(id) {
            BlockKind::Paragraph => {
                if self.parse_paragraph_refs(id)? && is_blank(self.sk.content(id)) {
                    self.unlink_last(id);
                }
            }
            BlockKind::CodeBlock => self.finalize_code(id),
            BlockKind::HtmlBlock => self.finalize_html(id),
            BlockKind::List => self.finalize_list(id)?,
            BlockKind::FootnoteDef | BlockKind::DefinitionList => self.finalize_myst(id),
            _ => {}
        }
        self.tip = above;
        Ok(())
    }

    /// Drops the blank lines that end an HTML block.
    fn finalize_html(&mut self, id: Bid) {
        let bytes = self.sk.arena.as_bytes();
        let blk = *self.blk(id);
        let mut e = blk.content_end;
        loop {
            let mut j = e;
            while j > blk.content_start && bytes[j - 1] == b' ' {
                j -= 1;
            }
            if j <= blk.content_start || bytes[j - 1] != b'\n' {
                break;
            }
            e = j - 1;
        }
        if e < blk.content_end {
            self.blk_mut(id).content_end = e + 1;
        }
    }

    fn finalize_code(&mut self, id: Bid) {
        let bytes = self.sk.arena.as_bytes();
        let blk = *self.blk(id);
        if blk.fenced {
            let mut nl = blk.content_start;
            while nl < blk.content_end && bytes[nl] != b'\n' {
                nl += 1;
            }
            let (mut s, mut e) = (blk.content_start, nl);
            while s < e && guidedoc::is_ascii_space(bytes[s]) {
                s += 1;
            }
            while e > s && guidedoc::is_ascii_space(bytes[e - 1]) {
                e -= 1;
            }
            let b = self.blk_mut(id);
            b.info_start = s;
            b.info_end = e;
            b.content_start = (nl + 1).min(blk.content_end);
            return;
        }
        // Drop trailing lines made only of spaces and tabs.
        let mut e = blk.content_end;
        while e > blk.content_start {
            let mut k = e - 1; // the newline ending the last line.
            while k > blk.content_start && is_space_or_tab(bytes[k - 1]) {
                k -= 1;
            }
            if k > blk.content_start && bytes[k - 1] != b'\n' {
                break;
            }
            e = k;
        }
        self.blk_mut(id).content_end = e;
    }

    /// Decides tightness: a list is loose when an item, or a child of an item, ends with
    /// a blank line and something follows it.
    fn finalize_list(&mut self, id: Bid) -> Result<(), Fail> {
        self.blk_mut(id).tight = true;
        let mut item = self.blk(id).first;
        while let Some(it) = item {
            self.h.spend(1)?;
            let next = self.blk(it).next;
            if self.blk(it).last_line_blank && next.is_some() {
                self.blk_mut(id).tight = false;
                return Ok(());
            }
            let mut sub = self.blk(it).first;
            while let Some(s) = sub {
                if (next.is_some() || self.blk(s).next.is_some()) && self.ends_with_blank_line(s) {
                    self.blk_mut(id).tight = false;
                    return Ok(());
                }
                sub = self.blk(s).next;
            }
            item = next;
        }
        Ok(())
    }

    fn ends_with_blank_line(&self, id: Bid) -> bool {
        let mut cur = Some(id);
        while let Some(c) = cur {
            let blk = self.blk(c);
            if (blk.kind != BlockKind::List && blk.kind != BlockKind::Item) || blk.last.is_none() {
                return blk.last_line_blank;
            }
            cur = blk.last;
        }
        false
    }
}

/// Tells whether blocks of kind `k` take lines of content.
pub(crate) fn accepts_lines(k: BlockKind) -> bool {
    matches!(
        k,
        BlockKind::Paragraph
            | BlockKind::CodeBlock
            | BlockKind::HtmlBlock
            | BlockKind::Comment
            | BlockKind::MathBlock
            | BlockKind::Table
            | BlockKind::Target
            | BlockKind::BlockBreak
    )
}

fn can_contain(parent: BlockKind, child: BlockKind) -> bool {
    use BlockKind as K;
    match parent {
        K::Document | K::BlockQuote | K::Item | K::FootnoteDef | K::Definition | K::Field => {
            !matches!(child, K::Item | K::Definition | K::Field | K::DefinitionTerm)
        }
        K::List => child == K::Item,
        K::DefinitionList => matches!(child, K::Paragraph | K::DefinitionTerm | K::Definition),
        K::FieldList => child == K::Field,
        _ => false,
    }
}

/// Containers whose content follows a marker, as a list item's does.
pub(crate) fn is_item_like(k: BlockKind) -> bool {
    matches!(
        k,
        BlockKind::Item | BlockKind::Definition | BlockKind::Field | BlockKind::FootnoteDef
    )
}

/// Tells whether an HTML block of type `t` (1 to 5) ends on a line holding `s`.
fn html_block_end(t: u8, s: &str) -> bool {
    crate::inline::html_scan::html_block_end(t, s)
}

/// Splits lines that hold one bullet list into its items' content lines.
pub(crate) fn list_item_lines<'l>(
    h: &mut Host<'_, '_>,
    lines: &[Line<'l>],
    source: SourceId,
) -> Result<Option<ListItems<'l>>, Fail> {
    if lines.is_empty() {
        return Ok(None);
    }
    let text = h.ctx.source_of(source);
    let sk = parse_skeleton(h, lines, source, &text, false, false)?;
    let Some(list) = sk.blocks[0].first else { return Ok(None) };
    let lb = sk.blocks[list];
    if lb.kind != BlockKind::List || lb.ordered || lb.next.is_some() {
        return Ok(None);
    }
    let mut out = ListItems::default();
    for it in sk.children(list) {
        let b = sk.blocks[it];
        let first = b.start_line - 1;
        let end = b.next.map_or(lines.len(), |n| sk.blocks[n].start_line - 1);
        out.starts.push(first);
        out.items.push(item_lines(&lines[first..end], b.marker_offset + b.padding));
    }
    Ok(Some(out))
}

/// Cuts an item's lines at its content column; lazy continuation lines, indented less,
/// lose all their indentation. Trailing blank lines are dropped.
fn item_lines<'l>(lines: &[Line<'l>], column: usize) -> Vec<Line<'l>> {
    let mut n = lines.len();
    while n > 0 && lines[n - 1].text.trim_matches([' ', '\t']).is_empty() {
        n -= 1;
    }
    let mut out = Vec::with_capacity(n);
    for (k, l) in lines[..n].iter().enumerate() {
        let mut lead = l.text.bytes().take(column).take_while(|&c| c == b' ').count();
        if k == 0 {
            lead = column.min(l.text.len());
        }
        let text = l.text.get(lead..).unwrap_or("");
        out.push(Line { text, offset: l.offset + lead as u32 });
    }
    out
}
