//! A reader for the Python that Pygments' and Sphinx's style modules are written in,
//! read as data, never run: module and class assignments, functions that return an
//! expression, and expressions of strings, f-strings, names, attributes, subscripts,
//! calls, dicts (with `**`) and `+`. Assignments are evaluated only when a style uses
//! them, so the rest of a module is only skipped.

use std::collections::HashMap;

/// What reading or evaluating fails with: a message naming the module.
pub(super) type PyResult<T> = Result<T, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A name or a number.
    Name,
    String,
    FString,
    Op,
    /// The end of a logical line.
    Newline,
}

#[derive(Debug, Clone)]
struct Tok {
    kind: Kind,
    /// A name, an operator, or a string's contents.
    text: String,
    /// The column of a line's first token, `None` for the others.
    indent: Option<usize>,
}

impl Tok {
    fn is_op(&self, op: &str) -> bool {
        self.kind == Kind::Op && self.text == op
    }

    fn is_word(&self, word: &str) -> bool {
        self.kind == Kind::Name && self.text == word
    }

    fn starts_line(&self) -> bool {
        self.indent == Some(0)
    }
}

/// A value an expression evaluates to.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Value {
    Str(String),
    /// A token type's dotted name without "Token.", such as "Literal.String"; the
    /// root `Token` is "".
    Token(String),
    /// A dict's entries in insertion order.
    Dict(Vec<(Value, Value)>),
    /// A class, by its index in [`Program::classes`].
    Class(usize),
}

/// The names `pygments.token` defines; `Token` is the root, whose name is empty.
const TOKEN_ROOTS: [(&str, &str); 15] = [
    ("Token", ""),
    ("Text", "Text"),
    ("Whitespace", "Text.Whitespace"),
    ("Escape", "Escape"),
    ("Error", "Error"),
    ("Other", "Other"),
    ("Keyword", "Keyword"),
    ("Name", "Name"),
    ("Literal", "Literal"),
    ("String", "Literal.String"),
    ("Number", "Literal.Number"),
    ("Punctuation", "Punctuation"),
    ("Operator", "Operator"),
    ("Comment", "Comment"),
    ("Generic", "Generic"),
];

#[derive(Debug)]
struct Module {
    path: String,
    toks: Vec<Tok>,
    /// A module-level name's expression, as its first token.
    vars: HashMap<String, usize>,
    /// A function's parameter and the first token of the expression it returns.
    funcs: HashMap<String, (String, usize)>,
}

/// A class statement: its name, its first base, and its body's assignments.
#[derive(Debug)]
pub(super) struct Class {
    pub name: String,
    pub base: String,
    module: usize,
    attrs: HashMap<String, usize>,
}

/// Every module read, and every class in them, for names one module imports from
/// another.
#[derive(Debug, Default)]
pub(super) struct Program {
    modules: Vec<Module>,
    /// The classes in the order they were read.
    pub classes: Vec<Class>,
}

#[derive(Debug, Clone)]
struct Scope {
    module: usize,
    /// The class whose body is evaluated.
    class: Option<usize>,
    /// A function's parameter and its argument.
    param: Option<(String, Value)>,
}

impl Program {
    /// Reads a module's top-level assignments, functions and classes.
    pub fn read_module(&mut self, path: &str, source: &str) -> PyResult<()> {
        let toks = tokenize(source).map_err(|err| format!("{path}: {err}"))?;
        let index = self.modules.len();
        let (mut vars, mut funcs) = (HashMap::new(), HashMap::new());
        let mut i = 0;
        while i < toks.len() {
            let t = &toks[i];
            let assigns = toks.get(i + 1).is_some_and(|n| n.is_op("="));
            if !t.starts_line() {
                // Statements inside functions and classes are read with their header.
            } else if t.is_word("def") && i + 3 < toks.len() {
                if let Some(body) = returned_expression(&toks, i) {
                    funcs.insert(toks[i + 1].text.clone(), (toks[i + 3].text.clone(), body));
                }
            } else if t.is_word("class") && i + 1 < toks.len() {
                self.classes.push(read_class(&toks, i, index));
            } else if t.kind == Kind::Name && assigns {
                vars.insert(t.text.clone(), i + 2);
            }
            i = next_line(&toks, i);
        }
        self.modules.push(Module { path: path.to_string(), toks, vars, funcs });
        Ok(())
    }

    /// The first class read with this name.
    pub fn find_class(&self, name: &str) -> Option<usize> {
        self.classes.iter().position(|c| c.name == name)
    }

    /// Evaluates a class attribute, looked up through the class's bases as Python's
    /// attribute access does, in the scope of the class that assigns it.
    pub fn class_attr(&self, class: usize, name: &str) -> PyResult<Option<Value>> {
        let mut current = Some(class);
        while let Some(c) = current {
            let found = &self.classes[c];
            if let Some(&pos) = found.attrs.get(name) {
                let scope = Scope { module: found.module, class: Some(c), param: None };
                return self.evaluate(pos, scope).map(Some);
            }
            current = self.find_class(&found.base);
        }
        Ok(None)
    }

    /// The path of the module a class is in, for messages.
    pub fn class_path(&self, class: usize) -> &str {
        &self.modules[self.classes[class].module].path
    }

    fn evaluate(&self, pos: usize, scope: Scope) -> PyResult<Value> {
        let toks = &self.modules[scope.module].toks;
        let mut parser = Parser { program: self, toks, pos, scope };
        parser.expression()
    }
}

/// The first token of the expression a top-level `def` at `toks[i]` returns, when its
/// body is a `return` statement.
fn returned_expression(toks: &[Tok], i: usize) -> Option<usize> {
    let mut body = next_line(toks, i);
    while body < toks.len() && !toks[body].starts_line() && !toks[body].is_word("return") {
        body = next_line(toks, body);
    }
    (body < toks.len() && toks[body].is_word("return")).then_some(body + 1)
}

/// Reads the class whose header is `toks[i]`: its base and its body's assignments.
fn read_class(toks: &[Tok], i: usize, module: usize) -> Class {
    let mut class = Class {
        name: toks[i + 1].text.clone(),
        base: String::new(),
        module,
        attrs: HashMap::new(),
    };
    if toks.get(i + 2).is_some_and(|t| t.is_op("(")) {
        class.base = toks.get(i + 3).map(|t| t.text.clone()).unwrap_or_default();
    }
    let mut j = next_line(toks, i);
    while j < toks.len() && !toks[j].starts_line() {
        if toks[j].kind == Kind::Name && toks.get(j + 1).is_some_and(|t| t.is_op("=")) {
            class.attrs.insert(toks[j].text.clone(), j + 2);
        }
        j = next_line(toks, j);
    }
    class
}

/// The first token after the logical line holding `toks[i]`.
fn next_line(toks: &[Tok], i: usize) -> usize {
    let mut j = i;
    while j < toks.len() && toks[j].kind != Kind::Newline {
        j += 1;
    }
    j + 1
}

/// Splits Python source into tokens, dropping comments and the line breaks inside
/// brackets or after a backslash.
fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let bytes = src.as_bytes();
    let mut toks = Vec::new();
    let (mut depth, mut i, mut line_begin, mut line_start) = (0i32, 0, 0, true);
    while i < bytes.len() {
        let c = bytes[i];
        let indent = (line_start && depth == 0).then_some(i - line_begin);
        match c {
            b'\n' => {
                if depth == 0 && !line_start {
                    toks.push(Tok { kind: Kind::Newline, text: String::new(), indent: None });
                }
                line_start = line_start || depth == 0;
                i += 1;
                line_begin = i;
                continue;
            }
            b' ' | b'\t' | b'\r' => {
                i += 1;
                continue;
            }
            b'#' => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'\\' if bytes.get(i + 1) == Some(&b'\n') => {
                i += 2;
                continue;
            }
            b'"' | b'\'' => toks.push(scan_string(src, &mut i, "", indent)?),
            _ if is_name_byte(c) => toks.push(scan_name(src, &mut i, indent)?),
            _ => toks.push(scan_operator(src, &mut i, &mut depth, indent)),
        }
        line_start = false;
    }
    if !line_start {
        toks.push(Tok { kind: Kind::Newline, text: String::new(), indent: None });
    }
    Ok(toks)
}

fn is_name_byte(c: u8) -> bool {
    c == b'_' || c.is_ascii_alphanumeric()
}

/// Reads a name or number, or a string with a prefix such as `r` or `f`.
fn scan_name(src: &str, i: &mut usize, indent: Option<usize>) -> Result<Tok, String> {
    let bytes = src.as_bytes();
    let mut j = *i;
    while j < bytes.len() && is_name_byte(bytes[j]) {
        j += 1;
    }
    let word = &src[*i..j];
    let is_prefix = word.len() <= 2 && word.bytes().all(|b| b"rRbBuUfF".contains(&b));
    if is_prefix && matches!(bytes.get(j), Some(b'"' | b'\'')) {
        *i = j;
        return scan_string(src, i, word, indent);
    }
    *i = j;
    Ok(Tok { kind: Kind::Name, text: word.to_string(), indent })
}

fn scan_operator(src: &str, i: &mut usize, depth: &mut i32, indent: Option<usize>) -> Tok {
    let bytes = src.as_bytes();
    let n = if bytes[*i] == b'*' && bytes.get(*i + 1) == Some(&b'*') { 2 } else { 1 };
    match bytes[*i] {
        b'(' | b'[' | b'{' => *depth += 1,
        b')' | b']' | b'}' => *depth -= 1,
        _ => {}
    }
    // Operators are ASCII; anything else is taken whole, as one character.
    let len = src[*i..].chars().next().map_or(1, char::len_utf8).max(n);
    let tok = Tok { kind: Kind::Op, text: src[*i..*i + len].to_string(), indent };
    *i += len;
    tok
}

/// Reads a string literal at `src[*i]`, single or triple quoted; escapes of quotes and
/// backslashes are undone unless the prefix makes it raw.
fn scan_string(
    src: &str,
    i: &mut usize,
    prefix: &str,
    indent: Option<usize>,
) -> Result<Tok, String> {
    let bytes = src.as_bytes();
    let q = bytes[*i];
    let triple = bytes.get(*i + 1) == Some(&q) && bytes.get(*i + 2) == Some(&q);
    let n = if triple { 3 } else { 1 };
    let start = *i + n;
    let mut j = start;
    while j < bytes.len() {
        if bytes[j] == b'\\' {
            j += 2;
            continue;
        }
        let closes = !triple || (bytes.get(j + 1) == Some(&q) && bytes.get(j + 2) == Some(&q));
        if bytes[j] == q && closes {
            break;
        }
        j += 1;
    }
    if j >= bytes.len() {
        return Err("an unterminated string literal".to_string());
    }
    let mut text = src[start..j].to_string();
    *i = j + n;
    if !prefix.contains(['r', 'R']) && text.contains('\\') {
        text = text
            .replace("\\\\", "\0")
            .replace("\\'", "'")
            .replace("\\\"", "\"")
            .replace('\0', "\\");
    }
    let kind = if prefix.contains(['f', 'F']) { Kind::FString } else { Kind::String };
    Ok(Tok { kind, text, indent })
}

/// Evaluates one expression from its first token.
struct Parser<'p> {
    program: &'p Program,
    toks: &'p [Tok],
    pos: usize,
    scope: Scope,
}

impl Parser<'_> {
    fn path(&self) -> &str {
        &self.program.modules[self.scope.module].path
    }

    fn error(&self, message: &str) -> String {
        format!("{}: {message}", self.path())
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn accept(&mut self, op: &str) -> bool {
        let found = self.peek().is_some_and(|t| t.is_op(op));
        if found {
            self.pos += 1;
        }
        found
    }

    fn expect(&mut self, op: &str) -> PyResult<()> {
        if self.accept(op) {
            return Ok(());
        }
        let found = self.peek().map_or("the end", |t| t.text.as_str());
        Err(self.error(&format!("expected {op} but found {found}")))
    }

    fn expression(&mut self) -> PyResult<Value> {
        let mut value = self.postfix()?;
        while self.accept("+") {
            let right = self.postfix()?;
            match (&mut value, right) {
                (Value::Str(left), Value::Str(right)) => left.push_str(&right),
                _ => return Err(self.error("+ joins strings only")),
            }
        }
        Ok(value)
    }

    fn postfix(&mut self) -> PyResult<Value> {
        let mut value = self.primary()?;
        loop {
            if self.accept(".") {
                let name = match self.peek() {
                    Some(t) if t.kind == Kind::Name => t.text.clone(),
                    _ => return Err(self.error("expected a name after .")),
                };
                self.pos += 1;
                value = self.attribute(value, &name)?;
            } else if self.accept("[") {
                let key = self.expression()?;
                self.expect("]")?;
                value = match value {
                    Value::Dict(entries) => entries.into_iter().find(|(k, _)| *k == key),
                    _ => None,
                }
                .map(|(_, v)| v)
                .ok_or_else(|| self.error(&format!("no item {key:?}")))?;
            } else {
                return Ok(value);
            }
        }
    }

    fn attribute(&self, value: Value, name: &str) -> PyResult<Value> {
        match value {
            // pygments.token sets Token.Token to Token itself.
            Value::Token(path) if path.is_empty() && name == "Token" => Ok(Value::Token(path)),
            Value::Token(path) if path.is_empty() => Ok(Value::Token(name.to_string())),
            Value::Token(path) => Ok(Value::Token(format!("{path}.{name}"))),
            Value::Class(class) => self
                .program
                .class_attr(class, name)?
                .ok_or_else(|| self.error(&format!("no attribute {name}"))),
            _ => Err(self.error(&format!("no attribute {name}"))),
        }
    }

    fn primary(&mut self) -> PyResult<Value> {
        let Some(t) = self.peek().cloned() else {
            return Err(self.error("an expression is missing"));
        };
        self.pos += 1;
        match t.kind {
            Kind::String | Kind::FString => self.strings(&t),
            Kind::Name if self.accept("(") => self.call(&t.text),
            Kind::Name => self.lookup(&t.text),
            Kind::Op if t.text == "{" => self.dict(),
            Kind::Op if t.text == "(" => {
                let value = self.expression()?;
                self.expect(")")?;
                Ok(value)
            }
            _ => Err(self.error(&format!("cannot read an expression at {:?}", t.text))),
        }
    }

    /// One string literal, joined with those that directly follow it.
    fn strings(&mut self, first: &Tok) -> PyResult<Value> {
        let mut text = self.literal(first)?;
        while let Some(next) =
            self.peek().filter(|t| matches!(t.kind, Kind::String | Kind::FString))
        {
            let next = next.clone();
            self.pos += 1;
            text.push_str(&self.literal(&next)?);
        }
        Ok(Value::Str(text))
    }

    fn literal(&self, tok: &Tok) -> PyResult<String> {
        if tok.kind == Kind::String { Ok(tok.text.clone()) } else { self.fstring(&tok.text) }
    }

    /// A call of a module function with one argument.
    fn call(&mut self, name: &str) -> PyResult<Value> {
        let module = &self.program.modules[self.scope.module];
        let Some((param, body)) = module.funcs.get(name).cloned() else {
            return Err(self.error(&format!("no function {name}")));
        };
        let arg = self.expression()?;
        self.expect(")")?;
        let scope = Scope { module: self.scope.module, class: None, param: Some((param, arg)) };
        self.program.evaluate(body, scope)
    }

    /// Finds a name as Python would in a class body or function: the parameter, the
    /// class body's own names, the module's, then the classes and token types imported.
    fn lookup(&self, name: &str) -> PyResult<Value> {
        if let Some((param, arg)) = &self.scope.param
            && param == name
        {
            return Ok(arg.clone());
        }
        if let Some(class) = self.scope.class
            && let Some(&pos) = self.program.classes[class].attrs.get(name)
        {
            return self.program.evaluate(pos, self.scope.clone());
        }
        let module = &self.program.modules[self.scope.module];
        if let Some(&pos) = module.vars.get(name) {
            let scope = Scope { module: self.scope.module, class: None, param: None };
            return self.program.evaluate(pos, scope);
        }
        if let Some(class) = self.program.find_class(name) {
            return Ok(Value::Class(class));
        }
        match TOKEN_ROOTS.iter().find(|(root, _)| *root == name) {
            Some((_, path)) => Ok(Value::Token(path.to_string())),
            None => Err(self.error(&format!("unknown name {name}"))),
        }
    }

    fn dict(&mut self) -> PyResult<Value> {
        let mut entries: Vec<(Value, Value)> = Vec::new();
        while !self.accept("}") {
            if self.accept("**") {
                let Value::Dict(more) = self.expression()? else {
                    return Err(self.error("** takes a dict"));
                };
                more.into_iter().for_each(|(k, v)| dict_set(&mut entries, k, v));
            } else {
                let key = self.expression()?;
                self.expect(":")?;
                let value = self.expression()?;
                dict_set(&mut entries, key, value);
            }
            if !self.accept(",") {
                self.expect("}")?;
                break;
            }
        }
        Ok(Value::Dict(entries))
    }

    /// Evaluates an f-string's replacement fields, which here are plain expressions.
    fn fstring(&self, text: &str) -> PyResult<String> {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(at) = rest.find(['{', '}']) {
            out.push_str(&rest[..at]);
            let c = &rest[at..at + 1];
            if rest[at + 1..].starts_with(c) {
                out.push_str(c);
                rest = &rest[at + 2..];
                continue;
            }
            if c == "}" {
                out.push('}');
                rest = &rest[at + 1..];
                continue;
            }
            let Some(end) = rest[at..].find('}') else {
                return Err(self.error("an unclosed { in an f-string"));
            };
            let field = &rest[at + 1..at + end];
            out.push_str(&self.field(field)?);
            rest = &rest[at + end + 1..];
        }
        out.push_str(rest);
        Ok(out)
    }

    fn field(&self, field: &str) -> PyResult<String> {
        let toks = tokenize(field).map_err(|err| self.error(&err))?;
        let mut sub =
            Parser { program: self.program, toks: &toks, pos: 0, scope: self.scope.clone() };
        match sub.expression()? {
            Value::Str(text) if sub.pos + 1 == toks.len() => Ok(text),
            _ => Err(self.error(&format!("cannot read the f-string field {field}"))),
        }
    }
}

/// Sets a key as Python does: a key already there keeps its place.
fn dict_set(entries: &mut Vec<(Value, Value)>, key: Value, value: Value) {
    match entries.iter_mut().find(|(k, _)| *k == key) {
        Some(entry) => entry.1 = value,
        None => entries.push((key, value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODULE: &str = "\
from pygments.style import Style
from pygments.token import Comment, Name

RED = '#f00'

def bold(color):
    return 'bold ' + color

class Base(Style):
    name = 'base'
    background_color = f'#{\"fff\"}'
    styles = {
        Comment: 'italic',  # a comment
        Name.Builtin: bold(RED),
    }

class Child(Base):
    name = \"child\"
    styles = {**Base.styles, Comment: '#0f0'}
";

    #[test]
    fn evaluates_classes_dicts_calls_and_fstrings() {
        let mut program = Program::default();
        assert_eq!(program.read_module("m.py", MODULE), Ok(()));
        let child = program.find_class("Child").unwrap_or(usize::MAX);
        let styles = program.class_attr(child, "styles");
        let expected = Value::Dict(vec![
            (Value::Token("Comment".into()), Value::Str("#0f0".into())),
            (Value::Token("Name.Builtin".into()), Value::Str("bold #f00".into())),
        ]);
        assert_eq!(styles, Ok(Some(expected)));
        let background = program.class_attr(child, "background_color");
        assert_eq!(background, Ok(Some(Value::Str("#fff".into()))));
    }
}
