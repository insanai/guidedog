//! Explicit markup blocks (".. "): footnotes, citations, hyperlink targets, comments;
//! substitution definitions and directives live in their own modules.

use std::borrow::Cow;

use guidedoc::{Fail, INVALID_NODE, LinkInfo, LinkKind, NodeFlags, NodeKind, SourceSpan, Text};

use crate::lines::{Line, indent_of, indented_block, join_lines, trim_blank_end};
use crate::messages::TARGET_MALFORMED;
use crate::names::{all_digits, is_email, is_reference, is_simplename, parse_int, write_uri};
use crate::parser::Parser;

/// Finds the name of a target in its block: up to the first colon that is followed by
/// a space or the end of a line, or a backquoted phrase before the colon. Returns the
/// name, the line it ends on, and the column after its colon.
fn target_name<'s>(ls: &[Line<'s>]) -> Option<(Cow<'s, str>, usize, usize)> {
    let t = ls[0].text;
    let b = t.as_bytes();
    if b.is_empty() || b[0] == b' ' || b[0] == b'_' {
        return None;
    }
    if b[0] != b'`' {
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'\\' {
                i += 2;
                continue;
            }
            if b[i] == b':' && i > 0 && (i + 1 >= b.len() || b[i + 1] == b' ') {
                let end = if b[i - 1] == b' ' { i - 1 } else { i };
                return (end > 0).then(|| (Cow::Borrowed(&t[..end]), 0, i + 1));
            }
            i += 1;
        }
        return None;
    }
    if b.len() < 2 || b[1] == b' ' || b[1] == b'`' {
        return None;
    }
    for (k, l) in ls.iter().enumerate() {
        let s = l.text.as_bytes();
        let mut j = usize::from(k == 0);
        while j < s.len() {
            if s[j] == b'\\' {
                j += 2;
                continue;
            }
            if s[j] == b'`' {
                let mut c = j + 1;
                if c < s.len() && s[c] == b' ' {
                    c += 1;
                }
                if c < s.len() && s[c] == b':' && (c + 1 >= s.len() || s[c + 1] == b' ') {
                    return Some((quoted_name(ls, k, j), k, c + 1));
                }
            }
            j += 1;
        }
    }
    None
}

/// The backquoted name that ends at column `j` of line `k`, lines joined by spaces.
fn quoted_name<'s>(ls: &[Line<'s>], k: usize, j: usize) -> Cow<'s, str> {
    if k == 0 {
        return Cow::Borrowed(&ls[0].text[1..j]);
    }
    let mut parts = Vec::with_capacity(k + 1);
    parts.push(ls[0].cut(1));
    parts.extend(ls[1..k].iter().map(|l| l.trim()));
    parts.push(ls[k].trim().head(j.saturating_sub(indent_of(ls[k].text))));
    Cow::Owned(join_lines(&parts, " ").into_owned())
}

impl Parser<'_, '_> {
    /// Reads an explicit markup block at line `i`.
    pub(crate) fn explicit(
        &mut self,
        lines: &[Line<'_>],
        i: usize,
        sections: bool,
    ) -> Result<usize, Fail> {
        let t = lines[i].text;
        let col = 2 + indent_of(&t[2..]);
        let rest = &t[col..];
        let b = rest.as_bytes();
        if let Some(&first) = b.first() {
            match first {
                b'[' => {
                    if let Some(n) = self.footnote(lines, i, col)? {
                        return Ok(n);
                    }
                }
                b'_' if b.len() > 1 && b[1] != b' ' => {
                    return self.hyperlink_target(lines, i, col);
                }
                b'|' if b.len() > 1 && b[1] != b' ' => {
                    return self.substitution_def(lines, i, col);
                }
                _ => {}
            }
            if let Some(n) = self.directive(lines, i, col, sections) {
                return n;
            }
        }
        self.comment(lines, i, col)
    }

    fn comment(&mut self, lines: &[Line<'_>], i: usize, col: usize) -> Result<usize, Fail> {
        if col >= lines[i].text.len() && lines.get(i + 1).is_none_or(|l| l.is_blank()) {
            self.ctx.b.add_leaf(NodeKind::Comment, self.line_span(&lines[i]), Text::default())?;
            return Ok(1);
        }
        let blk = indented_block(lines, i, Some(col), false, true, self.line_buf());
        let body = trim_blank_end(&blk.lines);
        let span = self.lines_span(&lines[i..i + blk.consumed.max(1)]);
        let text = self.pool_lines(body, "\n")?;
        self.ctx.b.add_leaf(NodeKind::Comment, span, text)?;
        self.give_lines(blk.lines);
        Ok(blk.consumed.max(1))
    }

    /// Reads ".. [label] body". Labels are numbers, "#", "#name", or "*" for footnotes,
    /// and simple reference names for citations; `None` for another label.
    fn footnote(
        &mut self,
        lines: &[Line<'_>],
        i: usize,
        col: usize,
    ) -> Result<Option<usize>, Fail> {
        let t = &lines[i].text[col..];
        let Some(close_at) = t.bytes().skip(1).position(|c| c == b']').map(|k| k + 1) else {
            return Ok(None);
        };
        if t.as_bytes().get(close_at + 1).is_some_and(|&c| c != b' ') {
            return Ok(None);
        }
        let label = &t[1..close_at];
        let (mut kind, mut flags, mut value) = (NodeKind::Footnote, NodeFlags::default(), 0);
        if all_digits(label) {
            value = parse_int(label);
        } else if label == "#" || label.strip_prefix('#').is_some_and(is_simplename) {
            flags = NodeFlags::AUTO;
        } else if label == "*" {
            flags = NodeFlags::SYMBOL;
        } else if is_simplename(label) {
            kind = NodeKind::Citation;
        } else {
            return Ok(None);
        }
        let body_col = col + close_at + 1 + indent_of(&t[close_at + 1..]);
        let blk = indented_block(lines, i, Some(body_col), false, true, self.line_buf());
        let span = self.lines_span(&lines[i..i + blk.consumed.max(1)]);
        let id = self.open(kind, span)?;
        let text = self.ctx.b.add_text(label)?;
        self.ctx.b.set_text(id, text);
        self.ctx.b.add_flags(id, flags);
        if value > 0 {
            self.ctx.b.set_value(id, i32::try_from(value).unwrap_or(i32::MAX));
        }
        self.parse_body(&blk.lines, false, None)?;
        self.close(id)?;
        self.give_lines(blk.lines);
        Ok(Some(blk.consumed.max(1)))
    }

    /// Reads ".. _name: link" and ".. __: link". A backquoted name may continue on the
    /// following lines.
    fn hyperlink_target(
        &mut self,
        lines: &[Line<'_>],
        i: usize,
        col: usize,
    ) -> Result<usize, Fail> {
        let blk = indented_block(lines, i, Some(col + 1), true, false, self.line_buf());
        let consumed = blk.consumed.max(1);
        let span = self.lines_span(&lines[i..i + consumed]);
        let mut ls = blk.lines;
        let result = self.target_lines(&mut ls, span);
        self.give_lines(ls);
        result.map(|()| consumed)
    }

    /// Emits the target whose block (from its name on) is `ls`.
    fn target_lines(&mut self, ls: &mut [Line<'_>], span: SourceSpan) -> Result<(), Fail> {
        let t = ls[0].text;
        if t.starts_with("_:") && (t.len() == 2 || t.as_bytes()[2] == b' ') {
            ls[0] = ls[0].cut(2);
            return self.emit_target("", true, ls, span);
        }
        let Some((name, k, after)) = target_name(ls) else {
            self.report_at(TARGET_MALFORMED, span);
            return Ok(());
        };
        ls[k] = ls[k].cut(after);
        self.emit_target(&name, false, &ls[k..], span)
    }

    /// Reads an anonymous target ("__ link") at line `i`.
    pub(crate) fn anonymous_target(
        &mut self,
        lines: &[Line<'_>],
        i: usize,
    ) -> Result<usize, Fail> {
        let t = lines[i].text;
        let col = 2 + indent_of(&t[2..]);
        let blk = indented_block(lines, i, Some(col), true, true, self.line_buf());
        let span = self.lines_span(&lines[i..i + blk.consumed.max(1)]);
        self.emit_target("", true, &blk.lines, span)?;
        self.give_lines(blk.lines);
        Ok(blk.consumed.max(1))
    }

    /// Emits a Target. An empty link block makes an internal target, a reference
    /// ("name_") an indirect one, and anything else an external URI.
    fn emit_target(
        &mut self,
        name: &str,
        anonymous: bool,
        link: &[Line<'_>],
        span: SourceSpan,
    ) -> Result<(), Fail> {
        let id = self.ctx.b.begin_node(NodeKind::Target, span)?;
        if !name.is_empty() {
            let text = self.pool_name(name)?;
            self.ctx.b.set_text(id, text);
        }
        if anonymous {
            self.ctx.b.add_flags(id, NodeFlags::ANONYMOUS);
        }
        let mut info =
            LinkInfo { resolved: INVALID_NODE, kind: LinkKind::Unresolved, ..LinkInfo::default() };
        let parts: Vec<Line<'_>> =
            link.iter().map(|l| l.trim()).filter(|l| !l.is_blank()).collect();
        if let Some(last) = parts.last() {
            let joined = join_lines(&parts, " ");
            let refname = if last.text.ends_with('_') { is_reference(&joined) } else { None };
            if let Some(refname) = refname {
                info.name = self.pool_name(refname)?;
            } else {
                let mut uri = String::new();
                write_uri(&mut uri, &parts);
                info.destination = self.ctx.b.add_text(&uri)?;
                info.kind = if is_email(&uri) { LinkKind::Email } else { LinkKind::External };
            }
        }
        self.ctx.b.set_link(id, info)?;
        self.close(id)
    }
}
