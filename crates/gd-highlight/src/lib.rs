//! Syntax highlighting with Pygments' token classes (`k`, `nf`, `s2`, `c1`) and
//! stylesheets, for 35 languages, without a Python runtime.
//!
//! The common path is one call:
//!
//! ```
//! let html = gd_highlight::highlight_html("def f(x):\n    return x\n", "python");
//! assert!(html.starts_with("<span class=\"k\">def</span>"));
//! ```
//!
//! The staged path finds a lexer ([`find_lexer`]), resolves Sphinx's pseudo languages
//! ([`resolve_lexer`]), scans tokens ([`scan_tokens`], [`tokenize_into`]) and writes them
//! ([`write_html`], [`write_typst`]) into a caller's `String`, which can be reused.
//! Styles ([`Style`], [`find_style`], [`write_css`], [`write_typst_theme`]) and problems
//! ([`check_code`], [`write_problem`]) complete the surface. Nothing here keeps global
//! state; the same input always gives the same output.

mod code;
mod code_tables;
mod data;
mod docs;
mod html;
mod lexers;
mod markup;
mod misc;
mod problems;
mod python;
mod scanner;
mod shell;
mod style_tables;
mod styles;
mod tex;
mod token;
mod typst;

#[cfg(test)]
mod tests;

pub use html::{
    HtmlLines, push_html, write_code_html, write_code_html_lines, write_html, write_html_lines,
};
pub use lexers::{
    Lexer, LexerInfo, find_lexer, find_lexer_for_filename, lexer_alias, lexer_info, lexer_name,
    resolve_lexer, suggest_lexer,
};
pub use misc::guess_lexer;
pub use problems::{Problem, ProblemKind, check_code, check_style, write_problem};
pub use scanner::{Scanner, count_tokens, has_errors, scan_tokens, tokenize, tokenize_into};
pub use style_tables::Style;
pub use styles::{
    Color, Look, StyleInfo, StyleLooks, StyleRule, find_style, kind_look, resolve_style,
    style_info, style_name, write_css,
};
pub use token::{Token, TokenKind, kind_class, kind_is, kind_name, kind_parent, token_text};
pub use typst::{push_typst_string, write_code_typst, write_typst, write_typst_theme};

/// Highlights `code` as `language` (an alias, file name or MIME type) and returns
/// Pygments' HTML spans: what goes inside `<div class="highlight"><pre>`.
///
/// The language is resolved as Sphinx resolves it; an unknown language is written as
/// escaped plain text (use [`check_code`] to report it).
pub fn highlight_html(code: &str, language: &str) -> String {
    let mut out = String::with_capacity(code.len() * 2);
    write_code_html(language_lexer(language), code, &mut out);
    out
}

/// Highlights `code` as `language` and returns the Typst array [`write_typst`] writes.
///
/// An unknown language is written as plain text.
pub fn highlight_typst(code: &str, language: &str) -> String {
    let mut out = String::with_capacity(code.len() * 2);
    write_code_typst(language_lexer(language), code, &mut out);
    out
}

/// Returns the stylesheet [`write_css`] writes for `style` with `selector`.
pub fn style_css(style: Style, selector: &str) -> String {
    let mut out = String::new();
    write_css(style, selector, &mut out);
    out
}

/// The lexer for a language name; plain text when the name is unknown.
fn language_lexer(language: &str) -> Lexer {
    find_lexer(language).unwrap_or(Lexer::Text)
}
