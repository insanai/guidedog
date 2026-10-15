//! The table directives: table, list-table (csv-table lives in `csv`).

use guidedoc::{Fail, NodeFlags, NodeKind, arg_span, arg_text};

use crate::directives::Directive;
use crate::lines::{Line, indent_of, trim_blank_start};
use crate::lists::{bullet_width, item_body};
use crate::messages::{LIST_TABLE_SHAPE, TABLE_SHAPE};
use crate::names::parse_int;
use crate::parser::Parser;
use crate::tables::{TableExtra, is_grid_border, is_simple_border};

/// One item of a bullet list: its content, and the lines it spans in the list.
#[derive(Debug, Clone)]
pub(crate) struct BulletItem<'s> {
    pub lines: Vec<Line<'s>>,
    pub first: usize,
    pub end: usize,
}

/// The rows of a list table and the cells of each row.
type ListShape<'s> = (Vec<BulletItem<'s>>, Vec<Vec<BulletItem<'s>>>);

impl Parser<'_, '_> {
    /// Collects the title and options shared by the table directives.
    pub(crate) fn table_extra(&mut self, d: &Directive<'_>) -> Result<TableExtra, Fail> {
        let mut e = TableExtra {
            title: self.title_ranges(&d.arg),
            title_source: self.src(),
            ..TableExtra::default()
        };
        if let Some(v) = d.option_value("class") {
            e.classes = self.pool_classes(&v)?;
        }
        if let Some(v) = d.option_value("name") {
            e.name = Some(self.pool_name(&v)?);
        }
        e.align = d.option_value("align").unwrap_or_default().into_owned();
        e.width = d.option_value("width").unwrap_or_default().into_owned();
        let Some(v) = d.option_value("widths") else {
            return Ok(e);
        };
        // Renderers size columns by these classes, as the specification's writers do.
        e.width_class = if v == "auto" { "colwidths-auto" } else { "colwidths-given" };
        match &*v {
            "grid" => {}
            "auto" => e.auto_widths = true,
            _ => {
                let numbers = v.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty());
                e.widths = Some(numbers.map(|s| parse_int(s).max(0) as usize).collect());
            }
        }
        Ok(e)
    }

    /// Reports `:widths:` that do not list one width per column.
    fn check_widths(&mut self, d: &Directive<'_>, e: &TableExtra, cols: usize) -> bool {
        let (Some(w), Some(o)) = (&e.widths, d.option("widths")) else {
            return true;
        };
        if w.len() == cols {
            return true;
        }
        let why = arg_text("the number of widths differs from the number of columns");
        self.report(TABLE_SHAPE, o.span, arg_span(d.name_span), why);
        false
    }

    fn shape_problem(&mut self, d: &Directive<'_>, why: &'static str) {
        self.report(TABLE_SHAPE, d.span, arg_span(d.name_span), arg_text(why));
    }

    pub(crate) fn table_directive(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        let mut e = self.table_extra(d)?;
        if self.st.hosted && self.host.is_some() {
            return self.hosted_table(d, e);
        }
        let content = &d.content;
        let first = content[0].text;
        let n = if is_grid_border(first) {
            self.grid_table(content, 0, Some(&mut e))?
        } else if is_simple_border(first) {
            self.simple_table(content, 0, Some(&mut e))?
        } else {
            self.shape_problem(d, "its content must be a grid table or a simple table");
            return Ok(());
        };
        if !trim_blank_start(&content[n.min(content.len())..]).is_empty() {
            self.shape_problem(d, "its content must be exactly one table");
        }
        Ok(())
    }

    /// Gives the table directive's title and options to the first table the host
    /// markup emits in the content (`take_table_extra`).
    fn hosted_table(&mut self, d: &Directive<'_>, e: TableExtra) -> Result<(), Fail> {
        let saved = self.st.table_extra.replace(e);
        let r = self.parse_body(&d.content, false, None);
        let unused = std::mem::replace(&mut self.st.table_extra, saved);
        r?;
        if unused.is_some() {
            self.shape_problem(d, "its content must be a table");
        }
        Ok(())
    }

    /// Validates `:header-rows:` and `:stub-columns:` against the data.
    pub(crate) fn header_checks(
        &mut self,
        d: &Directive<'_>,
        rows: usize,
        cols: usize,
    ) -> Option<(usize, usize)> {
        let count = |name| d.option_value(name).map_or(0, |v| parse_int(&v).max(0) as usize);
        let (head, stub) = (count("header-rows"), count("stub-columns"));
        if head > rows {
            self.shape_problem(d, "it has more header rows than rows");
            return None;
        }
        if stub > cols {
            self.shape_problem(d, "it has more stub columns than columns");
            return None;
        }
        Some((head, stub))
    }

    /// The bullet list items of `lines`: the host's in a hosted directive, else read as
    /// reStructuredText; `None` when the lines are not one bullet list.
    fn bullet_items<'s>(&mut self, lines: &[Line<'s>]) -> Option<Vec<BulletItem<'s>>> {
        if self.st.hosted
            && let Some(host) = self.host
        {
            let items = host.items(self, lines)?;
            let starts: Vec<usize> = items.iter().map(|it| it.start).collect();
            let out = items.into_iter().enumerate().map(|(k, it)| BulletItem {
                lines: it.lines,
                first: it.start,
                end: starts.get(k + 1).copied().unwrap_or(lines.len()),
            });
            return Some(out.collect());
        }
        let mut out = Vec::new();
        let mut bullet: Option<&str> = None;
        let mut pos = 0;
        loop {
            while pos < lines.len() && lines[pos].is_blank() {
                pos += 1;
            }
            if pos >= lines.len() {
                return Some(out);
            }
            let t = lines[pos].text;
            let w = bullet_width(t);
            if w == 0 || bullet.is_some_and(|b| b != &t[..w]) {
                return None;
            }
            bullet = Some(&t[..w]);
            // As in a bullet list: text on the marker line fixes the item's indentation.
            let blk = item_body(lines, pos, w + indent_of(&t[w..]), Vec::new());
            let first = pos;
            pos += blk.consumed.max(1);
            out.push(BulletItem { lines: blk.lines, first, end: pos });
        }
    }

    /// Checks that the content is a bullet list of bullet lists with one length, and
    /// returns them.
    fn list_shape<'s>(&mut self, content: &[Line<'s>]) -> Result<ListShape<'s>, &'static str> {
        let rows = self.bullet_items(content).ok_or("the content must be one bullet list")?;
        let mut cells = Vec::with_capacity(rows.len());
        for row in &rows {
            let inner = self.bullet_items(&row.lines).ok_or("each row must be one bullet list")?;
            if inner.is_empty() {
                return Err("each row must be one bullet list");
            }
            if cells.first().is_some_and(|c: &Vec<BulletItem<'s>>| c.len() != inner.len()) {
                return Err("the rows have different numbers of cells");
            }
            cells.push(inner);
        }
        if rows.is_empty() {
            return Err("the content must be one bullet list");
        }
        Ok((rows, cells))
    }

    pub(crate) fn list_table(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        let (rows, cells) = match self.list_shape(&d.content) {
            Ok(shape) => shape,
            Err(why) => {
                let span = self.lines_span(&d.content);
                self.report(LIST_TABLE_SHAPE, span, arg_span(d.name_span), arg_text(why));
                return Ok(());
            }
        };
        let cols = cells[0].len();
        let Some((head, stub)) = self.header_checks(d, rows.len(), cols) else {
            return Ok(());
        };
        let mut e = self.table_extra(d)?;
        if !self.check_widths(d, &e, cols) {
            return Ok(());
        }
        let widths = equal_widths(&e, cols);
        let id = self.begin_table(d.span, cols, &widths, Some(&mut e))?;
        let kind = if head > 0 { NodeKind::TableHead } else { NodeKind::TableBody };
        let mut group = self.ctx.b.begin_node(kind, d.span)?;
        for (r, (row, row_cells)) in rows.iter().zip(cells).enumerate() {
            if r == head && head > 0 {
                self.close(group)?;
                group = self.ctx.b.begin_node(NodeKind::TableBody, d.span)?;
            }
            let span = self.lines_span(&d.content[row.first..row.end]);
            let row_id = self.ctx.b.begin_node(NodeKind::Row, span)?;
            if r < head {
                self.ctx.b.add_flags(row_id, NodeFlags::HEADER);
            }
            for (c, mut cell) in row_cells.into_iter().enumerate() {
                let span = self.lines_span(&row.lines[cell.first..cell.end]);
                self.cell(span, 1, 1, &mut cell.lines, c < stub)?;
            }
            self.close(row_id)?;
        }
        self.close(group)?;
        self.close(id)
    }
}

/// Equal widths for a table whose directive gives none.
pub(crate) fn equal_widths(e: &TableExtra, cols: usize) -> Vec<usize> {
    if e.widths.is_some() || e.auto_widths { Vec::new() } else { vec![1; cols] }
}
