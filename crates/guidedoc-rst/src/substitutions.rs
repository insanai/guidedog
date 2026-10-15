//! Substitution definitions (".. |name| directive:: data") and the directives that only
//! work inside them: replace, unicode, and date.

use guidedoc::{Fail, NodeKind, SourceSpan};

use crate::directives::Directive;
use crate::lines::{Line, indent_of, indented_block, join_lines};
use crate::messages::*;
use crate::names::{all_digits, parse_int};
use crate::parser::Parser;
use crate::specs::{DirKind, equal_fold};

/// Converts a character code: decimal, or hexadecimal written `0x`, `x`, `\x`, `U+`,
/// `u`, `\u`, or `&#x...;`. `None` for text that is not a code; a code past the Unicode
/// range is returned as 0x110000.
pub(crate) fn unicode_code(s: &str) -> Option<u32> {
    if all_digits(s) {
        return Some(if s.len() > 8 { 0x110000 } else { parse_int(s) as u32 });
    }
    let hex = if s.len() > 4
        && s.is_char_boundary(3)
        && equal_fold(&s[..3], "&#x")
        && s.ends_with(';')
    {
        &s[3..s.len() - 1]
    } else {
        ["0x", "\\x", "x", "u+", "\\u", "u"].iter().find_map(|prefix| {
            let n = prefix.len();
            (s.len() > n && s.is_char_boundary(n) && equal_fold(&s[..n], prefix)).then(|| &s[n..])
        })?
    };
    if hex.is_empty() {
        return None;
    }
    let mut v: u32 = 0;
    for c in hex.chars() {
        v = (v * 16 + c.to_digit(16)?).min(0x110000);
    }
    Some(v)
}

/// Finds the "|" that ends a substitution name: not after white space, and followed by
/// white space or the end of the line. The name starts at column `col` of line `i` and
/// may continue on the block's following lines.
fn substitution_close(
    lines: &[Line<'_>],
    i: usize,
    col: usize,
    consumed: usize,
) -> Option<(usize, usize)> {
    for (l, line) in lines.iter().enumerate().take(i + consumed).skip(i) {
        if line.is_blank() {
            return None;
        }
        let t = line.text.as_bytes();
        let mut k = if l == i { col + 1 } else { indent_of(line.text) };
        while k < t.len() {
            if t[k] == b'\\' {
                k += 2;
                continue;
            }
            let closes = t[k] == b'|'
                && k > 0
                && t[k - 1] != b' '
                && (k + 1 == t.len() || t[k + 1] == b' ');
            if closes && (l > i || k > col + 1) {
                return Some((l, k));
            }
            k += 1;
        }
    }
    None
}

/// Joins the lines of a name with single spaces.
fn substitution_name(lines: &[Line<'_>], i: usize, col: usize, cl: usize, cc: usize) -> String {
    if cl == i {
        return lines[i].text[col + 1..cc].to_owned();
    }
    let mut parts = Vec::with_capacity(cl - i + 1);
    parts.push(lines[i].cut(col + 1).trim());
    parts.extend(lines[i + 1..cl].iter().map(|l| l.trim()));
    parts.push(lines[cl].head(cc).trim());
    join_lines(&parts, " ").into_owned()
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS: [&str; 7] =
    ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/// Reads "YYYY-MM-DD".
pub(crate) fn parse_date(s: &str) -> Option<(i64, usize, i64)> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    if !all_digits(&s[..4]) || !all_digits(&s[5..7]) || !all_digits(&s[8..]) {
        return None;
    }
    let (y, m, d) = (parse_int(&s[..4]), parse_int(&s[5..7]), parse_int(&s[8..]));
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some((y, m as usize, d))
}

fn day_of_year(y: i64, m: usize, d: i64) -> i64 {
    let mut days = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
        days[1] = 29;
    }
    d + days[..m - 1].iter().sum::<i64>()
}

fn weekday(y: i64, m: usize, d: i64) -> usize {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let yy = if m < 3 { y - 1 } else { y };
    ((yy + yy / 4 - yy / 100 + yy / 400 + t[m - 1] + d).rem_euclid(7)) as usize
}

/// Formats date (y, m, d) with strftime-style codes into `out`.
fn format_date(out: &mut String, format: &str, (y, mo, da): (i64, usize, i64)) {
    let mut chars = format.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let Some(code) = chars.next() else {
            out.push('%');
            break;
        };
        match code {
            'Y' => out.push_str(&y.max(0).to_string()),
            'y' => out.push_str(&format!("{:02}", y.rem_euclid(100))),
            'm' => out.push_str(&format!("{mo:02}")),
            'd' => out.push_str(&format!("{da:02}")),
            'e' => out.push_str(&da.to_string()),
            'j' => out.push_str(&format!("{:03}", day_of_year(y, mo, da))),
            'B' => out.push_str(MONTHS[mo - 1]),
            'b' => out.push_str(&MONTHS[mo - 1][..3]),
            'A' => out.push_str(WEEKDAYS[weekday(y, mo, da)]),
            'a' => out.push_str(&WEEKDAYS[weekday(y, mo, da)][..3]),
            'H' | 'M' | 'S' => out.push_str("00"),
            '%' => out.push('%'),
            other => {
                out.push('%');
                out.push(other);
            }
        }
    }
}

impl Parser<'_, '_> {
    /// Reads ".. |name| directive:: data". The name may continue on the following lines
    /// of the block, and the directive may start on the line after the name. Only
    /// directives that produce inline content are allowed.
    pub(crate) fn substitution_def(
        &mut self,
        lines: &[Line<'_>],
        i: usize,
        col: usize,
    ) -> Result<usize, Fail> {
        let blk = indented_block(lines, i, Some(col), false, false);
        let consumed = blk.consumed.max(1);
        let span = self.lines_span(&lines[i..i + consumed]);
        let Some((cl, cc)) = substitution_close(lines, i, col, consumed) else {
            self.report_at(SUBSTITUTION_INVALID, span);
            return Ok(consumed);
        };
        let name = substitution_name(lines, i, col, cl, cc);
        let t = lines[cl].text;
        let (mut j, mut dc) = (cl, cc + 1 + indent_of(&t[(cc + 1).min(t.len())..]));
        if dc >= t.len() {
            j = cl + 1;
            while j < i + consumed && lines[j].is_blank() {
                j += 1;
            }
            if j >= i + consumed {
                self.report_at(SUBSTITUTION_INVALID, span);
                return Ok(consumed);
            }
            dc = indent_of(lines[j].text);
        }
        let Some((used, d)) = self.read_directive(lines, j, dc, false) else {
            self.report_at(SUBSTITUTION_INVALID, span);
            return Ok(consumed);
        };
        if let Some(mut d) = d {
            d.subst = true;
            d.subst_name = name.clone();
            self.emit_substitution(&d, &name, span)?;
        }
        Ok(consumed.max(j - i + used))
    }

    fn emit_substitution(
        &mut self,
        d: &Directive<'_>,
        name: &str,
        span: SourceSpan,
    ) -> Result<(), Fail> {
        let kind = d.standard().map(|s| s.kind);
        let Some(kind) = kind.filter(|k| {
            matches!(
                k,
                DirKind::Replace
                    | DirKind::Unicode
                    | DirKind::Date
                    | DirKind::Image
                    | DirKind::Raw
            )
        }) else {
            self.report_at(SUBSTITUTION_DIRECTIVE, d.name_span);
            return Ok(());
        };
        if kind == DirKind::Image && !self.check_image_align(d) {
            return Ok(());
        }
        if kind == DirKind::Replace && d.content.iter().any(|l| l.is_blank()) {
            let span = self.lines_span(&d.content);
            self.report_at(SUBSTITUTION_REPLACE, span);
            return Ok(());
        }
        let pending = self.st.pending.take();
        let r = self.substitution_node(d, kind, name, span);
        self.st.pending = pending;
        r
    }

    fn substitution_node(
        &mut self,
        d: &Directive<'_>,
        kind: DirKind,
        name: &str,
        span: SourceSpan,
    ) -> Result<(), Fail> {
        let id = self.ctx.b.begin_node(NodeKind::SubstitutionDefinition, span)?;
        let text = self.pool_name(name)?;
        self.ctx.b.set_text(id, text);
        if kind == DirKind::Unicode {
            let trim = d.has_option("trim");
            if trim || d.has_option("ltrim") {
                self.custom(id, "ltrim", "1")?;
            }
            if trim || d.has_option("rtrim") {
                self.custom(id, "rtrim", "1")?;
            }
        }
        match kind {
            DirKind::Replace => self.inline(&d.content)?,
            DirKind::Unicode => self.unicode_text(d)?,
            DirKind::Date => self.date_text(d)?,
            DirKind::Raw => self.raw_directive(d)?,
            _ => {
                let target = self.open_image_target(d, d.span)?;
                self.emit_image(d, d.span, false)?;
                if let Some(target) = target {
                    self.close(target)?;
                }
            }
        }
        self.close(id)
    }

    /// Writes the characters of a unicode directive; ".. " starts a comment.
    fn unicode_text(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        let joined = join_lines(&d.arg, " ");
        let mut s: &str = &joined;
        let b = s.as_bytes();
        for k in 0..b.len().saturating_sub(1) {
            if &b[k..k + 2] == b".."
                && (k == 0 || b[k - 1] == b' ')
                && (k + 2 == b.len() || b[k + 2] == b' ')
            {
                s = &s[..k];
                break;
            }
        }
        let span = self.lines_span(&d.arg);
        let mut out = String::new();
        for word in s.split(|c: char| c.is_ascii_whitespace()).filter(|w| !w.is_empty()) {
            match unicode_code(word) {
                None => out.push_str(word),
                Some(v) => match char::from_u32(v) {
                    Some(c) => out.push(c),
                    None => self.report_at(UNICODE_INVALID, span),
                },
            }
        }
        self.ctx.b.add_text_leaf(NodeKind::Text, span, &out)?;
        Ok(())
    }

    /// Formats `ReadOptions::today` with strftime-style codes; without a date it warns
    /// and keeps the format as written.
    fn date_text(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        let format = if d.arg.is_empty() { "%Y-%m-%d".into() } else { join_lines(&d.arg, "\n") };
        let mut out = String::new();
        match parse_date(self.ctx.options.today) {
            Some(date) => format_date(&mut out, &format, date),
            None => {
                self.report_at(DATE_NO_TODAY, d.span);
                out.push_str(&format);
            }
        }
        self.ctx.b.add_text_leaf(NodeKind::Text, d.span, &out)?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_dates() {
        assert_eq!(unicode_code("U+00A9"), Some(0xa9));
        assert_eq!(unicode_code("&#x2014;"), Some(0x2014));
        assert_eq!(unicode_code("169"), Some(169));
        assert_eq!(unicode_code("copy"), None);
        let mut s = String::new();
        format_date(&mut s, "%A %d %B %Y, day %j", parse_date("2026-09-29").unwrap());
        assert_eq!(s, "Tuesday 29 September 2026, day 272");
    }
}
