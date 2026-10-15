//! Lines in columns. The block parser works on lines whose tabs are expanded to tab
//! stops and whose trailing white space is stripped. A line that held a tab, form feed,
//! or vertical tab is an expanded copy; it remembers its source line (`raw`) so columns
//! map back to source bytes. Nested blocks use cut views of their parent's lines, so
//! every node keeps exact spans.

use std::borrow::Cow;

use guidedoc::{LineView, decode_last_rune};

/// One input line in columns: tabs expanded, trailing white space stripped. Lines are
/// `Copy` views; cutting one never copies text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Line<'s> {
    /// The line's text from its current column.
    pub text: &'s str,
    /// The source line of an expanded copy, else "".
    raw: &'s str,
    /// Source offset of `text[0]`, or of `raw[0]` for an expanded copy.
    offset: u32,
    /// Byte column of `text[0]` in the expanded line, for an expanded copy.
    col: u32,
}

impl<'s> Line<'s> {
    /// Makes a line of source text that needs no expansion (a host's view of a line
    /// whose tabs it handles itself, or text without tabs).
    pub fn from_view(view: LineView<'s>) -> Line<'s> {
        Line { text: view.text, raw: "", offset: view.offset, col: 0 }
    }

    /// Tells whether the line has no text (white space is stripped when lines are made).
    pub fn is_blank(&self) -> bool {
        self.text.is_empty()
    }

    /// Tells whether the line starts with a space.
    pub fn is_indented(&self) -> bool {
        self.text.as_bytes().first() == Some(&b' ')
    }

    /// Drops the first `k` columns (bytes of the expanded text).
    pub fn cut(self, k: usize) -> Line<'s> {
        let n = ceil_boundary(self.text, k.min(self.text.len()));
        let mut r = self;
        r.text = &self.text[n..];
        if self.raw.is_empty() {
            r.offset += n as u32;
        } else {
            r.col += n as u32;
        }
        r
    }

    /// Keeps the first `n` columns, without trailing spaces.
    pub fn head(self, n: usize) -> Line<'s> {
        let n = ceil_boundary(self.text, n.min(self.text.len()));
        Line { text: strip_right(&self.text[..n]), ..self }
    }

    /// The columns from `from` to `to`, without trailing spaces.
    pub(crate) fn part(self, from: usize, to: usize) -> Line<'s> {
        self.cut(from).head(to.saturating_sub(from))
    }

    /// The line without its indentation.
    pub(crate) fn trim(self) -> Line<'s> {
        let n = indent_of(self.text);
        let len = self.text.len();
        self.cut(n).head(len)
    }

    /// Returns the source offset where the line's text starts, with tab stops `tab`.
    pub fn start(&self, tab: usize) -> u32 {
        if self.raw.is_empty() {
            return self.offset;
        }
        self.offset + raw_index(self.raw, self.col as usize, tab) as u32
    }

    /// Returns the source offset where the line's text ends, with tab stops `tab`.
    pub fn end(&self, tab: usize) -> u32 {
        if self.raw.is_empty() {
            return self.offset + self.text.len() as u32;
        }
        let col = self.col as usize + self.text.len();
        self.offset + raw_index(self.raw, col, tab) as u32
    }

    /// Returns a view of the source bytes the line covers, tabs unexpanded, for inline
    /// parsing, whose spans then stay exact.
    pub fn view(&self, tab: usize) -> LineView<'s> {
        if self.raw.is_empty() {
            return LineView { text: self.text, offset: self.offset };
        }
        let a = raw_index(self.raw, self.col as usize, tab);
        let z = raw_index(self.raw, self.col as usize + self.text.len(), tab).max(a);
        LineView { text: self.raw.get(a..z).unwrap_or(""), offset: self.offset + a as u32 }
    }
}

/// An indented block taken from a parent's lines: its lines (cut views), the parent
/// lines it consumed, trailing blank lines included, and whether a blank line or the end
/// of the lines finished it.
#[derive(Debug, Clone, Default)]
pub(crate) struct Block<'s> {
    pub lines: Vec<Line<'s>>,
    pub consumed: usize,
    pub blank_finish: bool,
}

/// Counts the spaces `s` starts with.
pub fn indent_of(s: &str) -> usize {
    s.bytes().take_while(|&c| c == b' ').count()
}

/// Returns `lines` without trailing blank lines.
pub fn trim_blank_end<'l, 's>(lines: &'l [Line<'s>]) -> &'l [Line<'s>] {
    let n = lines.iter().rposition(|l| !l.is_blank()).map_or(0, |k| k + 1);
    &lines[..n]
}

/// Returns `lines` without leading blank lines.
pub fn trim_blank_start<'l, 's>(lines: &'l [Line<'s>]) -> &'l [Line<'s>] {
    let n = lines.iter().position(|l| !l.is_blank()).unwrap_or(lines.len());
    &lines[n..]
}

/// Strips the least indentation of the non-blank lines, in place.
pub fn dedent(lines: &mut [Line<'_>]) {
    let least = lines.iter().filter(|l| !l.is_blank()).map(|l| indent_of(l.text)).min();
    let Some(least) = least.filter(|&n| n > 0) else {
        return;
    };
    for l in lines.iter_mut() {
        *l = l.cut(least);
    }
}

/// Joins the lines' text with `sep`: borrowed for a single line, else a copy.
pub fn join_lines<'s>(lines: &[Line<'s>], sep: &str) -> Cow<'s, str> {
    match lines {
        [] => Cow::Borrowed(""),
        [one] => Cow::Borrowed(one.text),
        _ => {
            let size = lines.iter().map(|l| l.text.len() + sep.len()).sum();
            let mut out = String::with_capacity(size);
            for (i, l) in lines.iter().enumerate() {
                if i > 0 {
                    out.push_str(sep);
                }
                out.push_str(l.text);
            }
            Cow::Owned(out)
        }
    }
}

/// Returns the smallest character boundary of `s` at or after `n`.
pub(crate) fn ceil_boundary(s: &str, n: usize) -> usize {
    let mut k = n.min(s.len());
    while !s.is_char_boundary(k) {
        k += 1;
    }
    k
}

/// Returns `s` without trailing white space (see `trailing_space_start`).
pub(crate) fn strip_right(s: &str) -> &str {
    &s[..trailing_space_start(s)]
}

/// ASCII white space as Python's `str.strip` sees it.
pub(crate) fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | b'\x0c' | b'\x0b')
}

/// Returns `s` without leading and trailing ASCII white space.
pub(crate) fn trim_space(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_ascii() && is_space(c as u8))
}

/// Returns the byte index in `raw` at which expanded byte column `col` begins. A column
/// inside an expanded tab maps to the byte after the tab.
fn raw_index(raw: &str, col: usize, tab: usize) -> usize {
    let tab = tab.max(1);
    let (mut x, mut c) = (0usize, 0usize);
    for (i, ch) in raw.bytes().enumerate() {
        if x >= col {
            return i;
        }
        if ch == b'\t' {
            let n = tab - c % tab;
            x += n;
            c += n;
        } else {
            x += 1;
            if !(0x80..0xc0).contains(&ch) {
                c += 1;
            }
        }
    }
    raw.len()
}

/// Returns where the white space that ends `s` begins. White space is ASCII space, tab,
/// form feed, and vertical tab, the separators U+001C to U+001F, and Unicode white space,
/// as the specification strips it from line ends.
pub(crate) fn trailing_space_start(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut n = s.len();
    while n > 0 {
        let c = bytes[n - 1];
        if c < 0x80 {
            if matches!(c, b' ' | b'\t' | b'\x0c' | b'\x0b' | 0x1c..=0x1f) {
                n -= 1;
                continue;
            }
            break;
        }
        let (r, w) = decode_last_rune(s, n);
        if !unicode_space(r) {
            break;
        }
        n -= w;
    }
    n
}

fn unicode_space(r: char) -> bool {
    matches!(
        r,
        '\u{85}'
            | '\u{a0}'
            | '\u{1680}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
            | '\u{2000}'..='\u{200a}'
    )
}

/// Where one line of a split source lives: its stripped source text, and its expanded
/// copy in the buffer when it needed one.
#[derive(Debug)]
struct RawLine {
    start: usize,
    end: usize,
    expanded: Option<(usize, usize)>,
}

/// Splits `text` into lines at "\n", "\r\n", and "\r", as Python's `splitlines` does (a
/// final line break does not start another line). `base` is the source offset of
/// `text[0]`. Lines that hold tabs, form feeds, or vertical tabs are expanded into `buf`,
/// which the lines then borrow.
pub(crate) fn split_lines<'s>(
    text: &'s str,
    base: u32,
    tab: usize,
    buf: &'s mut String,
) -> Vec<Line<'s>> {
    let mut lines = Vec::new();
    split_lines_into(text, base, tab, buf, &mut SplitWork::default(), &mut lines);
    lines
}

/// The storage `split_lines_into` reuses: where each line lives.
#[derive(Debug, Default)]
pub(crate) struct SplitWork {
    raws: Vec<RawLine>,
}

/// `split_lines` into `lines`, which it clears first, reusing `work` and the vector.
pub(crate) fn split_lines_into<'s>(
    text: &'s str,
    base: u32,
    tab: usize,
    buf: &'s mut String,
    work: &mut SplitWork,
    lines: &mut Vec<Line<'s>>,
) {
    buf.clear();
    let raws = &mut work.raws;
    raws.clear();
    let bytes = text.as_bytes();
    let mut start = 0;
    while start < text.len() {
        let mut end = start;
        while end < text.len() && bytes[end] != b'\n' && bytes[end] != b'\r' {
            end += 1;
        }
        let mut next = end + 1;
        if end < text.len() && bytes[end] == b'\r' && bytes.get(next) == Some(&b'\n') {
            next += 1;
        }
        let stripped = start + trailing_space_start(&text[start..end]);
        let expanded = expand_into(buf, &text[start..stripped], tab);
        raws.push(RawLine { start, end: stripped, expanded });
        start = next;
    }
    let buf: &'s str = buf;
    lines.clear();
    lines.extend(raws.iter().map(|r| make_line(text, buf, r, base)));
}

/// Makes lines of a host's views of source text, expanding tabs into `buf` as
/// `split_lines` does.
pub fn expand_views<'s>(views: &[LineView<'s>], tab: usize, buf: &'s mut String) -> Vec<Line<'s>> {
    buf.clear();
    let mut raws = Vec::with_capacity(views.len());
    for v in views {
        let stripped = &v.text[..trailing_space_start(v.text)];
        raws.push((*v, stripped, expand_into(buf, stripped, tab)));
    }
    let buf: &'s str = buf;
    raws.iter()
        .map(|&(v, stripped, expanded)| match expanded {
            None => Line { text: stripped, raw: "", offset: v.offset, col: 0 },
            Some((a, z)) => Line { text: &buf[a..z], raw: stripped, offset: v.offset, col: 0 },
        })
        .collect()
}

fn make_line<'s>(text: &'s str, buf: &'s str, r: &RawLine, base: u32) -> Line<'s> {
    let raw = &text[r.start..r.end];
    let offset = base + r.start as u32;
    match r.expanded {
        None => Line { text: raw, raw: "", offset, col: 0 },
        Some((a, z)) => Line { text: &buf[a..z], raw, offset, col: 0 },
    }
}

/// Appends the expansion of `raw` to `buf` when it holds a tab, form feed, or vertical
/// tab, and returns where it went.
fn expand_into(buf: &mut String, raw: &str, tab: usize) -> Option<(usize, usize)> {
    if !raw.bytes().any(|c| matches!(c, b'\t' | b'\x0c' | b'\x0b')) {
        return None;
    }
    let tab = tab.max(1);
    let start = buf.len();
    let mut col = 0;
    for ch in raw.chars() {
        match ch {
            '\t' => {
                let w = tab - col % tab;
                buf.extend(std::iter::repeat_n(' ', w));
                col += w;
            }
            '\x0c' | '\x0b' => {
                buf.push(' ');
                col += 1;
            }
            _ => {
                buf.push(ch);
                col += 1;
            }
        }
    }
    Some((start, buf.len()))
}

/// Collects the lines from `start` that are blank or indented and strips their least
/// indentation. With `first`, the first line always belongs to the block and is cut at
/// that column. `until_blank` ends the block at the first blank line; `strip_top` drops
/// leading blank lines from the result. The block's lines go into `out`, cleared first
/// (a buffer from `Parser::line_buf`).
pub(crate) fn indented_block<'s>(
    lines: &[Line<'s>],
    start: usize,
    first: Option<usize>,
    until_blank: bool,
    strip_top: bool,
    mut out: Vec<Line<'s>>,
) -> Block<'s> {
    let mut end = start + usize::from(first.is_some());
    let mut least: Option<usize> = None;
    let mut blank_finish = true;
    while end < lines.len() {
        let l = lines[end];
        if !l.is_blank() && !l.is_indented() {
            blank_finish = end > start && lines[end - 1].is_blank();
            break;
        }
        if l.is_blank() {
            if until_blank {
                break;
            }
        } else {
            let n = indent_of(l.text);
            least = Some(least.map_or(n, |m| m.min(n)));
        }
        end += 1;
    }
    let end = end.min(lines.len()).max(start.min(lines.len()));
    let least = least.unwrap_or(0);
    out.clear();
    out.reserve(end - start);
    for (k, l) in lines[start..end].iter().enumerate() {
        match first {
            Some(col) if k == 0 => out.push(l.cut(col)),
            _ => out.push(l.cut(least)),
        }
    }
    if strip_top {
        let top = out.iter().position(|l| !l.is_blank()).unwrap_or(out.len());
        out.drain(..top);
    }
    Block { lines: out, consumed: end - start, blank_finish }
}

/// Collects the lines from `start` that are blank or indented by at least `indent`
/// columns, and cuts every line, the first included, at that column. List items whose
/// text starts on the marker line use it: their text fixes the indentation.
pub(crate) fn known_indented_block<'s>(
    lines: &[Line<'s>],
    start: usize,
    indent: usize,
    mut out: Vec<Line<'s>>,
) -> Block<'s> {
    let mut end = start + 1;
    let mut blank_finish = true;
    while end < lines.len() {
        let t = lines[end].text;
        if !t.is_empty() && (!lines[end].is_indented() || indent_of(t) < indent) {
            blank_finish = lines[end - 1].is_blank();
            break;
        }
        end += 1;
    }
    let end = end.min(lines.len());
    out.clear();
    out.extend(lines[start..end].iter().map(|l| l.cut(indent)));
    let top = out.iter().position(|l| !l.is_blank()).unwrap_or(out.len());
    out.drain(..top);
    Block { lines: out, consumed: end - start, blank_finish }
}

/// Returns the end of the text block that starts at `start`: the next blank line, or
/// with `flush_left` the next indented line, which the caller then reports.
pub(crate) fn text_end(lines: &[Line<'_>], start: usize, flush_left: bool) -> (usize, bool) {
    let mut end = start;
    while end < lines.len() && !lines[end].is_blank() {
        if flush_left && end > start && lines[end].is_indented() {
            return (end, true);
        }
        end += 1;
    }
    (end, false)
}

/// Removes a UTF-8 byte order mark: the text after it and its length.
pub(crate) fn strip_bom(text: &str) -> (&str, u32) {
    match text.strip_prefix('\u{feff}') {
        Some(rest) => (rest, 3),
        None => (text, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_handles_every_line_ending() {
        let mut buf = String::new();
        let lines = split_lines("a\r\nb\rc\n\nd", 0, 8, &mut buf);
        let texts: Vec<&str> = lines.iter().map(|l| l.text).collect();
        assert_eq!(texts, ["a", "b", "c", "", "d"]);
        assert_eq!(lines[4].start(8), 8);
    }

    #[test]
    fn tabs_expand_and_map_back() {
        let mut buf = String::new();
        let lines = split_lines("\tx\ty  \n", 10, 4, &mut buf);
        assert_eq!(lines[0].text, "    x   y");
        let cut = lines[0].cut(4);
        assert_eq!(cut.text, "x   y");
        assert_eq!(cut.start(4), 11);
        assert_eq!(cut.end(4), 14);
        assert_eq!(cut.view(4).text, "x\ty");
    }

    #[test]
    fn trailing_unicode_space_is_stripped() {
        assert_eq!(strip_right("a\u{3000} \u{a0}"), "a");
        assert_eq!(strip_right("a\x1f"), "a");
    }
}
