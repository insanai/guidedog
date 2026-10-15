//! Tables: grid tables for tables with spans or those a grid table was read from, and
//! list tables for tables whose cells hold blocks.

use guidedoc::{AttributeKey, NodeId, NodeKind};

use crate::blocks::Opt;
use crate::escape::width;
use crate::inline::Lines;
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

/// A table's working storage, kept for the next table; a table in a cell takes
/// another from the writer's pool.
#[derive(Debug, Default)]
pub(crate) struct TableWork {
    layout: Layout,
    /// Per row: which columns cells cover.
    taken: Vec<Vec<bool>>,
    /// The table's widths attribute.
    widths: Vec<usize>,
    grid: GridWork,
}

/// What drawing a grid table reuses.
#[derive(Debug, Default)]
pub(crate) struct GridWork {
    columns: Vec<usize>,
    least: Vec<usize>,
    /// Per cell: its lines at its width.
    contents: Vec<Lines>,
    heights: Vec<usize>,
    xs: Vec<usize>,
    ys: Vec<usize>,
    canvas: Vec<char>,
    text: String,
    lines: Lines,
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
    /// Places the cells of a table into `work.layout`, as a grid table reader would
    /// see them.
    fn layout_into(&self, id: NodeId, work: &mut TableWork) {
        let TableWork { layout: l, taken, .. } = work;
        l.cells.clear();
        (l.rows, l.cols, l.head_rows, l.title) = (0, 0, 0, None);
        // Rows past `used` hold stale marks from an earlier table.
        let mut used = 0;
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
                    while r < used && taken[r].get(col).copied().unwrap_or(false) {
                        col += 1;
                    }
                    let (rows, cols) =
                        (usize::from(info.row_span.max(1)), usize::from(info.col_span.max(1)));
                    for rr in r..r + rows {
                        mark_taken(taken, &mut used, rr, col, cols);
                    }
                    l.cells.push(Placed { id: cell, row: r, col, rows, cols });
                    col += cols;
                    l.cols = l.cols.max(col);
                }
            }
        }
        l.rows = l.rows.max(used);
    }

    /// Writes a table as a grid table or a list table.
    pub fn table(&mut self, id: NodeId) {
        let mut work = self.tables.pop().unwrap_or_default();
        self.layout_into(id, &mut work);
        let TableWork { layout, widths, grid, .. } = &mut work;
        widths.clear();
        if let Some(w) = self.attr(id, AttributeKey::Widths) {
            widths.extend(w.split_whitespace().filter_map(|n| n.parse::<usize>().ok()));
        }
        self.table_with(id, layout, widths, grid);
        self.tables.push(work);
    }

    /// Writes the table `layout` places, whose widths attribute holds `widths`.
    fn table_with(&mut self, id: NodeId, layout: &Layout, widths: &[usize], g: &mut GridWork) {
        let spans = layout.cells.iter().any(|c| c.rows > 1 || c.cols > 1);
        let given = self.doc.has_class(id, "colwidths-given");
        let from_list = !widths.is_empty() && widths.iter().all(|&w| w == 1);
        let from_grid = widths.len() == layout.cols && !from_list && !given;
        let simple = layout.cells.iter().all(|c| self.is_simple_cell(c.id));
        if self.stub_columns(layout) > 0 && !spans {
            return self.list_table(id, layout, widths, given);
        }
        if layout.cols == 0 || layout.rows == layout.head_rows {
            self.lossy(id, "a table without body rows", "a list table");
            return self.list_table(id, layout, widths, given);
        }
        if spans || from_grid || (simple && !from_list && !given && self.grid_fits(layout, g)) {
            self.grid_table(id, layout, if from_grid { widths } else { &[] }, given, g);
        } else {
            self.list_table(id, layout, widths, given);
        }
    }

    /// Tells whether a table's words fit the line width as a grid; otherwise it is
    /// written as a list table.
    fn grid_fits(&mut self, layout: &Layout, g: &mut GridWork) -> bool {
        self.natural_widths(layout, g);
        g.columns.iter().sum::<usize>() + layout.cols < self.available()
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
        let body = || layout.cells.iter().filter(|c| c.row >= layout.head_rows);
        if body().next().is_none() {
            return 0;
        }
        (0..layout.cols)
            .take_while(|&col| {
                body()
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
    fn grid_table(
        &mut self,
        id: NodeId,
        layout: &Layout,
        widths: &[usize],
        given: bool,
        g: &mut GridWork,
    ) {
        let options = self.table_options(id, widths, given);
        let wrapped = layout.title.is_some() || !options.is_empty();
        if wrapped {
            let title = layout.title.map(|t| self.inline_line(t)).unwrap_or_default();
            self.directive_head("table", &title, &options);
            self.recycle(title);
            self.push_frame(3, "");
        } else {
            self.class_prefix(id, &["colwidths-given", "colwidths-auto"]);
        }
        self.fit_columns(layout, widths, g);
        let grew = !widths.is_empty() && g.columns != widths;
        let simple = if grew { self.simple_table_lines(layout, widths) } else { None };
        self.block_start();
        match simple {
            Some(lines) => lines.iter().for_each(|line| self.line(line)),
            None => {
                g.draw(layout);
                g.lines.iter().for_each(|line| self.line(line));
            }
        }
        self.block_end();
        if wrapped {
            self.pop_frame();
            self.block_end();
        }
    }

    /// Chooses the column widths (inside the borders, padding included) into
    /// `g.columns` and renders each cell at its width into `g.contents`. Widths read
    /// from a grid are kept unless a cell's content does not fit them.
    fn fit_columns(&mut self, layout: &Layout, widths: &[usize], g: &mut GridWork) {
        if widths.len() == layout.cols {
            g.columns.clear();
            g.columns.extend(widths.iter().map(|&w| w.max(3)));
        } else {
            self.natural_widths(layout, g);
        }
        if g.contents.len() < layout.cells.len() {
            g.contents.resize_with(layout.cells.len(), Lines::default);
        }
        for _ in 0..4 {
            for (c, lines) in layout.cells.iter().zip(&mut g.contents) {
                let w = span_width(&g.columns, c.col, c.cols).saturating_sub(2);
                self.cell_lines(c.id, w, lines);
            }
            let mut grew = false;
            for (c, lines) in layout.cells.iter().zip(&g.contents) {
                let need = lines.iter().map(|l| width(l)).max().unwrap_or(0) + 2;
                let have = span_width(&g.columns, c.col, c.cols);
                if need > have {
                    g.columns[c.col + c.cols - 1] += need - have;
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }
    }

    /// Column widths from the cells' content, shrunk to fit the line width, into
    /// `g.columns`.
    fn natural_widths(&mut self, layout: &Layout, g: &mut GridWork) {
        refill(&mut g.columns, NARROWEST + 2, layout.cols);
        refill(&mut g.least, NARROWEST + 2, layout.cols);
        let mut lines = self.take_lines();
        for c in layout.cells.iter().filter(|c| c.cols == 1) {
            self.cell_lines(c.id, usize::MAX / 4, &mut lines);
            let need = lines.iter().map(|l| width(l)).max().unwrap_or(0) + 2;
            g.columns[c.col] = g.columns[c.col].max(need);
            let word = lines.iter().flat_map(|l| l.split(' ')).map(width).max().unwrap_or(0);
            g.least[c.col] = g.least[c.col].max((word + 2).min(need));
        }
        self.give_lines(lines);
        // Too wide: narrow the widest column that can still give, one column at a time,
        // so short columns keep their width and words stay whole where they can.
        let room = self.available().saturating_sub(layout.cols + 1);
        let (columns, least) = (&mut g.columns, &g.least);
        while columns.iter().sum::<usize>() > room {
            let widest =
                (0..layout.cols).filter(|&i| columns[i] > least[i]).max_by_key(|&i| columns[i]);
            match widest {
                Some(i) => columns[i] -= 1,
                None => break,
            }
        }
    }

    /// Renders a cell's blocks at `width` columns into `out`, without leading empty
    /// lines.
    fn cell_lines(&mut self, cell: NodeId, width: usize, out: &mut Lines) {
        let text = self.capture(width.max(NARROWEST), |w| w.blocks(cell));
        out.clear();
        let mut lead = true;
        for line in text.trim_end_matches('\n').split('\n') {
            if lead && line.is_empty() {
                continue;
            }
            lead = false;
            out.push(line);
        }
        self.recycle(text);
    }
}

/// Marks columns `col..col + cols` of row `r` taken; rows from `used` on are cleared
/// before their first use.
fn mark_taken(taken: &mut Vec<Vec<bool>>, used: &mut usize, r: usize, col: usize, cols: usize) {
    while *used <= r {
        if taken.len() == *used {
            taken.push(Vec::new());
        }
        taken[*used].clear();
        *used += 1;
    }
    let row = &mut taken[r];
    if row.len() < col + cols {
        row.resize(col + cols, false);
    }
    row[col..col + cols].fill(true);
}

/// Makes `v` hold `n` copies of `value`, reusing its storage.
fn refill<T: Clone>(v: &mut Vec<T>, value: T, n: usize) {
    v.clear();
    v.resize(n, value);
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

impl GridWork {
    /// Draws the grid into `lines`: a character canvas holding every cell's box and
    /// content (`contents`, at `columns`). A wide character takes two canvas columns,
    /// the second left as a NUL that is dropped.
    fn draw(&mut self, layout: &Layout) {
        let contents = &self.contents[..layout.cells.len()];
        let heights = &mut self.heights;
        refill(heights, 1, layout.rows);
        for (c, lines) in layout.cells.iter().zip(contents).filter(|(c, _)| c.rows == 1) {
            heights[c.row] = heights[c.row].max(lines.len());
        }
        for (c, lines) in layout.cells.iter().zip(contents).filter(|(c, _)| c.rows > 1) {
            let have: usize = heights[c.row..c.row + c.rows].iter().sum::<usize>() + c.rows - 1;
            if lines.len() > have {
                heights[c.row + c.rows - 1] += lines.len() - have;
            }
        }
        offsets(&mut self.ys, heights);
        offsets(&mut self.xs, &self.columns);
        let (ys, xs) = (&self.ys, &self.xs);
        let (height, wide) = (ys[layout.rows] + 1, xs[layout.cols] + 1);
        let canvas = &mut self.canvas;
        refill(canvas, ' ', height * wide);
        for (c, lines) in layout.cells.iter().zip(contents) {
            let (y0, y1) = (ys[c.row], ys[c.row + c.rows]);
            let (x0, x1) = (xs[c.col], xs[c.col + c.cols]);
            let header_rule = |y: usize| layout.head_rows > 0 && y == ys[layout.head_rows];
            draw_box(canvas, wide, (y0, y1), (x0, x1), header_rule);
            for (i, line) in lines.iter().enumerate() {
                let row = &mut canvas[(y0 + 1 + i) * wide..(y0 + 2 + i) * wide];
                let mut x = x0 + 2;
                for ch in line.chars() {
                    row[x] = ch;
                    x += 1;
                    if width(ch.encode_utf8(&mut [0; 4])) == 2 {
                        row[x] = '\0';
                        x += 1;
                    }
                }
            }
        }
        self.lines.clear();
        let mut text = std::mem::take(&mut self.text);
        for row in canvas.chunks(wide) {
            text.clear();
            text.extend(row.iter().filter(|&&c| c != '\0'));
            self.lines.push(text.trim_end());
        }
        self.text = text;
    }
}

/// Fills `out` with the running offsets of `sizes`, each followed by a border: 0, then
/// one past each size.
fn offsets(out: &mut Vec<usize>, sizes: &[usize]) {
    out.clear();
    out.push(0);
    for s in sizes {
        out.push(out.last().copied().unwrap_or(0) + s + 1);
    }
}

/// Draws a cell's box on the canvas, `wide` columns per row: rows `y0` and `y1`,
/// columns `x0` and `x1`. `header_rule(y)` tells whether the rule at `y` is the
/// header separator ("=").
fn draw_box(
    canvas: &mut [char],
    wide: usize,
    (y0, y1): (usize, usize),
    (x0, x1): (usize, usize),
    header_rule: impl Fn(usize) -> bool,
) {
    for y in [y0, y1] {
        let ch = if header_rule(y) { '=' } else { '-' };
        for (x, cell) in canvas[y * wide..(y + 1) * wide].iter_mut().enumerate() {
            if x < x0 || x > x1 {
                continue;
            }
            if x == x0 || x == x1 {
                *cell = '+';
            } else if *cell != '+' {
                *cell = ch;
            }
        }
    }
    for y in y0 + 1..y1 {
        for x in [x0, x1] {
            let cell = &mut canvas[y * wide + x];
            if *cell != '+' {
                *cell = '|';
            }
        }
    }
}
