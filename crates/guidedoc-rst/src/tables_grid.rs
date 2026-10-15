//! Grid tables.

use guidedoc::{Fail, NodeFlags, NodeKind};

use crate::lines::{Line, text_end};
use crate::messages::INDENT_UNEXPECTED;
use crate::parser::Parser;
use crate::tables::{ColMap, TableExtra, is_grid_border, is_grid_head_border};

/// One cell: its corner rows and columns.
#[derive(Debug, Clone, Copy)]
struct GridCell {
    top: usize,
    left: usize,
    bottom: usize,
    right: usize,
}

/// A grid table being read.
struct Grid<'l, 's> {
    m: ColMap<'l, 's>,
    /// Lines, borders included.
    rows: usize,
    /// Columns of every line.
    width: usize,
    /// The "=" separator line.
    head: Option<usize>,
    /// Per column: the last row the cells found so far cover; -1 for none.
    done: Vec<isize>,
    corner: Vec<bool>,
    cells: Vec<GridCell>,
    max_cells: usize,
}

/// Why a grid table is malformed.
type Why = &'static str;

impl<'l, 's> Grid<'l, 's> {
    fn new(block: &'l [Line<'s>]) -> Result<Self, Why> {
        let m = ColMap::new(block);
        let rows = block.len();
        if rows < 2 {
            return Err("it has no rows");
        }
        let width = m.width(0);
        let mut head = None;
        let mut corners = 0;
        for (r, l) in block.iter().enumerate() {
            if m.width(r) != width {
                return Err("the lines differ in width");
            }
            if !l.text.ends_with(['+', '|']) {
                return Err("a line does not end at the right border");
            }
            if is_grid_head_border(l.text) {
                if head.is_some() {
                    return Err("it has more than one header separator");
                }
                head = Some(r);
            }
            corners += l.text.bytes().filter(|&c| c == b'+').count();
        }
        if head == Some(0) || head == Some(rows - 1) {
            return Err("the header separator is the first or last line");
        }
        let corner = vec![false; rows * width];
        Ok(Grid {
            m,
            rows,
            width,
            head,
            done: vec![-1; width],
            corner,
            cells: Vec::new(),
            max_cells: corners + 1,
        })
    }

    /// The border character at a line and column; the header separator reads as an
    /// ordinary row border.
    fn at(&self, r: usize, c: usize) -> u8 {
        let ch = self.m.char_at(r, c);
        if Some(r) == self.head && ch == b'=' { b'-' } else { ch }
    }
}

impl Parser<'_, '_> {
    /// Reads a grid table starting at line `i` and returns the lines it used.
    pub(crate) fn grid_table(
        &mut self,
        lines: &[Line<'_>],
        i: usize,
        extra: Option<&mut TableExtra>,
    ) -> Result<usize, Fail> {
        let (end, bad_indent) = text_end(lines, i, true);
        let mut block = &lines[i..end];
        let n = block.iter().take_while(|l| l.text.starts_with(['+', '|'])).count();
        block = &block[..n];
        if bad_indent {
            let span = self.line_span(&lines[end]);
            self.report_at(INDENT_UNEXPECTED, span);
        }
        if !is_grid_border(block[block.len() - 1].text) {
            let k =
                (2..block.len().saturating_sub(1)).rev().find(|&k| is_grid_border(block[k].text));
            let Some(k) = k else {
                self.malformed(block, "the bottom border is missing");
                return Ok(block.len());
            };
            block = &block[..k + 1];
        }
        let mut g = match Grid::new(block) {
            Ok(g) => g,
            Err(why) => {
                self.malformed(block, why);
                return Ok(block.len());
            }
        };
        if let Err(why) = self.grid_parse(&mut g)? {
            self.malformed(block, why);
            return Ok(block.len());
        }
        self.grid_emit(&g, extra)?;
        Ok(block.len())
    }

    /// Finds every cell. Corners are visited in row-major order, which is the order in
    /// which a cell's top-left corner becomes known.
    fn grid_parse(&mut self, g: &mut Grid<'_, '_>) -> Result<Result<(), Why>, Fail> {
        let (bottom, right) = (g.rows - 1, g.width - 1);
        g.corner[0] = true;
        for r in 0..bottom {
            self.ctx.b.spend(g.width as u64)?;
            for c in 0..right {
                if !g.corner[r * g.width + c] || r as isize <= g.done[c] {
                    continue;
                }
                let Some((cb, cr)) = self.scan_cell(g, r, c)? else {
                    continue;
                };
                for col in c..cr {
                    if g.done[col] != r as isize - 1 {
                        return Ok(Err("cells overlap"));
                    }
                    g.done[col] = cb as isize - 1;
                }
                if g.cells.len() >= g.max_cells {
                    return Ok(Err("it has too many cells"));
                }
                g.cells.push(GridCell { top: r, left: c, bottom: cb, right: cr });
                g.corner[r * g.width + cr] = true;
                g.corner[cb * g.width + c] = true;
            }
        }
        if g.done[..right].iter().any(|&d| d != bottom as isize - 1) {
            return Ok(Err("the cells do not cover it"));
        }
        Ok(Ok(()))
    }

    fn scan_cell(
        &mut self,
        g: &Grid<'_, '_>,
        top: usize,
        left: usize,
    ) -> Result<Option<(usize, usize)>, Fail> {
        if g.at(top, left) != b'+' {
            return Ok(None);
        }
        for c in left + 1..g.width {
            match g.at(top, c) {
                b'+' => {
                    if let Some(b) = self.scan_down(g, top, left, c)? {
                        return Ok(Some((b, c)));
                    }
                }
                b'-' => {}
                _ => return Ok(None),
            }
        }
        Ok(None)
    }

    fn scan_down(
        &mut self,
        g: &Grid<'_, '_>,
        top: usize,
        left: usize,
        right: usize,
    ) -> Result<Option<usize>, Fail> {
        self.ctx.b.spend((g.rows - top) as u64)?;
        for r in top + 1..g.rows {
            match g.at(r, right) {
                b'+' => {
                    if self.scan_left(g, top, left, r, right)? {
                        return Ok(Some(r));
                    }
                }
                b'|' => {}
                _ => return Ok(None),
            }
        }
        Ok(None)
    }

    fn scan_left(
        &mut self,
        g: &Grid<'_, '_>,
        top: usize,
        left: usize,
        bottom: usize,
        right: usize,
    ) -> Result<bool, Fail> {
        self.ctx.b.spend((right - left + bottom - top) as u64)?;
        let row_ok = (left + 1..right).all(|c| matches!(g.at(bottom, c), b'+' | b'-'));
        let col_ok = (top + 1..bottom).all(|r| matches!(g.at(r, left), b'+' | b'|'));
        Ok(row_ok && g.at(bottom, left) == b'+' && col_ok)
    }

    fn grid_emit(&mut self, g: &Grid<'_, '_>, extra: Option<&mut TableExtra>) -> Result<(), Fail> {
        // Row and column boundaries are the cell edges; their ranks give the spans.
        let mut row_marks = vec![false; g.rows];
        let mut col_marks = vec![false; g.width];
        for c in &g.cells {
            row_marks[c.top] = true;
            col_marks[c.left] = true;
        }
        row_marks[g.rows - 1] = true;
        col_marks[g.width - 1] = true;
        let (col_rank, ncols) = rank(&col_marks);
        let (row_rank, _) = rank(&row_marks);
        let mut widths = Vec::with_capacity(ncols.saturating_sub(1));
        let mut prev = 0;
        for c in 1..g.width {
            if col_rank[c] != col_rank[c - 1] {
                widths.push(c - prev - 1);
                prev = c;
            }
        }
        widths.truncate(ncols.saturating_sub(1));
        let id =
            self.begin_table(self.lines_span(g.m.lines), ncols.saturating_sub(1), &widths, extra)?;
        self.grid_rows(g, &row_rank, &col_rank)?;
        self.close(id)
    }

    fn grid_rows(
        &mut self,
        g: &Grid<'_, '_>,
        row_rank: &[isize],
        col_rank: &[isize],
    ) -> Result<(), Fail> {
        let table_span = self.lines_span(g.m.lines);
        let mut in_head = g.head.is_some();
        let kind = if in_head { NodeKind::TableHead } else { NodeKind::TableBody };
        let mut group = self.ctx.b.begin_node(kind, table_span)?;
        let mut k = 0;
        for r in 0..g.rows - 1 {
            if r > 0 && row_rank[r] == row_rank[r - 1] {
                continue;
            }
            let header = g.head.is_some_and(|h| r < h);
            if in_head && !header {
                self.close(group)?;
                group = self.ctx.b.begin_node(NodeKind::TableBody, table_span)?;
                in_head = false;
            }
            let row = self.ctx.b.begin_node(NodeKind::Row, self.line_span(&g.m.lines[r]))?;
            if header {
                self.ctx.b.add_flags(row, NodeFlags::HEADER);
            }
            while k < g.cells.len() && g.cells[k].top == r {
                self.grid_cell(g, g.cells[k], row_rank, col_rank)?;
                k += 1;
            }
            self.close(row)?;
        }
        self.close(group)
    }

    fn grid_cell(
        &mut self,
        g: &Grid<'_, '_>,
        c: GridCell,
        row_rank: &[isize],
        col_rank: &[isize],
    ) -> Result<(), Fail> {
        let mut body: Vec<Line<'_>> =
            (c.top + 1..c.bottom).map(|r| g.m.part(r, c.left + 1, c.right)).collect();
        let first = g.m.part(c.top, c.left, c.left + 1);
        let last = g.m.part(c.bottom, c.right, c.right + 1);
        let span = self.span_of(&first, &last);
        let rows = (row_rank[c.bottom] - row_rank[c.top]) as usize;
        let cols = (col_rank[c.right] - col_rank[c.left]) as usize;
        self.cell(span, rows, cols, &mut body, false)
    }
}

/// Turns boundary marks into ranks: the boundaries before each index, plus one for a
/// boundary at it, less one. Returns the ranks and the number of boundaries.
fn rank(marks: &[bool]) -> (Vec<isize>, usize) {
    let mut n: isize = 0;
    let ranks = marks
        .iter()
        .map(|&m| {
            if m {
                n += 1;
                n - 1
            } else {
                n - 1
            }
        })
        .collect();
    (ranks, n as usize)
}
