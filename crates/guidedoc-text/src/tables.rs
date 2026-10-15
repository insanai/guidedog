//! Tables, drawn as grid tables, as Sphinx's text builder draws them:
//!
//! ```text
//! +-------+-----+
//! | Head  | B   |
//! +=======+=====+
//! | a     | b   |
//! +-------+-----+
//! ```
//!
//! Every cell is laid out: without wrapping, to find the width its column wants; at
//! width one, to find the width it needs; and finally at the column widths chosen to
//! fit the page. Spanning cells widen the last column or row they cover.

use guidedoc::{NodeFlag, NodeId, NodeKind};

use crate::state::{Fill, State, char_count};

/// A cell placed in the grid.
#[derive(Debug, Default)]
struct TableCell {
    id: NodeId,
    row: usize,
    column: usize,
    rows: usize,
    columns: usize,
    /// The width of its widest line, unwrapped.
    natural: usize,
    /// The width of its longest word.
    least: usize,
    /// Its text at its final width.
    text: String,
}

#[derive(Debug, Default)]
pub(crate) struct Table {
    cells: Vec<TableCell>,
    /// rows * columns: the index of the cell covering each place.
    grid: Vec<Option<usize>>,
    widths: Vec<usize>,
    least: Vec<usize>,
    heights: Vec<usize>,
    rows: usize,
    columns: usize,
    /// Header rows.
    head: usize,
}

impl Table {
    fn at(&self, row: usize, column: usize) -> Option<usize> {
        self.grid[row * self.columns + column]
    }

    /// The width of a cell's columns and the borders between them.
    fn span_width(&self, c: &TableCell) -> usize {
        3 * (c.columns - 1) + self.widths[c.column..c.column + c.columns].iter().sum::<usize>()
    }
}

impl State<'_, '_> {
    pub fn render_table(&mut self, id: NodeId) {
        let doc = self.doc;
        if let Some(title) = doc.first_child(id).filter(|&t| doc.kind(t) == NodeKind::Title) {
            let mut lead = self.buffer();
            lead.push_str(self.custom(id, "number-label"));
            if !lead.is_empty() {
                lead.push(' ');
            }
            self.paragraph(title, &lead, Fill::Inlines);
            self.recycle(lead);
        }
        let mut t = self.tables.pop().unwrap_or_default();
        self.table_shape(id, &mut t);
        if t.rows == 0 {
            self.tables.push(t);
            return;
        }
        self.place_cells(id, &mut t);
        for i in 0..t.cells.len() {
            let cell = t.cells[i].id;
            t.cells[i].natural = self.widest_line(cell, 1 << 20);
            t.cells[i].least = self.widest_line(cell, 1);
        }
        self.choose_widths(&mut t);
        for i in 0..t.cells.len() {
            let width = t.span_width(&t.cells[i]);
            let text = self.cell_layout(t.cells[i].id, width, false);
            t.cells[i].text = text;
        }
        choose_heights(&mut t);
        self.block_start();
        for r in 0..t.rows {
            self.draw_border(&t, r);
            for l in 0..t.heights[r] {
                self.draw_line(&t, r, l);
            }
        }
        self.draw_border(&t, t.rows);
        self.block_end();
        for cell in t.cells.drain(..) {
            self.recycle(cell.text);
        }
        self.tables.push(t);
    }

    /// Counts rows and columns and finds the header rows.
    fn table_shape(&self, id: NodeId, t: &mut Table) {
        let doc = self.doc;
        t.cells.clear();
        (t.rows, t.columns, t.head) = (0, 0, 0);
        let mut first_width = 0;
        for part in doc.children(id) {
            let kind = doc.kind(part);
            if kind != NodeKind::TableHead && kind != NodeKind::TableBody {
                continue;
            }
            for row in doc.children(part) {
                let width: usize =
                    doc.children(row).map(|c| usize::from(doc.cell_info(c).col_span)).sum();
                if t.rows == 0 {
                    first_width = width;
                }
                let header =
                    kind == NodeKind::TableHead || doc.node(row).flags.has(NodeFlag::Header);
                if header && t.head == t.rows {
                    t.head += 1;
                }
                t.rows += 1;
            }
        }
        t.columns = usize::try_from(doc.node(id).value).unwrap_or(0).max(first_width).max(1);
        if t.head == t.rows {
            t.head = 0;
        }
        refill(&mut t.grid, None, t.rows * t.columns);
        refill(&mut t.widths, 1, t.columns);
        refill(&mut t.least, 1, t.columns);
        refill(&mut t.heights, 0, t.rows);
    }

    /// Puts each cell in the first free column of its row, as grid tables do.
    fn place_cells(&self, id: NodeId, t: &mut Table) {
        let doc = self.doc;
        let rows = doc
            .children(id)
            .filter(|&p| matches!(doc.kind(p), NodeKind::TableHead | NodeKind::TableBody))
            .flat_map(|p| doc.children(p));
        for (r, row) in rows.enumerate() {
            let mut c = 0;
            for cell in doc.children(row) {
                while c < t.columns && t.at(r, c).is_some() {
                    c += 1;
                }
                if c >= t.columns {
                    break;
                }
                let info = doc.cell_info(cell);
                let rows = usize::from(info.row_span).clamp(1, t.rows - r);
                let columns = usize::from(info.col_span).clamp(1, t.columns - c);
                let index = t.cells.len();
                for dr in 0..rows {
                    for dc in 0..columns {
                        let place = &mut t.grid[(r + dr) * t.columns + c + dc];
                        place.get_or_insert(index);
                    }
                }
                t.cells.push(TableCell {
                    id: cell,
                    row: r,
                    column: c,
                    rows,
                    columns,
                    ..TableCell::default()
                });
                c += columns;
            }
        }
    }

    /// The width of a cell's widest line when laid out at `width`.
    fn widest_line(&mut self, id: NodeId, width: usize) -> usize {
        let text = self.cell_layout(id, width, true);
        let widest = text.lines().map(char_count).max().unwrap_or(0);
        self.recycle(text);
        widest
    }

    /// Gives each column the width its cells want, then narrows the columns with the
    /// most room to spare until the table fits the line, but never below the width
    /// their longest words need.
    fn choose_widths(&self, t: &mut Table) {
        for c in t.cells.iter().filter(|c| c.columns == 1) {
            t.widths[c.column] = t.widths[c.column].max(c.natural);
            t.least[c.column] = t.least[c.column].max(c.least);
        }
        for c in t.cells.iter().filter(|c| c.columns != 1) {
            let last = c.column + c.columns - 1;
            let gap = c.natural.saturating_sub(t.span_width(c));
            t.widths[last] += gap;
            let room = 3 * (c.columns - 1) + t.least[c.column..=last].iter().sum::<usize>();
            t.least[last] += c.least.saturating_sub(room);
        }
        for (w, least) in t.widths.iter_mut().zip(&t.least) {
            *w = (*w).max(*least);
        }
        let mut total = 3 * t.columns + 1 + t.widths.iter().sum::<usize>();
        let fit = self.width.saturating_sub(self.indentation()).max(self.narrow);
        while total > fit {
            let spare = |i: usize| t.widths[i] - t.least[i];
            let widest = (0..t.columns)
                .filter(|&i| spare(i) > 0)
                .reduce(|best, i| if spare(i) > spare(best) { i } else { best });
            let Some(widest) = widest else {
                break;
            };
            t.widths[widest] -= 1;
            total -= 1;
        }
    }

    /// Writes the border above row `r`: "=" under the header rows, and blanks where a
    /// cell continues across it.
    fn draw_border(&mut self, t: &Table, r: usize) {
        self.line_begin();
        self.put_char('+');
        let c = if t.head > 0 && r == t.head && r < t.rows { '=' } else { '-' };
        for column in 0..t.columns {
            let spanned = r > 0
                && r < t.rows
                && t.at(r, column).is_some()
                && t.at(r, column) == t.at(r - 1, column);
            self.put_repeat(if spanned { ' ' } else { c }, t.widths[column] + 2);
            self.put_char('+');
        }
        self.line_end();
    }

    /// Writes line `l` of row `r`: each cell's line, padded to the cell's width.
    fn draw_line(&mut self, t: &Table, r: usize, l: usize) {
        self.line_begin();
        self.put_char('|');
        let mut column = 0;
        while column < t.columns {
            let Some(index) = t.at(r, column) else {
                self.put_repeat(' ', t.widths[column] + 2);
                self.put_char('|');
                column += 1;
                continue;
            };
            let c = &t.cells[index];
            let offset = l + t.heights[c.row..r].iter().sum::<usize>();
            let text = c.text.split('\n').nth(offset).unwrap_or("");
            self.put_char(' ');
            self.put(text);
            self.put_repeat(' ', (t.span_width(c) + 1).saturating_sub(char_count(text)));
            self.put_char('|');
            column = (column + 1).max(c.column + c.columns);
        }
        self.line_end();
    }
}

/// Makes `v` hold `n` copies of `value`, reusing its storage.
fn refill<T: Clone>(v: &mut Vec<T>, value: T, n: usize) {
    v.clear();
    v.resize(n, value);
}

/// Makes each row as tall as its tallest cell; a cell spanning rows that needs more
/// lines than they have adds them to its last row.
fn choose_heights(t: &mut Table) {
    let lines = |c: &TableCell| c.text.matches('\n').count();
    for c in t.cells.iter().filter(|c| c.rows == 1) {
        t.heights[c.row] = t.heights[c.row].max(lines(c));
    }
    for c in t.cells.iter().filter(|c| c.rows != 1) {
        let have: usize = t.heights[c.row..c.row + c.rows].iter().sum();
        t.heights[c.row + c.rows - 1] += lines(c).saturating_sub(have);
    }
    for h in &mut t.heights {
        *h = (*h).max(1);
    }
}
