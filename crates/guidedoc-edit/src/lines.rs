//! Line arithmetic on source text: every function takes byte offsets on character
//! boundaries and returns offsets on line boundaries of the same text.

use crate::Span;

/// The offset where the line holding `at` starts.
pub(crate) fn line_start(text: &str, at: usize) -> usize {
    text[..at.min(text.len())].rfind('\n').map_or(0, |i| i + 1)
}

/// The offset of the line break ending the line holding `at`, or the text's length.
pub(crate) fn line_end(text: &str, at: usize) -> usize {
    let at = at.min(text.len());
    text[at..].find('\n').map_or(text.len(), |i| at + i)
}

/// The end of the line holding `at` without a trailing carriage return.
pub(crate) fn line_end_trimmed(text: &str, at: usize) -> usize {
    let end = line_end(text, at);
    if end > 0 && text.as_bytes()[end - 1] == b'\r' { end - 1 } else { end }
}

/// The offset where the next line starts, or the text's length on the last line.
pub(crate) fn next_line(text: &str, at: usize) -> usize {
    let end = line_end(text, at);
    if end < text.len() { end + 1 } else { end }
}

/// The start of the line before the line holding `at`; `None` on the first line.
pub(crate) fn previous_line(text: &str, at: usize) -> Option<usize> {
    let start = line_start(text, at);
    if start == 0 { None } else { Some(line_start(text, start - 1)) }
}

/// The line holding `at`, without its line break.
pub(crate) fn line_at(text: &str, at: usize) -> &str {
    &text[line_start(text, at)..line_end_trimmed(text, at)]
}

/// Tells whether `line` holds only white space.
pub(crate) fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

/// The number of leading spaces and tabs of `line`, in bytes.
pub(crate) fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches([' ', '\t']).len()
}

/// The starts of the lines that `span` touches, in order.
pub(crate) fn line_starts(text: &str, span: Span) -> Vec<usize> {
    let mut out = Vec::new();
    let mut at = line_start(text, span.start);
    while at < span.end.max(at + 1) && at <= text.len() {
        out.push(at);
        let next = next_line(text, at);
        if next == at || next > span.end {
            break;
        }
        at = next;
    }
    out
}

/// `span` without the white space at its end.
pub(crate) fn trim_end(text: &str, span: Span) -> Span {
    let kept = text[span.start..span.end].trim_end();
    Span { start: span.start, end: span.start + kept.len() }
}

/// The offset after the line breaks and blank lines that follow `at`, stopping at the
/// start of the next non-blank line (or the text's end).
pub(crate) fn skip_blank_lines(text: &str, at: usize) -> usize {
    let mut at = next_line(text, at);
    while at < text.len() && is_blank(line_at(text, at)) {
        at = next_line(text, at);
    }
    at
}

/// `prefix` with every character but white space (and `>` in Markdown quotes) turned
/// into a space: the indentation that continues a block started after `prefix`.
pub(crate) fn continuation(prefix: &str, keep_quotes: bool) -> String {
    prefix.chars().map(|c| if c == '\t' || (keep_quotes && c == '>') { c } else { ' ' }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_and_indentation() {
        let text = "one\n  two\r\n\nfour";
        assert_eq!((line_start(text, 6), line_end(text, 6)), (4, 10));
        assert_eq!(line_end_trimmed(text, 6), 9);
        assert_eq!(line_at(text, 5), "  two");
        assert_eq!(previous_line(text, 6), Some(0));
        assert_eq!(skip_blank_lines(text, 5), 12);
        assert_eq!(line_starts(text, Span { start: 1, end: 13 }), vec![0, 4, 11, 12]);
        assert_eq!(indent_of("  \tx"), 3);
        assert_eq!(continuation("> - ", true), ">   ");
        assert_eq!(continuation("#. ", false), "   ");
    }
}
