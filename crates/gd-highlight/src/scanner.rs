//! The lexer engine.
//!
//! A lexer is a step function over a [`Scanner`]: each call looks at the input at `s.pos`
//! and emits one or more tokens that advance it. Emission is the only way to advance, so
//! tokens are contiguous and cover every byte exactly once; a step that emits nothing has
//! its next character emitted as `Error`, so no input can loop. Lexer state is a
//! fixed-size [`State`] (a mode and a small frame stack), so scanning never allocates and
//! the same input always gives the same tokens.

use crate::lexers::{Lexer, resolve_lexer, step};
use crate::token::{Token, TokenKind, kind_is};

/// The deepest a lexer's frame stack goes; deeper constructs lose colour, not coverage.
pub(crate) const MAX_FRAMES: usize = 24;
const QUEUE_SIZE: usize = 32;
/// How far a step looks for something it may not find, such as the end of a regular
/// expression literal, so that no input costs more than linear time.
pub(crate) const LOOKAHEAD: usize = 256;

/// One frame of a lexer's stack: a construct and its data.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Frame {
    pub mode: u8,
    pub data: u8,
    pub count: u16,
}

/// A lexer's state between steps.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct State {
    pub lexer: Lexer,
    pub mode: u8,
    /// Frames in use.
    pub depth: u8,
    pub stack: [Frame; MAX_FRAMES],
    /// A span of the input: a heredoc word or a raw-string delimiter.
    pub mark: [usize; 2],
    /// Lexer-specific: bracket depth, hash count.
    pub count: u32,
    /// Lexer-specific bits.
    pub flags: u32,
    /// The kind the next name takes, after "def" or "class".
    pub lead: TokenKind,
    pub leading: bool,
}

impl State {
    pub fn new(lexer: Lexer) -> State {
        State { lexer, ..State::default() }
    }

    /// Pushes a frame; false when the stack is full, and lexers then carry on without the
    /// construct's state, which only costs colour, never coverage.
    pub fn push(&mut self, mode: u8, data: u8, count: u16) -> bool {
        let depth = usize::from(self.depth);
        if depth >= MAX_FRAMES {
            return false;
        }
        self.stack[depth] = Frame { mode, data, count };
        self.depth += 1;
        true
    }

    pub fn pop(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    /// The top frame, if any.
    pub fn top(&self) -> Option<Frame> {
        usize::from(self.depth).checked_sub(1).map(|i| self.stack[i])
    }

    /// The top frame to change, if any.
    pub fn top_mut(&mut self) -> Option<&mut Frame> {
        let i = usize::from(self.depth).checked_sub(1)?;
        Some(&mut self.stack[i])
    }

    pub fn frames(&self) -> &[Frame] {
        &self.stack[..usize::from(self.depth)]
    }
}

/// The last significant token a lexer emitted: not space, comment or text.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Track {
    pub last: TokenKind,
    /// Its final byte.
    pub last_byte: u8,
    /// Whether there was one.
    pub started: bool,
}

/// Tokenizes one input, yielding merged tokens as an iterator.
///
/// Create it with [`scan_tokens`]; it borrows the code and allocates nothing.
#[derive(Debug, Clone)]
pub struct Scanner<'a> {
    pub(crate) code: &'a str,
    pub(crate) bytes: &'a [u8],
    pub(crate) pos: usize,
    /// Steps see `code[..limit]`; an inner lexer sees its region only.
    pub(crate) limit: usize,
    pub(crate) state: State,
    /// A nested lexer: Python after a pycon prompt, CSS in `<style>`.
    pub(crate) inner: State,
    /// The inner lexer runs while `pos < region`.
    pub(crate) region: usize,
    pub(crate) region_start: usize,
    pub(crate) nested: bool,
    /// The last significant tokens of the outer and the inner lexer.
    pub(crate) tracks: [Track; 2],
    queue: [Token; QUEUE_SIZE],
    head: usize,
    count: usize,
}

#[cfg(test)]
thread_local! {
    /// Dispatches and cursor inspections, for the tests that hold scanning to linear time.
    pub(crate) static WORK: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Counts scanning work in tests; nothing in other builds.
#[inline]
pub(crate) fn work(_n: usize) {
    #[cfg(test)]
    WORK.with(|w| w.set(w.get() + _n as u64));
}

/// Starts scanning `code` with `lexer`; the pseudo lexers [`Lexer::Default`] and
/// [`Lexer::Guess`] are resolved against the code first. Inputs are limited to 4 GiB; the
/// rest of a longer input is not scanned.
pub fn scan_tokens(lexer: Lexer, code: &str) -> Scanner<'_> {
    let mut n = code.len().min(u32::MAX as usize);
    while !code.is_char_boundary(n) {
        n -= 1;
    }
    let code = &code[..n];
    Scanner {
        code,
        bytes: code.as_bytes(),
        pos: 0,
        limit: n,
        state: State::new(resolve_lexer(lexer, code)),
        inner: State::default(),
        region: 0,
        region_start: 0,
        nested: false,
        tracks: [Track::default(); 2],
        queue: [Token::default(); QUEUE_SIZE],
        head: 0,
        count: 0,
    }
}

impl Iterator for Scanner<'_> {
    type Item = Token;

    /// The next token; neighbours of one kind come back merged.
    fn next(&mut self) -> Option<Token> {
        self.fill();
        if self.count == 0 {
            return None;
        }
        let mut t = self.pop_token();
        loop {
            self.fill();
            if self.count == 0 || self.queue[self.head].kind != t.kind {
                break;
            }
            t.end = self.pop_token().end;
        }
        Some(t)
    }
}

/// Lexes `code` into `tokens`, clearing it first so a caller can reuse its buffer.
pub fn tokenize_into(lexer: Lexer, code: &str, tokens: &mut Vec<Token>) {
    tokens.clear();
    tokens.extend(scan_tokens(lexer, code));
}

/// The tokens of `code` in a new vector.
pub fn tokenize(lexer: Lexer, code: &str) -> Vec<Token> {
    scan_tokens(lexer, code).collect()
}

/// How many tokens `code` lexes into, without storing them.
pub fn count_tokens(lexer: Lexer, code: &str) -> usize {
    scan_tokens(lexer, code).count()
}

/// Whether lexing `code` produces an `Error` token, which is when Sphinx warns that a
/// block does not lex as its language.
pub fn has_errors(lexer: Lexer, code: &str) -> bool {
    scan_tokens(lexer, code).any(|t| t.kind == TokenKind::Error)
}

impl<'a> Scanner<'a> {
    fn fill(&mut self) {
        while self.count == 0 && self.pos < self.bytes.len() {
            work(1);
            let before = self.pos;
            if self.pos < self.region {
                self.limit = self.region;
                self.nested = true;
                let mut st = self.inner;
                step(self, &mut st);
                self.inner = st;
                self.limit = self.bytes.len();
                self.nested = false;
            } else {
                let mut st = self.state;
                step(self, &mut st);
                self.state = st;
            }
            if self.pos == before && self.region <= self.pos {
                self.emit_rune(TokenKind::Error);
            }
        }
    }

    fn pop_token(&mut self) -> Token {
        let t = self.queue[self.head];
        self.head = (self.head + 1) % QUEUE_SIZE;
        self.count -= 1;
        t
    }

    /// Makes `lexer` tokenize `code[pos..end]` before the outer lexer resumes. The inner
    /// lexer keeps its state between regions unless `fresh`; lexers do not nest deeper.
    pub(crate) fn start_inner(&mut self, lexer: Lexer, region_end: usize, fresh: bool) {
        let mut end = region_end.clamp(self.pos, self.limit);
        while !self.code.is_char_boundary(end) {
            end += 1;
        }
        if self.nested {
            self.emit(end, TokenKind::Text);
            return;
        }
        if fresh || self.inner.lexer != lexer {
            self.inner = State::new(lexer);
            self.tracks[1] = Track::default();
        }
        self.region = end;
        self.region_start = self.pos;
    }

    /// The last significant token of the running lexer.
    pub(crate) fn track(&self) -> Track {
        self.tracks[usize::from(self.nested)]
    }

    /// Makes `code[pos..stop]` one token of `kind` and moves past it; the end moves forward
    /// to a character boundary.
    pub(crate) fn emit(&mut self, stop: usize, kind: TokenKind) {
        let mut end = stop.min(self.limit);
        if end <= self.pos {
            return;
        }
        while !self.code.is_char_boundary(end) {
            end += 1;
        }
        if kind != TokenKind::Whitespace
            && kind != TokenKind::Text
            && !kind_is(kind, TokenKind::Comment)
        {
            let last_byte = self.bytes[end - 1];
            self.tracks[usize::from(self.nested)] = Track { last: kind, last_byte, started: true };
        }
        let t = Token { start: self.pos as u32, end: end as u32, kind };
        self.pos = end;
        if self.count > 0 {
            let last = &mut self.queue[(self.head + self.count - 1) % QUEUE_SIZE];
            if last.kind == kind || self.count == QUEUE_SIZE {
                last.end = t.end;
                return;
            }
        }
        self.queue[(self.head + self.count) % QUEUE_SIZE] = t;
        self.count += 1;
    }

    pub(crate) fn emit_n(&mut self, n: usize, kind: TokenKind) {
        self.emit(self.pos + n, kind);
    }

    /// Emits the character at `pos`.
    pub(crate) fn emit_rune(&mut self, kind: TokenKind) {
        self.emit(self.pos + self.rune_size(self.pos), kind);
    }

    /// The length of the UTF-8 character at `at`; 0 at the limit.
    pub(crate) fn rune_size(&self, at: usize) -> usize {
        work(4);
        if at >= self.limit {
            return 0;
        }
        let n = match self.bytes[at] {
            0..0x80 => 1,
            0xf0.. => 4,
            0xe0.. => 3,
            0xc0.. => 2,
            _ => 1,
        };
        if at + n > self.limit { 1 } else { n }
    }

    /// The character at `at`, when one starts there.
    pub(crate) fn char_at(&self, at: usize) -> Option<char> {
        self.code.get(at..self.limit)?.chars().next()
    }

    // Cursor helpers. Offsets are relative to `pos`; positions are absolute.

    pub(crate) fn peek(&self, offset: usize) -> u8 {
        self.byte_at(self.pos + offset)
    }

    pub(crate) fn byte_at(&self, at: usize) -> u8 {
        work(1);
        if at < self.limit { self.bytes[at] } else { 0 }
    }

    /// The byte before `pos`, or 0 at the start.
    pub(crate) fn before(&self) -> u8 {
        if self.pos > 0 { self.bytes[self.pos - 1] } else { 0 }
    }

    /// The input from `a` to `b`.
    pub(crate) fn slice(&self, a: usize, b: usize) -> &'a [u8] {
        &self.bytes[a..b]
    }

    pub(crate) fn looking_at(&self, text: &str, offset: usize) -> bool {
        work(text.len());
        let at = self.pos + offset;
        at + text.len() <= self.limit && &self.bytes[at..at + text.len()] == text.as_bytes()
    }

    /// Like `looking_at`, comparing ASCII letters without case.
    pub(crate) fn looking_at_fold(&self, text: &str, offset: usize) -> bool {
        work(text.len());
        let at = self.pos + offset;
        at + text.len() <= self.limit
            && self.bytes[at..at + text.len()].eq_ignore_ascii_case(text.as_bytes())
    }

    /// Whether `pos` begins a line, or begins an inner lexer's region.
    pub(crate) fn at_line_start(&self) -> bool {
        self.pos == 0
            || self.bytes[self.pos - 1] == b'\n'
            || (self.nested && self.pos == self.region_start)
    }

    /// The position of the next newline at or after `from`, or the limit.
    pub(crate) fn line_end(&self, from: usize) -> usize {
        let mut i = from;
        while i < self.limit && self.bytes[i] != b'\n' {
            i += 1;
        }
        work(i.saturating_sub(from) + 1);
        i
    }

    /// The position after the next newline at or after `from`, or the limit.
    pub(crate) fn line_after(&self, from: usize) -> usize {
        (self.line_end(from) + 1).min(self.limit)
    }

    pub(crate) fn skip_spaces(&self, from: usize) -> usize {
        self.skip_set(from, " \t")
    }

    pub(crate) fn skip_set(&self, from: usize, set: &str) -> usize {
        self.skip_by(from, |c| set.as_bytes().contains(&c))
    }

    pub(crate) fn skip_digits(&self, from: usize) -> usize {
        self.skip_by(from, |c| c.is_ascii_digit())
    }

    /// The first position at or after `from` whose byte fails `keep`, or the limit.
    pub(crate) fn skip_by(&self, from: usize, keep: impl Fn(u8) -> bool) -> usize {
        let mut i = from;
        while i < self.limit && keep(self.bytes[i]) {
            i += 1;
        }
        work(i.saturating_sub(from) + 1);
        i
    }

    /// The position of `text` at or after `from` on the same line, within
    /// [`LOOKAHEAD`] bytes.
    pub(crate) fn find_on_line(&self, from: usize, text: &str) -> Option<usize> {
        let stop = self.limit.min(from + LOOKAHEAD);
        let t = text.as_bytes();
        let mut i = from;
        while i + t.len() <= stop {
            work(t.len() + 1);
            if self.bytes[i] == b'\n' {
                return None;
            }
            if &self.bytes[i..i + t.len()] == t {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    /// The position of `text` at or after `from`; it may scan to the limit.
    pub(crate) fn find_from(&self, from: usize, text: &str) -> Option<usize> {
        let t = text.as_bytes();
        if t.is_empty() {
            return Some(from);
        }
        let mut i = from;
        while i + t.len() <= self.limit {
            work(t.len() + 1);
            if self.bytes[i] == t[0] && &self.bytes[i..i + t.len()] == t {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    /// The end of the identifier at `from`; `extra` holds more identifier bytes.
    pub(crate) fn ident_end(&self, from: usize, extra: &str) -> usize {
        self.skip_by(from, |c| is_ident_char(c) || extra.as_bytes().contains(&c))
    }

    /// Emits a run of whitespace, newlines included, as `kind`; false when there is none.
    pub(crate) fn emit_spaces(&mut self, kind: TokenKind) -> bool {
        let i = self.skip_by(self.pos, is_space);
        if i == self.pos {
            return false;
        }
        self.emit(i, kind);
        true
    }

    /// Emits the rest of the line, without its newline, as `kind`.
    pub(crate) fn emit_line(&mut self, kind: TokenKind) {
        self.emit(self.line_end(self.pos), kind);
    }

    /// Whether only spaces and tabs precede `pos` on its line (within an inner region).
    pub(crate) fn only_space_before_line(&self) -> bool {
        let lower = if self.nested { self.region_start } else { 0 };
        for &c in self.bytes[lower..self.pos].iter().rev() {
            match c {
                b'\n' => return true,
                b' ' | b'\t' => {}
                _ => return false,
            }
        }
        true
    }

    /// The end of the line at `from`, following backslash continuations, with its newline.
    pub(crate) fn logical_line_end(&self, from: usize) -> usize {
        let mut i = from;
        loop {
            let stop = self.line_end(i);
            if stop >= self.limit {
                return stop;
            }
            let mut j = stop;
            if j > i && self.bytes[j - 1] == b'\r' {
                j -= 1;
            }
            if j > i && self.bytes[j - 1] == b'\\' {
                i = stop + 1;
                continue;
            }
            return stop + 1;
        }
    }

    /// The position after the block comment that opens at `pos`, or the limit.
    pub(crate) fn block_comment_end(&self, open: &str, close: &str, nested: bool) -> usize {
        if !nested {
            return self
                .find_from(self.pos + open.len(), close)
                .map_or(self.limit, |e| e + close.len());
        }
        let mut level = 0;
        let mut i = self.pos;
        while i < self.limit {
            if self.looking_at(open, i - self.pos) {
                level += 1;
                i += open.len();
            } else if self.looking_at(close, i - self.pos) {
                level -= 1;
                i += close.len();
                if level == 0 {
                    return i;
                }
            } else {
                i += 1;
            }
        }
        self.limit
    }

    /// The length of the escape sequence at a backslash: `\n`, `\x41`, `\u{1F600}`,
    /// `\U0001F600`, `\101`, an escaped character, or a backslash and a newline.
    pub(crate) fn escape_length(&self, at: usize) -> usize {
        if at + 1 >= self.limit {
            return 1;
        }
        let hex = match self.byte_at(at + 1) {
            b'x' => 2,
            b'u' if self.byte_at(at + 2) == b'{' => {
                let mut end = at + 3;
                while end < self.limit && end < at + 12 && self.bytes[end].is_ascii_hexdigit() {
                    end += 1;
                }
                return if self.byte_at(end) == b'}' { end + 1 - at } else { 2 };
            }
            b'u' => 4,
            b'U' => 8,
            b'0'..=b'7' => {
                let mut n = 2;
                while n < 4 && (b'0'..=b'7').contains(&self.byte_at(at + n)) {
                    n += 1;
                }
                return n;
            }
            b'\r' => return if self.byte_at(at + 2) == b'\n' { 3 } else { 2 },
            _ => return 1 + self.rune_size(at + 1),
        };
        let mut n = 2;
        while n < 2 + hex && self.byte_at(at + n).is_ascii_hexdigit() {
            n += 1;
        }
        n
    }
}

pub(crate) fn is_ident_start(c: u8) -> bool {
    c == b'_' || c.is_ascii_alphabetic() || c >= 0x80
}

pub(crate) fn is_ident_char(c: u8) -> bool {
    is_ident_start(c) || c.is_ascii_digit()
}

pub(crate) fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0c | 0x0b)
}

pub(crate) fn has(set: &str, c: u8) -> bool {
    set.as_bytes().contains(&c)
}

pub(crate) fn in_list(word: &[u8], list: &[&str]) -> bool {
    list.iter().any(|w| w.as_bytes() == word)
}

pub(crate) fn in_list_fold(word: &[u8], list: &[&str]) -> bool {
    list.iter().any(|w| w.as_bytes().eq_ignore_ascii_case(word))
}

/// `s` without leading and trailing whitespace.
pub(crate) fn trim_spaces(s: &[u8]) -> &[u8] {
    let a = s.iter().position(|&c| !is_space(c)).unwrap_or(s.len());
    let b = s.iter().rposition(|&c| !is_space(c)).map_or(a, |i| i + 1);
    &s[a..b]
}

/// `s` without trailing whitespace.
pub(crate) fn trim_right(s: &[u8]) -> &[u8] {
    let b = s.iter().rposition(|&c| !is_space(c)).map_or(0, |i| i + 1);
    &s[..b]
}

pub(crate) fn is_blank(line: &[u8]) -> bool {
    line.iter().all(|&c| is_space(c))
}

pub(crate) fn index_byte_from(s: &[u8], from: usize, c: u8) -> Option<usize> {
    s.get(from..)?.iter().position(|&b| b == c).map(|i| i + from)
}
