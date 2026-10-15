//! Scanning Python source: a cursor over one statement or expression, and the bracket-
//! and string-aware searches the statement reader shares with the evaluator. Every index
//! these return is at an ASCII byte (a quote, bracket, separator or newline) or at the
//! end, so slicing there is always on a character boundary.

/// A position in Python source, with the small vocabulary the readers need.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Cursor<'s> {
    pub(crate) src: &'s str,
    pub(crate) at: usize,
}

impl<'s> Cursor<'s> {
    pub(crate) fn new(src: &'s str) -> Cursor<'s> {
        Cursor { src, at: 0 }
    }

    pub(crate) fn at_end(&self) -> bool {
        self.at >= self.src.len()
    }

    /// The byte at the cursor, or `None` at the end.
    pub(crate) fn byte(&self) -> Option<u8> {
        self.src.as_bytes().get(self.at).copied()
    }

    pub(crate) fn advance(&mut self, n: usize) {
        self.at = (self.at + n).min(self.src.len());
    }

    /// What is left, trimmed, for messages.
    pub(crate) fn rest(&self) -> &'s str {
        self.src[self.at..].trim()
    }

    /// What is left, untrimmed.
    pub(crate) fn tail(&self) -> &'s str {
        &self.src[self.at..]
    }

    /// Skips spaces, comments and backslash line continuations.
    pub(crate) fn space(&mut self) {
        let bytes = self.src.as_bytes();
        while let Some(&c) = bytes.get(self.at) {
            match c {
                b' ' | b'\t' | b'\r' | b'\n' | 0x0c => self.at += 1,
                b'#' => self.skip_line(),
                b'\\' if matches!(bytes.get(self.at + 1), Some(b'\n' | b'\r')) => self.at += 2,
                _ => return,
            }
        }
    }

    fn skip_line(&mut self) {
        self.at = self.src[self.at..].find('\n').map_or(self.src.len(), |i| self.at + i);
    }

    /// Whether `s` follows, after spaces.
    pub(crate) fn peek(&mut self, s: &str) -> bool {
        self.space();
        self.tail().starts_with(s)
    }

    /// Consumes `s` when it follows, after spaces.
    pub(crate) fn take(&mut self, s: &str) -> bool {
        if !self.peek(s) {
            return false;
        }
        self.at += s.len();
        true
    }

    /// Reads a Python name (ASCII letters, digits, `_`); empty when none is here.
    pub(crate) fn word(&mut self) -> &'s str {
        let start = self.at;
        let bytes = self.src.as_bytes();
        while bytes.get(self.at).is_some_and(|&c| name_char(c, self.at == start)) {
            self.at += 1;
        }
        &self.src[start..self.at]
    }

    /// Whether the keyword `word` follows, after spaces, as a whole word.
    pub(crate) fn word_ahead(&mut self, word: &str) -> bool {
        self.space();
        let rest = self.tail();
        rest.starts_with(word)
            && rest.as_bytes().get(word.len()).is_none_or(|&c| !name_char(c, false))
    }

    /// Whether a string literal starts here, with an optional prefix such as `r` or `f`.
    pub(crate) fn string_ahead(&self) -> bool {
        let bytes = self.src.as_bytes();
        let mut i = self.at;
        while i < bytes.len() && i - self.at < 3 && name_char(bytes[i], true) {
            i += 1;
        }
        if !matches!(bytes.get(i), Some(b'"' | b'\'')) {
            return false;
        }
        let prefix = self.src[self.at..i].to_ascii_lowercase();
        matches!(prefix.as_str(), "" | "r" | "u" | "f" | "rf" | "fr" | "b" | "rb" | "br")
    }

    /// Reads the characters of a number literal: name characters, dots, and the sign of
    /// an exponent.
    pub(crate) fn number_token(&mut self) -> &'s str {
        let start = self.at;
        let bytes = self.src.as_bytes();
        while let Some(&c) = bytes.get(self.at) {
            let exponent = matches!(c, b'+' | b'-')
                && self.at > start
                && matches!(bytes[self.at - 1], b'e' | b'E');
            if !name_char(c, false) && c != b'.' && !exponent {
                break;
            }
            self.at += 1;
        }
        &self.src[start..self.at]
    }
}

/// Whether `c` may be part of a Python name; digits are not allowed `first`.
pub(crate) fn name_char(c: u8, first: bool) -> bool {
    c == b'_' || c.is_ascii_alphabetic() || (!first && c.is_ascii_digit())
}

/// The index of the newline ending the statement at `at`: the first newline outside
/// brackets and strings that no backslash continues, or the end of `src`.
pub(crate) fn statement_end(src: &str, at: usize) -> usize {
    let bytes = src.as_bytes();
    let mut depth = 0usize;
    let mut i = at;
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' => {
                i = string_end(src, i).0;
                continue;
            }
            b'#' => {
                i = src[i..].find('\n').map_or(src.len(), |n| i + n);
                continue;
            }
            b'\\' => {
                i += 2;
                continue;
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b'\n' if depth == 0 => return i,
            _ => {}
        }
        i += 1;
    }
    src.len()
}

/// The index after the string literal whose quote is at `at`, and whether it is closed.
/// An unclosed one-line string ends at its newline.
pub(crate) fn string_end(src: &str, at: usize) -> (usize, bool) {
    let bytes = src.as_bytes();
    let q = bytes[at];
    let triple = bytes.get(at + 1) == Some(&q) && bytes.get(at + 2) == Some(&q);
    let mut i = at + if triple { 3 } else { 1 };
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\\' {
            i += 2;
            continue;
        }
        if c == q && !triple {
            return (i + 1, true);
        }
        if c == q && bytes.get(i + 1) == Some(&q) && bytes.get(i + 2) == Some(&q) {
            return (i + 3, true);
        }
        if c == b'\n' && !triple {
            return (i, false);
        }
        i += 1;
    }
    (src.len(), false)
}

/// The index after the bracket group opening at `at`, or the end of `src` when it is
/// not closed.
pub(crate) fn group_end(src: &str, at: usize) -> usize {
    let bytes = src.as_bytes();
    let mut depth = 0usize;
    let mut i = at;
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' => {
                i = string_end(src, i).0;
                continue;
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    src.len()
}

/// Splits Python source at `sep` where it is outside brackets, strings and comments.
pub(crate) fn split_top(src: &str, sep: u8) -> Vec<&str> {
    let bytes = src.as_bytes();
    let mut parts = Vec::new();
    let (mut depth, mut start, mut i) = (0isize, 0, 0);
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' => {
                i = string_end(src, i).0;
                continue;
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'#' => {
                i = src[i..].find('\n').map_or(src.len(), |n| i + n);
                continue;
            }
            c if c == sep && depth == 0 => {
                parts.push(&src[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(&src[start..]);
    parts
}
