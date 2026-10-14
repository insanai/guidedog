//! Problems a documentation build reports about code blocks, as Sphinx warns about them,
//! with Elm-style reports that say what happened and what to do.

use std::fmt;

use crate::lexers::{Lexer, edit_distance, find_lexer, resolve_lexer, suggest_lexer};
use crate::scanner::scan_tokens;
use crate::style_tables::Style;
use crate::styles::{find_style, style_info};
use crate::token::TokenKind;

/// What is wrong with a code block or a style name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemKind {
    /// Sphinx: "Pygments lexer name 'x' is not known".
    UnknownLanguage,
    /// `pygments_style` names no style.
    UnknownStyle,
    /// Sphinx: "Lexing literal_block ... resulted in an error".
    LexingError,
}

/// A problem with a code block or a style name. It borrows the name and the code it was
/// found in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Problem<'a> {
    /// What is wrong.
    pub kind: ProblemKind,
    /// The language or style as written.
    pub name: &'a str,
    /// The lexer the code was lexed with (`LexingError`).
    pub lexer: Lexer,
    /// A close name, for "Did you mean".
    pub suggestion: Option<&'static str>,
    /// `LexingError`: the 1-based line of the first Error token.
    pub line: usize,
    /// `LexingError`: its 1-based column, in characters.
    pub column: usize,
    /// `LexingError`: the line it is on.
    pub excerpt: &'a str,
    /// `LexingError`: its text, up to the line's end.
    pub near: &'a str,
}

impl<'a> Problem<'a> {
    fn new(kind: ProblemKind, name: &'a str) -> Problem<'a> {
        Problem {
            kind,
            name,
            lexer: Lexer::Text,
            suggestion: None,
            line: 0,
            column: 0,
            excerpt: "",
            near: "",
        }
    }

    /// The report's title: `"UNKNOWN LANGUAGE"`.
    pub fn title(&self) -> &'static str {
        match self.kind {
            ProblemKind::UnknownLanguage => "UNKNOWN LANGUAGE",
            ProblemKind::UnknownStyle => "UNKNOWN STYLE",
            ProblemKind::LexingError => "CODE DOES NOT LEX",
        }
    }
}

/// Finds what Sphinx would warn about when it highlights `code` as `language`: an unknown
/// language, or code that does not lex as the language it is marked as. `None` when there
/// is nothing to report; blocks in the `default` language never warn, since they fall
/// back to plain text.
pub fn check_code<'a>(language: &'a str, code: &'a str) -> Option<Problem<'a>> {
    if language.trim().is_empty() {
        return None;
    }
    let Some(lexer) = find_lexer(language) else {
        let mut p = Problem::new(ProblemKind::UnknownLanguage, language);
        p.suggestion = suggest_lexer(language);
        return Some(p);
    };
    if lexer == Lexer::Default {
        return None;
    }
    let lexer = resolve_lexer(lexer, code);
    let error = scan_tokens(lexer, code).find(|t| t.kind == TokenKind::Error)?;
    let start = error.start as usize;
    let line_start = code[..start].rfind('\n').map_or(0, |i| i + 1);
    let line_stop = code[line_start..].find('\n').map_or(code.len(), |i| line_start + i);
    let mut p = Problem::new(ProblemKind::LexingError, language);
    p.lexer = lexer;
    p.line = 1 + code[..start].bytes().filter(|&b| b == b'\n').count();
    p.column = 1 + code[line_start..start].chars().count();
    p.excerpt = &code[line_start..line_stop];
    p.near = &code[start..(error.end as usize).min(line_stop).max(start)];
    Some(p)
}

/// Reports a `pygments_style` that names no style; `None` when it names one.
pub fn check_style(name: &str) -> Option<Problem<'_>> {
    if find_style(name).is_some() {
        return None;
    }
    let mut best = None;
    let mut best_distance = 4;
    for style in Style::ALL {
        let candidate = style_info(style).name;
        let d = edit_distance(name.as_bytes(), candidate.as_bytes());
        if d < best_distance {
            best = Some(candidate);
            best_distance = d;
        }
    }
    let mut p = Problem::new(ProblemKind::UnknownStyle, name);
    p.suggestion = best;
    Some(p)
}

/// Appends an Elm-style report of `problem` to `out`; `location`, such as
/// `"index.rst:12"`, heads it unless it is empty.
///
/// ```text
/// -- UNKNOWN LANGUAGE ----------------------------------------------- index.rst:12
///
/// I do not know how to highlight "pyhton", so this block is shown as plain text.
///
/// Hint: Did you mean "python"?
/// ```
pub fn write_problem(problem: &Problem<'_>, location: &str, out: &mut String) {
    let title = problem.title();
    out.push_str("-- ");
    out.push_str(title);
    out.push(' ');
    let used = title.len() + if location.is_empty() { 0 } else { location.len() + 1 };
    for _ in 0..76usize.saturating_sub(used).max(3) {
        out.push('-');
    }
    if !location.is_empty() {
        out.push(' ');
        out.push_str(location);
    }
    out.push_str("\n\n");
    match problem.kind {
        ProblemKind::UnknownLanguage => {
            push_wrapped(
                out,
                &[
                    "I do not know how to highlight \"",
                    problem.name,
                    "\", so this block is shown as plain text.",
                ],
            );
            push_hint(
                out,
                problem.suggestion,
                "Use one of the languages in the highlight \
                README, or \"text\" for no highlighting.",
            );
        }
        ProblemKind::UnknownStyle => {
            push_wrapped(
                out,
                &["pygments_style is \"", problem.name, "\", which is not a style I have."],
            );
            push_hint(
                out,
                problem.suggestion,
                "The styles are Pygments' built-in ones, such \
                as default, friendly, monokai, github-dark and solarized-dark, and Sphinx's \
                sphinx and none.",
            );
        }
        ProblemKind::LexingError => {
            push_wrapped(
                out,
                &[
                    "This block is marked as \"",
                    problem.name,
                    "\", but it does \
                not lex as that language, so the part that does not is marked as an error:",
                ],
            );
            push_excerpt(out, problem);
            out.push_str(
                "\nHint: Check the block's language, or mark it \"text\" to show it \
                without highlighting.\n",
            );
        }
    }
}

impl fmt::Display for Problem<'_> {
    /// The report without a location.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut report = String::new();
        write_problem(self, "", &mut report);
        f.write_str(&report)
    }
}

impl std::error::Error for Problem<'_> {}

fn push_hint(out: &mut String, suggestion: Option<&str>, otherwise: &str) {
    match suggestion {
        Some(s) => {
            out.push_str("\nHint: Did you mean \"");
            out.push_str(s);
            out.push_str("\"?\n");
        }
        None => {
            out.push_str("\nHint: ");
            out.push_str(otherwise);
            out.push('\n');
        }
    }
}

/// Appends a sentence made of `parts`, wrapped at 80 columns.
fn push_wrapped(out: &mut String, parts: &[&str]) {
    let mut column = 0;
    let mut space = false;
    for part in parts {
        let mut rest = *part;
        while !rest.is_empty() {
            if let Some(after) = rest.strip_prefix(' ') {
                space = true;
                rest = after;
                continue;
            }
            let end = rest.find(' ').unwrap_or(rest.len());
            let width = rest[..end].chars().count();
            if space {
                let wrap = column + 1 + width > 80;
                out.push(if wrap { '\n' } else { ' ' });
                column = if wrap { 0 } else { column + 1 };
                space = false;
            }
            out.push_str(&rest[..end]);
            column += width;
            rest = &rest[end..];
        }
    }
    out.push('\n');
}

fn push_excerpt(out: &mut String, p: &Problem<'_>) {
    let number = p.line.to_string();
    out.push('\n');
    out.push_str(&number);
    out.push_str("| ");
    out.push_str(p.excerpt);
    out.push('\n');
    for _ in 1..number.len() + 2 + p.column {
        out.push(' ');
    }
    for _ in 0..p.near.chars().count().max(1) {
        out.push('^');
    }
    out.push('\n');
}
