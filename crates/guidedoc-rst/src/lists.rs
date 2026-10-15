//! Bullet and enumerated lists.

use guidedoc::{Fail, ListInfo, ListStyle, NodeFlags, NodeId, NodeKind, SourceSpan};

use crate::lines::{Block, Line, indent_of, indented_block, known_indented_block};
use crate::parser::Parser;

/// Returns the byte length of a bullet that starts `t`, or 0.
pub fn bullet_width(t: &str) -> usize {
    let w = if t.starts_with(['*', '-', '+']) {
        1
    } else if t.starts_with(['•', '‣', '⁃']) {
        3
    } else {
        return 0;
    };
    if t.len() > w && t.as_bytes()[w] != b' ' {
        return 0;
    }
    w
}

/// Reads a list item whose marker ends at column `col` of line `i`. With text on the
/// marker line, that text's column is the item's indentation; otherwise the least
/// indentation of the following lines is.
pub(crate) fn item_body<'s>(lines: &[Line<'s>], i: usize, col: usize) -> Block<'s> {
    if col < lines[i].text.len() {
        return known_indented_block(lines, i, col);
    }
    indented_block(lines, i, Some(col), false, true)
}

/// An enumeration sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Seq {
    None,
    Arabic,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
    UpperRoman,
    Auto,
}

impl Seq {
    fn style(self) -> ListStyle {
        match self {
            Seq::None | Seq::Arabic | Seq::Auto => ListStyle::Arabic,
            Seq::LowerAlpha => ListStyle::LowerAlpha,
            Seq::UpperAlpha => ListStyle::UpperAlpha,
            Seq::LowerRoman => ListStyle::LowerRoman,
            Seq::UpperRoman => ListStyle::UpperRoman,
        }
    }
}

/// An enumerator: its format ('.', ')', or '(' for "(1)"), sequence, ordinal (`None`
/// when the text is no valid numeral), and the column where the item text starts.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Enumerator {
    pub format: u8,
    pub seq: Seq,
    pub ordinal: Option<i64>,
    pub col: usize,
}

/// Recognizes an enumerator: "1.", "a)", "(iv)", or "#.". `expected` resolves the
/// ambiguity between single letters and Roman numerals inside a list.
pub(crate) fn parse_enum(t: &str, expected: Seq) -> Option<Enumerator> {
    let b = t.as_bytes();
    let mut format = 0u8;
    let i = usize::from(b.first() == Some(&b'('));
    if i == 1 {
        format = b'(';
    }
    let j = i + b[i..].iter().take_while(|&&c| c.is_ascii_alphanumeric() || c == b'#').count();
    let text = &t[i..j];
    if text.is_empty() || j >= b.len() {
        return None;
    }
    if format == b'(' {
        if b[j] != b')' {
            return None;
        }
    } else if b[j] == b'.' || b[j] == b')' {
        format = b[j];
    } else {
        return None;
    }
    let mut k = j + 1;
    if k < b.len() && b[k] != b' ' {
        return None;
    }
    k += indent_of(&t[k..]);
    let seq = sequence_of(text, expected);
    if seq == Seq::None {
        return None;
    }
    Some(Enumerator { format, seq, ordinal: ordinal_of(text, seq), col: k })
}

fn is_roman_char(c: u8) -> bool {
    matches!(c, b'I' | b'V' | b'X' | b'L' | b'C' | b'D' | b'M')
}

fn sequence_of(text: &str, expected: Seq) -> Seq {
    if text == "#" {
        return Seq::Auto;
    }
    let b = text.as_bytes();
    let digits = b.iter().all(u8::is_ascii_digit);
    let lower = b.iter().all(u8::is_ascii_lowercase);
    let upper = b.iter().all(u8::is_ascii_uppercase);
    let lroman = lower && b.iter().all(|&c| is_roman_char(c - 32));
    let uroman = b.iter().all(|&c| is_roman_char(c));
    if digits {
        return Seq::Arabic;
    }
    let single = b.len() == 1;
    match expected {
        Seq::LowerAlpha if single && lower => return Seq::LowerAlpha,
        Seq::UpperAlpha if single && upper => return Seq::UpperAlpha,
        Seq::LowerRoman if lroman => return Seq::LowerRoman,
        Seq::UpperRoman if uroman => return Seq::UpperRoman,
        _ => {}
    }
    match () {
        () if text == "i" => Seq::LowerRoman,
        () if text == "I" => Seq::UpperRoman,
        () if single && lower => Seq::LowerAlpha,
        () if single && upper => Seq::UpperAlpha,
        () if lroman => Seq::LowerRoman,
        () if uroman => Seq::UpperRoman,
        () => Seq::None,
    }
}

fn ordinal_of(text: &str, seq: Seq) -> Option<i64> {
    let first = *text.as_bytes().first()?;
    match seq {
        Seq::Auto => Some(1),
        Seq::Arabic => {
            let mut n: i64 = 0;
            for c in text.bytes() {
                n = n * 10 + i64::from(c - b'0');
                if n > 1_000_000_000 {
                    return None;
                }
            }
            Some(n)
        }
        Seq::LowerAlpha => Some(i64::from(first - b'a') + 1),
        Seq::UpperAlpha => Some(i64::from(first - b'A') + 1),
        Seq::LowerRoman | Seq::UpperRoman => roman_value(text),
        Seq::None => None,
    }
}

const ROMAN: [(i64, &str); 13] = [
    (1000, "M"),
    (900, "CM"),
    (500, "D"),
    (400, "CD"),
    (100, "C"),
    (90, "XC"),
    (50, "L"),
    (40, "XL"),
    (10, "X"),
    (9, "IX"),
    (5, "V"),
    (4, "IV"),
    (1, "I"),
];

/// Appends `n` (1..4999) as an uppercase Roman numeral.
fn write_roman(out: &mut String, n: i64) {
    let mut v = n;
    for (value, digits) in ROMAN {
        while v >= value {
            out.push_str(digits);
            v -= value;
        }
    }
}

fn roman_digit(c: u8) -> i64 {
    match c.to_ascii_uppercase() {
        b'I' => 1,
        b'V' => 5,
        b'X' => 10,
        b'L' => 50,
        b'C' => 100,
        b'D' => 500,
        b'M' => 1000,
        _ => 0,
    }
}

/// Accepts only canonical numerals from 1 to 4999, in either case.
fn roman_value(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    if b.is_empty() || b.len() > 16 {
        return None;
    }
    let (mut total, mut i) = (0, 0);
    while i < b.len() {
        let v = roman_digit(b[i]);
        if v == 0 {
            return None;
        }
        let next = b.get(i + 1).map_or(0, |&c| roman_digit(c));
        if next > v {
            total += next - v;
            i += 2;
        } else {
            total += v;
            i += 1;
        }
    }
    if !(1..=4999).contains(&total) {
        return None;
    }
    let mut canon = String::new();
    write_roman(&mut canon, total);
    canon.eq_ignore_ascii_case(text).then_some(total)
}

/// Appends the enumerator for ordinal `n`; `false` when `n` cannot be written.
fn format_enum(out: &mut String, n: i64, seq: Seq, format: u8) -> bool {
    if format == b'(' {
        out.push('(');
    }
    match seq {
        Seq::Auto => out.push('#'),
        Seq::Arabic => out.push_str(&n.max(0).to_string()),
        Seq::LowerAlpha | Seq::UpperAlpha => {
            if !(1..=26).contains(&n) {
                return false;
            }
            let base = if seq == Seq::LowerAlpha { b'a' } else { b'A' };
            out.push((base + (n - 1) as u8) as char);
        }
        Seq::LowerRoman | Seq::UpperRoman => {
            if !(1..=4999).contains(&n) {
                return false;
            }
            let mut r = String::new();
            write_roman(&mut r, n);
            if seq == Seq::LowerRoman {
                r.make_ascii_lowercase();
            }
            out.push_str(&r);
        }
        Seq::None => return false,
    }
    out.push(if format == b'(' { ')' } else { format as char });
    true
}

/// The second-line rule: the next line is blank, indented, or starts with the next
/// enumerator, so ordinary text such as "A. Smith" stays text.
pub(crate) fn enum_item_valid(lines: &[Line<'_>], i: usize, e: &Enumerator) -> bool {
    let Some(ordinal) = e.ordinal else {
        return false;
    };
    let Some(next) = lines.get(i + 1).map(|l| l.text) else {
        return true;
    };
    if next.is_empty() || next.starts_with(' ') {
        return true;
    }
    let mut s = String::new();
    if format_enum(&mut s, ordinal + 1, e.seq, e.format) && next.starts_with(&s) {
        return true;
    }
    s.clear();
    format_enum(&mut s, 1, Seq::Auto, e.format);
    next.starts_with(&s)
}

impl Parser<'_, '_> {
    /// Emits one ListItem; returns its block, span, and whether it is simple enough for
    /// a tight list: at most one paragraph and nothing else.
    fn list_item<'s>(
        &mut self,
        lines: &[Line<'s>],
        i: usize,
        col: usize,
    ) -> Result<(Block<'s>, bool, SourceSpan), Fail> {
        let blk = item_body(lines, i, col);
        let span = self.lines_span(&lines[i..i + blk.consumed.max(1)]);
        let item = self.ctx.b.begin_node(NodeKind::ListItem, span)?;
        self.parse_body(&blk.lines, false, None)?;
        self.close(item)?;
        let simple = self.simple_item(item);
        Ok((blk, simple, span))
    }

    fn simple_item(&self, id: NodeId) -> bool {
        let nodes = &self.ctx.b.snap.nodes;
        let end = nodes[id].subtree_end;
        let child = id.index() + 1;
        if child as u32 >= end {
            return true;
        }
        nodes[child].kind == NodeKind::Paragraph && nodes[child].subtree_end == end
    }

    /// Reads a bullet list starting at line `i`.
    pub(crate) fn bullet_list(&mut self, lines: &[Line<'_>], i: usize) -> Result<usize, Fail> {
        let t = lines[i].text;
        let w = bullet_width(t);
        let bullet = &t[..w];
        let list = self.open(NodeKind::List, self.line_span(&lines[i]))?;
        let delimiter = if w == 1 { bullet.as_bytes()[0] } else { 0 };
        if w > 1 {
            self.ctx.b.add_custom_attribute(list, "bullet", bullet)?;
        }
        let info = ListInfo { style: ListStyle::Bullet, delimiter, start: 0 };
        self.ctx.b.set_list(list, info)?;
        let mut tight = true;
        let mut j = i;
        let mut end;
        loop {
            let lt = lines[j].text;
            let col = w + indent_of(&lt[w.min(lt.len())..]);
            let (blk, simple, span) = self.list_item(lines, j, col)?;
            tight &= simple;
            end = span.end;
            j += blk.consumed.max(1);
            if j < lines.len()
                && bullet_width(lines[j].text) == w
                && lines[j].text.starts_with(bullet)
            {
                continue;
            }
            if !blk.blank_finish && j < lines.len() {
                self.unindent_warning(&lines[j], "bullet list");
            }
            break;
        }
        self.finish_list(list, tight, end)?;
        Ok(j - i)
    }

    fn finish_list(&mut self, list: NodeId, tight: bool, end: u32) -> Result<(), Fail> {
        if tight {
            self.ctx.b.add_flags(list, NodeFlags::TIGHT);
        }
        self.ctx.b.set_span_end(list, end);
        self.close(list)
    }

    /// Reads an enumerated list starting at line `i`.
    pub(crate) fn enumerated_list(&mut self, lines: &[Line<'_>], i: usize) -> Result<usize, Fail> {
        let Some(first) = parse_enum(lines[i].text, Seq::None) else {
            return self.paragraph(lines, i);
        };
        let kind = if first.seq == Seq::Auto { Seq::Arabic } else { first.seq };
        let list = self.open(NodeKind::List, self.line_span(&lines[i]))?;
        let start = first.ordinal.unwrap_or(0) as i32;
        let info = ListInfo { style: first.seq.style(), delimiter: first.format, start };
        self.ctx.b.set_list(list, info)?;
        let mut auto = first.seq == Seq::Auto;
        let mut last = first.ordinal.unwrap_or(0);
        let mut e = first;
        let (mut tight, mut j, mut end) = (true, i, 0);
        loop {
            let (blk, simple, span) = self.list_item(lines, j, e.col)?;
            tight &= simple;
            end = end.max(span.end);
            j += blk.consumed.max(1);
            let next = lines.get(j).and_then(|l| parse_enum(l.text, kind));
            if let Some(next) = next.filter(|n| n.format == first.format)
                && enum_item_valid(lines, j, &next)
            {
                if next.seq == Seq::Auto {
                    (auto, e) = (true, next);
                    continue;
                }
                if next.seq == kind && !auto && next.ordinal == Some(last + 1) {
                    (last, e) = (last + 1, next);
                    continue;
                }
            }
            if !blk.blank_finish && j < lines.len() {
                self.unindent_warning(&lines[j], "enumerated list");
            }
            break;
        }
        self.finish_list(list, tight, end)?;
        Ok(j - i)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn enumerators() {
        let e = parse_enum("(iv) text", Seq::None).unwrap();
        assert_eq!((e.format, e.seq, e.ordinal, e.col), (b'(', Seq::LowerRoman, Some(4), 5));
        assert_eq!(parse_enum("i. x", Seq::LowerAlpha).unwrap().seq, Seq::LowerAlpha);
        assert_eq!(parse_enum("iiii. x", Seq::None).unwrap().ordinal, None);
        assert!(parse_enum("1.x", Seq::None).is_none());
        assert_eq!(bullet_width("• x"), 3);
        assert_eq!(bullet_width("-x"), 0);
    }
}
