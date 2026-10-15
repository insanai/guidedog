//! Field lists and option lists.

use guidedoc::{Fail, NodeKind, SourceSpan};

use crate::lines::{Line, indented_block};
use crate::parser::Parser;

/// Recognizes ":name: " at the start of `t`: the end of the name and the column of the
/// body. The name may contain escaped characters and colons that are not followed by a
/// space, a backquote, or the end.
pub(crate) fn field_marker(t: &str) -> Option<(usize, usize)> {
    let b = t.as_bytes();
    if b.len() < 3 || b[0] != b':' || b[1] == b':' || b[1] == b' ' {
        return None;
    }
    let mut i = 1;
    while i < b.len() {
        match b[i] {
            b'\\' => {
                if i + 1 >= b.len() {
                    return None;
                }
                i += 2;
                continue;
            }
            b':' if i + 1 >= b.len() || b[i + 1] == b' ' => {
                if b[i - 1] == b' ' {
                    return None;
                }
                let col = i + 1 + b[i + 1..].iter().take_while(|&&c| c == b' ').count();
                return Some((i, col));
            }
            b':' if b[i + 1] == b'`' => return None,
            _ => {}
        }
        i += 1;
    }
    None
}

/// One option of an option list marker: the option string ("-a", "--all"), its
/// argument (empty when none), and the delimiter before it (' ', '=', or 0).
#[derive(Debug, Clone, Copy, Default)]
struct OptionToken {
    start: usize,
    end: usize,
    arg_start: usize,
    arg_end: usize,
    delim: u8,
}

fn is_alnum_plus(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-'
}

fn option_name(b: &[u8], i: usize) -> usize {
    if i >= b.len() || !b[i].is_ascii_alphanumeric() {
        return i;
    }
    i + 1 + b[i + 1..].iter().take_while(|&&c| is_alnum_plus(c)).count()
}

fn option_arg(b: &[u8], i: usize) -> usize {
    if i >= b.len() {
        return i;
    }
    if b[i] == b'<' {
        let j = i + 1 + b[i + 1..].iter().take_while(|&&c| c != b'<' && c != b'>').count();
        return if j < b.len() && b[j] == b'>' && j > i + 1 { j + 1 } else { i };
    }
    if !b[i].is_ascii_alphabetic() {
        return i;
    }
    i + 1 + b[i + 1..].iter().take_while(|&&c| is_alnum_plus(c)).count()
}

fn scan_option(b: &[u8], i: usize, with_arg: bool) -> Option<OptionToken> {
    let n = b.len();
    if i + 1 >= n {
        return None;
    }
    let (e, long) = match (b[i], b[i + 1]) {
        (b'-', b'-') => (option_name(b, i + 2), true),
        (b'/', _) => (option_name(b, i + 1), true),
        (b'-' | b'+', c) if c.is_ascii_alphanumeric() => (i + 2, false),
        _ => return None,
    };
    if long && (e == i + 2 && b[i] == b'-' || e == i + 1) {
        return None;
    }
    let mut tok = OptionToken { start: i, end: e, arg_start: e, arg_end: e, delim: 0 };
    if !with_arg {
        return Some(tok);
    }
    let mut a = e;
    if long {
        if a >= n || (b[a] != b' ' && b[a] != b'=') {
            return None;
        }
        tok.delim = b[a];
        a += 1;
    } else if a < n && b[a] == b' ' {
        tok.delim = b' ';
        a += 1;
    }
    let z = option_arg(b, a);
    if z == a {
        return None;
    }
    (tok.arg_start, tok.arg_end) = (a, z);
    Some(tok)
}

/// Reads one option of a marker and what follows it: the token, where the next option
/// or the description starts, and whether another option follows.
fn next_option(t: &str, i: usize) -> Option<(OptionToken, usize, bool)> {
    let b = t.as_bytes();
    let n = b.len();
    for with_arg in [true, false] {
        let Some(tok) = scan_option(b, i, with_arg) else {
            continue;
        };
        let z = tok.arg_end;
        if z + 1 < n && b[z] == b',' && b[z + 1] == b' ' {
            return Some((tok, z + 2, true));
        }
        if z == n {
            return Some((tok, n, false));
        }
        if z + 1 < n && b[z] == b' ' && b[z + 1] == b' ' {
            let k = z + b[z..].iter().take_while(|&&c| c == b' ').count();
            return Some((tok, k, false));
        }
    }
    None
}

/// Validates the options that start `t` and returns the description column.
fn option_marker(t: &str) -> Option<usize> {
    let mut i = 0;
    loop {
        let (_, next, more) = next_option(t, i)?;
        if !more {
            return Some(next);
        }
        i = next;
    }
}

/// An option list item needs a description, on the marker line or indented below it.
pub(crate) fn option_item_valid(lines: &[Line<'_>], i: usize) -> bool {
    let t = lines[i].text;
    if !t.starts_with(['-', '+', '/']) {
        return false;
    }
    let Some(desc) = option_marker(t) else {
        return false;
    };
    if desc < t.len() {
        return true;
    }
    lines[i + 1..].iter().take_while(|l| l.is_blank() || l.is_indented()).any(|l| !l.is_blank())
}

impl Parser<'_, '_> {
    /// Reads a field list starting at line `i`.
    pub(crate) fn field_list(&mut self, lines: &[Line<'_>], i: usize) -> Result<usize, Fail> {
        let list = self.open(NodeKind::FieldList, self.line_span(&lines[i]))?;
        let mut j = i;
        let mut end;
        loop {
            let (name_end, col) = field_marker(lines[j].text).unwrap_or((1, 1));
            let blk = indented_block(lines, j, Some(col), false, true);
            let span = self.lines_span(&lines[j..j + blk.consumed.max(1)]);
            let field = self.ctx.b.begin_node(NodeKind::Field, span)?;
            self.inline_node(NodeKind::FieldName, &[lines[j].part(1, name_end)])?;
            let body_span = if blk.lines.is_empty() {
                SourceSpan::new(self.src(), span.end, span.end)
            } else {
                self.lines_span(&blk.lines)
            };
            let body = self.ctx.b.begin_node(NodeKind::FieldBody, body_span)?;
            self.parse_body(&blk.lines, false, None)?;
            self.close(body)?;
            self.close(field)?;
            end = span.end;
            j += blk.consumed.max(1);
            if j < lines.len() && field_marker(lines[j].text).is_some() {
                continue;
            }
            if !blk.blank_finish && j < lines.len() {
                self.unindent_warning(&lines[j], "field list");
            }
            break;
        }
        self.ctx.b.set_span_end(list, end);
        self.close(list)?;
        Ok(j - i)
    }

    /// Reads an option list starting at line `i`.
    pub(crate) fn option_list(&mut self, lines: &[Line<'_>], i: usize) -> Result<usize, Fail> {
        let list = self.open(NodeKind::OptionList, self.line_span(&lines[i]))?;
        let mut j = i;
        let mut end;
        loop {
            let l = lines[j];
            let desc = option_marker(l.text).unwrap_or(0);
            let blk = indented_block(lines, j, Some(desc), false, true);
            let span = self.lines_span(&lines[j..j + blk.consumed.max(1)]);
            let item = self.ctx.b.begin_node(NodeKind::OptionItem, span)?;
            self.option_group(l, desc)?;
            let d =
                self.ctx.b.begin_node(NodeKind::OptionDescription, self.lines_span(&blk.lines))?;
            self.parse_body(&blk.lines, false, None)?;
            self.close(d)?;
            self.close(item)?;
            end = span.end;
            j += blk.consumed.max(1);
            if j < lines.len() && option_item_valid(lines, j) {
                continue;
            }
            if !blk.blank_finish && j < lines.len() {
                self.unindent_warning(&lines[j], "option list");
            }
            break;
        }
        self.ctx.b.set_span_end(list, end);
        self.close(list)?;
        Ok(j - i)
    }

    fn option_group(&mut self, l: Line<'_>, desc: usize) -> Result<(), Fail> {
        let t = l.text;
        let group =
            self.ctx.b.begin_node(NodeKind::OptionGroup, self.line_span(&l.part(0, desc)))?;
        let mut i = 0;
        while let Some((tok, next, more)) = next_option(t, i) {
            let span = self.line_span(&l.part(tok.start, tok.arg_end));
            let o = self.ctx.b.begin_node(NodeKind::Option, span)?;
            let text = self.ctx.b.add_text(&t[tok.start..tok.end])?;
            self.ctx.b.set_text(o, text);
            if tok.arg_end > tok.arg_start {
                let span = self.line_span(&l.part(tok.arg_start, tok.arg_end));
                let a = self.ctx.b.begin_node(NodeKind::OptionArgument, span)?;
                let text = self.ctx.b.add_text(&t[tok.arg_start..tok.arg_end])?;
                self.ctx.b.set_text(a, text);
                self.ctx.b.set_value(a, i32::from(tok.delim));
                self.close(a)?;
            }
            self.close(o)?;
            if !more {
                break;
            }
            i = next;
        }
        self.close(group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_markers() {
        assert_eq!(field_marker(":name: body"), Some((5, 7)));
        assert_eq!(field_marker(":a\\: b: c"), Some((6, 8)));
        assert_eq!(field_marker(":a:`x`: c"), None);
        assert_eq!(field_marker(":: x"), None);
    }

    #[test]
    fn option_markers() {
        assert_eq!(option_marker("-a  desc"), Some(4));
        assert_eq!(option_marker("--all=FILE, -b X  desc"), Some(18));
        assert_eq!(option_marker("--all desc"), Some(10));
        assert_eq!(option_marker("/V  desc"), Some(4));
    }
}
