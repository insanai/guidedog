//! The four string kinds: basic, literal, and their multi-line forms.

use crate::ErrorKind;
use crate::parse::{Parser, Result};

fn is_control(c: u8) -> bool {
    (c < 0x20 && c != b'\t') || c == 0x7f
}

const CONTROL_HINT: &str = "Write control characters as escapes in a \"basic\" string, \
                            such as \\t or \\u001b.";

impl Parser<'_> {
    fn unclosed(&self, open: usize, what: &str, hint: &str) -> crate::Error {
        let message = format!("This {what} is not closed.");
        self.fail(ErrorKind::String, open, open + 1, message, hint)
    }

    fn control(&self) -> crate::Error {
        let message = "This string holds a raw control character.";
        self.fail(ErrorKind::String, self.at, self.at + 1, message, CONTROL_HINT)
    }

    /// Reads `"..."` with escapes.
    pub(crate) fn basic_string(&mut self) -> Result<String> {
        let open = self.at;
        self.at += 1;
        let mut out = String::new();
        let mut run = self.at;
        loop {
            match self.byte() {
                Some(b'"') => {
                    out.push_str(&self.src[run..self.at]);
                    self.at += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    out.push_str(&self.src[run..self.at]);
                    self.escape(&mut out)?;
                    run = self.at;
                }
                None | Some(b'\n' | b'\r') => {
                    let hint = "End it with \" on the same line, or use \"\"\" for text \
                                spanning lines.";
                    return Err(self.unclosed(open, "string", hint));
                }
                Some(c) if is_control(c) => return Err(self.control()),
                Some(_) => self.at += 1,
            }
        }
    }

    /// Reads `'...'`, borrowed from the source.
    pub(crate) fn literal_string(&mut self) -> Result<&str> {
        let open = self.at;
        self.at += 1;
        let start = self.at;
        loop {
            match self.byte() {
                Some(b'\'') => {
                    self.at += 1;
                    return Ok(&self.src[start..self.at - 1]);
                }
                None | Some(b'\n' | b'\r') => {
                    let hint = "End it with ' on the same line, or use ''' for text \
                                spanning lines.";
                    return Err(self.unclosed(open, "literal string", hint));
                }
                Some(c) if is_control(c) => return Err(self.control()),
                Some(_) => self.at += 1,
            }
        }
    }

    fn skip_first_newline(&mut self) {
        if self.peek("\r\n") {
            self.at += 2;
        } else if self.peek("\n") {
            self.at += 1;
        }
    }

    /// At a run of `quote` bytes: when it closes the string, returns the number of quotes
    /// that belong to the content (0 to 2) and moves past the run.
    fn closing_run(&mut self, quote: u8) -> Result<Option<usize>> {
        let n = self.b[self.at..].iter().take_while(|&&c| c == quote).count();
        if n < 3 {
            self.at += n;
            return Ok(None);
        }
        if n > 5 {
            let message = "Too many quotes end this multi-line string.";
            let hint = "At most two quotes may come right before the closing three; escape \
                        the others or use the other string kind.";
            return Err(self.fail(ErrorKind::String, self.at, self.at + n, message, hint));
        }
        self.at += n;
        Ok(Some(n - 3))
    }

    /// Reads `"""..."""` with escapes and line-ending backslashes.
    pub(crate) fn multiline_basic(&mut self) -> Result<String> {
        let open = self.at;
        self.at += 3;
        self.skip_first_newline();
        let mut out = String::new();
        let mut run = self.at;
        loop {
            match self.byte() {
                Some(b'"') => {
                    let before = self.at;
                    if let Some(extra) = self.closing_run(b'"')? {
                        out.push_str(&self.src[run..before + extra]);
                        return Ok(out);
                    }
                }
                Some(b'\\') => {
                    out.push_str(&self.src[run..self.at]);
                    if !self.line_ending_backslash() {
                        self.escape(&mut out)?;
                    }
                    run = self.at;
                }
                Some(b'\r') if self.b.get(self.at + 1) == Some(&b'\n') => self.at += 2,
                Some(b'\n') => self.at += 1,
                Some(c) if is_control(c) => return Err(self.control()),
                Some(_) => self.at += 1,
                None => {
                    let hint = "End it with \"\"\".";
                    return Err(self.unclosed(open, "multi-line string", hint));
                }
            }
        }
    }

    /// Reads `'''...'''`, borrowed from the source.
    pub(crate) fn multiline_literal(&mut self) -> Result<&str> {
        let open = self.at;
        self.at += 3;
        self.skip_first_newline();
        let start = self.at;
        loop {
            match self.byte() {
                Some(b'\'') => {
                    let before = self.at;
                    if let Some(extra) = self.closing_run(b'\'')? {
                        return Ok(&self.src[start..before + extra]);
                    }
                }
                Some(b'\r') if self.b.get(self.at + 1) == Some(&b'\n') => self.at += 2,
                Some(b'\n') => self.at += 1,
                Some(c) if is_control(c) => return Err(self.control()),
                Some(_) => self.at += 1,
                None => {
                    let hint = "End it with '''.";
                    return Err(self.unclosed(open, "multi-line literal string", hint));
                }
            }
        }
    }

    /// At a backslash: when only white space follows it on its line, skips it, the line
    /// break and all white space after, and returns true.
    fn line_ending_backslash(&mut self) -> bool {
        let mut i = self.at + 1;
        while matches!(self.b.get(i), Some(b' ' | b'\t')) {
            i += 1;
        }
        match (self.b.get(i), self.b.get(i + 1)) {
            (Some(b'\n'), _) | (Some(b'\r'), Some(b'\n')) => {}
            _ => return false,
        }
        while matches!(self.b.get(i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            i += 1;
        }
        self.at = i;
        true
    }

    /// At a backslash: appends the escaped character to `out` and moves past it.
    fn escape(&mut self, out: &mut String) -> Result<()> {
        let start = self.at;
        self.at += 1;
        let c = self.byte().unwrap_or(b' ');
        self.at += 1;
        let plain = match c {
            b'b' => '\u{8}',
            b't' => '\t',
            b'n' => '\n',
            b'f' => '\u{c}',
            b'r' => '\r',
            b'e' => '\u{1b}',
            b'"' => '"',
            b'\\' => '\\',
            b'u' | b'U' => self.unicode_escape(start, if c == b'u' { 4 } else { 8 })?,
            _ => {
                let message = "This escape sequence is not one TOML knows.";
                let hint = "Use \\b \\t \\n \\f \\r \\\" \\\\ \\uXXXX or \\UXXXXXXXX; write \
                            \\\\ for a backslash, or use a 'literal' string.";
                return Err(self.fail(ErrorKind::String, start, start + 2, message, hint));
            }
        };
        out.push(plain);
        Ok(())
    }

    fn unicode_escape(&mut self, start: usize, n: usize) -> Result<char> {
        let digits =
            self.b.get(self.at..self.at + n).filter(|d| d.iter().all(u8::is_ascii_hexdigit));
        let code = digits
            .and_then(|d| std::str::from_utf8(d).ok())
            .and_then(|d| u32::from_str_radix(d, 16).ok())
            .and_then(char::from_u32);
        let Some(c) = code else {
            let message =
                format!("This escape needs {n} hexadecimal digits naming a Unicode scalar value.");
            let hint = "Write \\u00e9 or \\U0001F600; surrogates D800 to DFFF are not allowed.";
            return Err(self.fail(ErrorKind::String, start, start + 2 + n, message, hint));
        };
        self.at += n;
        Ok(c)
    }
}
