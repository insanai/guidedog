//! A reader for the literal part of Python: enough to take a conf.py's settings without
//! running it. Expressions are literals, names bound earlier in the file, `+`, `%`
//! formatting, f-strings, a few `str` methods, and the current year from `datetime` or
//! `time`. Anything else fails with a sentence saying what Python would have computed;
//! nothing is guessed.

mod scan;
mod text;

use std::collections::{HashMap, HashSet};

pub(crate) use scan::{Cursor, group_end, name_char, split_top, statement_end, string_end};

/// How deeply brackets and f-string fields may nest before the reader gives up, so a
/// hostile conf.py cannot overflow the stack.
const MAX_DEPTH: usize = 64;

/// A Python value that conf.toml can hold.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) enum Value {
    #[default]
    None,
    Bool(bool),
    Int(i64),
    /// A float as written, normalized so TOML reads it (`.5` is `0.5`).
    Float(String),
    Str(String),
    /// Lists, tuples and sets.
    List(Vec<Value>),
    /// A dictionary, in the order its keys were written.
    Dict(Vec<(Value, Value)>),
}

impl Value {
    /// Python's name for the value's type, for messages.
    pub(crate) fn kind_name(&self) -> &'static str {
        match self {
            Value::None => "None",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Str(_) => "str",
            Value::List(_) => "list",
            Value::Dict(_) => "dict",
        }
    }

    /// The text of a string, or `None`.
    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// `str(v)` for the values `str()` can show in a setting.
    pub(crate) fn to_text(&self) -> Option<String> {
        match self {
            Value::Str(s) | Value::Float(s) => Some(s.clone()),
            Value::Int(n) => Some(n.to_string()),
            Value::Bool(true) => Some("True".into()),
            Value::Bool(false) => Some("False".into()),
            Value::None => Some("None".into()),
            Value::List(_) | Value::Dict(_) => None,
        }
    }

    /// Whether the value is a list of strings.
    pub(crate) fn is_string_list(&self) -> bool {
        matches!(self, Value::List(items) if items.iter().all(|i| matches!(i, Value::Str(_))))
    }

    /// Python's `==` for keys: scalars only, so lists and dictionaries never match.
    pub(crate) fn same_key(&self, other: &Value) -> bool {
        !matches!(self, Value::List(_) | Value::Dict(_)) && self == other
    }
}

/// `a + b` for text, lists and whole numbers.
pub(crate) fn combine(a: Value, b: Value) -> Result<Value, String> {
    match (a, b) {
        (Value::Str(mut x), Value::Str(y)) => {
            x.push_str(&y);
            Ok(Value::Str(x))
        }
        (Value::List(mut x), Value::List(y)) => {
            x.extend(y);
            Ok(Value::List(x))
        }
        (Value::Int(x), Value::Int(y)) => x
            .checked_add(y)
            .map(Value::Int)
            .ok_or_else(|| "The sum is too large for a setting.".to_string()),
        (a, b) => {
            Err(format!("Adding {} to {} is computed by Python.", b.kind_name(), a.kind_name()))
        }
    }
}

/// What earlier assignments bound, so later expressions can name them.
#[derive(Debug, Default)]
pub(crate) struct Scope {
    pub(crate) values: HashMap<String, Value>,
    /// Names another expression read.
    pub(crate) used: HashSet<String>,
    /// What `datetime`'s and `time`'s current year evaluates to.
    pub(crate) year: Option<i64>,
}

/// Evaluates one Python expression. The error is a sentence saying what Python would
/// have had to compute.
pub(crate) fn eval(src: &str, scope: &mut Scope) -> Result<Value, String> {
    eval_nested(src, scope, 0)
}

fn eval_nested(src: &str, scope: &mut Scope, depth: usize) -> Result<Value, String> {
    let mut e = Evaluator { cur: Cursor::new(src), scope, depth };
    let value = e.expression()?;
    e.cur.space();
    if !e.cur.at_end() {
        return Err(format!("Only literal values are read, and \"{}\" is not one.", e.cur.rest()));
    }
    Ok(value)
}

/// A recursive-descent evaluator over one expression.
struct Evaluator<'s, 'v> {
    cur: Cursor<'s>,
    scope: &'v mut Scope,
    depth: usize,
}

type Eval = Result<Value, String>;

impl Evaluator<'_, '_> {
    fn expression(&mut self) -> Eval {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err("The expression nests too deeply to read.".into());
        }
        let mut left = self.term()?;
        while self.cur.peek("+") && !self.cur.peek("+=") {
            self.cur.advance(1);
            let right = self.term()?;
            left = combine(left, right)?;
        }
        self.depth -= 1;
        Ok(left)
    }

    fn term(&mut self) -> Eval {
        let mut left = self.postfix()?;
        while self.cur.peek("%") && !self.cur.peek("%=") {
            self.cur.advance(1);
            let right = self.postfix()?;
            left = text::percent(&left, &right)?;
        }
        Ok(left)
    }

    fn postfix(&mut self) -> Eval {
        let mut v = self.primary()?;
        loop {
            if self.cur.take("[") {
                let index = self.expression()?;
                if !self.cur.take("]") {
                    return Err("Slices are computed by Python.".into());
                }
                v = index_of(&v, &index)?;
            } else if self.cur.take(".") {
                self.cur.space();
                let method = self.cur.word();
                if !self.cur.take("(") {
                    return Err(format!("The attribute .{method} is computed by Python."));
                }
                let (args, _) = self.items(b')')?;
                v = text::method(&v, method, &args)?;
            } else {
                return Ok(v);
            }
        }
    }

    fn primary(&mut self) -> Eval {
        self.cur.space();
        let Some(c) = self.cur.byte() else {
            return Err("A value is missing.".into());
        };
        match c {
            b'(' => {
                self.cur.advance(1);
                let (mut items, comma) = self.items(b')')?;
                if items.len() == 1 && !comma {
                    return Ok(items.remove(0)); // parentheses, not a tuple.
                }
                Ok(Value::List(items))
            }
            b'[' => {
                self.cur.advance(1);
                Ok(Value::List(self.items(b']')?.0))
            }
            b'{' => self.braces(),
            _ if self.cur.string_ahead() => self.strings(),
            b'-' | b'+' | b'.' | b'0'..=b'9' => self.number(),
            _ if scan::name_char(c, true) => self.name(),
            _ => Err(format!("\"{}\" is not a Python literal.", self.cur.rest())),
        }
    }

    /// Reads comma-separated expressions up to `close`, which it consumes. The flag tells
    /// whether a comma was seen, which makes `(x,)` a tuple.
    fn items(&mut self, close: u8) -> Result<(Vec<Value>, bool), String> {
        let close_text = if close == b')' { ")" } else { "]" };
        let mut list = Vec::new();
        let mut comma = false;
        loop {
            if self.cur.take(close_text) {
                return Ok((list, comma));
            }
            if self.cur.peek("*") {
                return Err("Unpacking with * is computed by Python.".into());
            }
            list.push(self.expression()?);
            if self.cur.word_ahead("for") {
                return Err("A comprehension is computed by Python.".into());
            }
            if self.cur.take(",") {
                comma = true;
                continue;
            }
            if !self.cur.take(close_text) {
                return Err(format!(
                    "Expected a comma or {close_text} before \"{}\".",
                    self.cur.rest()
                ));
            }
            return Ok((list, comma));
        }
    }

    /// Reads a dictionary or a set.
    fn braces(&mut self) -> Eval {
        self.cur.advance(1);
        let mut entries = Vec::new();
        let mut set = Vec::new();
        loop {
            if self.cur.take("}") {
                break;
            }
            if self.cur.peek("**") {
                return Err("Merging dictionaries with ** is computed by Python.".into());
            }
            let first = self.expression()?;
            if self.cur.take(":") {
                entries.push((first, self.expression()?));
            } else {
                set.push(first);
            }
            if self.cur.word_ahead("for") {
                return Err("A comprehension is computed by Python.".into());
            }
            if !self.cur.take(",") {
                if !self.cur.take("}") {
                    return Err("Expected a comma or } in the dictionary.".into());
                }
                break;
            }
        }
        match (set.is_empty(), entries.is_empty()) {
            (false, false) => Err("A dictionary entry is missing its key.".into()),
            (false, true) => Ok(Value::List(set)),
            _ => Ok(Value::Dict(entries)),
        }
    }

    fn number(&mut self) -> Eval {
        let start = self.cur.at;
        let sign = self.cur.byte();
        if matches!(sign, Some(b'-' | b'+')) {
            self.cur.advance(1);
        }
        self.cur.space();
        let written = self.cur.number_token();
        let digits: String = written.chars().filter(|&c| c != '_').collect();
        if !digits.starts_with(|c: char| c.is_ascii_digit() || c == '.') {
            self.cur.at = start;
            return Err(format!("\"{}\" is computed by Python.", self.cur.rest()));
        }
        parse_number(&digits, sign == Some(b'-'))
    }

    fn name(&mut self) -> Eval {
        let start = self.cur.at;
        let word = self.cur.word();
        match word {
            "True" => return Ok(Value::Bool(true)),
            "False" => return Ok(Value::Bool(false)),
            "None" => return Ok(Value::None),
            "str" if self.cur.take("(") => {
                let (args, _) = self.items(b')')?;
                return match args.as_slice() {
                    [one] => one
                        .to_text()
                        .map(Value::Str)
                        .ok_or_else(|| "This str() call is computed by Python.".into()),
                    _ => Err("This str() call is computed by Python.".into()),
                };
            }
            _ => {}
        }
        if let Some(v) = self.scope.values.get(word) {
            let v = v.clone();
            self.scope.used.insert(word.to_string());
            return Ok(v);
        }
        self.opaque(start)
    }

    /// Reads a chain of attributes and calls on a name conf.py did not bind, such as
    /// `datetime.date.today().year`. Only the current year is known without Python.
    fn opaque(&mut self, start: usize) -> Eval {
        let mut last_call = "";
        while let Some(c) = self.cur.byte() {
            match c {
                b'.' => {
                    self.cur.advance(1);
                    self.cur.word();
                }
                b'(' | b'[' => {
                    let open = self.cur.at;
                    self.cur.at = group_end(self.cur.src, open);
                    if c == b'(' {
                        last_call = &self.cur.src[open..self.cur.at];
                    }
                }
                _ => break,
            }
        }
        let chain = &self.cur.src[start..self.cur.at];
        let attribute = chain.ends_with(".year");
        let strftime = chain.contains("strftime(")
            && chain.ends_with(last_call)
            && last_call.contains("%Y")
            && last_call.len() <= 6;
        if let Some(year) = self.scope.year.filter(|_| attribute || strftime) {
            return Ok(if attribute { Value::Int(year) } else { Value::Str(year.to_string()) });
        }
        if chain.contains(['.', '(', '[']) {
            Err(format!("{chain} is computed by Python when Sphinx runs conf.py."))
        } else {
            Err(format!("{chain} is not a literal assigned earlier in conf.py."))
        }
    }

    fn strings(&mut self) -> Eval {
        let mut out = String::new();
        loop {
            self.cur.space();
            if self.cur.at_end() || !self.cur.string_ahead() {
                return Ok(Value::Str(out));
            }
            self.string(&mut out)?;
        }
    }

    /// Reads one string literal, with its prefix, into `out`.
    fn string(&mut self, out: &mut String) -> Result<(), String> {
        let src = self.cur.src;
        let quote = self.cur.at + src[self.cur.at..].find(['"', '\'']).unwrap_or(0);
        let prefix = src[self.cur.at..quote].to_ascii_lowercase();
        if prefix.contains('b') {
            return Err("Bytes literals are not settings.".into());
        }
        let (end, closed) = string_end(src, quote);
        if !closed {
            return Err("A string is not closed.".into());
        }
        let q = &src[quote..quote + 1];
        let triple = src[quote..].starts_with(&q.repeat(3)) && end - quote >= 6;
        let open = if triple { 3 } else { 1 };
        let body = &src[quote + open..end - open];
        let raw = prefix.contains('r');
        self.cur.at = end;
        if prefix.contains('f') {
            self.fstring(body, raw, out)
        } else {
            text::unescape(out, body, raw);
            Ok(())
        }
    }

    /// Writes an f-string, evaluating each `{expression}` it holds.
    fn fstring(&mut self, body: &str, raw: bool, out: &mut String) -> Result<(), String> {
        let bytes = body.as_bytes();
        let (mut start, mut i) = (0, 0);
        while i < bytes.len() {
            let c = bytes[i];
            if matches!(c, b'{' | b'}') && bytes.get(i + 1) == Some(&c) {
                text::unescape(out, &body[start..i], raw);
                out.push(c as char);
                i += 2;
                start = i;
                continue;
            }
            if c == b'}' {
                return Err("A single } in an f-string is not valid Python.".into());
            }
            if c != b'{' {
                i += 1;
                continue;
            }
            text::unescape(out, &body[start..i], raw);
            let close = group_end(body, i);
            if close <= i + 1 || bytes[close - 1] != b'}' {
                return Err("An f-string has an unclosed {.".into());
            }
            let inner = &body[i + 1..close - 1];
            if inner.contains(['!', ':', '=']) {
                return Err(format!("The f-string field {{{inner}}} is computed by Python."));
            }
            let v = eval_nested(inner, self.scope, self.depth + 1)?;
            let shown =
                v.to_text().ok_or_else(|| format!("{{{inner}}} is not text or a number."))?;
            out.push_str(&shown);
            i = close;
            start = i;
        }
        text::unescape(out, &body[start..], raw);
        Ok(())
    }
}

/// Parses the digits of a number literal (underscores removed) as Python reads them.
fn parse_number(digits: &str, negative: bool) -> Eval {
    let lower = digits.to_ascii_lowercase();
    let not_number = || format!("{digits} is not a number.");
    let radix = match lower.get(..2) {
        Some("0x") => Some(16),
        Some("0o") => Some(8),
        Some("0b") => Some(2),
        _ => None,
    };
    if let Some(radix) = radix {
        let n = i64::from_str_radix(&lower[2..], radix).map_err(|_| not_number())?;
        return Ok(Value::Int(if negative { -n } else { n }));
    }
    if lower.contains(['.', 'e']) {
        lower.parse::<f64>().map_err(|_| not_number())?;
        let mut text = String::from(if negative { "-" } else { "" });
        if lower.starts_with('.') {
            text.push('0');
        }
        text.push_str(&lower);
        if lower.ends_with('.') {
            text.push('0');
        }
        return Ok(Value::Float(text));
    }
    let n: i64 = lower.parse().map_err(|_| not_number())?;
    Ok(Value::Int(if negative { -n } else { n }))
}

/// `v[index]` for dictionaries, lists and strings.
fn index_of(v: &Value, index: &Value) -> Eval {
    match (v, index) {
        (Value::Dict(_), key) => lookup(v, key),
        (Value::List(items), &Value::Int(n)) => {
            position(n, items.len()).map(|i| items[i].clone()).ok_or_else(computed_index)
        }
        (Value::Str(s), &Value::Int(n)) => position(n, s.chars().count())
            .and_then(|i| s.chars().nth(i))
            .map(|c| Value::Str(c.to_string()))
            .ok_or_else(computed_index),
        _ => Err(computed_index()),
    }
}

fn computed_index() -> String {
    "This index is computed by Python.".into()
}

/// The position Python's index `n` (negative from the end) names in `count` items.
fn position(n: i64, count: usize) -> Option<usize> {
    let count = i64::try_from(count).ok()?;
    let n = if n < 0 { n + count } else { n };
    (0..count).contains(&n).then(|| usize::try_from(n).ok()).flatten()
}

/// `dict[key]`.
pub(crate) fn lookup(dict: &Value, key: &Value) -> Eval {
    if let Value::Dict(entries) = dict
        && let Some((_, v)) = entries.iter().find(|(k, _)| k.same_key(key))
    {
        return Ok(v.clone());
    }
    let shown = key.to_text().unwrap_or_default();
    Err(format!("The key {shown} is not in the dictionary."))
}

#[cfg(test)]
mod tests;
