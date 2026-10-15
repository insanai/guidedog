//! Tables: the wrapper, captions, column widths, and cells with spans and alignment.

use guidedoc::{Align, AttributeKey, NodeFlag, NodeId, NodeKind};

use crate::state::Html;
use crate::walk::{Close, Visit};

impl Html<'_, '_> {
    pub fn enter_table(&mut self, id: NodeId) -> Visit {
        self.w.cr();
        if !self.w.reference {
            self.w.put("<div class=\"table-wrapper\">\n");
        }
        self.tag("table", id, "");
        self.w.cr();
        let titled = self.doc.first_child(id).is_some_and(|f| self.doc.kind(f) == NodeKind::Title);
        if !titled {
            self.write_colgroup(id);
        }
        Visit::Descend(Close::Table)
    }

    pub fn leave_table(&mut self) {
        self.block_close("table");
        if !self.w.reference {
            self.w.put("</div>");
            self.w.cr();
        }
    }

    pub fn enter_cell(&mut self, id: NodeId) -> Visit {
        let name = if self.header_cell(id) { "th" } else { "td" };
        let c = self.doc.cell_info(id);
        self.open_tag(name, id, "");
        if c.col_span > 1 {
            self.w.put(" colspan=\"");
            self.w.put_int(i64::from(c.col_span));
            self.w.put_byte(b'"');
        }
        if c.row_span > 1 {
            self.w.put(" rowspan=\"");
            self.w.put_int(i64::from(c.row_span));
            self.w.put_byte(b'"');
        }
        match c.align {
            Align::None => {}
            Align::Left => self.w.put(" style=\"text-align: left\""),
            Align::Center => self.w.put(" style=\"text-align: center\""),
            Align::Right => self.w.put(" style=\"text-align: right\""),
        }
        self.w.put_byte(b'>');
        self.write_aliases(id);
        Visit::Descend(Close::Line(name))
    }

    /// Tells whether a cell is a header: flagged itself, in a header row, or in the
    /// table head.
    fn header_cell(&self, id: NodeId) -> bool {
        let header =
            |n: Option<NodeId>| n.is_some_and(|n| self.doc.node(n).flags.has(NodeFlag::Header));
        header(Some(id))
            || header(self.parent())
            || self.ancestor(2).is_some_and(|s| self.doc.kind(s) == NodeKind::TableHead)
    }

    /// Turns relative column widths, such as "2 1 1", into percentages.
    pub fn write_colgroup(&mut self, table: NodeId) {
        let Some(widths) = self.doc.attribute(table, AttributeKey::Widths) else {
            return;
        };
        let total: i64 = numbers(widths).sum();
        if total <= 0 || !numbers(widths).any(|v| v > 0) {
            return;
        }
        self.w.put("<colgroup>");
        for value in numbers(widths).filter(|&v| v > 0) {
            self.w.put("<col style=\"width: ");
            self.w.put_int((value * 100 + total / 2) / total);
            self.w.put("%\" />");
        }
        self.w.put("</colgroup>");
        self.w.cr();
    }
}

/// The decimal numbers of a list such as "2 1 1"; each stops growing past a million.
fn numbers(s: &str) -> impl Iterator<Item = i64> + '_ {
    s.split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(|digits| digits.bytes().take(7).fold(0i64, |v, c| v * 10 + i64::from(c - b'0')))
}
