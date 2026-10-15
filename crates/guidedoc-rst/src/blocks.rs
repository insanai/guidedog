//! Block quotes with attributions, doctest blocks, and line blocks.

use guidedoc::{AttributeKey, Fail, NodeId, NodeKind, arg_span, arg_text};

use crate::lines::{Line, dedent, indent_of, indented_block, text_end, trim_blank_start};
use crate::messages::BLOCK_UNINDENT;
use crate::parser::{Parser, marker};

/// One line of a line block: its source lines, the column after "| ", and its
/// indentation (spaces after "| " less one; `None` for an empty line).
#[derive(Debug, Clone, Copy)]
pub(crate) struct LineItem {
    pub first: usize,
    pub count: usize,
    pub col: usize,
    pub indent: Option<usize>,
}

/// The column after "--", "---", or an em dash and spaces; 0 when `t` is no
/// attribution.
fn attribution_col(t: &str) -> usize {
    let n = if t.starts_with("---") || t.starts_with('—') {
        3
    } else if t.starts_with("--") {
        2
    } else {
        return 0;
    };
    let b = t.as_bytes();
    if n < b.len() && b[0] == b'-' && b[n] == b'-' {
        return 0;
    }
    let k = n + indent_of(&t[n..]);
    if k >= b.len() { 0 } else { k }
}

/// Finds an attribution: a line starting with a dash after a blank line, whose
/// continuation lines share one indentation. Returns the end of the body, the end of
/// the attribution, and its text column; without one the whole block is the body.
fn find_attribution(lines: &[Line<'_>]) -> (usize, usize, usize) {
    let mut seen = false;
    let mut blank_at: Option<usize> = None;
    for (i, l) in lines.iter().enumerate() {
        if l.is_blank() {
            blank_at = Some(i);
            continue;
        }
        if seen && blank_at.is_some_and(|b| b + 1 == i) {
            let c = attribution_col(l.text);
            if c > 0
                && let Some(end) = attribution_end(lines, i)
            {
                return (i, end, c);
            }
        }
        seen = true;
    }
    (lines.len(), lines.len(), 0)
}

fn attribution_end(lines: &[Line<'_>], start: usize) -> Option<usize> {
    let mut indent = None;
    let mut k = start + 1;
    while k < lines.len() && !lines[k].is_blank() {
        let n = indent_of(lines[k].text);
        match indent {
            None => indent = Some(n),
            Some(m) if m != n => return None,
            Some(_) => {}
        }
        k += 1;
    }
    Some(k)
}

impl Parser<'_, '_> {
    /// Reports a construct that ends without a blank line before `at`.
    pub(crate) fn unindent_warning(&mut self, at: &Line<'_>, what: &'static str) {
        let span = self.line_span(at);
        self.report(BLOCK_UNINDENT, span, arg_span(span), arg_text(what));
    }

    /// Reads an indented block at line `i` as block quotes.
    pub(crate) fn block_quote(&mut self, lines: &[Line<'_>], i: usize) -> Result<usize, Fail> {
        let blk = indented_block(lines, i, None, false, true);
        self.emit_quotes(&blk.lines, "")?;
        let next = i + blk.consumed;
        if !blk.blank_finish && next < lines.len() {
            self.unindent_warning(&lines[next], "block quote");
        }
        Ok(blk.consumed.max(1))
    }

    /// Emits dedented lines as block quotes. An attribution ends a quote; text after it
    /// starts another quote. `class` is added to every quote.
    pub(crate) fn emit_quotes(&mut self, lines: &[Line<'_>], class: &str) -> Result<(), Fail> {
        let mut rest = lines;
        loop {
            rest = trim_blank_start(rest);
            if rest.is_empty() {
                return Ok(());
            }
            let (body_end, attr_end, attr_col) = find_attribution(rest);
            let span = self.lines_span(&rest[..attr_end]);
            let q = self.open(NodeKind::BlockQuote, span)?;
            if !class.is_empty() {
                self.ctx.b.add_attribute_string(q, AttributeKey::Class, class)?;
            }
            self.parse_body(&rest[..body_end], false, None)?;
            if attr_end > body_end {
                self.attribution(&rest[body_end..attr_end], attr_col)?;
            }
            self.close(q)?;
            rest = &rest[attr_end..];
        }
    }

    fn attribution(&mut self, lines: &[Line<'_>], col: usize) -> Result<(), Fail> {
        let mut parts: Vec<Line<'_>> = lines.iter().map(|l| l.trim()).collect();
        parts[0] = lines[0].cut(col);
        self.inline_node(NodeKind::Attribution, &parts)
    }

    /// Reads a doctest block (">>> ") at line `i`.
    pub(crate) fn doctest_block(&mut self, lines: &[Line<'_>], i: usize) -> Result<usize, Fail> {
        let (end, _) = text_end(lines, i, false);
        let block = &lines[i..end];
        let text = self.pool_lines(block, "\n")?;
        self.leaf(NodeKind::DoctestBlock, self.lines_span(block), text)?;
        Ok(end - i)
    }

    /// Reads a line block ("| ") at line `i`.
    pub(crate) fn line_block(&mut self, lines: &[Line<'_>], i: usize) -> Result<usize, Fail> {
        let mut j = i;
        while j < lines.len() && marker(lines[j].text, "|") {
            j += 1;
            while j < lines.len() && lines[j].is_indented() {
                j += 1;
            }
        }
        let mut items = Vec::new();
        let mut k = i;
        while k < j {
            let t = lines[k].text;
            let sp = indent_of(&t[1..]);
            let mut e = k + 1;
            while e < j && lines[e].is_indented() {
                e += 1;
            }
            let indent = (t.len() > 1 + sp).then(|| sp.saturating_sub(1));
            items.push(LineItem { first: k, count: e - k, col: 1 + sp, indent });
            k = e;
        }
        fill_indents(&mut items);
        let id = self.open(NodeKind::LineBlock, self.lines_span(&lines[i..j]))?;
        self.emit_line_block(lines, &items, id)?;
        if j < lines.len() && !lines[j].is_blank() {
            self.unindent_warning(&lines[j], "line block");
        }
        Ok(j - i)
    }

    /// Emits `items` inside the open LineBlock `id` and closes it: items indented more
    /// than the least indentation form nested line blocks.
    pub(crate) fn emit_line_block(
        &mut self,
        lines: &[Line<'_>],
        items: &[LineItem],
        id: NodeId,
    ) -> Result<(), Fail> {
        let span = self.line_span(&lines[items[0].first]);
        self.nested(span, |p| p.line_block_items(lines, items))?;
        self.close(id)
    }

    fn line_block_items(&mut self, lines: &[Line<'_>], items: &[LineItem]) -> Result<(), Fail> {
        let least = items.iter().filter_map(|it| it.indent).min().unwrap_or(0);
        let deeper = |it: &LineItem| it.indent.unwrap_or(0) > least;
        let mut k = 0;
        while k < items.len() {
            if deeper(&items[k]) {
                let r = k + items[k..].iter().take_while(|it| deeper(it)).count();
                let last = items[r - 1];
                let span =
                    self.span_of(&lines[items[k].first], &lines[last.first + last.count - 1]);
                let inner = self.ctx.b.begin_node(NodeKind::LineBlock, span)?;
                self.emit_line_block(lines, &items[k..r], inner)?;
                k = r;
                continue;
            }
            self.line_item(lines, items[k])?;
            k += 1;
        }
        Ok(())
    }

    fn line_item(&mut self, lines: &[Line<'_>], it: LineItem) -> Result<(), Fail> {
        let src = &lines[it.first..it.first + it.count];
        let id = self.ctx.b.begin_node(NodeKind::Line, self.lines_span(src))?;
        if it.col < src[0].text.len() || it.count > 1 {
            let mut parts = Vec::with_capacity(it.count);
            parts.push(src[0].cut(it.col));
            parts.extend_from_slice(&src[1..]);
            dedent(&mut parts[1..]);
            self.inline(trim_blank_start(&parts))?;
        }
        self.close(id)
    }

    /// Runs the line-block directive: every content line is a line.
    pub(crate) fn line_block_directive(&mut self, d: &crate::Directive<'_>) -> Result<(), Fail> {
        let mut items: Vec<LineItem> = d
            .content
            .iter()
            .enumerate()
            .map(|(k, l)| {
                let n = indent_of(l.text);
                LineItem { first: k, count: 1, col: n, indent: (!l.is_blank()).then_some(n) }
            })
            .collect();
        fill_indents(&mut items);
        let id = self.open(NodeKind::LineBlock, d.span)?;
        self.add_common(id, d, "class")?;
        self.emit_line_block(&d.content, &items, id)
    }
}

/// Gives empty lines the indentation of the line before them.
fn fill_indents(items: &mut [LineItem]) {
    let mut prev = 0;
    for it in items.iter_mut() {
        let n = *it.indent.get_or_insert(prev);
        prev = n;
    }
}
