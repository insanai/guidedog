//! Inline parsing. A leaf's content is parsed into a small tree of items (emphasis and
//! links wrap siblings only after their closers are seen), the text transforms of the
//! optional syntax run on it, and it is emitted in preorder. Item text lives in the
//! parser's own buffer; only emitted text reaches the document's text pool.

mod delimiters;
pub(crate) mod entities;
mod entity_table;
pub(crate) mod html_scan;
mod linkify;
mod links;
mod typography;

use guidedoc::{Fail, LinkKind, NodeKind, SourceSpan, Text};

use crate::blocks::{Bid, Skeleton};
use crate::host::Host;
use crate::settings::{Rule, SyntaxExtension};
use delimiters::Delim;
use entities::match_entity;
use html_scan::{HtmlMemo, scan_inline_html};
use links::Bracket;

/// The longest backtick run whose position is remembered.
const MAX_BACKTICKS: usize = 1000;

/// A range of the parser's text buffer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Txt {
    pub start: usize,
    pub len: usize,
}

/// One inline item. The root (index 0) holds the leaf's top-level items.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Item {
    pub kind: NodeKind,
    pub link_kind: LinkKind,
    pub parent: Option<usize>,
    pub first: Option<usize>,
    pub last: Option<usize>,
    pub prev: Option<usize>,
    pub next: Option<usize>,
    pub text: Txt,
    pub dest: Txt,
    pub title: Option<Txt>,
    /// Subject offsets.
    pub start: usize,
    pub end: usize,
    /// A role's name, a footnote label, or a substitution expression (subject range).
    pub aux: (usize, usize),
    /// A role's content (subject range).
    pub content: (usize, usize),
    /// A Link from an autolink or linkify, always external.
    pub autolink: bool,
}

impl Item {
    fn new(kind: NodeKind, start: usize, end: usize) -> Item {
        Item {
            kind,
            link_kind: LinkKind::External,
            parent: None,
            first: None,
            last: None,
            prev: None,
            next: None,
            text: Txt::default(),
            dest: Txt::default(),
            title: None,
            start,
            end,
            aux: (0, 0),
            content: (0, 0),
            autolink: false,
        }
    }
}

/// The inline parser's buffers, kept in the workspace's scratch between leaves and
/// reads (`build_items` takes them, dropping the parse gives them back).
#[derive(Debug, Default)]
pub(crate) struct InlineParts {
    items: Vec<Item>,
    text: String,
    delims: Vec<Delim>,
    brackets: Vec<Bracket>,
    ticks: Vec<usize>,
    /// The tree walks' stack of next siblings.
    walk: Vec<Option<usize>>,
    /// The items `text_items` found.
    found: Vec<usize>,
    /// Room for a text being rewritten.
    spare: String,
}

/// The inline parse of one leaf of a skeleton.
pub(crate) struct Inline<'p, 'h, 'a, 'k> {
    pub h: &'p mut Host<'h, 'a>,
    pub sk: &'k Skeleton<'k>,
    pub blk: Bid,
    pub subject: &'k str,
    /// The subject's arena offset.
    pub base: usize,
    pub pos: usize,
    pub items: Vec<Item>,
    pub text: String,
    pub delims: Vec<Delim>,
    pub top_delim: Option<usize>,
    pub brackets: Vec<Bracket>,
    pub top_bracket: Option<usize>,
    /// The latest start of a backtick run of each length, once scanned.
    ticks: Vec<usize>,
    /// A scan for a closing run reached the end of the subject.
    ticks_scanned: bool,
    memo: HtmlMemo,
    /// MyST scans that found no end from these positions on, so later starts need not
    /// rescan: a substitution's closing delimiters, and a role's closing backticks by
    /// run length.
    pub no_sub_end: usize,
    pub no_role_end: [usize; 8],
    /// The tree walks' stack of next siblings.
    walk: Vec<Option<usize>>,
    /// The items `text_items` found, lent to its caller.
    pub found: Vec<usize>,
    /// Room for a text being rewritten.
    pub spare: String,
}

impl Drop for Inline<'_, '_, '_, '_> {
    /// Gives the buffers back to the workspace for the next parse.
    fn drop(&mut self) {
        let parts = InlineParts {
            items: std::mem::take(&mut self.items),
            text: std::mem::take(&mut self.text),
            delims: std::mem::take(&mut self.delims),
            brackets: std::mem::take(&mut self.brackets),
            ticks: std::mem::take(&mut self.ticks),
            walk: std::mem::take(&mut self.walk),
            found: std::mem::take(&mut self.found),
            spare: std::mem::take(&mut self.spare),
        };
        self.h.ctx.put_scratch(parts);
    }
}

fn is_special(c: u8) -> bool {
    matches!(
        c,
        b'\n' | b'`' | b'[' | b']' | b'\\' | b'!' | b'<' | b'&' | b'*' | b'_' | b'{' | b'$'
    )
}

/// Parses and emits the inline content of a paragraph or heading, whose node is open.
pub(crate) fn parse_leaf(h: &mut Host<'_, '_>, sk: &Skeleton<'_>, id: Bid) -> Result<(), Fail> {
    let b = &sk.blocks[id];
    parse_leaf_range(h, sk, id, b.content_start, b.content_end)
}

/// Parses and emits the inline content of arena range `[start, end)` of block `id`.
pub(crate) fn parse_leaf_range(
    h: &mut Host<'_, '_>,
    sk: &Skeleton<'_>,
    id: Bid,
    start: usize,
    end: usize,
) -> Result<(), Fail> {
    if h.myst && h.ms.counting {
        h.ms.words += count_words(sk.arena_str(start, end));
    }
    match build_items(h, sk, id, start, end)? {
        Some(mut ip) => ip.emit_items(),
        None => Ok(()),
    }
}

/// Writes the plain text of a leaf's text items into `out`, as heading slugs use it.
pub(crate) fn leaf_text(
    h: &mut Host<'_, '_>,
    sk: &Skeleton<'_>,
    id: Bid,
    out: &mut String,
) -> Result<(), Fail> {
    let b = &sk.blocks[id];
    if let Some(mut ip) = build_items(h, sk, id, b.content_start, b.content_end)? {
        ip.items_text(0, out);
    }
    Ok(())
}

/// Tells whether a leaf's inline content is one image and nothing else.
pub(crate) fn leaf_is_image(
    h: &mut Host<'_, '_>,
    sk: &Skeleton<'_>,
    id: Bid,
) -> Result<bool, Fail> {
    let b = &sk.blocks[id];
    let Some(ip) = build_items(h, sk, id, b.content_start, b.content_end)? else {
        return Ok(false);
    };
    Ok(ip.items[0]
        .first
        .is_some_and(|f| ip.items[f].kind == NodeKind::Image && ip.items[f].next.is_none()))
}

/// Parses arena range `[start, end)` of block `id` into the inline tree; `None` when
/// it holds only white space.
fn build_items<'p, 'h, 'a, 'k>(
    h: &'p mut Host<'h, 'a>,
    sk: &'k Skeleton<'k>,
    id: Bid,
    start: usize,
    end: usize,
) -> Result<Option<Inline<'p, 'h, 'a, 'k>>, Fail> {
    let bytes = sk.arena.as_bytes();
    let (mut s, mut e) = (start, end.min(bytes.len()));
    while s < e && guidedoc::is_ascii_space(bytes[s]) {
        s += 1;
    }
    while e > s && guidedoc::is_ascii_space(bytes[e - 1]) {
        e -= 1;
    }
    if s == e {
        return Ok(None);
    }
    let subject = sk.arena_str(s, e);
    let mut parts: InlineParts = h.ctx.take_scratch();
    parts.items.clear();
    parts.text.clear();
    parts.text.reserve(subject.len());
    parts.delims.clear();
    parts.brackets.clear();
    parts.ticks.clear();
    let mut ip = Inline {
        h,
        sk,
        blk: id,
        subject,
        base: s,
        pos: 0,
        items: parts.items,
        text: parts.text,
        delims: parts.delims,
        top_delim: None,
        brackets: parts.brackets,
        top_bracket: None,
        ticks: parts.ticks,
        ticks_scanned: false,
        memo: HtmlMemo::default(),
        no_sub_end: usize::MAX,
        no_role_end: [usize::MAX; 8],
        walk: parts.walk,
        found: parts.found,
        spare: parts.spare,
    };
    ip.items.push(Item::new(NodeKind::Paragraph, 0, 0));
    while ip.pos < subject.len() {
        ip.step()?;
    }
    ip.process_emphasis(None)?;
    if ip.h.myst {
        ip.transform_text()?;
    }
    Ok(Some(ip))
}

/// Counts the white-space separated words of `s`, for the wordcount (myst-parser counts
/// the words of the Markdown source of every paragraph, heading, and table cell).
pub(crate) fn count_words(s: &str) -> usize {
    s.split(|c: char| c.is_ascii_whitespace()).filter(|w| !w.is_empty()).count()
}

impl<'p, 'h, 'a, 'k> Inline<'p, 'h, 'a, 'k> {
    /// The text of buffer range `t`.
    pub fn str(&self, t: Txt) -> &str {
        self.text.get(t.start..t.start + t.len).unwrap_or("")
    }

    /// Appends `s` to the buffer and returns its range.
    pub fn push_text(&mut self, s: &str) -> Txt {
        let start = self.text.len();
        self.text.push_str(s);
        Txt { start, len: s.len() }
    }

    fn step(&mut self) -> Result<(), Fail> {
        let c = self.subject.as_bytes()[self.pos];
        let handled = match c {
            b'\n' => self.h.on(Rule::Newline) && self.parse_newline(),
            b'\\' => self.h.on(Rule::Escape) && self.parse_backslash(),
            b'`' => self.h.on(Rule::Backticks) && self.parse_backticks()?,
            b'*' | b'_' => self.h.on(Rule::Emphasis) && self.handle_delim(c),
            b'[' => {
                (self.h.myst && self.parse_footnote_ref())
                    || (self.h.on(Rule::Link) && self.parse_open_bracket())
            }
            b'{' => self.h.myst && (self.parse_role()? || self.parse_substitution()?),
            b'$' => {
                self.h.has(SyntaxExtension::Dollarmath)
                    && self.h.on(Rule::MathInline)
                    && self.parse_dollar_math()?
            }
            b'!' => self.h.on(Rule::Image) && self.parse_bang(),
            b']' => self.parse_close_bracket()?,
            b'<' => {
                (self.h.on(Rule::Autolink) && self.parse_autolink())
                    || (self.h.on(Rule::HtmlInline) && self.parse_html()?)
            }
            b'&' => self.h.on(Rule::Entity) && self.parse_entity(),
            _ => {
                let start = self.pos;
                let b = self.subject.as_bytes();
                while self.pos < b.len() && !is_special(b[self.pos]) {
                    self.pos += 1;
                }
                self.add_text_item(start, self.pos);
                true
            }
        };
        if !handled {
            self.pos += 1;
            self.add_text_item(self.pos - 1, self.pos);
        }
        Ok(())
    }

    /// Makes an unlinked item.
    pub fn new_item(&mut self, kind: NodeKind, start: usize, end: usize) -> usize {
        self.items.push(Item::new(kind, start, end));
        self.items.len() - 1
    }

    /// Appends `child` to `parent`'s children.
    pub fn append_child(&mut self, parent: usize, child: usize) {
        let prev = self.items[parent].last;
        let it = &mut self.items[child];
        it.parent = Some(parent);
        it.next = None;
        it.prev = prev;
        match prev {
            Some(p) => self.items[p].next = Some(child),
            None => self.items[parent].first = Some(child),
        }
        self.items[parent].last = Some(child);
    }

    /// Removes `id` from its parent's children.
    pub fn unlink_item(&mut self, id: usize) {
        let it = self.items[id];
        let Some(parent) = it.parent else { return };
        match it.prev {
            Some(p) => self.items[p].next = it.next,
            None => self.items[parent].first = it.next,
        }
        match it.next {
            Some(n) => self.items[n].prev = it.prev,
            None => self.items[parent].last = it.prev,
        }
        let it = &mut self.items[id];
        it.parent = None;
        it.prev = None;
        it.next = None;
    }

    /// Inserts `id` as the sibling after `at`.
    pub fn insert_after(&mut self, at: usize, id: usize) {
        let parent = self.items[at].parent;
        let next = self.items[at].next;
        let it = &mut self.items[id];
        it.parent = parent;
        it.prev = Some(at);
        it.next = next;
        match next {
            Some(n) => self.items[n].prev = Some(id),
            None => {
                if let Some(p) = parent {
                    self.items[p].last = Some(id);
                }
            }
        }
        self.items[at].next = Some(id);
    }

    /// Adds an item at the top level.
    pub fn add_item(&mut self, kind: NodeKind, start: usize, end: usize) -> usize {
        let id = self.new_item(kind, start, end);
        self.append_child(0, id);
        id
    }

    /// Adds subject range `[start, end)` as a text item.
    pub fn add_text_item(&mut self, start: usize, end: usize) -> usize {
        let subject = self.subject;
        self.add_literal_item(&subject[start..end], start, end)
    }

    /// Adds text `s`, written at subject range `[start, end)`, as a text item.
    pub fn add_literal_item(&mut self, s: &str, start: usize, end: usize) -> usize {
        let id = self.add_item(NodeKind::Text, start, end);
        self.items[id].text = self.push_text(s);
        id
    }

    fn parse_newline(&mut self) -> bool {
        let start = self.pos;
        self.pos += 1;
        let mut hard = false;
        if let Some(last) = self.items[0].last.filter(|&l| self.items[l].kind == NodeKind::Text) {
            let t = self.items[last].text;
            let s = self.str(t).as_bytes();
            if s.last() == Some(&b' ') {
                hard = s.len() > 1 && s[s.len() - 2] == b' ';
                let spaces = s.iter().rev().take_while(|&&c| c == b' ').count();
                self.items[last].text.len -= spaces;
            }
        }
        let kind = if hard { NodeKind::HardBreak } else { NodeKind::SoftBreak };
        self.add_item(kind, start, self.pos);
        let b = self.subject.as_bytes();
        while self.pos < b.len() && b[self.pos] == b' ' {
            self.pos += 1;
        }
        true
    }

    fn parse_backslash(&mut self) -> bool {
        let start = self.pos;
        self.pos += 1;
        let b = self.subject.as_bytes();
        match b.get(self.pos) {
            Some(b'\n') => {
                self.pos += 1;
                self.add_item(NodeKind::HardBreak, start, self.pos);
            }
            Some(c) if c.is_ascii_punctuation() => {
                self.pos += 1;
                let subject = self.subject;
                self.add_literal_item(&subject[self.pos - 1..self.pos], start, self.pos);
            }
            _ => {
                self.add_literal_item("\\", start, self.pos);
            }
        }
        true
    }

    fn parse_backticks(&mut self) -> Result<bool, Fail> {
        let s = self.subject.as_bytes();
        let start = self.pos;
        let after = start + s[start..].iter().take_while(|&&c| c == b'`').count();
        let n = after - start;
        self.pos = after;
        let known = self.ticks.get(n).copied();
        if n > MAX_BACKTICKS || !self.ticks_scanned || known.is_some_and(|k| k > start) {
            if self.ticks.is_empty() {
                self.ticks.resize(MAX_BACKTICKS + 1, 0);
            }
            let mut i = after;
            while i < s.len() {
                if s[i] != b'`' {
                    i += 1;
                    continue;
                }
                let j = i + s[i..].iter().take_while(|&&c| c == b'`').count();
                if j - i <= MAX_BACKTICKS {
                    self.ticks[j - i] = i;
                }
                if j - i == n {
                    self.h.spend(j - start)?;
                    self.add_code_span(start, after, i, j);
                    self.pos = j;
                    return Ok(true);
                }
                i = j;
            }
            self.h.spend(s.len() - start)?;
            self.ticks_scanned = true;
        }
        self.add_text_item(start, after);
        Ok(true)
    }

    fn add_code_span(&mut self, start: usize, content: usize, close: usize, end: usize) {
        let id = self.add_item(NodeKind::Code, start, end);
        // Line endings become spaces.
        let at = self.text.len();
        for (k, piece) in self.subject[content..close].split('\n').enumerate() {
            if k > 0 {
                self.text.push(' ');
            }
            self.text.push_str(piece);
        }
        let mut t = Txt { start: at, len: self.text.len() - at };
        let b = &self.text.as_bytes()[at..];
        let only_spaces = b.iter().all(|&c| c == b' ');
        if b.len() >= 2 && !only_spaces && b[0] == b' ' && b[b.len() - 1] == b' ' {
            t = Txt { start: t.start + 1, len: t.len - 2 };
        }
        self.items[id].text = t;
    }

    fn parse_autolink(&mut self) -> bool {
        let s = &self.subject.as_bytes()[self.pos..];
        let (kind, n) = match scan_email_autolink(s) {
            0 => (LinkKind::External, scan_uri_autolink(s)),
            n => (LinkKind::Email, n),
        };
        if n == 0 {
            return false;
        }
        let start = self.pos;
        self.pos += n;
        let link = self.add_item(NodeKind::Link, start, self.pos);
        let child = self.new_item(NodeKind::Text, start + 1, self.pos - 1);
        let subject = self.subject;
        let t = self.push_text(&subject[start + 1..start + n - 1]);
        let it = &mut self.items[link];
        it.dest = t;
        it.link_kind = kind;
        it.autolink = true;
        self.items[child].text = t;
        self.append_child(link, child);
        true
    }

    fn parse_html(&mut self) -> Result<bool, Fail> {
        let n = scan_inline_html(self.subject.as_bytes(), self.pos, &mut self.memo);
        self.h.spend(n.max(1))?;
        if n == 0 {
            return Ok(false);
        }
        let id = self.add_item(NodeKind::RawInline, self.pos, self.pos + n);
        let subject = self.subject;
        self.items[id].text = self.push_text(&subject[self.pos..self.pos + n]);
        self.pos += n;
        Ok(true)
    }

    fn parse_entity(&mut self) -> bool {
        let Some((decoded, used)) = match_entity(&self.subject.as_bytes()[self.pos..]) else {
            return false;
        };
        let mut s = std::mem::take(&mut self.spare);
        s.clear();
        decoded.push_to(&mut s);
        self.add_literal_item(&s, self.pos, self.pos + used);
        self.spare = s;
        self.pos += used;
        true
    }

    /// The source span of an item.
    pub fn item_span(&self, start: usize, end: usize) -> SourceSpan {
        let a = self.sk.source_offset(self.blk, self.base + start);
        let z = self.sk.source_offset(self.blk, self.base + end.max(start));
        self.sk.span(a, z)
    }

    /// Writes the inline tree in preorder, merging adjacent text.
    fn emit_items(&mut self) -> Result<(), Fail> {
        let mut id = self.items[0].first;
        while let Some(i) = id {
            let it = self.items[i];
            let mut at = i;
            match it.kind {
                NodeKind::Text => at = self.emit_text_run(i)?,
                NodeKind::Emphasis | NodeKind::Strong | NodeKind::Link | NodeKind::Image => {
                    let node = self.open_inline(i)?;
                    if it.first.is_some() {
                        id = it.first;
                        continue;
                    }
                    self.h.b().end_node(node)?;
                }
                NodeKind::Role
                | NodeKind::Math
                | NodeKind::FootnoteReference
                | NodeKind::SubstitutionReference => self.emit_myst_item(it)?,
                NodeKind::RawInline => self.emit_raw_inline(it)?,
                _ => {
                    let span = self.item_span(it.start, it.end);
                    let Inline { h, text, .. } = self;
                    let s = text.get(it.text.start..it.text.start + it.text.len).unwrap_or("");
                    h.b().add_text_leaf(it.kind, span, s)?;
                }
            }
            id = self.next_in_preorder(at)?;
        }
        Ok(())
    }

    /// The item after `at` in preorder, closing the nodes of the items left.
    fn next_in_preorder(&mut self, at: usize) -> Result<Option<usize>, Fail> {
        let mut cur = at;
        loop {
            if let Some(next) = self.items[cur].next {
                return Ok(Some(next));
            }
            match self.items[cur].parent {
                Some(p) if p > 0 => {
                    let b = self.h.b();
                    if let Some(open) = b.current() {
                        b.end_node(open)?;
                    }
                    cur = p;
                }
                _ => return Ok(None),
            }
        }
    }

    fn emit_raw_inline(&mut self, it: Item) -> Result<(), Fail> {
        if self.h.myst && self.html_inline_image(it)? {
            return Ok(());
        }
        let span = self.item_span(it.start, it.end);
        let format = self.h.html_format()?;
        let Inline { h, text, .. } = self;
        let s = text.get(it.text.start..it.text.start + it.text.len).unwrap_or("");
        let b = h.b();
        let node = b.begin_node(NodeKind::RawInline, span)?;
        let t = b.add_text(s)?;
        b.set_text(node, t);
        b.add_attribute(node, guidedoc::AttributeKey::Format, format)?;
        b.end_node(node)
    }

    /// Emits one Text node for a run of text siblings and returns the last.
    fn emit_text_run(&mut self, first: usize) -> Result<usize, Fail> {
        let mut last = first;
        let mut cur = Some(first);
        while let Some(c) = cur.filter(|&c| self.items[c].kind == NodeKind::Text) {
            last = c;
            cur = self.items[c].next;
        }
        let span = self.item_span(self.items[first].start, self.items[last].end);
        let Inline { h, text, items, .. } = self;
        let b = h.b();
        let start = b.text_open();
        let mut cur = Some(first);
        while let Some(c) = cur {
            let t = items[c].text;
            b.text_push(text.get(t.start..t.start + t.len).unwrap_or(""))?;
            cur = if c == last { None } else { items[c].next };
        }
        let total = b.text_close(start);
        if total.len > 0 {
            b.add_leaf(NodeKind::Text, span, total)?;
        }
        Ok(last)
    }

    /// Opens the node of an emphasis, strong, link, or image item.
    fn open_inline(&mut self, id: usize) -> Result<guidedoc::NodeId, Fail> {
        let it = self.items[id];
        if it.kind == NodeKind::Link && self.h.myst && !it.autolink {
            return self.open_myst_link(id);
        }
        let span = self.item_span(it.start, it.end);
        if it.kind != NodeKind::Link && it.kind != NodeKind::Image {
            return self.h.b().begin_node(it.kind, span);
        }
        let Inline { h, text, sk, .. } = self;
        let piece = |t: Txt| text.get(t.start..t.start + t.len).unwrap_or("");
        let mut dest = std::borrow::Cow::Borrowed(piece(it.dest));
        if it.kind == NodeKind::Image && h.myst && h.rel_images {
            dest = crate::myst::relocate(h, sk.source, piece(it.dest), true);
        }
        let b = h.b();
        let node = b.begin_node(it.kind, span)?;
        let destination = b.add_text(&dest)?;
        let title = match it.title {
            Some(t) => b.add_text(piece(t))?,
            None => Text::default(),
        };
        let info = guidedoc::LinkInfo {
            destination,
            title,
            kind: it.link_kind,
            ..guidedoc::LinkInfo::default()
        };
        b.set_link(node, info)?;
        Ok(node)
    }

    /// Writes the text items below item `id` into `out`, as a heading's slug needs.
    pub fn items_text(&mut self, id: usize, out: &mut String) {
        let mut stack = std::mem::take(&mut self.walk);
        stack.clear();
        stack.push(self.items[id].first);
        while let Some(top) = stack.last_mut() {
            let Some(c) = *top else {
                stack.pop();
                continue;
            };
            *top = self.items[c].next;
            let it = self.items[c];
            match it.kind {
                NodeKind::Text => out.push_str(self.str(it.text)),
                NodeKind::SoftBreak | NodeKind::HardBreak => out.push(' '),
                NodeKind::Emphasis | NodeKind::Strong | NodeKind::Link => stack.push(it.first),
                _ => {}
            }
        }
        self.walk = stack;
    }

    /// The text items below `parent`, in preorder, descending into emphasis and strong
    /// (and links when `links` is set). The list is lent from the parser's buffers:
    /// give it back to `self.found` when done.
    pub fn text_items(&mut self, parent: usize, links: bool) -> Vec<usize> {
        let mut out = std::mem::take(&mut self.found);
        out.clear();
        let mut stack = std::mem::take(&mut self.walk);
        stack.clear();
        stack.push(self.items[parent].first);
        while let Some(top) = stack.last_mut() {
            let Some(c) = *top else {
                stack.pop();
                continue;
            };
            *top = self.items[c].next;
            match self.items[c].kind {
                NodeKind::Text => out.push(c),
                NodeKind::Emphasis | NodeKind::Strong => stack.push(self.items[c].first),
                NodeKind::Link if links => stack.push(self.items[c].first),
                _ => {}
            }
        }
        self.walk = stack;
        out
    }
}

fn scan_uri_autolink(s: &[u8]) -> usize {
    if s.len() < 4 || !s[1].is_ascii_alphabetic() {
        return 0;
    }
    let mut i = 2;
    while i < s.len() && (s[i].is_ascii_alphanumeric() || b".+-".contains(&s[i])) {
        i += 1;
    }
    if i - 1 < 2 || i - 1 > 32 || i >= s.len() || s[i] != b':' {
        return 0;
    }
    for (k, &c) in s.iter().enumerate().skip(i + 1) {
        if c == b'>' {
            return k + 1;
        }
        if c == b'<' || c <= b' ' {
            return 0;
        }
    }
    0
}

fn is_email_local(c: u8) -> bool {
    c.is_ascii_alphanumeric() || b".!#$%&'*+/=?^_`{|}~-".contains(&c)
}

fn scan_email_autolink(s: &[u8]) -> usize {
    let mut i = 1;
    while i < s.len() && is_email_local(s[i]) {
        i += 1;
    }
    if i == 1 || i >= s.len() || s[i] != b'@' {
        return 0;
    }
    i += 1;
    loop {
        let start = i;
        while i < s.len() && (s[i].is_ascii_alphanumeric() || s[i] == b'-') {
            i += 1;
        }
        let n = i - start;
        if n == 0 || n > 63 || s[start] == b'-' || s[i - 1] == b'-' || i >= s.len() {
            return 0;
        }
        match s[i] {
            b'>' => return i + 1,
            b'.' => i += 1,
            _ => return 0,
        }
    }
}
