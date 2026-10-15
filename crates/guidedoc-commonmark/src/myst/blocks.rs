//! Block starts of the MyST syntax. Each is tried only with the MyST syntax on; the
//! ones markdown-it lets interrupt a paragraph do so here too.

use guidedoc::Fail;

use super::table::{count_cells, delimiter_row};
use super::{trim_right, trim_space};
use crate::blocks::{Bid, BlockKind, BlockParser, Continue, Start, is_space_or_tab};
use crate::settings::{Rule, SyntaxExtension};

const AMSMATH_ENVIRONMENTS: [&str; 13] = [
    "equation", "multline", "gather", "align", "alignat", "flalign", "matrix", "pmatrix",
    "bmatrix", "Bmatrix", "vmatrix", "Vmatrix", "eqnarray",
];

/// Matches "\begin{env}" or "\begin{env*}" and returns "env" or "env*".
pub(crate) fn amsmath_begin(s: &str) -> Option<&str> {
    let rest = s.strip_prefix("\\begin{")?;
    let name = &rest[..rest.find('}')?];
    let base = name.strip_suffix('*').unwrap_or(name);
    AMSMATH_ENVIRONMENTS.contains(&base).then_some(name)
}

/// Matches "(name)=" with optional trailing space.
fn target_line(s: &str) -> bool {
    let t = trim_right(s);
    t.len() >= 4
        && t.starts_with('(')
        && t.ends_with(")=")
        && !trim_space(&t[1..t.len() - 2]).is_empty()
}

/// Returns the width of ":name:" at the start of `s`, or 0. The name ends at the first
/// unescaped colon followed by white space or the end of the line.
pub(crate) fn field_marker(s: &str) -> usize {
    let s = s.as_bytes();
    if s.len() < 3 || s[0] != b':' || s[1] == b' ' || s[1] == b':' {
        return 0;
    }
    let mut i = 1;
    while i < s.len() {
        let c = s[i];
        if c == b'\\' {
            i += 2;
            continue;
        }
        if c == b':' && i > 1 && (i + 1 >= s.len() || s[i + 1] == b' ' || s[i + 1] == b'\t') {
            return i + 1;
        }
        i += 1;
    }
    0
}

/// Splits "... (label)" off a closing math line when labels are allowed.
pub(crate) fn math_label(allow: bool, t: &str) -> (&str, &str) {
    if !allow || t.len() < 3 || !t.ends_with(')') {
        return (t, "");
    }
    let Some(open) = t[..t.len() - 1].rfind('(') else { return (t, "") };
    let name = &t[open + 1..t.len() - 1];
    if name.is_empty() || name.bytes().any(|c| c == b'(' || c == b')' || c.is_ascii_whitespace()) {
        return (t, "");
    }
    let rest = trim_right(&t[..open]);
    if !rest.ends_with("$$") {
        return (t, "");
    }
    (rest, name)
}

impl BlockParser<'_, '_, '_, '_> {
    pub(crate) fn try_myst_starts(&mut self, container: Bid) -> Result<Start, Fail> {
        let r = self.start_footnote_def()?;
        if r != Start::None {
            return Ok(r);
        }
        let r = self.start_myst_line()?;
        if r != Start::None {
            return Ok(r);
        }
        let r = self.start_dollar_math()?;
        if r != Start::None {
            return Ok(r);
        }
        let r = self.start_amsmath()?;
        if r != Start::None {
            return Ok(r);
        }
        let r = self.start_table(container)?;
        if r != Start::None {
            return Ok(r);
        }
        let r = self.start_definition(container)?;
        if r != Start::None {
            return Ok(r);
        }
        self.start_field()
    }

    /// Starts the one-line blocks: "% comment", "+++ break", "(target)=".
    fn start_myst_line(&mut self) -> Result<Start, Fail> {
        if self.indented {
            return Ok(Start::None);
        }
        let s = self.nonspace_rest();
        let (kind, skip) = if s.starts_with('%') && self.h.on(Rule::MystLineComment) {
            (BlockKind::Comment, 1)
        } else if s.starts_with("+++") && self.h.on(Rule::MystBlockBreak) {
            (BlockKind::BlockBreak, 3)
        } else if target_line(s) && self.h.on(Rule::MystTarget) {
            (BlockKind::Target, 0)
        } else {
            return Ok(Start::None);
        };
        self.close_unmatched()?;
        self.advance_next_nonspace();
        self.add_child(kind, self.next_nonspace)?;
        self.advance_offset(skip, false);
        Ok(Start::Leaf)
    }

    fn start_dollar_math(&mut self) -> Result<Start, Fail> {
        let on = self.h.has(SyntaxExtension::Dollarmath) && self.h.on(Rule::MathBlock);
        if self.indented || !on || !self.nonspace_rest().starts_with("$$") {
            return Ok(Start::None);
        }
        self.close_unmatched()?;
        self.advance_next_nonspace();
        let id = self.add_child(BlockKind::MathBlock, self.next_nonspace)?;
        self.blk_mut(id).level = 1;
        self.advance_offset(2, false);
        Ok(Start::Leaf)
    }

    fn start_amsmath(&mut self) -> Result<Start, Fail> {
        if self.indented || !self.h.has(SyntaxExtension::Amsmath) {
            return Ok(Start::None);
        }
        if amsmath_begin(self.nonspace_rest()).is_none() {
            return Ok(Start::None);
        }
        self.close_unmatched()?;
        self.advance_next_nonspace();
        let id = self.add_child(BlockKind::MathBlock, self.next_nonspace)?;
        self.blk_mut(id).level = 2;
        Ok(Start::Leaf)
    }

    /// Tells whether the line just added to math block `id` ends it: "$$" at the end,
    /// optionally followed by " (label)"; or "\end{env}" for amsmath.
    pub(crate) fn math_closes(&self, id: Bid) -> bool {
        let t = trim_right(self.rest_from(self.offset.min(self.line.len())));
        if self.blk(id).level == 2 {
            let Some(env) = amsmath_begin(self.sk.content(id)) else { return false };
            return t.ends_with(&format!("\\end{{{env}}}"));
        }
        let (body, _) = math_label(self.h.settings.dmath_allow_labels, t);
        body.len() >= 2 && body.ends_with("$$")
    }

    /// Turns the last line of the open paragraph into a table header when this line is
    /// a delimiter row with the same number of columns.
    fn start_table(&mut self, container: Bid) -> Result<Start, Fail> {
        if self.indented
            || !self.h.on(Rule::Table)
            || self.kind(container) != BlockKind::Paragraph
            || !self.all_closed
        {
            return Ok(Start::None);
        }
        let Some(cols) = delimiter_row(self.nonspace_rest()) else { return Ok(Start::None) };
        let para = *self.blk(container);
        if para.seg_end <= para.seg_first {
            return Ok(Start::None);
        }
        let last = para.seg_end - 1;
        let header = self.sk.arena_str(self.sk.segs[last].content, para.content_end);
        if count_cells(header) != cols || !header.contains('|') {
            return Ok(Start::None);
        }
        if para.seg_end - para.seg_first == 1 {
            self.blk_mut(container).kind = BlockKind::Table;
        } else {
            let start = self.sk.segs[last].content;
            let p = self.blk_mut(container);
            p.content_end = start;
            p.seg_end = last;
            self.finalize(container, self.prev_end)?;
            let source = self.sk.segs[last].source;
            let id = self.add_child(BlockKind::Table, 0)?;
            let arena_end = self.sk.arena.len();
            let t = self.blk_mut(id);
            t.content_start = start;
            t.content_end = arena_end;
            t.seg_first = last;
            t.seg_end = last + 1;
            t.start = source;
        }
        if let Some(tip) = self.tip {
            self.blk_mut(tip).columns = cols;
        }
        self.advance_next_nonspace();
        Ok(Start::Leaf)
    }

    /// Starts a definition ("deflist"): a line opening with ":" or "~" and a space,
    /// after a one-line paragraph (the term), at most one blank line before it.
    fn start_definition(&mut self, container: Bid) -> Result<Start, Fail> {
        if self.indented || !self.h.has(SyntaxExtension::Deflist) || !self.definition_marker() {
            return Ok(Start::None);
        }
        let (term, list) = self.definition_term(container);
        if term.is_none() && list.is_none() {
            return Ok(Start::None);
        }
        self.close_unmatched()?;
        if let Some(term) = term {
            self.parse_paragraph_refs(term)?;
            let t = *self.blk(term);
            if t.content_end <= t.content_start {
                return Ok(Start::None);
            }
            if t.open {
                self.finalize(term, self.prev_end)?;
            }
        }
        let list = match (list, term) {
            (Some(list), _) => list,
            (None, Some(term)) => self.wrap_term(term)?,
            (None, None) => return Ok(Start::None),
        };
        if let Some(term) = term {
            self.blk_mut(term).kind = BlockKind::DefinitionTerm;
        }
        while let Some(tip) = self.tip.filter(|&t| t != list) {
            self.finalize(tip, self.prev_end)?;
        }
        self.start_marked_container(BlockKind::Definition, 1)
    }

    fn definition_marker(&self) -> bool {
        let s = self.nonspace_rest().as_bytes();
        s.len() >= 2 && (s[0] == b':' || s[0] == b'~') && is_space_or_tab(s[1])
    }

    /// Finds the term a definition line belongs to, and the list to add to.
    fn definition_term(&self, container: Bid) -> (Option<Bid>, Option<Bid>) {
        let c = *self.blk(container);
        let one_line = |id: Bid| self.sk.seg_count(id) == 1;
        match c.kind {
            BlockKind::Paragraph => {
                if !one_line(container) {
                    return (None, None);
                }
                let parent = c.parent.unwrap_or(0);
                let list = (self.kind(parent) == BlockKind::DefinitionList).then_some(parent);
                (Some(container), list)
            }
            BlockKind::DefinitionList => {
                match c.last.filter(|&l| self.kind(l) == BlockKind::Paragraph) {
                    Some(last) if !one_line(last) => (None, None),
                    last => (last, Some(container)),
                }
            }
            _ => {
                let Some(last) = c.last else { return (None, None) };
                let b = self.blk(last);
                if b.kind != BlockKind::Paragraph || b.open || !one_line(last) {
                    return (None, None);
                }
                if b.start_line + 2 != self.line_number {
                    return (None, None);
                }
                (Some(last), None)
            }
        }
    }

    /// Replaces a closed term paragraph, its parent's last child, by a definition list
    /// holding it.
    fn wrap_term(&mut self, term: Bid) -> Result<Bid, Fail> {
        let parent = self.blk(term).parent.unwrap_or(0);
        self.unlink_last(term);
        while let Some(tip) = self.tip.filter(|&t| t != parent) {
            self.finalize(tip, self.prev_end)?;
        }
        self.tip = Some(parent);
        let list = self.add_child(BlockKind::DefinitionList, self.next_nonspace)?;
        let start = self.blk(term).start;
        let l = self.blk_mut(list);
        l.start = start;
        l.first = Some(term);
        l.last = Some(term);
        let t = self.blk_mut(term);
        t.parent = Some(list);
        t.prev = None;
        t.next = None;
        Ok(list)
    }

    /// Opens a container whose content follows a marker `width` columns wide, as a
    /// list item's does.
    fn start_marked_container(&mut self, kind: BlockKind, width: usize) -> Result<Start, Fail> {
        let marker_offset = self.indent;
        self.advance_next_nonspace();
        self.advance_offset(width, true);
        let padding = self.marker_padding(width);
        let id = self.add_child(kind, self.next_nonspace)?;
        let b = self.blk_mut(id);
        b.padding = padding;
        b.marker_offset = marker_offset;
        Ok(Start::Container)
    }

    /// Starts a field ("fieldlist"): ":name: body".
    fn start_field(&mut self) -> Result<Start, Fail> {
        if self.indented || !self.h.has(SyntaxExtension::Fieldlist) {
            return Ok(Start::None);
        }
        let s = self.nonspace_rest();
        let n = field_marker(s);
        if n == 0 {
            return Ok(Start::None);
        }
        let name_start = (self.line_start + self.next_nonspace + 1) as u32;
        let name_end = name_start + (n - 2) as u32;
        let body = !trim_right(&s[n..]).is_empty();
        self.close_unmatched()?;
        if self.tip_kind() != BlockKind::FieldList {
            self.add_child(BlockKind::FieldList, self.next_nonspace)?;
        }
        let r = self.start_marked_container(BlockKind::Field, n)?;
        if let Some(tip) = self.tip {
            let f = self.blk_mut(tip);
            f.name_start = name_start;
            f.name_end = name_end;
            // A body that starts on a later line sets the indentation with its first line.
            if !body {
                f.padding = 0;
            }
        }
        Ok(r)
    }

    /// Starts "[^label]: text"; its lines are kept for the end of the document.
    fn start_footnote_def(&mut self) -> Result<Start, Fail> {
        if self.indented || !self.h.on(Rule::FootnoteDef) {
            return Ok(Start::None);
        }
        let s = self.nonspace_rest();
        let b = s.as_bytes();
        if b.len() < 5 || b[0] != b'[' || b[1] != b'^' {
            return Ok(Start::None);
        }
        let Some(e) = b[2..].iter().position(|&c| c == b']' || is_space_or_tab(c)) else {
            return Ok(Start::None);
        };
        let e = e + 2;
        if b[e] != b']' || e == 2 || e + 1 >= b.len() || b[e + 1] != b':' {
            return Ok(Start::None);
        }
        self.close_unmatched()?;
        let at = self.line_start + self.next_nonspace;
        self.advance_next_nonspace();
        self.advance_offset(e + 2, false);
        while is_space_or_tab(self.peek(self.offset)) {
            self.advance_offset(1, true);
        }
        let id = self.add_child(BlockKind::FootnoteDef, self.next_nonspace)?;
        let span = self.sk.span(at, at + e + 2);
        let note = self.add_note(&s[2..e], span);
        let blk = self.blk_mut(id);
        blk.padding = 4;
        blk.marker_offset = 0;
        blk.name_start = (at + 2) as u32;
        blk.name_end = (at + e) as u32;
        blk.note = note;
        if note.is_some() {
            self.note = Some(id);
            self.note_at = Some(self.offset);
        }
        Ok(Start::Container)
    }

    /// Continues the MyST blocks (`continue_block` does CommonMark's).
    pub(crate) fn continue_myst(&mut self, id: Bid) -> Continue {
        let blk = *self.blk(id);
        match blk.kind {
            BlockKind::Comment => {
                if self.indented || self.peek(self.next_nonspace) != b'%' {
                    return Continue::Failed;
                }
                self.advance_next_nonspace();
                self.advance_offset(1, false);
                Continue::Matched
            }
            BlockKind::MathBlock => Continue::Matched,
            BlockKind::Table if self.blank => Continue::Failed,
            BlockKind::Table => Continue::Matched,
            BlockKind::FootnoteDef => {
                let r = self.continue_padded(id);
                if r == Continue::Matched && blk.note.is_some() && self.note == Some(id) {
                    self.note_at = Some(self.offset);
                }
                r
            }
            BlockKind::Definition => self.continue_padded(id),
            BlockKind::Field => {
                if blk.padding == 0 && !self.blank {
                    if self.indent <= blk.marker_offset {
                        return Continue::Failed;
                    }
                    self.blk_mut(id).padding = self.indent - blk.marker_offset;
                }
                self.continue_padded(id)
            }
            BlockKind::DefinitionList => self.continue_definitions(id),
            BlockKind::FieldList => {
                let more = self.blank
                    || self.indented
                    || field_marker(self.nonspace_rest()) > 0
                    || self.indent > 0;
                if more { Continue::Matched } else { Continue::Failed }
            }
            _ => Continue::Failed,
        }
    }

    /// Keeps a definition list open for its definitions, blank lines, and the next
    /// term; a paragraph that no definition follows ends it.
    fn continue_definitions(&mut self, id: Bid) -> Continue {
        if self.blank {
            return Continue::Matched;
        }
        let marker = !self.indented && self.definition_marker();
        let last = self.blk(id).last;
        if last.is_some_and(|l| self.kind(l) == BlockKind::Paragraph) && !marker {
            return Continue::Failed;
        }
        Continue::Matched
    }

    /// Completes the MyST blocks when they close.
    pub(crate) fn finalize_myst(&mut self, id: Bid) {
        let blk = *self.blk(id);
        match blk.kind {
            BlockKind::FootnoteDef => {
                if self.note == Some(id) {
                    self.note = None;
                }
            }
            BlockKind::DefinitionList => {
                // A trailing paragraph that no definition followed is not a term.
                if let Some(last) = blk
                    .last
                    .filter(|&l| self.kind(l) == BlockKind::Paragraph && Some(l) != blk.first)
                {
                    self.move_after(last, id);
                }
            }
            _ => {}
        }
    }

    /// Moves block `id`, the last child of its parent, to follow block `at`, its
    /// parent's last child.
    fn move_after(&mut self, id: Bid, at: Bid) {
        self.unlink_last(id);
        let parent = self.blk(at).parent.unwrap_or(0);
        let b = self.blk_mut(id);
        b.parent = Some(parent);
        b.prev = Some(at);
        b.next = None;
        self.blk_mut(at).next = Some(id);
        self.blk_mut(parent).last = Some(id);
    }

    /// Keeps the current line for the open footnote definition.
    pub(crate) fn record_note_line(&mut self) {
        let (Some(note), Some(at)) = (self.note, self.note_at) else { return };
        let blk = self.blk(note);
        let Some(index) = blk.note.filter(|_| blk.open) else { return };
        let at = at.min(self.line.len());
        let text = self.rest_from(at);
        let ms = &mut self.h.ms;
        let start = ms.note_text.len();
        ms.note_text.push_str(text);
        if let Some(n) = ms.notes.get_mut(index) {
            n.lines.push((start, ms.note_text.len(), (self.line_start + at) as u32));
        }
    }
}
