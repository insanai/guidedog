//! The bounded LaTeX-to-Typst math adapter. It translates while it parses, writing
//! straight to the output. Recursion is bounded by `MAX_DEPTH`. Rules that keep the
//! output valid Typst for any input:
//!
//! - atoms are separated by one space, so adjacent letters never form an identifier
//!   ("xy" becomes "x y") and a symbol name is never read as a function call;
//! - ASCII delimiters are escaped (`\(` `\)` `\[` `\]` `\{` `\}`), so unbalanced
//!   delimiters cannot break argument lists; `\left ... \right` becomes `lr(...)`;
//! - commas and semicolons are escaped, so they never split function arguments;
//! - the first unsupported command or construct is the error, and the caller applies
//!   the math policy.

mod commands;
mod symbols;

use std::fmt;

use guidedoc::{ContentPolicy, NodeId, arg_text};

use crate::escape::write_string_body;
use crate::messages::{MATH_OMITTED, MATH_UNSUPPORTED, MATH_VERBATIM};
use crate::state::State;
use symbols::{CommandKind, find_command};

#[cfg(test)]
pub(crate) use symbols::{COMMANDS, CommandKind as Kind};

/// The deepest nesting of groups and atoms the adapter follows.
const MAX_DEPTH: usize = 64;

/// The first construct of a formula the adapter cannot translate: a command such as
/// `\color`, borrowed from the formula, or a description such as "an unclosed {".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MathError<'a> {
    /// The command or the description of the construct.
    pub construct: &'a str,
}

impl fmt::Display for MathError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot translate {} in this LaTeX formula to Typst math", self.construct)
    }
}

impl std::error::Error for MathError<'_> {}

/// Appends the Typst math form of `latex` (without `$` delimiters) to `out`. On failure
/// it returns the first unsupported construct; `out` then holds an incomplete
/// translation the caller should discard. It never panics, for any input.
pub fn translate_math<'a>(latex: &'a str, out: &mut String) -> Result<(), MathError<'a>> {
    let cell = out.len();
    let mut m = Math {
        out,
        src: latex,
        pos: 0,
        depth: 0,
        space: false,
        mode: Mode::Plain,
        cell,
        bad: None,
    };
    m.sequence(Stop::End);
    match m.bad {
        Some(construct) => Err(MathError { construct }),
        None => Ok(()),
    }
}

/// What `&` and `\\` mean where the parser is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// `&` aligns, `\\` breaks the line.
    Plain,
    /// `&` separates cells, `\\` separates rows.
    Matrix,
    /// `&` aligns inside a case, `\\` separates cases.
    Cases,
}

/// What ends a sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    End,
    Brace,
    Bracket,
    Right,
    Env,
}

impl Stop {
    /// The construct left open when the input ends before this stop.
    fn unclosed(self) -> &'static str {
        match self {
            Stop::End => "",
            Stop::Brace => "an unclosed {",
            Stop::Bracket => "an unclosed [",
            Stop::Right => "\\left without \\right",
            Stop::Env => "\\begin without \\end",
        }
    }
}

/// The translator of one formula.
struct Math<'s, 'o> {
    out: &'o mut String,
    src: &'s str,
    pos: usize,
    depth: usize,
    /// An atom was written; the next one needs a separating space.
    space: bool,
    mode: Mode,
    /// Output offset where the current matrix cell or case began.
    cell: usize,
    bad: Option<&'s str>,
}

/// Maps an ASCII character outside letters and digits to Typst math.
fn ascii_math(c: u8) -> Option<&'static str> {
    Some(match c {
        b'+' => "+",
        b'-' => "-",
        b'=' => "=",
        b'<' => "<",
        b'>' => ">",
        b'!' => "!",
        b'?' => "?",
        b'(' => "\\(",
        b')' => "\\)",
        b'[' => "\\[",
        b']' => "\\]",
        b'|' => "|",
        b':' => ":",
        b';' => "\\;",
        b',' => "\\,",
        b'.' => ".",
        b'/' => "\\/",
        b'*' => "\\*",
        b'\'' => "prime",
        b'"' => "\\\"",
        b'#' => "\\#",
        b'$' => "\\$",
        b'@' => "\\@",
        b'`' => "\\`",
        b'~' => "space",
        _ => return None,
    })
}

/// Counts the calls a Typst opening such as "display(frac(" opens.
fn parens(s: &str) -> usize {
    s.bytes().filter(|&c| c == b'(').count()
}

impl<'s> Math<'s, '_> {
    /// Records the first unsupported construct.
    fn unsupported(&mut self, what: &'s str) {
        self.bad.get_or_insert(what);
    }

    fn descend(&mut self) -> bool {
        if self.depth >= MAX_DEPTH {
            self.unsupported("nesting deeper than 64 levels");
            return false;
        }
        self.depth += 1;
        true
    }

    fn peek(&self) -> Option<u8> {
        self.src.as_bytes().get(self.pos).copied()
    }

    /// Returns the character at the position (the whole of a multi-byte one).
    fn char_here(&self) -> &'s str {
        let rest = &self.src[self.pos..];
        let n = rest.chars().next().map_or(0, char::len_utf8);
        &rest[..n]
    }

    /// Starts an atom: one space after a previous atom.
    fn sep(&mut self) {
        if self.space {
            self.out.push(' ');
        }
        self.space = true;
    }

    fn atom(&mut self, typst: &str) {
        self.sep();
        self.out.push_str(typst);
    }

    /// Starts a function call: the next atom follows "(" without a space.
    fn open(&mut self, call: &str) {
        self.sep();
        self.out.push_str(call);
        self.space = false;
    }

    fn next_arg(&mut self) {
        self.out.push_str(", ");
        self.space = false;
    }

    fn close(&mut self, count: usize) {
        for _ in 0..count {
            self.out.push(')');
        }
        self.space = true;
    }

    /// Writes `s` as a Typst string literal.
    fn string(&mut self, s: &str) {
        self.out.push('"');
        write_string_body(self.out, s, false);
        self.out.push('"');
    }

    /// Skips white space and `%` comments.
    fn skip_space(&mut self) {
        while let Some(c) = self.peek() {
            if c == b'%' {
                let rest = &self.src[self.pos..];
                self.pos += rest.find('\n').unwrap_or(rest.len());
            } else if matches!(c, b' ' | b'\t' | b'\n' | b'\r') {
                self.pos += 1;
            } else {
                return;
            }
        }
    }

    /// Returns the command at the position ("\name" or "\c"), or "".
    fn peek_command(&self) -> &'s str {
        let rest = &self.src[self.pos..];
        if !rest.starts_with('\\') {
            return "";
        }
        let letters = rest[1..].bytes().take_while(u8::is_ascii_alphabetic).count();
        if letters > 0 {
            return &rest[..1 + letters];
        }
        let n = rest[1..].chars().next().map_or(0, char::len_utf8);
        &rest[..1 + n]
    }

    /// Translates atoms until the stop token, which it leaves unconsumed.
    fn sequence(&mut self, stop: Stop) {
        if !self.descend() {
            return;
        }
        while self.bad.is_none() {
            self.skip_space();
            let Some(c) = self.peek() else {
                if stop != Stop::End {
                    self.unsupported(stop.unclosed());
                }
                break;
            };
            if c == b'}' {
                if stop != Stop::Brace {
                    self.unsupported("an unmatched }");
                }
                break;
            }
            if c == b']' && stop == Stop::Bracket {
                break;
            }
            if c == b'&' {
                self.pos += 1;
                self.separator(false);
                continue;
            }
            match self.peek_command() {
                "\\\\" => {
                    self.pos += 2;
                    self.skip_optional();
                    self.separator(true);
                    continue;
                }
                "\\right" => {
                    if stop != Stop::Right {
                        self.unsupported("\\right without \\left");
                    }
                    break;
                }
                "\\end" => {
                    if stop != Stop::Env {
                        self.unsupported("\\end without \\begin");
                    }
                    break;
                }
                _ => self.scripted(),
            }
        }
        self.depth -= 1;
    }

    /// Skips an optional `[..]` argument, such as the spacing of `\\[2pt]`.
    fn skip_optional(&mut self) {
        let save = self.pos;
        self.skip_space();
        if self.peek() != Some(b'[') {
            self.pos = save;
            return;
        }
        let rest = &self.src[self.pos..];
        self.pos += rest.find(']').map_or(rest.len(), |i| i + 1);
    }

    /// Writes `&` or `\\` in the current mode. A trailing `\\` before `\end` is dropped.
    fn separator(&mut self, row: bool) {
        if row {
            let save = self.pos;
            self.skip_space();
            let at_end = self.peek().is_none() || self.peek_command() == "\\end";
            self.pos = save;
            if at_end {
                return;
            }
        }
        match (self.mode, row) {
            (Mode::Plain, true) => self.atom("\\"),
            (Mode::Plain | Mode::Cases, false) => self.atom("&"),
            (Mode::Matrix, _) | (Mode::Cases, true) => {
                self.fill_cell();
                self.out.push_str(if row && self.mode == Mode::Matrix { "; " } else { ", " });
                self.space = false;
                self.cell = self.out.len();
            }
        }
    }

    /// Writes an empty string for an empty matrix cell, which Typst requires.
    fn fill_cell(&mut self) {
        if self.out.len() == self.cell {
            self.out.push_str("\"\"");
        }
    }

    /// Translates an atom and its primes, subscripts, and superscripts.
    fn scripted(&mut self) {
        let before = self.out.len();
        if !matches!(self.peek(), Some(b'^' | b'_')) {
            self.atom_token();
        }
        while self.bad.is_none() {
            self.skip_space();
            let Some(c) = self.peek() else {
                return;
            };
            if c != b'\'' && c != b'^' && c != b'_' {
                return;
            }
            if self.out.len() == before {
                self.atom("\"\"");
            }
            self.pos += 1;
            self.out.push(char::from(c));
            if c == b'\'' {
                continue;
            }
            self.out.push('(');
            self.space = false;
            self.argument();
            self.close(1);
        }
    }

    /// Translates one mandatory argument: a braced group or a single token.
    fn argument(&mut self) {
        self.skip_space();
        match self.peek() {
            None | Some(b'}' | b'^' | b'_' | b'&') => self.unsupported("a missing argument"),
            Some(b'{') => self.group(),
            Some(c) if c.is_ascii_digit() => {
                self.sep();
                self.out.push(char::from(c));
                self.pos += 1;
            }
            Some(_) => self.atom_token(),
        }
    }

    /// Translates `{...}` in plain mode; the braces themselves are invisible.
    fn group(&mut self) {
        self.pos += 1;
        let saved = self.mode;
        self.mode = Mode::Plain;
        self.sequence(Stop::Brace);
        self.mode = saved;
        if self.bad.is_none() {
            self.pos += 1;
        }
    }

    /// Translates one token: a letter, a number, a symbol, a group, or a command.
    fn atom_token(&mut self) {
        if !self.descend() {
            return;
        }
        let Some(c) = self.peek() else {
            self.unsupported("a missing argument");
            self.depth -= 1;
            return;
        };
        if c.is_ascii_alphabetic() || c >= 0x80 {
            let token = self.char_here();
            self.pos += token.len();
            self.atom(token);
        } else if c.is_ascii_digit() {
            self.number();
        } else if c == b'{' {
            self.group();
        } else if c == b'\\' {
            self.command();
        } else {
            self.pos += 1;
            match ascii_math(c) {
                Some(typst) => self.atom(typst),
                None => self.unsupported(&self.src[self.pos - 1..self.pos]),
            }
        }
        self.depth -= 1;
    }

    /// Translates digits with an inner decimal point, such as 3.14.
    fn number(&mut self) {
        let b = self.src.as_bytes();
        let start = self.pos;
        while self.pos < b.len()
            && (b[self.pos].is_ascii_digit()
                || (b[self.pos] == b'.' && b.get(self.pos + 1).is_some_and(u8::is_ascii_digit)))
        {
            self.pos += 1;
        }
        let digits = &self.src[start..self.pos];
        self.atom(digits);
    }

    /// Translates a backslash command through the command table or its own rule.
    fn command(&mut self) {
        let name = self.peek_command();
        self.pos += name.len();
        if !name[1..].starts_with(|c: char| c.is_ascii_alphabetic()) {
            self.escape_command(name);
            return;
        }
        if self.special_command(name) {
            return;
        }
        let Some(entry) = find_command(&name[1..]) else {
            self.unsupported(name);
            return;
        };
        match entry.kind {
            CommandKind::Symbol => self.atom(entry.typst),
            CommandKind::Noop => {}
            CommandKind::SkipArg => {
                self.braced_raw();
            }
            CommandKind::Wrap => {
                self.open(entry.typst);
                self.argument();
                self.close(parens(entry.typst));
            }
            CommandKind::Font => self.font(entry.typst),
            CommandKind::Two => {
                self.open(entry.typst);
                self.argument();
                self.next_arg();
                self.argument();
                self.close(parens(entry.typst));
            }
            CommandKind::Text => self.text_command(entry.typst),
            CommandKind::Delimiter => {
                self.skip_space();
                self.delimiter();
            }
        }
    }

    /// Translates one-character commands: spacing and escaped symbols.
    fn escape_command(&mut self, name: &'s str) {
        let typst = match name.as_bytes().get(1) {
            Some(b',') => "thin",
            Some(b':' | b'>') => "med",
            Some(b';') => "thick",
            Some(b'!') => "#h(-0.1667em)",
            Some(b' ' | b'\n' | b'\t') => "space",
            Some(b'{') => "\\{",
            Some(b'}') => "\\}",
            Some(b'|') => "‖",
            Some(b'_') => "\\_",
            Some(b'%') => "%",
            Some(b'&') => "\\&",
            Some(b'#') => "\\#",
            Some(b'$') => "\\$",
            _ => return self.unsupported(name),
        };
        self.atom(typst);
    }

    /// Applies a font command; a plain word argument becomes a string, so
    /// `\mathrm{max}` stays one upright word rather than three variables.
    fn font(&mut self, typst: &str) {
        let save = self.pos;
        let (word, braced) = self.braced_raw();
        let is_word = word.bytes().all(|c| c.is_ascii_alphanumeric() || c == b' ');
        if braced && is_word && word.len() > 1 {
            self.open(typst);
            self.string(word);
            self.close(parens(typst));
            return;
        }
        self.pos = save;
        self.open(typst);
        self.argument();
        self.close(parens(typst));
    }

    /// Reads a `{...}` argument without translating it; unbraced, it reads one
    /// character. The result excludes the braces; the flag tells whether it was braced.
    fn braced_raw(&mut self) -> (&'s str, bool) {
        self.skip_space();
        if self.peek().is_none() {
            return ("", false);
        }
        if self.peek() != Some(b'{') {
            let token = self.char_here();
            self.pos += token.len();
            return (token, false);
        }
        let b = self.src.as_bytes();
        let start = self.pos + 1;
        let mut depth = 0;
        let mut i = self.pos;
        while i < b.len() {
            match b[i] {
                b'\\' => i += 1,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        self.pos = i + 1;
                        return (&self.src[start..i], true);
                    }
                }
                _ => {}
            }
            i += 1;
        }
        self.unsupported("an unclosed {");
        self.pos = self.src.len();
        ("", false)
    }

    /// Writes prose inside math with normal text layout, including shaping and the
    /// surrounding font's script fallbacks. Mathematical font commands remain separate:
    /// they select mathematical glyph variants on purpose.
    fn text_command(&mut self, typst: &str) {
        let (content, _) = self.braced_raw();
        if typst == "sans(" {
            self.open("#gd-math-sans-text(gd-math-font, ");
            self.text_literal(content);
            self.close(1);
            return;
        }
        self.open("#box(text(");
        if typst == "mono(" {
            self.out.push_str("font: gd-mono + gd-math-font, ");
        } else {
            self.out.push_str("font: gd-math-font, ");
            if typst == "bold(" {
                self.out.push_str("weight: \"bold\", ");
            }
            if typst == "italic(" {
                self.out.push_str("style: \"italic\", ");
            }
        }
        self.text_literal(content);
        self.close(2);
    }

    /// Writes the text of a `\text`-like argument as a Typst string. Escaped characters
    /// keep their character, spacing commands become spaces, and other commands and
    /// braces are dropped.
    fn text_literal(&mut self, content: &str) {
        self.out.push('"');
        let mut chars = content.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\\' => match chars.peek().copied() {
                    Some(next) if !next.is_ascii_alphabetic() => {
                        chars.next();
                        let mut buf = [0u8; 4];
                        let text = if matches!(next, ' ' | ',' | ';' | ':' | '!') {
                            " "
                        } else {
                            next.encode_utf8(&mut buf)
                        };
                        write_string_body(self.out, text, true);
                    }
                    _ => while chars.next_if(char::is_ascii_alphabetic).is_some() {},
                },
                '{' | '}' | '$' => {}
                '~' => self.out.push(' '),
                c => {
                    let mut buf = [0u8; 4];
                    write_string_body(self.out, c.encode_utf8(&mut buf), true);
                }
            }
        }
        self.out.push('"');
    }
}

impl State<'_, '_> {
    /// Writes inline (`$x$`) or display (`$ x $`) math, applying the math policy to
    /// formulas the adapter cannot translate.
    pub(crate) fn render_math(&mut self, id: NodeId, display: bool) {
        let n = *self.doc.node(id);
        let latex = self.doc.text(n.text);
        let mut typst = std::mem::take(&mut self.math);
        typst.clear();
        match translate_math(latex, &mut typst) {
            Ok(()) => {
                // Capture the prose face before Typst enters math mode. Math fonts do not
                // shape complex-script text, even when it is supplied as a quoted string.
                self.emit("#context { let gd-math-font = ");
                self.emit("if type(text.font) == str { (text.font,) } else { text.font }; ");
                self.emit(if display { "$ " } else { "$" });
                self.emit(&typst);
                self.emit(if display { " $ }\n" } else { "$ }" });
            }
            Err(e) => self.untranslatable_math(n.span, latex, e.construct, display),
        }
        self.math = typst;
    }

    fn untranslatable_math(
        &mut self,
        span: guidedoc::SourceSpan,
        latex: &str,
        construct: &str,
        display: bool,
    ) {
        let arg = arg_text(construct);
        match self.ctx.policy.math {
            ContentPolicy::Refuse => self.fail(MATH_UNSUPPORTED, span, arg),
            ContentPolicy::Omit => self.fail(MATH_OMITTED, span, arg),
            ContentPolicy::Allow => {
                self.fail(MATH_VERBATIM, span, arg);
                self.emit(if display {
                    "#raw(block: true, lang: \"latex\", "
                } else {
                    "#raw(lang: \"latex\", "
                });
                self.emit_string(latex);
                self.emit(if display { ")\n" } else { ")" });
            }
        }
    }
}
