//! Paragraphs, literal blocks, and definition lists.

use guidedoc::{Fail, NodeKind};

use crate::lines::{Line, indented_block, text_end, trim_blank_end};
use crate::messages::{INDENT_UNEXPECTED, LITERAL_MISSING, LITERAL_QUOTING};
use crate::parser::{Construct, Parser, detect, is_punct7};

/// Tells whether `t` ends in an unescaped "::".
pub(crate) fn ends_with_literal_marker(t: &str) -> bool {
    let Some(rest) = t.strip_suffix("::") else {
        return false;
    };
    let slashes = rest.bytes().rev().take_while(|&c| c == b'\\').count();
    slashes % 2 == 0
}

/// Returns the paragraph lines without the literal marker, reduced as the
/// specification says, and whether a literal block follows.
fn strip_literal_marker<'s>(para: &[Line<'s>]) -> (Vec<Line<'s>>, bool) {
    let last = para[para.len() - 1];
    let t = last.text;
    if !ends_with_literal_marker(t) {
        return (para.to_vec(), false);
    }
    if t == "::" {
        return (para[..para.len() - 1].to_vec(), true);
    }
    let mut body = para.to_vec();
    let keep = if t.as_bytes()[t.len() - 3] == b' ' { t.len() - 3 } else { t.len() - 1 };
    body[para.len() - 1] = last.head(keep);
    (body, true)
}

impl Parser<'_, '_> {
    /// Reads the paragraph at line `i`. A paragraph ending in "::" introduces a literal
    /// block.
    pub(crate) fn paragraph(&mut self, lines: &[Line<'_>], i: usize) -> Result<usize, Fail> {
        let (end, bad_indent) = text_end(lines, i, true);
        let end = end.max(i + 1);
        let para = &lines[i..end];
        let (body, literal) = strip_literal_marker(para);
        if !body.is_empty() {
            let id = self.open(NodeKind::Paragraph, self.lines_span(para))?;
            self.inline(&body)?;
            self.close(id)?;
        }
        if bad_indent {
            let span = self.line_span(&lines[end]);
            self.report_at(INDENT_UNEXPECTED, span);
        }
        if !literal {
            return Ok(end - i);
        }
        Ok(end - i + self.literal_block(lines, end, &para[para.len() - 1])?)
    }

    /// Reads the literal block that follows a "::" paragraph from line `j`: an indented
    /// block, or else a block quoted by a repeated punctuation character.
    fn literal_block(
        &mut self,
        lines: &[Line<'_>],
        j: usize,
        after: &Line<'_>,
    ) -> Result<usize, Fail> {
        let blk = indented_block(lines, j, None, false, true);
        let body = trim_blank_end(&blk.lines);
        if !body.is_empty() {
            let text = self.code_text(body)?;
            self.leaf(NodeKind::CodeBlock, self.lines_span(body), text)?;
            let next = j + blk.consumed;
            if !blk.blank_finish && next < lines.len() {
                self.unindent_warning(&lines[next], "literal block");
            }
            return Ok(blk.consumed);
        }
        let k = j + blk.consumed;
        if k < lines.len() && lines[k].text.bytes().next().is_some_and(is_punct7) {
            return Ok(blk.consumed + self.quoted_literal(lines, k)?);
        }
        let span = self.line_span(after);
        self.report_at(LITERAL_MISSING, span);
        Ok(blk.consumed)
    }

    fn quoted_literal(&mut self, lines: &[Line<'_>], k: usize) -> Result<usize, Fail> {
        let q = lines[k].text.as_bytes()[0];
        let mut e = k;
        while e < lines.len() && !lines[e].is_blank() {
            if lines[e].text.as_bytes()[0] != q {
                let span = self.line_span(&lines[e]);
                self.report_at(LITERAL_QUOTING, span);
                break;
            }
            e += 1;
        }
        let text = self.code_text(&lines[k..e])?;
        self.leaf(NodeKind::CodeBlock, self.lines_span(&lines[k..e]), text)?;
        Ok(e - k)
    }

    /// Reads a definition list: items made of a term line and an indented definition.
    pub(crate) fn definition_list(&mut self, lines: &[Line<'_>], i: usize) -> Result<usize, Fail> {
        let list = self.open(NodeKind::DefinitionList, self.line_span(&lines[i]))?;
        let mut j = i;
        let mut end;
        loop {
            let blk = indented_block(lines, j + 1, None, false, true);
            let span = self.lines_span(&lines[j..j + 1 + blk.consumed]);
            let item = self.ctx.b.begin_node(NodeKind::DefinitionItem, span)?;
            self.term_parts(lines[j])?;
            let def = self.ctx.b.begin_node(NodeKind::Definition, self.lines_span(&blk.lines))?;
            self.parse_body(&blk.lines, false, None)?;
            self.close(def)?;
            self.close(item)?;
            end = span.end;
            j += 1 + blk.consumed;
            if j + 1 < lines.len()
                && !lines[j].is_blank()
                && lines[j + 1].is_indented()
                && detect(lines, j) == Construct::Text
            {
                continue;
            }
            if !blk.blank_finish && j < lines.len() {
                self.unindent_warning(&lines[j], "definition list");
            }
            break;
        }
        self.ctx.b.set_span_end(list, end);
        self.close(list)?;
        Ok((j - i).max(1))
    }

    /// Emits the term and its classifiers, split at " : " outside inline literals and
    /// interpreted text.
    fn term_parts(&mut self, l: Line<'_>) -> Result<(), Fail> {
        let b = l.text.as_bytes();
        let mut kind = NodeKind::Term;
        let (mut start, mut i) = (0, 0);
        let (mut literal, mut quoted) = (false, false);
        while i < b.len() {
            match b[i] {
                b'\\' => i += 2,
                b'`' if b.get(i + 1) == Some(&b'`') => {
                    literal = !literal;
                    i += 2;
                }
                b'`' => {
                    quoted ^= !literal;
                    i += 1;
                }
                b' ' if !literal && !quoted => {
                    let a = i;
                    while i < b.len() && b[i] == b' ' {
                        i += 1;
                    }
                    if i + 1 < b.len() && b[i] == b':' && b[i + 1] == b' ' {
                        self.inline_node(kind, &[l.part(start, a)])?;
                        kind = NodeKind::Classifier;
                        i += 1;
                        while i < b.len() && b[i] == b' ' {
                            i += 1;
                        }
                        start = i;
                    }
                }
                _ => i += 1,
            }
        }
        let rest = l.part(start, b.len());
        if kind == NodeKind::Term || !rest.text.is_empty() {
            self.inline_node(kind, &[rest])?;
        }
        Ok(())
    }
}
