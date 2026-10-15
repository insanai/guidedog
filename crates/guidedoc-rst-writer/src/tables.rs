//! Tables: grid tables for tables with spans or those a grid table was read from, and
//! list tables for tables whose cells hold blocks.

use guidedoc::{AttributeKey, NodeId, NodeKind};

use crate::blocks::Opt;
use crate::escape::width;
use crate::state::Writer;

/// One cell placed in the table's grid.
#[derive(Debug, Clone, Copy)]
struct Placed {
    id: NodeId,
    row: usize,
    col: usize,
    rows: usize,
    cols: usize,
}

/// A table's rows and cells in grid positions.
#[derive(Debug, Default)]
struct Layout {
    cells: Vec<Placed>,
    rows: usize,
    cols: usize,
    head_rows: usize,
    title: Option<NodeId>,
}

/// The least content width a grid column gets.
const NARROWEST: usize = 3;

impl Writer<'_, '_> {
    /// Places the cells of a table, as a grid table reader would see them.
    fn layout(&self, id: NodeId) -> Layout {
        let mut l = Layout::default();
        let mut taken: Vec<Vec<bool>> = Vec::new();
        for part in self.doc.children(id) {
            let head = match self.kind(part) {
                NodeKind::Title => {
                    l.title = Some(part);
                    continue;
                }
                NodeKind::TableHead => true,
                _ => false,
            };
            for row in self.doc.children(part) {
                let r = l.rows;
                l.rows += 1;
                if head {
                    l.head_rows += 1;
                }
                let mut col = 0;
                for cell in self.doc.children(row) {
                    let info = self.doc.cell_info(cell);
                    while taken.get(r).is_some_and(|t| t.get(col).copied().unwrap_or(false)) {
                        col += 1;
                    }
                    let (rows, cols) =
                        (usize::from(info.row_span.max(1)), usize::from(info.col_span.max(1)));
                    for rr in r..r + rows {
                        if taken.len() <= rr {
                            taken.resize(rr + 1, Vec::new());
                        }
                        if taken[rr].len() < col + cols {
                            taken[rr].resize(col + cols, false);
                        }
                        taken[rr][col..col + cols].fill(true);
                    }
                    l.cells.push(Placed { id: cell, row: r, col, rows, cols });
                    col += cols;
                    l.cols = l.cols.max(col);
                }
            }
        }
        l.rows = l.rows.max(taken.len());
        l
    }

    /// Writes a table as a grid table or a list table.
    pub fn table(&mut self, id: NodeId) {
        let layout = self.layout(id);
        let spans = layout.cells.iter().any(|c| c.rows > 1 || c.cols > 1);
        let widths: Vec<usize> = self
            .attr(id, AttributeKey::Widths)
            .map(|w| w.split_whitespace().filter_map(|n| n.parse().ok()).collect())
            .unwrap_or_default();
        let given = self.doc.has_class(id, "colwidths-given");
        let from_list = !widths.is_empty() && widths.iter().all(|&w| w == 1);
        let from_grid = widths.len() == layout.cols && !from_list && !given;
        let simple = layout.cells.iter().all(|c| self.is_simple_cell(c.id));
        if self.stub_columns(&layout) > 0 && !spans {
            return self.list_table(id, &layout, &widths, given);
        }
        if layout.cols == 0 || layout.rows == layout.head_rows {
            self.lossy(id, "a table without body rows", "a list table");
            return self.list_table(id, &layout, &widths, given);
        }
        let mut fits = || {
            // A table whose words cannot fit the line width as a grid is a list table.
            let columns = self.natural_widths(&layout);
            columns.iter().sum::<usize>() + layout.cols < self.available()
        };
        if spans || from_grid || (simple && !from_list && !given && fits()) {
            self.grid_table(id, &layout, if from_grid { &widths } else { &[] }, given);
        } else {
            self.list_table(id, &layout, &widths, given);
        }
    }

    /// A simple table with exactly `widths`, when every cell is one line of text that
    /// fits its column (the last column may run on) and every row has a first cell.
    fn simple_table_lines(&mut self, layout: &Layout, widths: &[usize]) -> Option<Vec<String>> {
        if layout.cells.iter().any(|c| c.rows > 1 || !self.is_simple_cell(c.id)) {
            return None;
        }
        // Each row: its cells' (column, columns spanned, text), in order.
        let mut rows: Vec<Vec<(usize, usize, String)>> = vec![Vec::new(); layout.rows];
        for c in &layout.cells {
            let text = match self.doc.first_child(c.id) {
                Some(p) => self.inline_line(p),
                None => String::new(),
            };
            let last = c.col + c.cols == layout.cols;
            if (!last && width(&text) > span_width_simple(widths, c.col, c.cols))
                || text.contains('\n')
            {
                return None;
            }
            rows[c.row].push((c.col, c.cols, text));
        }
        if rows.iter().any(|r| r.first().is_none_or(|(col, _, t)| *col != 0 || t.is_empty())) {
            return None;
        }
        let border: Vec<String> = widths.iter().map(|&w| "=".repeat(w)).collect();
        let border = border.join("  ");
        let mut lines = vec![border.clone()];
        for (r, row) in rows.iter().enumerate() {
            if r == layout.head_rows && r > 0 {
                lines.push(border.clone());
            }
            simple_row(&mut lines, row, widths);
            if row.iter().any(|(_, cols, _)| *cols > 1) {
                let spans: Vec<String> = row
                    .iter()
                    .map(|&(col, cols, _)| "-".repeat(span_width_simple(widths, col, cols)))
                    .collect();
                lines.push(spans.join("  "));
            }
        }
        lines.push(border);
        Some(lines)
    }

    /// The leading columns whose body cells are all header cells.
    fn stub_columns(&self, layout: &Layout) -> usize {
        let body: Vec<&Placed> =
            layout.cells.iter().filter(|c| c.row >= layout.head_rows).collect();
        if body.is_empty() {
            return 0;
        }
        (0..layout.cols)
            .take_while(|&col| {
                body.iter()
                    .filter(|c| c.col == col)
                    .all(|c| self.doc.node(c.id).flags.has(guidedoc::NodeFlag::Header))
            })
            .count()
    }

    fn is_simple_cell(&self, cell: NodeId) -> bool {
        let mut children = self.doc.children(cell);
        match children.next() {
            None => true,
            Some(p) => self.kind(p) == NodeKind::Paragraph && children.next().is_none(),
        }
    }

    /// The table directive's options.
    fn table_options(&self, id: NodeId, widths: &[usize], given: bool) -> Vec<Opt> {
        let mut options: Vec<Opt> = Vec::new();
        if given && !widths.is_empty() {
            let w: Vec<String> = widths.iter().map(usize::to_string).collect();
            options.push(("widths", w.join(" ")));
        } else if self.doc.has_class(id, "colwidths-auto") {
            options.push(("widths", "auto".to_owned()));
        }
        if let Some(w) = self.attr(id, AttributeKey::Width) {
            options.push(("width", w.to_owned()));
        }
        if let Some(a) = self.attr(id, AttributeKey::Align) {
            options.push(("align", a.to_owned()));
        }
        self.common_options(id, &["colwidths-given", "colwidths-auto"], &mut options);
        options
    }

    /// `.. list-table::` with a nested bullet list per row.
    fn list_table(&mut self, id: NodeId, layout: &Layout, widths: &[usize], given: bool) {
        let title = layout.title.map(|t| self.inline_line(t)).unwrap_or_default();
        let mut options = Vec::new();
        if layout.head_rows > 0 {
            options.push(("header-rows", layout.head_rows.to_string()));
        }
        let stubs = self.stub_columns(layout);
        if stubs > 0 {
            options.push(("stub-columns", stubs.to_string()));
        }
        options.extend(self.table_options(id, widths, given));
        if !given && !widths.is_empty() && widths.iter().any(|&w| w != 1) {
            self.lossy(id, "a grid table's column widths", "a list table without them");
        }
        self.directive_head("list-table", &title, &options);
        let complex = layout.cells.iter().any(|c| !self.is_simple_cell(c.id));
        self.push_frame(3, "");
        self.block_start();
        for r in 0..layout.rows {
            if r > 0 {
                self.blank = complex;
                self.block_start();
            }
            self.push_frame(2, "* ");
            for (n, cell) in layout.cells.iter().filter(|c| c.row == r).enumerate() {
                if n > 0 {
                    self.blank = complex;
                    self.block_start();
                }
                self.push_frame(2, "- ");
                self.blocks(cell.id);
                self.pop_frame();
            }
            self.pop_frame();
        }
        self.pop_frame();
        self.block_end();
    }

    /// A grid table, in `.. table::` when it has a title or options.
    fn grid_table(&mut self, id: NodeId, layout: &Layout, widths: &[usize], given: bool) {
        let options = self.table_options(id, widths, given);
        let wrapped = layout.title.is_some() || !options.is_empty();
        if wrapped {
            let title = layout.title.map(|t| self.inline_line(t)).unwrap_or_default();
            self.directive_head("table", &title, &options);
            self.push_frame(3, "");
        } else {
            self.class_prefix(id, &["colwidths-given", "colwidths-auto"]);
        }
        let (columns, contents) = self.fit_columns(layout, widths);
        let grew = !widths.is_empty() && columns != widths;
        let simple = if grew { self.simple_table_lines(layout, widths) } else { None };
        let lines = simple.unwrap_or_else(|| draw_grid(layout, &columns, &contents));
        self.block_start();
        for line in &lines {
            self.line(line);
        }
        self.block_end();
        if wrapped {
            self.pop_frame();
            self.block_end();
        }
    }

    /// Chooses the column widths (inside the borders, padding included) and renders
    /// each cell at its width. Widths read from a grid are kept unless a cell's
    /// content does not fit them.
    fn fit_columns(
        &mut self,
        layout: &Layout,
        widths: &[usize],
    ) -> (Vec<usize>, Vec<Vec<String>>) {
        let mut columns: Vec<usize> = if widths.len() == layout.cols {
            widths.iter().map(|&w| w.max(3)).collect()
        } else {
            self.natural_widths(layout)
        };
        let mut contents = Vec::new();
        for _ in 0..4 {
            contents = layout
                .cells
                .iter()
                .map(|c| {
                    let w = span_width(&columns, c.col, c.cols).saturating_sub(2);
                    self.cell_lines(c.id, w)
                })
                .collect();
            let mut grew = false;
            for (c, lines) in layout.cells.iter().zip(&contents) {
                let need = lines.iter().map(|l| width(l)).max().unwrap_or(0) + 2;
                let have = span_width(&columns, c.col, c.cols);
                if need > have {
                    columns[c.col + c.cols - 1] += need - have;
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }
        (columns, contents)
    }

    /// Column widths from the cells' content, shrunk to fit the line width.
    fn natural_widths(&mut self, layout: &Layout) -> Vec<usize> {
        let mut columns = vec![NARROWEST + 2; layout.cols];
        let mut least = vec![NARROWEST + 2; layout.cols];
        for c in layout.cells.iter().filter(|c| c.cols == 1) {
            let lines = self.cell_lines(c.id, usize::MAX / 4);
            let need = lines.iter().map(|l| width(l)).max().unwrap_or(0) + 2;
            columns[c.col] = columns[c.col].max(need);
            let word = lines.iter().flat_map(|l| l.split(' ')).map(width).max().unwrap_or(0);
            least[c.col] = least[c.col].max((word + 2).min(need));
        }
        // Too wide: narrow the widest column that can still give, one column at a time,
        // so short columns keep their width and words stay whole where they can.
        let room = self.available().saturating_sub(layout.cols + 1);
        while columns.iter().sum::<usize>() > room {
            let widest =
                (0..layout.cols).filter(|&i| columns[i] > least[i]).max_by_key(|&i| columns[i]);
            match widest {
                Some(i) => columns[i] -= 1,
                None => break,
            }
        }
        columns
    }

    /// Renders a cell's blocks at `width` columns.
    fn cell_lines(&mut self, cell: NodeId, width: usize) -> Vec<String> {
        let text = self.capture(width.max(NARROWEST), |w| w.blocks(cell));
        let lines: Vec<String> =
            text.trim_end_matches('\n').split('\n').map(str::to_owned).collect();
        let lead = lines.iter().take_while(|l| l.is_empty()).count();
        lines[lead..].to_vec()
    }
}

/// The width of columns `col..col + cols` of a simple table, with the gaps between.
fn span_width_simple(widths: &[usize], col: usize, cols: usize) -> usize {
    widths[col..col + cols].iter().sum::<usize>() + 2 * (cols - 1)
}

/// Writes one row of a simple table; the last cell wraps onto continuation rows,
/// whose first cell is blank.
fn simple_row(lines: &mut Vec<String>, row: &[(usize, usize, String)], widths: &[usize]) {
    let mut line = String::new();
    let Some(((last_col, last_cols, last_text), rest)) = row.split_last() else { return };
    for (col, cols, text) in rest {
        line.push_str(text);
        let pad = span_width_simple(widths, *col, *cols) - width(text) + 2;
        line.extend(std::iter::repeat_n(' ', pad));
    }
    let indent = line.len();
    let room = span_width_simple(widths, *last_col, *last_cols);
    for (n, part) in wrap_text(last_text, room).iter().enumerate() {
        if n > 0 {
            line = " ".repeat(indent);
        }
        line.push_str(part);
        lines.push(line.trim_end().to_owned());
    }
}

/// Wraps escaped inline text at `limit` columns, breaking at spaces that are not
/// escaped.
fn wrap_text(text: &str, limit: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    let bytes = text.as_bytes();
    let mut start = 0;
    let mut words = Vec::new();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b' ' && i > 0 && bytes[i - 1] != b'\\' && bytes[i - 1] != b' ' {
            words.push(&text[start..i]);
            start = i + 1;
        }
    }
    words.push(&text[start..]);
    for word in words {
        let current = lines.last_mut().expect("one line");
        if !current.is_empty() && width(current) + 1 + width(word) > limit {
            lines.push(word.to_owned());
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
    }
    lines
}

fn span_width(columns: &[usize], col: usize, cols: usize) -> usize {
    columns[col..col + cols].iter().sum::<usize>() + cols - 1
}

/// Draws the grid: a character canvas holding every cell's box and content. A wide
/// character takes two canvas columns, the second left as a NUL that is dropped.
fn draw_grid(layout: &Layout, columns: &[usize], contents: &[Vec<String>]) -> Vec<String> {
    let mut heights = vec![1usize; layout.rows];
    for (c, lines) in layout.cells.iter().zip(contents).filter(|(c, _)| c.rows == 1) {
        heights[c.row] = heights[c.row].max(lines.len());
    }
    for (c, lines) in layout.cells.iter().zip(contents).filter(|(c, _)| c.rows > 1) {
        let have: usize = heights[c.row..c.row + c.rows].iter().sum::<usize>() + c.rows - 1;
        if lines.len() > have {
            heights[c.row + c.rows - 1] += lines.len() - have;
        }
    }
    let mut ys = vec![0usize];
    for h in &heights {
        ys.push(ys.last().copied().unwrap_or(0) + h + 1);
    }
    let mut xs = vec![0usize];
    for w in columns {
        xs.push(xs.last().copied().unwrap_or(0) + w + 1);
    }
    let (height, wide) = (ys[layout.rows] + 1, xs[layout.cols] + 1);
    let mut canvas = vec![vec![' '; wide]; height];
    for (c, lines) in layout.cells.iter().zip(contents) {
        let (y0, y1) = (ys[c.row], ys[c.row + c.rows]);
        let (x0, x1) = (xs[c.col], xs[c.col + c.cols]);
        let rule =
            |y: usize| if y == ys[layout.head_rows] && layout.head_rows > 0 { '=' } else { '-' };
        for y in [y0, y1] {
            let ch = rule(y);
            for (x, cell) in canvas[y].iter_mut().enumerate().take(x1 + 1).skip(x0) {
                if x == x0 || x == x1 {
                    *cell = '+';
                } else if *cell != '+' {
                    *cell = ch;
                }
            }
        }
        for row in canvas.iter_mut().take(y1).skip(y0 + 1) {
            for x in [x0, x1] {
                if row[x] != '+' {
                    row[x] = '|';
                }
            }
        }
        for (i, line) in lines.iter().enumerate() {
            let mut x = x0 + 2;
            for ch in line.chars() {
                canvas[y0 + 1 + i][x] = ch;
                x += 1;
                if width(ch.encode_utf8(&mut [0; 4])) == 2 {
                    canvas[y0 + 1 + i][x] = '\0';
                    x += 1;
                }
            }
        }
    }
    canvas
        .into_iter()
        .map(|row| {
            row.into_iter().filter(|&c| c != '\0').collect::<String>().trim_end().to_owned()
        })
        .collect()
}
