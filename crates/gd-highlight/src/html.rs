//! HTML output as Pygments' HtmlFormatter writes it inside `<div class="highlight"><pre>`:
//! each token in `<span class="k">...</span>`, Text without a span, neighbours of one class
//! in one span, and no span across a line break. Newlines stay plain text.

use std::fmt::Write as _;

use crate::lexers::Lexer;
use crate::scanner::scan_tokens;
use crate::token::{Token, kind_class, token_text};

/// Numbers and emphasizes lines as Pygments does for Sphinx's `:linenos:` and
/// `:emphasize-lines:` (`linenos="inline"`, `hl_lines`).
///
/// Each line starts with `<span class="linenos"> 7</span>`, and an emphasized line, number
/// included, is wrapped in `<span class="hll">...\n</span>`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HtmlLines<'a> {
    /// The first line's number; 0 leaves lines unnumbered.
    pub first: usize,
    /// The 1-based numbers of the lines to emphasize, in any order.
    pub emphasized: &'a [usize],
}

/// Carries the formatter's state between tokens, so tokens can be written as they are
/// scanned.
struct HtmlWriter<'o, 'l> {
    out: &'o mut String,
    /// The class of the span open on the current line.
    open: &'static str,
    /// Whether the current line has content.
    line: bool,
    lines: HtmlLines<'l>,
    /// The current line's number, from 1.
    number: usize,
    /// Digits of the last line number.
    width: usize,
    /// Whether the current line's prefix is written.
    started: bool,
}

/// Appends the HTML of `tokens`, which cover `code`, to `out`.
pub fn write_html(tokens: &[Token], code: &str, out: &mut String) {
    let mut w = HtmlWriter::new(out, HtmlLines::default(), code);
    for &t in tokens {
        w.token(kind_class(t.kind), token_text(t, code));
    }
    w.finish();
}

/// Lexes `code` with `lexer` and appends its HTML to `out`, without a token buffer.
pub fn write_code_html(lexer: Lexer, code: &str, out: &mut String) {
    write_code_html_lines(lexer, code, HtmlLines::default(), out);
}

/// Appends the HTML of `tokens` with numbered or emphasized lines. The whole block is
/// lexed at once, so a string or comment across lines keeps its colour on every line.
pub fn write_html_lines(tokens: &[Token], code: &str, lines: HtmlLines<'_>, out: &mut String) {
    let mut w = HtmlWriter::new(out, lines, code);
    for &t in tokens {
        w.token(kind_class(t.kind), token_text(t, code));
    }
    w.finish();
}

/// Lexes `code` and appends it as [`write_html_lines`] does.
pub fn write_code_html_lines(lexer: Lexer, code: &str, lines: HtmlLines<'_>, out: &mut String) {
    let mut w = HtmlWriter::new(out, lines, code);
    for t in scan_tokens(lexer, code) {
        w.token(kind_class(t.kind), token_text(t, code));
    }
    w.finish();
}

fn digit_count(n: usize) -> usize {
    n.checked_ilog10().map_or(1, |d| d as usize + 1)
}

impl<'o, 'l> HtmlWriter<'o, 'l> {
    fn new(out: &'o mut String, lines: HtmlLines<'l>, code: &str) -> HtmlWriter<'o, 'l> {
        let mut count = code.bytes().filter(|&b| b == b'\n').count();
        if !code.is_empty() && !code.ends_with('\n') {
            count += 1;
        }
        let last = (lines.first + count).saturating_sub(1).max(1);
        HtmlWriter {
            out,
            open: "",
            line: false,
            lines,
            number: 1,
            width: digit_count(last),
            started: false,
        }
    }

    fn decorates(&self) -> bool {
        self.lines.first > 0 || !self.lines.emphasized.is_empty()
    }

    fn emphasized(&self) -> bool {
        self.lines.emphasized.contains(&self.number)
    }

    /// Starts a line: the hll span of an emphasized line, then its number.
    fn line_prefix(&mut self) {
        if self.started || !self.decorates() {
            return;
        }
        self.started = true;
        if self.emphasized() {
            self.out.push_str("<span class=\"hll\">");
        }
        if self.lines.first > 0 {
            self.out.push_str("<span class=\"linenos\">");
            let n = self.lines.first + self.number - 1;
            for _ in digit_count(n)..self.width {
                self.out.push(' ');
            }
            // Writing to a String cannot fail.
            let _ = write!(self.out, "{n}");
            self.out.push_str("</span>");
        }
    }

    /// Ends a line after its newline, or after the code's last byte.
    fn line_suffix(&mut self) {
        if !self.started {
            return;
        }
        if self.emphasized() {
            self.out.push_str("</span>");
        }
        self.started = false;
        self.number += 1;
    }

    /// Writes one token, splitting it at newlines as Pygments does.
    fn token(&mut self, class: &'static str, value: &str) {
        let mut text = value;
        while let Some(nl) = text.find('\n') {
            self.line_prefix();
            self.line_part(class, &text[..nl]);
            self.out.push('\n');
            self.line_suffix();
            text = &text[nl + 1..];
        }
        if text.is_empty() {
            return;
        }
        self.line_prefix();
        if !self.line {
            open_span(self.out, class);
            self.open = class;
            self.line = true;
        } else if self.open != class {
            close_span(self.out, self.open);
            open_span(self.out, class);
            self.open = class;
        }
        push_html(self.out, text);
    }

    /// Writes the part of a token that ends a line.
    fn line_part(&mut self, class: &'static str, part: &str) {
        if self.line {
            if self.open != class && !part.is_empty() {
                close_span(self.out, self.open);
                open_span(self.out, class);
                push_html(self.out, part);
                close_span(self.out, class);
            } else {
                push_html(self.out, part);
                close_span(self.out, self.open);
            }
            self.line = false;
        } else if !part.is_empty() {
            open_span(self.out, class);
            push_html(self.out, part);
            close_span(self.out, class);
        }
    }

    /// Closes the last line's span. Unlike Pygments, which adds a final newline to its
    /// input, it writes no newline the code does not have.
    fn finish(&mut self) {
        if self.line {
            close_span(self.out, self.open);
        }
        self.line = false;
        self.line_suffix();
    }
}

fn open_span(out: &mut String, class: &str) {
    if !class.is_empty() {
        out.push_str("<span class=\"");
        out.push_str(class);
        out.push_str("\">");
    }
}

fn close_span(out: &mut String, class: &str) {
    if !class.is_empty() {
        out.push_str("</span>");
    }
}

/// Appends `text` escaped as Pygments escapes it: `& < > " '`.
pub fn push_html(out: &mut String, text: &str) {
    let mut run = 0;
    for (i, b) in text.bytes().enumerate() {
        let replacement = match b {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'"' => "&quot;",
            b'\'' => "&#39;",
            _ => continue,
        };
        out.push_str(&text[run..i]);
        out.push_str(replacement);
        run = i + 1;
    }
    out.push_str(&text[run..]);
}
