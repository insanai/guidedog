//! GFM tables. A table block's content lines are the header row, the delimiter row, and
//! the body rows; each row splits into cells at unescaped pipes, a leading and a
//! trailing pipe being optional.

use guidedoc::{AttributeKey, CellInfo, Fail, NodeFlags, NodeKind};

use super::{trim_right, trim_space};
use crate::blocks::{Bid, Skeleton};
use crate::emit::open_block;
use crate::host::Host;
use crate::inline::parse_leaf_range;

/// One cell of a row: its text and where the text starts in the row.
#[derive(Debug, Clone, Copy)]
struct Cell<'r> {
    text: &'r str,
    start: usize,
}

/// The cells of a row, in order.
fn cells(row: &str) -> Vec<Cell<'_>> {
    let t = trim_right(row);
    let b = t.as_bytes();
    let mut a = b.iter().take_while(|&&c| c == b' ' || c == b'\t').count();
    if b.get(a) == Some(&b'|') {
        a += 1;
    }
    let mut e = b.len();
    if e > a && b[e - 1] == b'|' && (e < 2 || b[e - 2] != b'\\') {
        e -= 1;
    }
    let row = &t[..e.max(a)];
    let b = row.as_bytes();
    let mut out = Vec::new();
    let mut start = a;
    let mut i = a;
    loop {
        while i < b.len() && b[i] != b'|' {
            i += if b[i] == b'\\' && i + 1 < b.len() { 2 } else { 1 };
        }
        out.push(Cell { text: &row[start..i.min(b.len())], start });
        if i >= b.len() {
            return out;
        }
        i += 1;
        start = i;
    }
}

pub(crate) fn count_cells(row: &str) -> usize {
    cells(row).len()
}

/// Checks a GFM table delimiter row and counts its columns.
pub(crate) fn delimiter_row(s: &str) -> Option<usize> {
    let t = trim_right(s);
    if t.is_empty()
        || (t.len() >= 2 && t.starts_with('-') && matches!(t.as_bytes()[1], b' ' | b'\t'))
    {
        return None;
    }
    if !t.bytes().all(|c| matches!(c, b'|' | b'-' | b':' | b' ' | b'\t')) {
        return None;
    }
    let mut cols = 0;
    for cell in cells(t) {
        let c = trim_space(cell.text);
        let c = c.strip_prefix(':').unwrap_or(c);
        let c = c.strip_suffix(':').unwrap_or(c);
        if c.is_empty() || !c.bytes().all(|x| x == b'-') {
            return None;
        }
        cols += 1;
    }
    (cols > 0).then_some(cols)
}

/// A column's alignment class.
fn align_class(cell: &str) -> &'static str {
    let c = trim_space(cell);
    match (c.starts_with(':'), !c.is_empty() && c.ends_with(':')) {
        (true, true) => "text-center",
        (true, false) => "text-left",
        (false, true) => "text-right",
        (false, false) => "",
    }
}

/// Emits a Table from its block.
pub(crate) fn emit_table(h: &mut Host<'_, '_>, sk: &Skeleton<'_>, id: Bid) -> Result<(), Fail> {
    let cols = sk.blocks[id].columns;
    let rows = sk.seg_count(id);
    if cols == 0 || rows < 2 {
        return Ok(());
    }
    let aligns: Vec<&str> =
        cells(sk.seg_text(id, 1)).iter().map(|c| align_class(c.text)).collect();
    let span = sk.block_span(id);
    let table = open_block(h, NodeKind::Table, span)?;
    h.b().set_value(table, cols as i32);
    let decorated = match h.ext() {
        Some(ext) => ext.table(h, table, span, cols)?,
        None => false,
    };
    if !decorated {
        h.b().add_attribute_string(table, AttributeKey::Class, "colwidths-auto")?;
    }
    let head = h.b().begin_node(NodeKind::TableHead, sk.seg_span(id, 0))?;
    emit_row(h, sk, id, 0, cols, &aligns)?;
    h.b().end_node(head)?;
    if rows > 2 {
        let body = h.b().begin_node(NodeKind::TableBody, sk.seg_span(id, 2))?;
        for r in 2..rows {
            emit_row(h, sk, id, r, cols, &aligns)?;
        }
        h.b().end_node(body)?;
    }
    h.b().end_node(table)
}

fn emit_row(
    h: &mut Host<'_, '_>,
    sk: &Skeleton<'_>,
    id: Bid,
    r: usize,
    cols: usize,
    aligns: &[&str],
) -> Result<(), Fail> {
    let header = r == 0;
    let row = h.b().begin_node(NodeKind::Row, sk.seg_span(id, r))?;
    if header {
        h.b().add_flags(row, NodeFlags::HEADER);
    }
    let base = sk.segs[sk.blocks[id].seg_first + r].content;
    let line = cells(sk.seg_text(id, r));
    for c in 0..cols {
        let cell = line.get(c).copied().unwrap_or(Cell { text: "", start: 0 });
        let text = trim_space(cell.text);
        let lead = cell.text.len() - cell.text.trim_start_matches([' ', '\t']).len();
        let s = base + cell.start + lead;
        let at = sk.source_offset(id, s);
        let cell_span = sk.span(at, at + text.len());
        let b = h.b();
        let node = b.begin_node(NodeKind::Cell, cell_span)?;
        b.set_cell(node, CellInfo { row_span: 1, col_span: 1, ..CellInfo::default() })?;
        if header {
            b.add_flags(node, NodeFlags::HEADER);
        }
        if let Some(class) = aligns.get(c).filter(|a| !a.is_empty()) {
            b.add_attribute_string(node, AttributeKey::Class, class)?;
        }
        if !text.is_empty() {
            let para = b.begin_node(NodeKind::Paragraph, cell_span)?;
            parse_leaf_range(h, sk, id, s, s + text.len())?;
            h.b().end_node(para)?;
        }
        h.b().end_node(node)?;
    }
    h.b().end_node(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_split_at_unescaped_pipes() {
        let texts: Vec<&str> = cells("| a | b \\| c |").iter().map(|c| c.text).collect();
        assert_eq!(texts, [" a ", " b \\| c "]);
        assert_eq!(delimiter_row("|:-|--:|"), Some(2));
        assert_eq!(delimiter_row("- a"), None);
    }
}
