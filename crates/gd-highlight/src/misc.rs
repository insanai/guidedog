//! Plain text, diffs, and guessing a language from code.

use crate::lexers::Lexer;
use crate::scanner::{Scanner, State, has, has_errors, trim_spaces};
use crate::token::TokenKind as K;

/// Emits everything as Text: the "text" and "none" lexers.
pub(crate) fn text_step(s: &mut Scanner<'_>, _st: &mut State) {
    s.emit(s.limit, K::Text);
}

/// Lexes unified and context diffs line by line, as Pygments' diff lexer does.
pub(crate) fn diff_step(s: &mut Scanner<'_>, _st: &mut State) {
    if !s.at_line_start() {
        s.emit(s.line_after(s.pos), K::Whitespace);
        return;
    }
    let stop = s.line_end(s.pos);
    let line = s.slice(s.pos, stop);
    let starts = |prefixes: &[&str]| prefixes.iter().any(|p| line.starts_with(p.as_bytes()));
    let kind = if line.starts_with(b" ") {
        s.emit_n(1, K::Whitespace);
        K::Text
    } else if line.starts_with(b"!") || trim_right_cr(line) == b"---" {
        K::GenericStrong
    } else if starts(&["< ", "-"]) {
        K::GenericDeleted
    } else if starts(&["> ", "+"]) {
        K::GenericInserted
    } else if line.starts_with(b"@") || normal_diff_command(line) {
        K::GenericSubheading
    } else if starts(&["Index", "index", "diff", "="]) {
        K::GenericHeading
    } else {
        K::Text
    };
    s.emit(stop, kind);
    s.emit(s.line_after(s.pos), K::Whitespace);
}

fn trim_right_cr(line: &[u8]) -> &[u8] {
    line.strip_suffix(b"\r").unwrap_or(line)
}

/// Matches the commands of a normal diff: `3a4`, `5,7c5,6`, `10d9`.
fn normal_diff_command(line: &[u8]) -> bool {
    let Some(i) = range_end(line, 0) else { return false };
    if i >= line.len() || !has("acd", line[i]) {
        return false;
    }
    range_end(line, i + 1) == Some(trim_right_cr(line).len())
}

/// The end of a line range (`5` or `5,7`) at `from`.
fn range_end(line: &[u8], from: usize) -> Option<usize> {
    let digits_end = |from: usize| {
        let n = line.get(from..)?.iter().take_while(|c| c.is_ascii_digit()).count();
        if n == 0 { None } else { Some(from + n) }
    };
    let i = digits_end(from)?;
    if line.get(i) == Some(&b',') { digits_end(i + 1) } else { Some(i) }
}

// Guessing. Each rule adds its weight when its text appears where it says; the lexer with
// the highest total wins, and Text wins when nothing matches.

#[derive(Debug, Clone, Copy)]
enum Where {
    /// At the start of the code.
    Start,
    /// At the start of any line.
    LineStart,
    Anywhere,
}

use Where::{Anywhere, LineStart, Start};

#[derive(Debug)]
struct GuessRule {
    lexer: Lexer,
    place: Where,
    text: &'static str,
    weight: u32,
}

const fn rule(lexer: Lexer, place: Where, text: &'static str, weight: u32) -> GuessRule {
    GuessRule { lexer, place, text, weight }
}

#[rustfmt::skip]
const GUESS_RULES: &[GuessRule] = &[
    rule(Lexer::Bash, Start, "#!/bin/bash", 20), rule(Lexer::Bash, Start, "#!/bin/sh", 20),
    rule(Lexer::Bash, Start, "#!/usr/bin/env bash", 20),
    rule(Lexer::Bash, Start, "#!/usr/bin/env sh", 20),
    rule(Lexer::Python, Start, "#!/usr/bin/env python", 20),
    rule(Lexer::Python, Start, "#!/usr/bin/python", 20),
    rule(Lexer::Pycon, Start, ">>> ", 20),
    rule(Lexer::Pytb, Start, "Traceback (most recent call last):", 20),
    rule(Lexer::Xml, Start, "<?xml", 20), rule(Lexer::Html, Start, "<!DOCTYPE html", 20),
    rule(Lexer::Html, Start, "<!doctype html", 20), rule(Lexer::Html, Start, "<html", 15),
    rule(Lexer::Html, Anywhere, "</div>", 3), rule(Lexer::Html, Anywhere, "<p>", 2),
    rule(Lexer::Diff, Start, "diff --git", 20), rule(Lexer::Diff, LineStart, "--- ", 3),
    rule(Lexer::Diff, LineStart, "+++ ", 3), rule(Lexer::Diff, LineStart, "@@ -", 6),
    rule(Lexer::Console, Start, "$ ", 8), rule(Lexer::Console, LineStart, "$ ", 3),
    rule(Lexer::Python, LineStart, "def ", 4), rule(Lexer::Python, LineStart, "import ", 2),
    rule(Lexer::Python, LineStart, "from ", 2), rule(Lexer::Python, Anywhere, "self.", 2),
    rule(Lexer::Python, Anywhere, "):\n", 2), rule(Lexer::Python, Anywhere, "elif ", 3),
    rule(Lexer::Python, Anywhere, "print(", 1), rule(Lexer::Python, Anywhere, "    def ", 3),
    rule(Lexer::C, LineStart, "#include", 6), rule(Lexer::C, Anywhere, "printf(", 2),
    rule(Lexer::C, Anywhere, "int main(", 3),
    rule(Lexer::Cpp, Anywhere, "std::", 6), rule(Lexer::Cpp, LineStart, "template", 3),
    rule(Lexer::Cpp, LineStart, "namespace ", 3),
    rule(Lexer::Cpp, Anywhere, "#include <iostream>", 6),
    rule(Lexer::Go, LineStart, "package ", 3), rule(Lexer::Go, LineStart, "func ", 4),
    rule(Lexer::Go, Anywhere, ":= ", 1), rule(Lexer::Go, Anywhere, "fmt.", 3),
    rule(Lexer::Rust, LineStart, "fn ", 4), rule(Lexer::Rust, Anywhere, "let mut ", 5),
    rule(Lexer::Rust, LineStart, "use std::", 6), rule(Lexer::Rust, LineStart, "impl ", 4),
    rule(Lexer::Rust, Anywhere, "println!", 5), rule(Lexer::Rust, Anywhere, "pub fn ", 5),
    rule(Lexer::Java, Anywhere, "public class ", 6),
    rule(Lexer::Java, Anywhere, "public static void", 6),
    rule(Lexer::Java, Anywhere, "System.out.", 6),
    rule(Lexer::Kotlin, LineStart, "fun ", 5), rule(Lexer::Kotlin, Anywhere, "val ", 2),
    rule(Lexer::Csharp, LineStart, "using System", 8),
    rule(Lexer::Csharp, Anywhere, "Console.Write", 6),
    rule(Lexer::Swift, LineStart, "import Foundation", 8),
    rule(Lexer::Swift, LineStart, "import SwiftUI", 8),
    rule(Lexer::Zig, Anywhere, "@import(\"std\")", 10),
    rule(Lexer::Odin, Anywhere, ":: proc", 8), rule(Lexer::Odin, LineStart, "package ", 1),
    rule(Lexer::Javascript, Anywhere, "function ", 2),
    rule(Lexer::Javascript, Anywhere, "console.log", 4),
    rule(Lexer::Javascript, Anywhere, "=> ", 2),
    rule(Lexer::Javascript, LineStart, "const ", 2),
    rule(Lexer::Javascript, Anywhere, "require(", 3),
    rule(Lexer::Typescript, LineStart, "interface ", 4),
    rule(Lexer::Typescript, Anywhere, ": string", 4),
    rule(Lexer::Typescript, Anywhere, ": number", 4),
    rule(Lexer::Yaml, Start, "---\n", 4), rule(Lexer::Yaml, LineStart, "- ", 1),
    rule(Lexer::Toml, LineStart, "[", 1), rule(Lexer::Toml, Anywhere, " = \"", 2),
    rule(Lexer::Ini, LineStart, "[", 1), rule(Lexer::Ini, LineStart, ";", 2),
    rule(Lexer::Sql, Anywhere, "SELECT ", 5), rule(Lexer::Sql, Anywhere, "select ", 2),
    rule(Lexer::Sql, Anywhere, "CREATE TABLE", 8), rule(Lexer::Sql, Anywhere, "INSERT INTO", 8),
    rule(Lexer::Sql, Anywhere, " FROM ", 2), rule(Lexer::Sql, Anywhere, " WHERE ", 3),
    rule(Lexer::Dockerfile, Start, "FROM ", 10),
    rule(Lexer::Dockerfile, LineStart, "RUN ", 5),
    rule(Lexer::Dockerfile, LineStart, "COPY ", 3),
    rule(Lexer::Makefile, LineStart, ".PHONY", 10), rule(Lexer::Makefile, Anywhere, "$(", 1),
    rule(Lexer::Makefile, Anywhere, ":\n\t", 4),
    rule(Lexer::Rst, LineStart, ".. ", 4), rule(Lexer::Rst, Anywhere, "::\n\n", 3),
    rule(Lexer::Rst, LineStart, "===", 2), rule(Lexer::Rst, Anywhere, "`_", 3),
    rule(Lexer::Markdown, LineStart, "# ", 3), rule(Lexer::Markdown, LineStart, "## ", 3),
    rule(Lexer::Markdown, LineStart, "```", 5), rule(Lexer::Markdown, Anywhere, "](", 3),
    rule(Lexer::Latex, Start, "\\documentclass", 20),
    rule(Lexer::Latex, Anywhere, "\\begin{", 5), rule(Lexer::Latex, Anywhere, "\\section{", 5),
    rule(Lexer::Latex, Anywhere, "\\usepackage", 8),
    rule(Lexer::Typst, LineStart, "#import ", 5), rule(Lexer::Typst, LineStart, "#set ", 6),
    rule(Lexer::Typst, LineStart, "#let ", 6), rule(Lexer::Typst, LineStart, "#show", 6),
    rule(Lexer::Dot, Start, "digraph", 15), rule(Lexer::Dot, Start, "graph ", 8),
    rule(Lexer::Dot, Start, "strict ", 5), rule(Lexer::Dot, Anywhere, " -> ", 2),
    rule(Lexer::Css, LineStart, "@media", 6), rule(Lexer::Css, Anywhere, "color:", 2),
    rule(Lexer::Css, Anywhere, "px;", 3), rule(Lexer::Css, LineStart, "}", 1),
    rule(Lexer::Powershell, Anywhere, "Write-Host", 8),
    rule(Lexer::Powershell, Anywhere, "$env:", 5), rule(Lexer::Powershell, Anywhere, "Get-", 3),
    rule(Lexer::Powershell, LineStart, "param(", 5),
];

/// Picks a lexer from the code alone, as Sphinx's "guess" language does: JSON that lexes
/// cleanly is JSON; otherwise a scored rule table decides, and [`Lexer::Text`] wins when
/// nothing matches.
pub fn guess_lexer(code: &str) -> Lexer {
    let first = trim_spaces(code.as_bytes());
    if matches!(first.first(), Some(b'{' | b'[')) && !has_errors(Lexer::Json, code) {
        return Lexer::Json;
    }
    let mut scores = [0u32; Lexer::ALL.len()];
    for rule in GUESS_RULES.iter().filter(|r| guess_matches(code, r)) {
        scores[rule.lexer as usize] += rule.weight;
    }
    let mut best = Lexer::Text;
    let mut best_score = 0;
    for (lexer, &score) in Lexer::ALL.iter().zip(&scores) {
        if score > best_score {
            best = *lexer;
            best_score = score;
        }
    }
    best
}

fn guess_matches(code: &str, rule: &GuessRule) -> bool {
    match rule.place {
        Start => code.starts_with(rule.text),
        LineStart => code.split('\n').any(|line| line.starts_with(rule.text)),
        Anywhere => code.contains(rule.text),
    }
}
