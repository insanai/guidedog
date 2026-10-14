//! The JSON grammar, read with bounded nesting.

use crate::{Error, ErrorKind, Object, Value};

/// Bounds on what [`parse_with`] accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The deepest nesting of arrays and objects (default 128).
    pub max_depth: usize,
    /// Whether an object may repeat a key; the last value then wins (default false).
    pub allow_duplicate_keys: bool,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits { max_depth: 128, allow_duplicate_keys: false }
    }
}

type Result<T> = std::result::Result<T, Error>;

struct Parser<'a> {
    src: &'a str,
    b: &'a [u8],
    at: usize,
    depth: usize,
    limits: Limits,
}

/// Reads `text` as one JSON value with the default [`Limits`]. The value owns its
/// strings. Fails with an [`Error`] at the first mistake, including text after the value.
pub fn parse(text: &str) -> Result<Value> {
    parse_with(text, &Limits::default())
}

/// Reads `text` as one JSON value within `limits`; a leading byte order mark is skipped.
/// Fails as [`parse`] does, and with [`ErrorKind::Depth`] beyond `limits.max_depth`.
pub fn parse_with(text: &str, limits: &Limits) -> Result<Value> {
    let start = if text.starts_with('\u{feff}') { 3 } else { 0 };
    let mut p = Parser { src: text, b: text.as_bytes(), at: start, depth: 0, limits: *limits };
    p.skip_space();
    let value = p.value()?;
    p.skip_space();
    if p.at < p.b.len() {
        return Err(p.fail(
            ErrorKind::Syntax,
            p.at,
            "There is more text after the JSON value.",
            "A JSON text holds one value; wrap several in an array [ ... ].",
        ));
    }
    Ok(value)
}

impl Parser<'_> {
    fn fail(&self, kind: ErrorKind, at: usize, message: impl Into<String>, hint: &str) -> Error {
        let offset = gd_report::floor_boundary(self.src, at);
        let end = gd_report::floor_boundary(self.src, offset + 1);
        let (line, column) = gd_report::line_column(self.src, offset);
        Error { kind, message: message.into(), hint: hint.to_string(), offset, end, line, column }
    }

    fn byte(&self) -> Option<u8> {
        self.b.get(self.at).copied()
    }

    fn skip_space(&mut self) {
        while matches!(self.byte(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn literal(&mut self, word: &str, value: Value) -> Result<Value> {
        if self.b[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            return Ok(value);
        }
        Err(self.unexpected())
    }

    fn unexpected(&self) -> Error {
        let message = match self.byte() {
            None => "The text ends where a value should be.",
            Some(_) => "Expected a value here.",
        };
        let hint = "A value is a \"string\", a number, true, false, null, an array [ ... ] or \
                    an object { ... }; JSON has no comments or trailing commas.";
        self.fail(ErrorKind::Syntax, self.at, message, hint)
    }

    /// Reads one value; arrays and objects recurse through here, bounded by depth.
    fn value(&mut self) -> Result<Value> {
        match self.byte() {
            Some(b'{') => self.nested(Parser::object),
            Some(b'[') => self.nested(Parser::array),
            Some(b'"') => self.string().map(Value::String),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(self.unexpected()),
        }
    }

    fn nested(&mut self, read: fn(&mut Self) -> Result<Value>) -> Result<Value> {
        if self.depth >= self.limits.max_depth {
            let message =
                format!("Arrays and objects nest more than {} deep.", self.limits.max_depth);
            let hint = "Flatten the data, or raise the nesting limit if the text is trusted.";
            return Err(self.fail(ErrorKind::Depth, self.at, message, hint));
        }
        self.depth += 1;
        let value = read(self);
        self.depth -= 1;
        value
    }

    /// After a value in an array or object: consumes `,` (returns true) or `close`.
    fn separator(&mut self, close: u8, what: &str) -> Result<bool> {
        self.skip_space();
        match self.byte() {
            Some(b',') => {
                self.at += 1;
                self.skip_space();
                Ok(true)
            }
            Some(c) if c == close => {
                self.at += 1;
                Ok(false)
            }
            _ => {
                let message = format!("Expected , or {} in this {what}.", close as char);
                let hint = "Separate items with commas and close every [ and { you open.";
                Err(self.fail(ErrorKind::Syntax, self.at, message, hint))
            }
        }
    }

    fn array(&mut self) -> Result<Value> {
        self.at += 1;
        self.skip_space();
        let mut items = Vec::new();
        if self.byte() == Some(b']') {
            self.at += 1;
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.value()?);
            if !self.separator(b']', "array")? {
                return Ok(Value::Array(items));
            }
        }
    }

    fn object(&mut self) -> Result<Value> {
        self.at += 1;
        self.skip_space();
        let mut object = Object::new();
        if self.byte() == Some(b'}') {
            self.at += 1;
            return Ok(Value::Object(object));
        }
        loop {
            let key_at = self.at;
            if self.byte() != Some(b'"') {
                let hint = "Object keys are strings in double quotes, as in {\"name\": 1}.";
                return Err(self.fail(
                    ErrorKind::Syntax,
                    key_at,
                    "Expected a key in quotes.",
                    hint,
                ));
            }
            let key = self.string()?;
            self.skip_space();
            if self.byte() != Some(b':') {
                let hint = "Write each member as \"key\": value.";
                return Err(self.fail(
                    ErrorKind::Syntax,
                    self.at,
                    "Expected : after the key.",
                    hint,
                ));
            }
            self.at += 1;
            self.skip_space();
            let value = self.value()?;
            if object.contains_key(&key) {
                if !self.limits.allow_duplicate_keys {
                    let message = format!("The key \"{key}\" appears twice in this object.");
                    let hint = "Remove one of them; each key may appear once.";
                    return Err(self.fail(ErrorKind::Duplicate, key_at, message, hint));
                }
                object.insert(&key, value);
            } else {
                object.push(key, value);
            }
            if !self.separator(b'}', "object")? {
                return Ok(Value::Object(object));
            }
        }
    }

    fn number(&mut self) -> Result<Value> {
        let start = self.at;
        let digits = |p: &mut Self| {
            let from = p.at;
            while p.byte().is_some_and(|c| c.is_ascii_digit()) {
                p.at += 1;
            }
            p.at - from
        };
        if self.byte() == Some(b'-') {
            self.at += 1;
        }
        let int_start = self.at;
        let whole = digits(self);
        let leading_zero = whole > 1 && self.b[int_start] == b'0';
        let mut is_float = false;
        let mut ok = whole > 0 && !leading_zero;
        if ok && self.byte() == Some(b'.') {
            self.at += 1;
            is_float = true;
            ok = digits(self) > 0;
        }
        if ok && matches!(self.byte(), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.byte(), Some(b'+' | b'-')) {
                self.at += 1;
            }
            is_float = true;
            ok = digits(self) > 0;
        }
        if !ok {
            let hint = "Write numbers as 12, -3.5 or 1e6: no leading zeros, digits on both \
                        sides of the point, no + sign, and no NaN or Infinity.";
            return Err(self.fail(ErrorKind::Number, start, "This number is malformed.", hint));
        }
        let text = &self.src[start..self.at];
        if !is_float && let Ok(n) = text.parse::<i64>() {
            return Ok(Value::Integer(n));
        }
        // A well-formed number always parses as f64 (huge ones become infinite).
        Ok(Value::Float(text.parse::<f64>().unwrap_or(f64::NAN)))
    }

    /// Reads a string at its opening quote, resolving escapes.
    fn string(&mut self) -> Result<String> {
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
                Some(c) if c < 0x20 => {
                    let hint = "Write line breaks and tabs in strings as \\n and \\t.";
                    let message = "This string holds a raw control character.";
                    return Err(self.fail(ErrorKind::String, self.at, message, hint));
                }
                Some(_) => self.at += 1,
                None => {
                    let hint = "End the string with a double quote.";
                    return Err(self.fail(
                        ErrorKind::String,
                        open,
                        "This string is not closed.",
                        hint,
                    ));
                }
            }
        }
    }

    fn escape(&mut self, out: &mut String) -> Result<()> {
        let start = self.at;
        let c = self.b.get(start + 1).copied();
        self.at += 2;
        let plain = match c {
            Some(b'"') => '"',
            Some(b'\\') => '\\',
            Some(b'/') => '/',
            Some(b'b') => '\u{8}',
            Some(b'f') => '\u{c}',
            Some(b'n') => '\n',
            Some(b'r') => '\r',
            Some(b't') => '\t',
            Some(b'u') => self.unicode_escape(start)?,
            _ => {
                let hint = "Use \\\" \\\\ \\/ \\b \\f \\n \\r \\t or \\uXXXX.";
                return Err(self.fail(ErrorKind::String, start, "Unknown escape sequence.", hint));
            }
        };
        out.push(plain);
        Ok(())
    }

    fn hex4(&mut self) -> Option<u32> {
        let digits = self.b.get(self.at..self.at + 4)?;
        let mut n = 0;
        for &d in digits {
            n = n * 16 + (d as char).to_digit(16)?;
        }
        self.at += 4;
        Some(n)
    }

    /// Reads the digits of `\uXXXX`, joining a UTF-16 surrogate pair into one character.
    fn unicode_escape(&mut self, start: usize) -> Result<char> {
        let bad = |p: &Self| {
            let hint = "Write \\u with four hex digits; characters beyond U+FFFF are a \
                        surrogate pair such as \\ud83d\\ude00.";
            p.fail(ErrorKind::String, start, "This \\u escape is not a valid character.", hint)
        };
        let high = self.hex4().ok_or_else(|| bad(self))?;
        if !(0xD800..0xDC00).contains(&high) {
            return char::from_u32(high).ok_or_else(|| bad(self));
        }
        if !self.b[self.at..].starts_with(b"\\u") {
            return Err(bad(self));
        }
        self.at += 2;
        let low = self.hex4().ok_or_else(|| bad(self))?;
        if !(0xDC00..0xE000).contains(&low) {
            return Err(bad(self));
        }
        char::from_u32(0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00)).ok_or_else(|| bad(self))
    }
}
