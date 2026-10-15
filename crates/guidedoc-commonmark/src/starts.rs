//! Block starts of CommonMark: block quotes, headings, fences, HTML blocks, thematic
//! breaks, list items, and indented code. MyST's own starts are tried first
//! (`myst/blocks.rs`).

use guidedoc::Fail;

use crate::blocks::{BlockKind, BlockParser, CODE_INDENT, Start, is_blank, is_space_or_tab};
use crate::inline::html_scan::html_block_start;
use crate::settings::{Rule, SyntaxExtension};

/// A parsed list marker.
#[derive(Debug, Clone, Copy, Default)]
struct Marker {
    ordered: bool,
    marker: u8,
    start: i32,
    padding: usize,
    marker_offset: usize,
}

impl BlockParser<'_, '_, '_, '_> {
    pub(crate) fn try_starts(&mut self, container: usize) -> Result<Start, Fail> {
        self.h.spend(self.line.len().saturating_sub(self.next_nonspace) + 1)?;
        if self.h.myst {
            let r = self.try_myst_starts(container)?;
            if r != Start::None {
                return Ok(r);
            }
        }
        let r = self.start_block_quote()?;
        if r != Start::None {
            return Ok(r);
        }
        let r = self.start_atx_heading()?;
        if r != Start::None {
            return Ok(r);
        }
        let r = self.start_fence()?;
        if r != Start::None {
            return Ok(r);
        }
        let r = self.start_html_block(container)?;
        if r != Start::None {
            return Ok(r);
        }
        let r = self.start_setext_heading(container)?;
        if r != Start::None {
            return Ok(r);
        }
        let r = self.start_thematic_break()?;
        if r != Start::None {
            return Ok(r);
        }
        let r = self.start_list_item(container)?;
        if r != Start::None {
            return Ok(r);
        }
        self.start_indented_code()
    }

    fn start_block_quote(&mut self) -> Result<Start, Fail> {
        if self.indented || !self.h.on(Rule::Blockquote) || self.peek(self.next_nonspace) != b'>' {
            return Ok(Start::None);
        }
        self.advance_next_nonspace();
        self.advance_offset(1, false);
        if is_space_or_tab(self.peek(self.offset)) {
            self.advance_offset(1, true);
        }
        self.close_unmatched()?;
        self.add_child(BlockKind::BlockQuote, self.next_nonspace)?;
        Ok(Start::Container)
    }

    fn start_atx_heading(&mut self) -> Result<Start, Fail> {
        if self.indented || !self.h.on(Rule::Heading) {
            return Ok(Start::None);
        }
        let s = self.nonspace_rest().as_bytes();
        let n = s.iter().take_while(|&&c| c == b'#').count();
        if n == 0 || n > 6 || (n < s.len() && !is_space_or_tab(s[n])) {
            return Ok(Start::None);
        }
        let m = n + s[n..].iter().take_while(|&&c| is_space_or_tab(c)).count();
        self.advance_next_nonspace();
        self.advance_offset(m, false);
        self.close_unmatched()?;
        let id = self.add_child(BlockKind::Heading, self.next_nonspace)?;
        self.blk_mut(id).level = n as u8;
        let rest = self.rest_from(self.offset);
        self.begin_seg(id, self.line_start + self.offset);
        self.arena_write(strip_closing_sequence(rest));
        self.blk_mut(id).content_end = self.sk.arena.len();
        self.advance_offset(self.line.len() - self.offset.min(self.line.len()), false);
        Ok(Start::Leaf)
    }

    fn start_fence(&mut self) -> Result<Start, Fail> {
        if self.indented || !self.h.on(Rule::Fence) {
            return Ok(Start::None);
        }
        let s = self.nonspace_rest().as_bytes();
        let colon = s.first() == Some(&b':') && self.h.has(SyntaxExtension::ColonFence);
        if s.len() < 3 || (s[0] != b'`' && s[0] != b'~' && !colon) {
            return Ok(Start::None);
        }
        let ch = s[0];
        let n = s.iter().take_while(|&&c| c == ch).count();
        if n < 3 || (ch == b'`' && s[n..].contains(&b'`')) {
            return Ok(Start::None);
        }
        self.close_unmatched()?;
        let id = self.add_child(BlockKind::CodeBlock, self.next_nonspace)?;
        let indent = self.indent;
        let blk = self.blk_mut(id);
        blk.fenced = true;
        blk.fence_char = ch;
        blk.fence_length = n;
        blk.fence_offset = indent;
        self.advance_next_nonspace();
        self.advance_offset(n, false);
        Ok(Start::Leaf)
    }

    fn start_html_block(&mut self, container: usize) -> Result<Start, Fail> {
        if self.indented || !self.h.on(Rule::HtmlBlock) || self.peek(self.next_nonspace) != b'<' {
            return Ok(Start::None);
        }
        let s = self.nonspace_rest();
        for t in 1..=7u8 {
            if !html_block_start(t, s) {
                continue;
            }
            let in_paragraph = self.kind(container) == BlockKind::Paragraph
                || (!self.all_closed && !self.blank && self.tip_kind() == BlockKind::Paragraph);
            if t == 7 && in_paragraph {
                continue;
            }
            self.close_unmatched()?;
            let id = self.add_child(BlockKind::HtmlBlock, self.offset)?;
            self.blk_mut(id).html_type = t;
            return Ok(Start::Leaf);
        }
        Ok(Start::None)
    }

    fn start_setext_heading(&mut self, container: usize) -> Result<Start, Fail> {
        if self.indented
            || !self.h.on(Rule::Lheading)
            || self.kind(container) != BlockKind::Paragraph
        {
            return Ok(Start::None);
        }
        let s = self.nonspace_rest().as_bytes();
        if s.is_empty() || (s[0] != b'=' && s[0] != b'-') {
            return Ok(Start::None);
        }
        let n = s.iter().take_while(|&&c| c == s[0]).count();
        if !s[n..].iter().all(|&c| is_space_or_tab(c)) {
            return Ok(Start::None);
        }
        let level = if s[0] == b'=' { 1 } else { 2 };
        self.close_unmatched()?;
        self.parse_paragraph_refs(container)?;
        let blk = self.blk(container);
        if blk.content_end <= blk.content_start {
            return Ok(Start::None);
        }
        let blk = self.blk_mut(container);
        blk.kind = BlockKind::Heading;
        blk.level = level;
        self.tip = Some(container);
        self.advance_offset(self.line.len() - self.offset.min(self.line.len()), false);
        Ok(Start::Leaf)
    }

    fn start_thematic_break(&mut self) -> Result<Start, Fail> {
        if self.indented || !self.h.on(Rule::Hr) {
            return Ok(Start::None);
        }
        let s = self.nonspace_rest().as_bytes();
        let Some(&first) = s.first().filter(|&&c| c == b'*' || c == b'_' || c == b'-') else {
            return Ok(Start::None);
        };
        let mut n = 0;
        for &c in s {
            if c == first {
                n += 1;
            } else if !is_space_or_tab(c) {
                return Ok(Start::None);
            }
        }
        if n < 3 {
            return Ok(Start::None);
        }
        self.close_unmatched()?;
        self.add_child(BlockKind::ThematicBreak, self.next_nonspace)?;
        self.advance_offset(self.line.len() - self.offset.min(self.line.len()), false);
        Ok(Start::Leaf)
    }

    fn start_indented_code(&mut self) -> Result<Start, Fail> {
        if !self.indented || !self.h.on(Rule::Code) {
            return Ok(Start::None);
        }
        if self.tip_kind() == BlockKind::Paragraph || self.blank {
            return Ok(Start::None);
        }
        self.advance_offset(CODE_INDENT, true);
        self.close_unmatched()?;
        self.add_child(BlockKind::CodeBlock, self.offset)?;
        Ok(Start::Leaf)
    }

    fn start_list_item(&mut self, container: usize) -> Result<Start, Fail> {
        if !self.h.on(Rule::List) || (self.indented && self.kind(container) != BlockKind::List) {
            return Ok(Start::None);
        }
        let Some(m) = self.parse_list_marker(container) else {
            return Ok(Start::None);
        };
        self.close_unmatched()?;
        let tip = self.tip.map(|t| *self.blk(t)).unwrap_or_default();
        if tip.kind != BlockKind::List || tip.ordered != m.ordered || tip.marker != m.marker {
            let list = self.add_child(BlockKind::List, self.next_nonspace)?;
            let l = self.blk_mut(list);
            l.ordered = m.ordered;
            l.marker = m.marker;
            l.list_start = m.start;
        }
        let item = self.add_child(BlockKind::Item, self.next_nonspace)?;
        let it = self.blk_mut(item);
        it.ordered = m.ordered;
        it.marker = m.marker;
        it.list_start = m.start;
        it.padding = m.padding;
        it.marker_offset = m.marker_offset;
        Ok(Start::Container)
    }

    fn parse_list_marker(&mut self, container: usize) -> Option<Marker> {
        if self.indent >= 4 {
            return None;
        }
        let s = self.nonspace_rest().as_bytes();
        let mut m = Marker { marker_offset: self.indent, ..Marker::default() };
        let in_paragraph = self.kind(container) == BlockKind::Paragraph;

        let length = if matches!(s.first(), Some(b'*' | b'+' | b'-')) {
            m.marker = s[0];
            1
        } else {
            let digits = s.iter().take(9).take_while(|c| c.is_ascii_digit()).count();
            if digits == 0 || digits >= s.len() || (s[digits] != b'.' && s[digits] != b')') {
                return None;
            }
            let value = s[..digits].iter().fold(0i32, |v, &d| v * 10 + i32::from(d - b'0'));
            if in_paragraph && value != 1 {
                return None;
            }
            m.ordered = true;
            m.marker = s[digits];
            m.start = value;
            digits + 1
        };
        let after = self.peek(self.next_nonspace + length);
        if after != 0 && !is_space_or_tab(after) {
            return None;
        }
        let rest = std::str::from_utf8(&s[length..]).unwrap_or("x");
        if in_paragraph && is_blank(rest) {
            return None;
        }
        self.advance_next_nonspace();
        self.advance_offset(length, true);
        m.padding = self.marker_padding(length);
        Some(m)
    }

    /// Consumes the spaces after a marker of `width` columns and returns the item's
    /// padding: one to four spaces set the content column; none, five or more, or an
    /// empty item make it one column after the marker.
    pub(crate) fn marker_padding(&mut self, width: usize) -> usize {
        let (start_col, start_off) = (self.column, self.offset);
        loop {
            self.advance_offset(1, true);
            if self.column - start_col >= 5 || !is_space_or_tab(self.peek(self.offset)) {
                break;
            }
        }
        let blank_item = self.offset >= self.line.len();
        let spaces = self.column - start_col;
        if !(1..5).contains(&spaces) || blank_item {
            self.column = start_col;
            self.offset = start_off;
            self.partial_tab = false;
            if is_space_or_tab(self.peek(self.offset)) {
                self.advance_offset(1, true);
            }
            return width + 1;
        }
        width + spaces
    }
}

/// Removes an optional closing run of '#' preceded by a space.
fn strip_closing_sequence(s: &str) -> &str {
    let b = s.as_bytes();
    let mut e = b.len();
    while e > 0 && is_space_or_tab(b[e - 1]) {
        e -= 1;
    }
    let mut h = e;
    while h > 0 && b[h - 1] == b'#' {
        h -= 1;
    }
    if h == e {
        return s;
    }
    if h == 0 {
        return "";
    }
    if !is_space_or_tab(b[h - 1]) {
        return s;
    }
    while h > 0 && is_space_or_tab(b[h - 1]) {
        h -= 1;
    }
    &s[..h]
}
