//! What the table parsers share: column maps, the Table node with its directive
//! attributes, cells, and border recognition.

use guidedoc::{
    AttributeKey, CellInfo, Fail, LineView, NodeFlags, NodeId, NodeKind, SourceId, SourceSpan,
    Text,
};
use guidedoc::{arg_span, arg_text};

use crate::lines::{Line, dedent};
use crate::messages::{TABLE_MALFORMED, TABLE_SHAPE};
use crate::parser::Parser;
use crate::width::char_columns;

/// What a table directive adds to its table: a title and attributes. A host markup
/// that emits the table of a hosted table directive takes it with
/// `Parser::take_table_extra` and passes it to `Parser::table_decorate`.
#[derive(Debug, Clone, Default)]
pub struct TableExtra {
    /// The title: source ranges (offset, length) of its lines in `title_source`.
    pub(crate) title: Vec<(u32, u32)>,
    pub(crate) title_source: SourceId,
    pub(crate) classes: Option<Text>,
    pub(crate) name: Option<Text>,
    pub(crate) align: String,
    pub(crate) width: String,
    /// From `:widths:`; `None` keeps the table's own widths.
    pub(crate) widths: Option<Vec<usize>>,
    /// `:widths: auto` leaves widths to the renderer.
    pub(crate) auto_widths: bool,
    /// "colwidths-auto" or "colwidths-given" from `:widths:`; a host may set it.
    pub width_class: &'static str,
}

/// Maps display columns of table lines to byte offsets, so tables may hold UTF-8 text,
/// East Asian wide characters included. ASCII lines need no map.
#[derive(Debug)]
pub(crate) struct ColMap<'l, 's> {
    pub lines: &'l [Line<'s>],
    /// Per line: first index into `cols`, or `None` for an ASCII line.
    start: Vec<Option<usize>>,
    /// Per line: its columns.
    width: Vec<usize>,
    /// The byte offset of every column, plus the line end.
    cols: Vec<u32>,
}

/// A column map's storage, kept for the next table (`ColMap::reuse`).
#[derive(Debug, Default)]
pub(crate) struct ColStorage {
    start: Vec<Option<usize>>,
    width: Vec<usize>,
    cols: Vec<u32>,
}

impl<'l, 's> ColMap<'l, 's> {
    pub fn new(lines: &'l [Line<'s>]) -> Self {
        ColMap::reuse(lines, ColStorage::default())
    }

    /// Maps `lines` in the storage of an earlier map.
    pub fn reuse(lines: &'l [Line<'s>], storage: ColStorage) -> Self {
        let ColStorage { mut start, mut width, mut cols } = storage;
        start.clear();
        start.reserve(lines.len());
        width.clear();
        cols.clear();
        let mut m = ColMap { lines, start, width, cols };
        for l in lines {
            if l.text.is_ascii() {
                m.start.push(None);
                m.width.push(l.text.len());
                continue;
            }
            m.start.push(Some(m.cols.len()));
            // A wide character covers two columns; both map to its first byte.
            let mut w = 0;
            for (i, ch) in l.text.char_indices() {
                for _ in 0..char_columns(ch) {
                    m.cols.push(i as u32);
                    w += 1;
                }
            }
            m.cols.push(l.text.len() as u32);
            m.width.push(w);
        }
        m
    }

    /// Returns the map's storage, for the next table.
    pub fn into_storage(self) -> ColStorage {
        ColStorage { start: self.start, width: self.width, cols: self.cols }
    }

    /// The columns of line `r`.
    pub fn width(&self, r: usize) -> usize {
        self.width[r]
    }

    /// The byte offset of column `c` of line `r`, clamped to the line.
    pub fn byte(&self, r: usize, c: usize) -> usize {
        let cc = c.min(self.width[r]);
        match self.start[r] {
            None => cc,
            Some(s) => self.cols[s + cc] as usize,
        }
    }

    /// The byte at column `c` of line `r`; a space outside the line.
    pub fn char_at(&self, r: usize, c: usize) -> u8 {
        if c >= self.width[r] {
            return b' ';
        }
        self.lines[r].text.as_bytes()[self.byte(r, c)]
    }

    /// The text of line `r` between columns `a` and `b`, without trailing spaces.
    pub fn part(&self, r: usize, a: usize, b: usize) -> Line<'s> {
        self.lines[r].part(self.byte(r, a), self.byte(r, b))
    }

    /// Tells whether line `r` holds text between columns `a` and `b`.
    pub fn has_text(&self, r: usize, a: usize, b: usize) -> bool {
        !self.part(r, a, b).text.trim_start_matches(' ').is_empty()
    }
}

/// Appends the decimal digits of `n` to the text being built, without allocating.
fn push_decimal(b: &mut guidedoc::Builder<'_>, n: usize) -> Result<(), Fail> {
    let mut digits = [0u8; 20];
    let mut at = digits.len();
    let mut v = n;
    loop {
        at -= 1;
        // A digit is below ten, so the cast keeps it.
        digits[at] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    b.text_push_bytes(&digits[at..])
}

/// Tells whether `t` is a grid table border ("+---+").
pub(crate) fn is_grid_border(t: &str) -> bool {
    is_grid_rule(t, b'-')
}

/// Tells whether `t` is a grid table header separator ("+===+").
pub(crate) fn is_grid_head_border(t: &str) -> bool {
    is_grid_rule(t, b'=')
}

fn is_grid_rule(t: &str, fill: u8) -> bool {
    let b = t.as_bytes();
    let n = b.len();
    n >= 5
        && b[0] == b'+'
        && b[1] == fill
        && b[n - 2] == fill
        && b[n - 1] == b'+'
        && b.iter().all(|&c| c == fill || c == b'+')
}

/// Matches the top border of a simple table: two or more columns of "=".
pub(crate) fn is_simple_border(t: &str) -> bool {
    let b = t.as_bytes();
    if b.first() != Some(&b'=') || !b.iter().all(|&c| c == b'=' || c == b' ') {
        return false;
    }
    (0..b.len()).filter(|&i| b[i] == b'=' && (i == 0 || b[i - 1] == b' ')).count() >= 2
}

impl Parser<'_, '_> {
    /// Opens a Table with its column count, widths, directive attributes, and title.
    /// Rows follow as children.
    pub(crate) fn begin_table(
        &mut self,
        span: SourceSpan,
        cols: usize,
        widths: &[usize],
        extra: Option<&mut TableExtra>,
    ) -> Result<NodeId, Fail> {
        let id = self.open(NodeKind::Table, span)?;
        self.ctx.b.set_value(id, i32::try_from(cols).unwrap_or(i32::MAX));
        self.table_decorate(id, span, cols, widths, extra)?;
        Ok(id)
    }

    /// Takes the title and options of the hosted table directive whose content is being
    /// parsed, for the first table the host emits; `None` outside one or once taken.
    pub fn take_table_extra(&mut self) -> Option<TableExtra> {
        self.st.table_extra.take()
    }

    /// Gives an open Table, before its rows, its widths and a table directive's
    /// attributes and title. A host calls it for the table it emits inside a table
    /// directive (`take_table_extra`).
    pub fn table_decorate(
        &mut self,
        id: NodeId,
        span: SourceSpan,
        cols: usize,
        widths: &[usize],
        extra: Option<&mut TableExtra>,
    ) -> Result<(), Fail> {
        let mut w: Option<&[usize]> = Some(widths);
        if let Some(e) = extra.as_deref() {
            self.table_attributes(id, e)?;
            match &e.widths {
                Some(given) if given.len() != cols => {
                    let why = arg_text("the number of widths differs from the number of columns");
                    self.report(TABLE_SHAPE, span, arg_span(span), why);
                }
                Some(given) => w = Some(given),
                None => {}
            }
            if e.auto_widths {
                w = None;
            }
        }
        if let Some(w) = w.filter(|w| !w.is_empty()) {
            // Written straight into the text pool: "10 20 30".
            let text = self.ctx.b.build_text(|t| {
                for (k, &n) in w.iter().enumerate() {
                    if k > 0 {
                        t.text_push_byte(b' ')?;
                    }
                    push_decimal(t, n)?;
                }
                Ok(())
            })?;
            self.ctx.b.add_attribute(id, AttributeKey::Widths, text)?;
        }
        if let Some(e) = extra.filter(|e| !e.title.is_empty()) {
            self.extra_title(e)?;
        }
        Ok(())
    }

    fn table_attributes(&mut self, id: NodeId, e: &TableExtra) -> Result<(), Fail> {
        if e.classes.is_some() || !e.width_class.is_empty() {
            let mut class =
                e.classes.map(|t| self.ctx.b.pool_string(t).to_owned()).unwrap_or_default();
            if !class.is_empty() && !e.width_class.is_empty() {
                class.push(' ');
            }
            class.push_str(e.width_class);
            self.ctx.b.add_attribute_string(id, AttributeKey::Class, &class)?;
        }
        if let Some(name) = e.name {
            self.ctx.b.add_attribute(id, AttributeKey::Name, name)?;
        }
        if !e.align.is_empty() {
            self.ctx.b.add_attribute_string(id, AttributeKey::Align, &e.align)?;
        }
        if !e.width.is_empty() {
            self.ctx.b.add_attribute_string(id, AttributeKey::Width, &e.width)?;
        }
        Ok(())
    }

    /// Emits a table directive's title, parsed from its source ranges.
    fn extra_title(&mut self, e: &TableExtra) -> Result<(), Fail> {
        let text = self.ctx.source_of(e.title_source);
        let lines: Vec<Line<'_>> = e
            .title
            .iter()
            .map(|&(offset, len)| {
                let range = offset as usize..(offset + len) as usize;
                Line::from_view(LineView { text: text.get(range).unwrap_or(""), offset })
            })
            .collect();
        let frame = self.enter_source(e.title_source, self.st.tab);
        let r = self.inline_node(NodeKind::Title, &lines);
        self.leave_source(frame);
        r
    }

    /// Makes a table directive's title (`TableExtra`) of its argument lines.
    pub(crate) fn title_ranges(&self, lines: &[Line<'_>]) -> Vec<(u32, u32)> {
        let tab = self.st.tab;
        lines.iter().map(|l| l.view(tab)).map(|v| (v.offset, v.text.len() as u32)).collect()
    }

    /// Emits a Cell whose lines are parsed as body elements.
    pub(crate) fn cell(
        &mut self,
        span: SourceSpan,
        rows: usize,
        cols: usize,
        lines: &mut [Line<'_>],
        header: bool,
    ) -> Result<(), Fail> {
        let id = self.ctx.b.begin_node(NodeKind::Cell, span)?;
        let span16 = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
        self.ctx.b.set_cell(
            id,
            CellInfo { row_span: span16(rows), col_span: span16(cols), ..CellInfo::default() },
        )?;
        if header {
            self.ctx.b.add_flags(id, NodeFlags::HEADER);
        }
        dedent(lines);
        self.parse_body(lines, false, None)?;
        self.close(id)
    }

    /// Reports a malformed table.
    pub(crate) fn malformed(&mut self, lines: &[Line<'_>], why: &'static str) {
        let span = self.lines_span(lines);
        self.report(TABLE_MALFORMED, span, arg_span(span), arg_text(why));
    }
}
