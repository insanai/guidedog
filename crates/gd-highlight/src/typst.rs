//! Typst output.
//!
//! [`write_typst`] writes highlighted code as Typst data, an array of lines that each hold
//! `(class, text)` pairs:
//!
//! ```text
//! ((("k", "def"), ("w", " "), ("nf", "f"), ("p", "():"), ), (("", "    "), ...), )
//! ```
//!
//! and [`write_typst_theme`] writes a style as a dictionary from class to text settings,
//! so a Typst function can set the same colours the HTML stylesheet sets. Text never holds
//! a newline and control characters are escaped, so the output is always a valid Typst
//! expression and no code can change the document.

use std::fmt::Write as _;

use crate::lexers::Lexer;
use crate::scanner::scan_tokens;
use crate::style_tables::Style;
use crate::styles::{Look, is_standard, resolve_style, style_info};
use crate::token::{Token, TokenKind, kind_class, kind_parent, token_text};

/// Appends `tokens` of `code` to `out` as a Typst array of lines. A final newline does not
/// start another line.
pub fn write_typst(tokens: &[Token], code: &str, out: &mut String) {
    let mut w = TypstWriter { out, open: false };
    w.out.push('(');
    for &t in tokens {
        w.token(typst_class(t.kind), token_text(t, code));
    }
    w.finish();
}

/// Lexes `code` with `lexer` and appends its Typst array to `out`, without a token buffer.
pub fn write_code_typst(lexer: Lexer, code: &str, out: &mut String) {
    let mut w = TypstWriter { out, open: false };
    w.out.push('(');
    for t in scan_tokens(lexer, code) {
        w.token(typst_class(t.kind), token_text(t, code));
    }
    w.finish();
}

struct TypstWriter<'o> {
    out: &'o mut String,
    /// Whether a line's array is open.
    open: bool,
}

impl TypstWriter<'_> {
    fn token(&mut self, class: &str, value: &str) {
        let mut text = value;
        while !text.is_empty() {
            let nl = text.find('\n');
            let piece = &text[..nl.unwrap_or(text.len())];
            if !piece.is_empty() {
                if !self.open {
                    self.out.push('(');
                    self.open = true;
                }
                self.out.push_str("(\"");
                self.out.push_str(class);
                self.out.push_str("\", ");
                push_typst_string(self.out, piece);
                self.out.push_str("), ");
            }
            let Some(nl) = nl else { break };
            self.out.push_str(if self.open { "), " } else { "(), " });
            self.open = false;
            text = &text[nl + 1..];
        }
    }

    fn finish(&mut self) {
        if self.open {
            self.out.push_str("), ");
        }
        self.out.push(')');
    }
}

/// The class Typst output uses: the standard class for kinds outside Pygments' standard
/// set, so a theme needs only the standard classes.
fn typst_class(kind: TokenKind) -> &'static str {
    let mut k = kind;
    while !is_standard(k) {
        match kind_parent(k) {
            Some(parent) => k = parent,
            None => break,
        }
    }
    kind_class(k)
}

/// Appends `text` as a Typst string literal.
pub fn push_typst_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0}'..='\u{1f}' | '\u{7f}' => {
                // Writing to a String cannot fail.
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            _ => out.push(c),
        }
    }
    out.push('"');
}

/// Appends `style` to `out` as a Typst dictionary: `"background"` is the block's fill,
/// `"text"` the default text settings, and each class maps to its text settings (`fill`,
/// `weight`, `style`), with `underline`, `background` and `border` where set.
pub fn write_typst_theme(style: Style, out: &mut String) {
    let looks = resolve_style(style);
    out.push_str("(\"background\": rgb(\"");
    out.push_str(style_info(style).background);
    out.push_str("\"), \"text\": ");
    push_typst_look(out, looks.look(TokenKind::Text));
    for kind in TokenKind::ALL {
        let look = looks.look(kind);
        if kind == TokenKind::Text || !is_standard(kind) || !look.is_styled() {
            continue;
        }
        out.push_str(", \"");
        out.push_str(kind_class(kind));
        out.push_str("\": ");
        push_typst_look(out, look);
    }
    out.push(')');
}

fn push_typst_look(out: &mut String, look: Look) {
    out.push_str("(fill: ");
    match look.color {
        Some(c) => push_rgb(out, c.as_str()),
        None => out.push_str("none"),
    }
    if look.bold {
        out.push_str(", weight: \"bold\"");
    }
    if look.italic {
        out.push_str(", style: \"italic\"");
    }
    if look.underline {
        out.push_str(", underline: true");
    }
    if let Some(c) = look.background {
        out.push_str(", background: ");
        push_rgb(out, c.as_str());
    }
    if let Some(c) = look.border {
        out.push_str(", border: ");
        push_rgb(out, c.as_str());
    }
    out.push(')');
}

fn push_rgb(out: &mut String, hex: &str) {
    out.push_str("rgb(\"#");
    out.push_str(hex);
    out.push_str("\")");
}
