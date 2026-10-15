//! The csv-table directive.

use guidedoc::{
    Fail, LineView, NodeFlags, NodeId, NodeKind, SourceId, SourceSpan, arg_span, arg_text,
};

use crate::directives::Directive;
use crate::directives_tables::equal_widths;
use crate::lines::{Line, trim_blank_end};
use crate::messages::{
    CSV_ERROR, DIRECTIVE_CONTENT_MISSING, DIRECTIVE_FILE_AND_CONTENT, DIRECTIVE_URL,
};
use crate::parser::Parser;
use crate::substitutions::unicode_code;

/// The csv-table dialect: the delimiter, quote, and escape are single characters,
/// quotes are doubled unless an escape character is given, and spaces after a
/// delimiter are skipped unless `:keepspace:` is set.
#[derive(Debug, Clone, Default)]
struct Dialect {
    delim: String,
    quote: String,
    escape: String,
    keepspace: bool,
}

/// One field of a record: one decoded piece per physical line, with the source offset
/// where the piece starts.
type Field = Vec<(u32, String)>;

/// CSV text: raw source views of its lines, and their source.
struct CsvSource<'s> {
    views: Vec<LineView<'s>>,
    src: SourceId,
}

fn has(t: &str, i: usize, s: &str) -> bool {
    !s.is_empty() && t.get(i..).is_some_and(|rest| rest.starts_with(s))
}

fn char_len(t: &str, i: usize) -> usize {
    t[i..].chars().next().map_or(1, char::len_utf8)
}

/// Reads one record starting at view `r0`: its fields and the next view, or why it is
/// malformed.
fn csv_record(
    d: &Dialect,
    v: &[LineView<'_>],
    r0: usize,
) -> Result<(usize, Vec<Field>), &'static str> {
    let mut r = r0;
    let mut t = v[r].text;
    let mut i = 0;
    let mut fields: Vec<Field> = Vec::new();
    loop {
        let mut field: Field = Vec::new();
        if !d.keepspace {
            i += t[i..].bytes().take_while(|&c| c == b' ').count();
        }
        let mut piece = (v[r].offset + i as u32, String::new());
        if has(t, i, &d.quote) {
            i += d.quote.len();
            loop {
                if i >= t.len() {
                    field.push(std::mem::take(&mut piece));
                    r += 1;
                    if r >= v.len() {
                        return Err("a quoted field is not closed");
                    }
                    (t, i) = (v[r].text, 0);
                    piece.0 = v[r].offset;
                    continue;
                }
                if has(t, i, &d.escape) && i + d.escape.len() < t.len() {
                    i += d.escape.len();
                } else if has(t, i, &d.quote) {
                    i += d.quote.len();
                    if !d.escape.is_empty() || !has(t, i, &d.quote) {
                        break;
                    }
                }
                let n = char_len(t, i);
                piece.1.push_str(&t[i..i + n]);
                i += n;
            }
            field.push(piece);
            fields.push(field);
            if i >= t.len() {
                return Ok((r + 1, fields));
            }
            if !has(t, i, &d.delim) {
                return Err("a delimiter must follow a closing quote");
            }
            i += d.delim.len();
            continue;
        }
        while i < t.len() && !has(t, i, &d.delim) {
            if has(t, i, &d.escape) && i + d.escape.len() < t.len() {
                i += d.escape.len();
            }
            let n = char_len(t, i);
            piece.1.push_str(&t[i..i + n]);
            i += n;
        }
        field.push(piece);
        fields.push(field);
        if i >= t.len() {
            return Ok((r + 1, fields));
        }
        i += d.delim.len();
    }
}

/// Counts the records and the widest record of a CSV source; on a malformed record,
/// why and at which view.
fn csv_shape(p: &mut Parser<'_, '_>, d: &Dialect, s: &CsvSource<'_>) -> Result<Shape, Fail> {
    let (mut rows, mut cols, mut r) = (0, 0, 0);
    while r < s.views.len() {
        if s.views[r].text.is_empty() {
            r += 1;
            continue;
        }
        let (next, fields) = match csv_record(d, &s.views, r) {
            Ok(rec) => rec,
            Err(why) => return Ok(Err((why, r))),
        };
        p.ctx.b.spend((next - r) as u64)?;
        rows += 1;
        cols = cols.max(fields.len());
        r = next;
    }
    Ok(Ok((rows, cols)))
}

/// The records and widest record of CSV text, or why and at which view it is
/// malformed.
type Shape = Result<(usize, usize), (&'static str, usize)>;

/// Where the rows go: the open group, and how many header rows are still to come.
struct RowSink {
    group: NodeId,
    head_left: usize,
    span: SourceSpan,
}

impl Parser<'_, '_> {
    /// Emits the records of a CSV source as rows; short rows get empty cells.
    fn csv_rows(
        &mut self,
        d: &Dialect,
        s: &CsvSource<'_>,
        cols: usize,
        stub: usize,
        sink: &mut RowSink,
    ) -> Result<(), Fail> {
        let frame = self.enter_source(s.src, self.st.tab);
        let r = self.csv_rows_in(d, s, cols, stub, sink);
        self.leave_source(frame);
        r
    }

    fn csv_rows_in(
        &mut self,
        d: &Dialect,
        s: &CsvSource<'_>,
        cols: usize,
        stub: usize,
        sink: &mut RowSink,
    ) -> Result<(), Fail> {
        let mut r = 0;
        while r < s.views.len() {
            if s.views[r].text.is_empty() {
                r += 1;
                continue;
            }
            if sink.head_left == 0 && self.ctx.b.snap.nodes[sink.group].kind == NodeKind::TableHead
            {
                self.close(sink.group)?;
                sink.group = self.ctx.b.begin_node(NodeKind::TableBody, sink.span)?;
            }
            let Ok((next, fields)) = csv_record(d, &s.views, r) else {
                return Ok(());
            };
            let last = s.views[next - 1];
            let row_span =
                SourceSpan::new(s.src, s.views[r].offset, last.offset + last.text.len() as u32);
            let row = self.ctx.b.begin_node(NodeKind::Row, row_span)?;
            if sink.head_left > 0 {
                self.ctx.b.add_flags(row, NodeFlags::HEADER);
            }
            for c in 0..cols {
                let mut lines: Vec<Line<'_>> = fields.get(c).map_or_else(Vec::new, |f| {
                    f.iter()
                        .map(|(offset, text)| Line::from_view(LineView { text, offset: *offset }))
                        .collect()
                });
                let span = if lines.is_empty() { row_span } else { self.lines_span(&lines) };
                let kept = trim_blank_end(&lines).len();
                lines.truncate(kept);
                self.cell(span, 1, 1, &mut lines, c < stub)?;
            }
            self.close(row)?;
            sink.head_left = sink.head_left.saturating_sub(1);
            r = next;
        }
        Ok(())
    }

    /// A dialect character option: "tab", "space", one character, or a code.
    fn dialect_char(&self, d: &Directive<'_>, name: &str, default: &str) -> String {
        let Some(v) = d.option_value(name) else {
            return default.to_owned();
        };
        match &*v {
            "tab" => "\t".to_owned(),
            "space" => " ".to_owned(),
            v if v.chars().count() == 1 => v.to_owned(),
            v => unicode_code(v)
                .and_then(char::from_u32)
                .map_or_else(|| default.to_owned(), String::from),
        }
    }

    pub(crate) fn csv_table(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        if let Some(o) = d.option("url") {
            self.report_at(DIRECTIVE_URL, o.span);
            return Ok(());
        }
        if !self.check_encoding(d) {
            return Ok(());
        }
        let tab = self.st.tab;
        let header = CsvSource {
            views: d.option_lines("header").map_or_else(Vec::new, |l| views(l, tab)),
            src: self.src(),
        };
        let dialect = Dialect {
            delim: self.dialect_char(d, "delim", ","),
            quote: self.dialect_char(d, "quote", "\""),
            escape: self.dialect_char(d, "escape", ""),
            keepspace: d.has_option("keepspace"),
        };
        let Some(o) = d.option("file") else {
            if d.content.is_empty() {
                self.report(
                    DIRECTIVE_CONTENT_MISSING,
                    d.span,
                    arg_span(d.name_span),
                    guidedoc::Arg::None,
                );
                return Ok(());
            }
            let data = CsvSource { views: views(&d.content, tab), src: self.src() };
            return self.csv_emit(d, &dialect, &header, &data);
        };
        if !d.content.is_empty() {
            self.report(
                DIRECTIVE_FILE_AND_CONTENT,
                d.span,
                arg_span(d.name_span),
                guidedoc::Arg::None,
            );
            return Ok(());
        }
        let Some((id, text)) = self.resource_text(o) else {
            return Ok(());
        };
        let (body, base) = crate::lines::strip_bom(&text);
        let mut buf = String::new();
        let lines = crate::lines::split_lines(body, base, tab, &mut buf);
        let data = CsvSource { views: views(&lines, tab), src: id };
        self.csv_emit(d, &dialect, &header, &data)
    }

    fn csv_emit(
        &mut self,
        d: &Directive<'_>,
        dia: &Dialect,
        header: &CsvSource<'_>,
        data: &CsvSource<'_>,
    ) -> Result<(), Fail> {
        let hshape = csv_shape(self, dia, header)?;
        let shape = csv_shape(self, dia, data)?;
        let ((hrows, hcols), (rows, cols)) = match (hshape, shape) {
            (Ok(h), Ok(b)) => (h, b),
            (Err((why, k)), _) => {
                return {
                    self.csv_error(header, why, k);
                    Ok(())
                };
            }
            (_, Err((why, k))) => {
                return {
                    self.csv_error(data, why, k);
                    Ok(())
                };
            }
        };
        let cols = cols.max(hcols);
        let Some((head, stub)) = self.header_checks(d, rows, cols) else {
            return Ok(());
        };
        if rows + hrows == 0 {
            self.report(
                DIRECTIVE_CONTENT_MISSING,
                d.span,
                arg_span(d.name_span),
                guidedoc::Arg::None,
            );
            return Ok(());
        }
        let mut e = self.table_extra(d)?;
        if !self.check_csv_widths(d, &e, cols) {
            return Ok(());
        }
        let widths = equal_widths(&e, cols);
        let id = self.begin_table(d.span, cols, &widths, Some(&mut e))?;
        let head_left = hrows + head;
        let kind = if head_left > 0 { NodeKind::TableHead } else { NodeKind::TableBody };
        let group = self.ctx.b.begin_node(kind, d.span)?;
        let mut sink = RowSink { group, head_left, span: d.span };
        self.csv_rows(dia, header, cols, stub, &mut sink)?;
        self.csv_rows(dia, data, cols, stub, &mut sink)?;
        self.close(sink.group)?;
        self.close(id)
    }

    fn check_csv_widths(
        &mut self,
        d: &Directive<'_>,
        e: &crate::tables::TableExtra,
        cols: usize,
    ) -> bool {
        match (&e.widths, d.option("widths")) {
            (Some(w), Some(o)) if w.len() != cols => {
                let why = arg_text("the number of widths differs from the number of columns");
                self.report(crate::messages::TABLE_SHAPE, o.span, arg_span(d.name_span), why);
                false
            }
            _ => true,
        }
    }

    fn csv_error(&mut self, s: &CsvSource<'_>, why: &'static str, k: usize) {
        let v = s.views[k];
        let span = SourceSpan::new(s.src, v.offset, v.offset + v.text.len() as u32);
        self.report(CSV_ERROR, span, arg_span(span), arg_text(why));
    }
}

/// Raw source views of lines, so decoded cells never outgrow the bytes they came from.
fn views<'s>(lines: &[Line<'s>], tab: usize) -> Vec<LineView<'s>> {
    lines.iter().map(|l| l.view(tab)).collect()
}
