//! Simple tables.

use guidedoc::{Fail, NodeFlags, NodeKind};

use crate::lines::Line;
use crate::parser::Parser;
use crate::tables::{ColMap, TableExtra};
use crate::width::column_width;

/// A column: its start and end columns.
#[derive(Debug, Clone, Copy)]
struct Col {
    start: usize,
    end: usize,
}

/// A row: its lines in the table block, and the span underline or border that ends it.
#[derive(Debug, Clone, Copy)]
struct Row {
    first: usize,
    last: usize,
    rule: Option<usize>,
}

/// A simple table being read.
struct Simple<'l, 's> {
    m: ColMap<'l, 's>,
    /// Columns of the top border; the last one grows with overflowing text.
    main: Vec<Col>,
    border_end: usize,
    head: Option<usize>,
    rows: Vec<Row>,
}

type Why = &'static str;

fn is_rule(t: &str, fill: u8) -> bool {
    let b = t.as_bytes();
    b.first() == Some(&fill) && b.iter().all(|&c| c == fill || c == b' ')
}

/// The column runs of "-" or "=" in a border or span underline.
fn runs(t: &str) -> Vec<Col> {
    let b = t.as_bytes();
    let mut cols = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b' ' {
            i += 1;
            continue;
        }
        let a = i;
        while i < b.len() && b[i] != b' ' {
            i += 1;
        }
        cols.push(Col { start: a, end: i });
    }
    cols
}

impl<'l, 's> Simple<'l, 's> {
    fn new(block: &'l [Line<'s>]) -> Result<Self, Why> {
        let m = ColMap::new(block);
        let main = runs(block[0].text);
        let border_end = main.last().map_or(0, |c| c.end);
        let head = (1..block.len() - 1).rev().find(|&r| is_rule(block[r].text, b'='));
        let mut s = Simple { m, main, border_end, head, rows: Vec::new() };
        let first = s.main[0];
        let (mut start, mut text_found) = (1, false);
        for off in 1..block.len() {
            let t = block[off].text;
            if off == block.len() - 1 || Some(off) == s.head || is_rule(t, b'-') {
                s.add_row(start, off, Some(off));
                (start, text_found) = (off + 1, false);
            } else if s.m.has_text(off, first.start, first.end) {
                if text_found && off != start {
                    s.add_row(start, off, None);
                }
                (start, text_found) = (off, true);
            } else if !text_found {
                start = off + 1;
            }
        }
        if s.rows.is_empty() {
            return Err("it has no rows");
        }
        Ok(s)
    }

    fn add_row(&mut self, first: usize, last: usize, rule: Option<usize>) {
        if last > first {
            self.rows.push(Row { first, last, rule });
        }
    }

    /// The columns of a row: the main columns, or those of its span underline, whose
    /// last column reaches as far as the main last column.
    fn row_columns(&self, row: Row) -> Result<Vec<Col>, Why> {
        let ends_table = |r: usize| r == self.m.lines.len() - 1 || Some(r) == self.head;
        let Some(rule) = row.rule.filter(|&r| !ends_table(r)) else {
            return Ok(self.main.clone());
        };
        let mut cols = runs(self.m.lines[rule].text);
        match cols.last_mut() {
            Some(last) if last.end == self.border_end => {
                last.end = self.main[self.main.len() - 1].end;
                Ok(cols)
            }
            _ => Err("a column span underline is incomplete"),
        }
    }

    /// Validates every row before anything is emitted: no text in column margins, text
    /// overflowing the last column widens it, spans align with columns.
    fn check(&mut self) -> Result<(), Why> {
        for row in self.rows.clone() {
            let mut cols = self.row_columns(row)?;
            let last = cols.len() - 1;
            for ci in 0..cols.len() {
                let c = cols[ci];
                let next = if ci < last { cols[ci + 1].start } else { usize::MAX };
                for r in row.first..row.last {
                    let w = self.m.width(r);
                    if ci == last && w > c.end && self.m.has_text(r, c.end, w) {
                        let text_end = c.start + column_width(self.m.part(r, c.start, w).text);
                        let main = self.main.len() - 1;
                        self.main[main].end = self.main[main].end.max(text_end);
                        cols[ci].end = self.main[main].end;
                    } else if ci < last && self.m.has_text(r, c.end, next) {
                        return Err("text is in a column margin");
                    }
                }
            }
            if self.spans_of(&cols).is_none() {
                return Err("a column span is not aligned");
            }
        }
        Ok(())
    }

    /// The number of extra main columns each row column covers; `None` when the row's
    /// columns do not align with the main columns.
    fn spans_of(&self, cols: &[Col]) -> Option<Vec<usize>> {
        let mut k = 0;
        let mut spans = Vec::with_capacity(cols.len());
        for c in cols {
            if k >= self.main.len() || c.start != self.main[k].start {
                return None;
            }
            let mut more = 0;
            while c.end != self.main[k].end {
                k += 1;
                more += 1;
                if k >= self.main.len() {
                    return None;
                }
            }
            spans.push(more);
            k += 1;
        }
        Some(spans)
    }
}

impl Parser<'_, '_> {
    /// Reads a simple table starting at line `i` and returns the lines it used.
    pub(crate) fn simple_table(
        &mut self,
        lines: &[Line<'_>],
        i: usize,
        extra: Option<&mut TableExtra>,
    ) -> Result<usize, Fail> {
        let toplen = lines[i].text.len();
        let (mut end, mut found, mut found_at) = (None, 0, 0);
        for k in i + 1..lines.len() {
            let t = lines[k].text;
            if !is_rule(t, b'=') {
                continue;
            }
            if t.len() != toplen {
                self.malformed(&lines[i..=k], "a border does not match the top border");
                return Ok(k - i + 1);
            }
            found += 1;
            found_at = k;
            if found == 2 || k == lines.len() - 1 || lines[k + 1].is_blank() {
                end = Some(k);
                break;
            }
        }
        let Some(end) = end else {
            if found > 0 {
                self.malformed(&lines[i..=found_at], "no blank line follows the bottom border");
                return Ok(found_at - i + 1);
            }
            self.malformed(&lines[i..], "the bottom border is missing");
            return Ok(lines.len() - i);
        };
        let block = &lines[i..=end];
        let checked = Simple::new(block).and_then(|mut s| s.check().map(|()| s));
        match checked {
            Ok(s) => self.simple_emit(&s, extra)?,
            Err(why) => self.malformed(block, why),
        }
        Ok(block.len())
    }

    fn simple_emit(
        &mut self,
        s: &Simple<'_, '_>,
        extra: Option<&mut TableExtra>,
    ) -> Result<(), Fail> {
        let widths: Vec<usize> = s.main.iter().map(|c| c.end - c.start).collect();
        let span = self.lines_span(s.m.lines);
        let id = self.begin_table(span, s.main.len(), &widths, extra)?;
        let mut in_head = s.head.is_some_and(|h| s.rows[0].first < h);
        let kind = if in_head { NodeKind::TableHead } else { NodeKind::TableBody };
        let mut group = self.ctx.b.begin_node(kind, span)?;
        for &row in &s.rows {
            let header = s.head.is_some_and(|h| row.first < h);
            if in_head && !header {
                self.close(group)?;
                group = self.ctx.b.begin_node(NodeKind::TableBody, span)?;
                in_head = false;
            }
            self.simple_row(s, row, header)?;
        }
        self.close(group)?;
        self.close(id)
    }

    fn simple_row(&mut self, s: &Simple<'_, '_>, row: Row, header: bool) -> Result<(), Fail> {
        let row_lines = &s.m.lines[row.first..row.last];
        let r = self.ctx.b.begin_node(NodeKind::Row, self.lines_span(row_lines))?;
        if header {
            self.ctx.b.add_flags(r, NodeFlags::HEADER);
        }
        let Ok(bounds) = s.row_columns(row) else {
            return self.close(r);
        };
        let spans = s.spans_of(&bounds).unwrap_or_default();
        for (ci, c) in bounds.iter().enumerate() {
            let end = if ci + 1 < bounds.len() { c.end } else { usize::MAX / 2 };
            let mut body: Vec<Line<'_>> =
                (row.first..row.last).map(|k| s.m.part(k, c.start, end)).collect();
            let cell_span = self.lines_span(&body);
            let more = spans.get(ci).copied().unwrap_or(0);
            self.cell(cell_span, 1, more + 1, &mut body, false)?;
        }
        self.close(r)
    }
}
