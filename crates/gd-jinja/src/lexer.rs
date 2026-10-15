//! The lexer: template text into tokens, with Jinja's whitespace control.

use std::borrow::Cow;

use crate::convert::quoted;
use crate::error::{Error, ErrorKind};
use crate::text::is_space;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TokenKind {
    Eof,
    Data,          // template text, already whitespace-controlled
    VariableBegin, // {{
    VariableEnd,   // }}
    BlockBegin,    // {%
    BlockEnd,      // %}
    Name,
    String,
    Integer,
    Float,
    Operator,
}

/// One token; text borrows the source unless a string literal had escapes to decode.
#[derive(Debug, Clone)]
pub(crate) struct Token<'s> {
    pub kind: TokenKind,
    pub text: Cow<'s, str>,
    pub number: i64,
    pub float: f64,
    pub line: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct LexOptions {
    pub trim_blocks: bool,
    pub lstrip_blocks: bool,
    pub keep_trailing_newline: bool,
}

struct Lexer<'s> {
    src: &'s str,
    pos: usize,
    line: usize,
    opts: LexOptions,
    tokens: Vec<Token<'s>>,
    err: Option<Error>,
}

/// Normalizes newlines to "\n" and drops the final newline unless it is kept; the
/// source is borrowed when it needs neither.
pub(crate) fn normalize(source: &str, keep_trailing_newline: bool) -> Cow<'_, str> {
    let mut src = Cow::Borrowed(source);
    if source.contains('\r') {
        src = Cow::Owned(source.replace("\r\n", "\n").replace('\r', "\n"));
    }
    if !keep_trailing_newline && src.ends_with('\n') {
        match &mut src {
            Cow::Borrowed(s) => *s = &s[..s.len() - 1],
            Cow::Owned(s) => {
                s.pop();
            }
        }
    }
    src
}

/// Splits normalized template text into tokens.
pub(crate) fn tokenize(src: &str, opts: LexOptions) -> Result<Vec<Token<'_>>, Error> {
    let mut lx = Lexer {
        src,
        pos: 0,
        line: 1,
        opts,
        tokens: Vec::with_capacity(src.len() / 8 + 8),
        err: None,
    };
    while lx.pos < src.len() && lx.err.is_none() {
        lex_step(&mut lx);
    }
    if let Some(err) = lx.err {
        return Err(err);
    }
    lx.emit(TokenKind::Eof, Cow::Borrowed(""));
    Ok(lx.tokens)
}

impl<'s> Lexer<'s> {
    fn fail(&mut self, message: impl Into<String>) {
        if self.err.is_none() {
            self.err = Some(Error::new(ErrorKind::Syntax, self.line, message));
        }
    }

    fn emit(&mut self, kind: TokenKind, text: Cow<'s, str>) {
        self.tokens.push(Token { kind, text, number: 0, float: 0.0, line: self.line });
    }

    fn advance(&mut self, n: usize) {
        self.line += self.src[self.pos..self.pos + n].bytes().filter(|&b| b == b'\n').count();
        self.pos += n;
    }

    fn byte(&self, at: usize) -> u8 {
        self.src.as_bytes().get(at).copied().unwrap_or(0)
    }
}

/// Finds the next "{{", "{%" or "{#" at or after from.
fn find_tag(src: &str, from: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    let mut i = from;
    loop {
        let j = i + src[i..].find('{')?;
        let c = *bytes.get(j + 1)?;
        if matches!(c, b'{' | b'%' | b'#') {
            return Some(j);
        }
        i = j + 1;
    }
}

/// Lexes the data before the next tag and then the tag itself.
fn lex_step(lx: &mut Lexer<'_>) {
    let src = lx.src;
    let Some(start) = find_tag(src, lx.pos) else {
        lx.emit(TokenKind::Data, Cow::Borrowed(&src[lx.pos..]));
        lx.advance(src.len() - lx.pos);
        return;
    };
    let opener = lx.byte(start + 1);
    let sign = lx.byte(start + 2);
    let mut data = &src[lx.pos..start];
    let is_raw = opener == b'%' && raw_begins(&src[start + 2..]);
    if sign == b'-' {
        data = data.trim_end_matches(is_space);
    } else if lx.opts.lstrip_blocks && sign != b'+' && opener != b'{' {
        data = lstrip_line(src, lx.pos, data);
    }
    if !data.is_empty() {
        lx.emit(TokenKind::Data, Cow::Borrowed(data));
    }
    lx.advance(start - lx.pos);
    let skip = if sign == b'-' || sign == b'+' { 3 } else { 2 };
    if opener == b'#' {
        lex_comment(lx, skip);
    } else if is_raw {
        lex_raw(lx);
    } else if opener == b'{' {
        lx.emit(TokenKind::VariableBegin, Cow::Borrowed("{{"));
        lx.advance(skip);
        lex_expression(lx, TokenKind::VariableEnd);
    } else {
        lx.emit(TokenKind::BlockBegin, Cow::Borrowed("{%"));
        lx.advance(skip);
        lex_expression(lx, TokenKind::BlockEnd);
    }
}

/// Removes spaces and tabs before a block tag when they are the only thing on the tag's
/// line (lstrip_blocks).
fn lstrip_line<'a>(src: &str, data_start: usize, data: &'a str) -> &'a str {
    let l = data.rfind('\n').map_or(0, |i| i + 1);
    let at_line_start = l > 0 || data_start == 0 || src.as_bytes()[data_start - 1] == b'\n';
    if !at_line_start || !data[l..].bytes().all(|c| c == b' ' || c == b'\t') {
        return data;
    }
    &data[..l]
}

/// Applies "-" (strip all whitespace) or trim_blocks (drop one newline).
fn after_tag(lx: &mut Lexer<'_>, sign: u8, is_block: bool) {
    if sign == b'-' {
        let rest = &lx.src[lx.pos..];
        let n = rest.len() - rest.trim_start_matches(is_space).len();
        lx.advance(n);
    } else if sign != b'+' && is_block && lx.opts.trim_blocks && lx.byte(lx.pos) == b'\n' {
        lx.advance(1);
    }
}

fn lex_comment(lx: &mut Lexer<'_>, skip: usize) {
    let begin_line = lx.line;
    lx.advance(skip);
    let Some(end) = lx.src[lx.pos..].find("#}") else {
        lx.line = begin_line;
        lx.fail("Missing end of comment tag");
        return;
    };
    let sign = if end > 0 { lx.byte(lx.pos + end - 1) } else { 0 };
    lx.advance(end + 2);
    after_tag(lx, sign, true);
}

fn raw_begins(s: &str) -> bool {
    let rest = s.strip_prefix(['-', '+']).unwrap_or(s).trim_start_matches([' ', '\t', '\n']);
    let Some(rest) = rest.strip_prefix("raw") else { return false };
    let rest = rest.trim_start_matches([' ', '\t', '\n']);
    rest.strip_prefix(['-', '+']).unwrap_or(rest).starts_with("%}")
}

/// Copies everything up to {% endraw %} as data.
fn lex_raw(lx: &mut Lexer<'_>) {
    let src = lx.src;
    let close = src[lx.pos..].find("%}").unwrap_or(0);
    let begin_sign = lx.byte(lx.pos + close - 1);
    lx.advance(close + 2);
    if begin_sign == b'-' {
        after_tag(lx, b'-', true);
    }
    let body_start = lx.pos;
    let mut search = lx.pos;
    loop {
        let Some(tag) = src[search..].find("{%") else {
            lx.fail("Missing end of raw directive");
            return;
        };
        let at = search + tag;
        if let Some((end_sign, tag_end, sign)) = endraw_at(&src[at + 2..]) {
            let mut body = &src[body_start..at];
            if sign == b'-' {
                body = body.trim_end_matches(is_space);
            } else if lx.opts.lstrip_blocks && sign != b'+' {
                body = lstrip_line(src, body_start, body);
            }
            if !body.is_empty() {
                lx.emit(TokenKind::Data, Cow::Borrowed(body));
            }
            lx.advance(at + 2 + tag_end - lx.pos);
            after_tag(lx, end_sign, true);
            return;
        }
        search = at + 2;
    }
}

/// Reads "endraw %}" (with signs) after a "{%": the end sign, the length up to and
/// including "%}", and the begin sign.
fn endraw_at(inner: &str) -> Option<(u8, usize, u8)> {
    let sign = inner.as_bytes().first().copied().unwrap_or(0);
    let body = if sign == b'-' || sign == b'+' { &inner[1..] } else { inner };
    let rest = body.trim_start_matches([' ', '\t', '\n']).strip_prefix("endraw")?;
    let rest = rest.trim_start_matches([' ', '\t', '\n']);
    let end_sign = rest.as_bytes().first().copied().unwrap_or(0);
    let rest = if end_sign == b'-' || end_sign == b'+' { &rest[1..] } else { rest };
    let end_sign = if end_sign == b'-' || end_sign == b'+' { end_sign } else { 0 };
    rest.starts_with("%}").then(|| (end_sign, inner.len() - rest.len() + 2, sign))
}

/// Lexes the tokens of a {{ }} or {% %} tag up to its end.
fn lex_expression(lx: &mut Lexer<'_>, end_kind: TokenKind) {
    let closer = if end_kind == TokenKind::VariableEnd { "}}" } else { "%}" };
    let mut depth = 0usize;
    while lx.err.is_none() {
        let rest = &lx.src[lx.pos..];
        lx.advance(rest.len() - rest.trim_start_matches(is_space).len());
        let rest = &lx.src[lx.pos..];
        if rest.is_empty() {
            let what = if end_kind == TokenKind::VariableEnd {
                "print statement"
            } else {
                "statement block"
            };
            lx.fail(format!("unexpected end of template, expected 'end of {what}'."));
            return;
        }
        if depth == 0 {
            let signed = rest.strip_prefix(['-', '+']).filter(|r| r.starts_with(closer));
            let sign = if signed.is_some() { rest.as_bytes()[0] } else { 0 };
            if rest.starts_with(closer) || signed.is_some() {
                lx.emit(end_kind, Cow::Borrowed(closer));
                lx.advance(if sign == 0 { 2 } else { 3 });
                after_tag(lx, sign, end_kind == TokenKind::BlockEnd);
                return;
            }
        }
        lex_token(lx, &mut depth);
    }
}

const OPERATORS: &[&str] = &[
    "//", "**", "==", "!=", ">=", "<=", "+", "-", "*", "/", "%", "~", "[", "]", "(", ")", "{",
    "}", ">", "<", "=", ".", ":", "|", ",", ";",
];

fn lex_token(lx: &mut Lexer<'_>, depth: &mut usize) {
    let src = lx.src;
    let rest = &src[lx.pos..];
    let c = rest.chars().next().unwrap_or(' ');
    if c == '"' || c == '\'' {
        lex_string(lx);
    } else if c.is_ascii_digit() {
        lex_number(lx);
    } else if c == '_' || c.is_alphabetic() {
        let n = rest.find(|r: char| r != '_' && !r.is_alphanumeric()).unwrap_or(rest.len());
        lx.emit(TokenKind::Name, Cow::Borrowed(&rest[..n]));
        lx.advance(n);
    } else if let Some(op) = OPERATORS.iter().find(|op| rest.starts_with(**op)) {
        match *op {
            "(" | "[" | "{" => *depth += 1,
            ")" | "]" | "}" => {
                if *depth == 0 {
                    lx.fail(format!("unexpected '{op}'"));
                    return;
                }
                *depth -= 1;
            }
            _ => {}
        }
        lx.emit(TokenKind::Operator, Cow::Borrowed(op));
        lx.advance(op.len());
    } else {
        lx.fail(format!("unexpected char {}", quoted(&rest[..c.len_utf8()])));
    }
}

fn lex_string(lx: &mut Lexer<'_>) {
    let src = lx.src;
    let rest = &src[lx.pos..];
    let bytes = rest.as_bytes();
    let quote = bytes[0];
    let mut i = 1;
    while i < bytes.len() && bytes[i] != quote {
        if bytes[i] == b'\\' {
            i += 1;
        }
        i += 1;
    }
    if i >= bytes.len() {
        lx.fail("unexpected end of string");
        return;
    }
    lx.emit(TokenKind::String, unescape_literal(&rest[1..i]));
    lx.advance(i + 1);
}

/// Decodes Python string escapes; unknown escapes keep the backslash.
pub(crate) fn unescape_literal(s: &str) -> Cow<'_, str> {
    if !s.contains('\\') {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some((_, e)) = chars.next() else {
            out.push('\\');
            break;
        };
        let simple = match e {
            'n' => Some('\n'),
            't' => Some('\t'),
            'r' => Some('\r'),
            'a' => Some('\x07'),
            'b' => Some('\x08'),
            'f' => Some('\x0c'),
            'v' => Some('\x0b'),
            '0' => Some('\0'),
            '\\' | '\'' | '"' => Some(e),
            _ => None,
        };
        if let Some(x) = simple {
            out.push(x);
        } else if e == '\n' {
            // A backslash before a newline joins the lines.
        } else if let Some(x) = hex_escape(s, i + 2, e) {
            out.push(x);
            let width = if e == 'x' {
                2
            } else if e == 'u' {
                4
            } else {
                8
            };
            for _ in 0..width {
                chars.next();
            }
        } else {
            out.push('\\');
            out.push(e);
        }
    }
    Cow::Owned(out)
}

/// Decodes the digits of a \x, \u or \U escape that start at `at`.
fn hex_escape(s: &str, at: usize, e: char) -> Option<char> {
    let width = match e {
        'x' => 2,
        'u' => 4,
        'U' => 8,
        _ => return None,
    };
    let digits = s.get(at..at + width)?;
    char::from_u32(u32::from_str_radix(digits, 16).ok()?)
}

fn lex_number(lx: &mut Lexer<'_>) {
    let src = lx.src;
    let rest = &src[lx.pos..];
    let bytes = rest.as_bytes();
    let after_dot =
        lx.tokens.last().is_some_and(|t| t.kind == TokenKind::Operator && t.text == ".");
    if bytes.len() > 1 && bytes[0] == b'0' && b"xXoObB".contains(&bytes[1]) {
        let base = match bytes[1] {
            b'o' | b'O' => 8,
            b'b' | b'B' => 2,
            _ => 16,
        };
        let mut n = 2;
        while n < bytes.len() && (bytes[n] == b'_' || digit_value(bytes[n]) < base) {
            n += 1;
        }
        push_integer(lx, &rest[..n], &rest[2..n], base);
        return;
    }
    let mut n = scan_digits(bytes, 0);
    let mut is_float = false;
    if !after_dot && n + 1 < bytes.len() && bytes[n] == b'.' && bytes[n + 1].is_ascii_digit() {
        n = scan_digits(bytes, n + 1);
        is_float = true;
    }
    if !after_dot && n < bytes.len() && (bytes[n] == b'e' || bytes[n] == b'E') {
        let mut m = n + 1;
        if m < bytes.len() && (bytes[m] == b'+' || bytes[m] == b'-') {
            m += 1;
        }
        if m < bytes.len() && bytes[m].is_ascii_digit() {
            n = scan_digits(bytes, m);
            is_float = true;
        }
    }
    if !is_float {
        push_integer(lx, &rest[..n], &rest[..n], 10);
        return;
    }
    let f = rest[..n].replace('_', "").parse::<f64>().unwrap_or(0.0);
    let line = lx.line;
    lx.tokens.push(Token {
        kind: TokenKind::Float,
        text: Cow::Borrowed(&rest[..n]),
        number: 0,
        float: f,
        line,
    });
    lx.advance(n);
}

fn push_integer<'s>(lx: &mut Lexer<'s>, text: &'s str, digits: &str, base: u32) {
    let Some(v) = parse_digits(&digits.replace('_', ""), base) else {
        lx.fail("integer literal out of range");
        return;
    };
    let line = lx.line;
    lx.tokens.push(Token {
        kind: TokenKind::Integer,
        text: Cow::Borrowed(text),
        number: v,
        float: 0.0,
        line,
    });
    lx.advance(text.len());
}

fn scan_digits(s: &[u8], from: usize) -> usize {
    let mut n = from;
    while n < s.len()
        && (s[n].is_ascii_digit()
            || (s[n] == b'_' && n + 1 < s.len() && s[n + 1].is_ascii_digit()))
    {
        n += 1;
    }
    n
}

/// The value of a digit in bases up to 36; 99 for anything else.
pub(crate) fn digit_value(c: u8) -> u32 {
    match c {
        b'0'..=b'9' => u32::from(c - b'0'),
        b'a'..=b'z' => u32::from(c - b'a') + 10,
        b'A'..=b'Z' => u32::from(c - b'A') + 10,
        _ => 99,
    }
}

/// Parses unsigned digits in base, None on overflow or a bad digit.
pub(crate) fn parse_digits(text: &str, base: u32) -> Option<i64> {
    if text.is_empty() {
        return None;
    }
    text.bytes().try_fold(0i64, |v, c| {
        let d = digit_value(c);
        if d >= base {
            return None;
        }
        v.checked_mul(i64::from(base))?.checked_add(i64::from(d))
    })
}
