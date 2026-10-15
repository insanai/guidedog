//! Reading conf.py's top-level statements without running them: assignments bind names
//! in a [`Scope`], and the rest is reported (blocks, `setup()`, `sys.path` changes).

use std::path::PathBuf;

use crate::migrate::Migration;
use crate::pyconf::{self, Cursor, Scope, Value, combine, group_end, split_top, statement_end};

/// A name conf.py assigns, and the statement that last did.
pub(crate) struct Binding<'a> {
    pub(crate) name: String,
    /// 1-based line of the first assignment (or of the failing one).
    pub(crate) line: usize,
    /// The statement that last assigned it, kept as a comment when not converted.
    pub(crate) source: &'a str,
    /// Why its value is unknown.
    pub(crate) failure: Option<String>,
}

/// What reading conf.py bound: the values known, and every name assigned, in order.
pub(crate) struct ConfPy<'a> {
    pub(crate) scope: Scope,
    pub(crate) bindings: Vec<Binding<'a>>,
}

impl<'a> ConfPy<'a> {
    /// Records that `source` at `line` assigned `name` the value `result`.
    fn bind(&mut self, name: &str, line: usize, source: &'a str, result: Result<Value, String>) {
        let failure = match result {
            Ok(value) => {
                self.scope.values.insert(name.to_string(), value);
                None
            }
            Err(failure) => {
                self.scope.values.remove(name);
                Some(failure)
            }
        };
        if let Some(b) = self.bindings.iter_mut().find(|b| b.name == name) {
            if failure.is_some() {
                b.line = line;
            }
            b.source = source;
            b.failure = failure;
            return;
        }
        self.bindings.push(Binding { name: name.to_string(), line, source, failure });
    }
}

/// Runs through conf.py's top-level statements, binding what they assign. Indented
/// bodies of blocks are skipped (the block itself is reported).
pub(crate) fn read_conf_py<'a>(m: &mut Migration<'_>, text: &'a str, year: i64) -> ConfPy<'a> {
    let mut c = ConfPy { scope: Scope { year: Some(year), ..Scope::default() }, bindings: vec![] };
    let (mut at, mut line) = (0, 1);
    while at < text.len() {
        let end = statement_end(text, at);
        let statement = &text[at..end];
        let trimmed = statement.trim();
        let indented = statement.starts_with([' ', '\t']);
        let mut block = false;
        if !trimmed.is_empty() && !trimmed.starts_with('#') && !indented {
            block = read_statement(m, &mut c, trimmed, line);
        }
        let mut next = (end + 1).min(text.len());
        if block {
            next = block_end(text, next);
        }
        line += text[at..next].matches('\n').count();
        at = next;
    }
    c
}

/// Skips the indented lines of a compound statement's body.
fn block_end(text: &str, mut at: usize) -> usize {
    while let Some(&c) = text.as_bytes().get(at) {
        if !matches!(c, b'\n' | b'\r' | b' ' | b'\t' | b'#') {
            break;
        }
        at = (statement_end(text, at) + 1).min(text.len());
    }
    at
}

/// Reads one statement. Returns true when the statement opens a block.
fn read_statement<'a>(m: &mut Migration<'_>, c: &mut ConfPy<'a>, s: &'a str, line: usize) -> bool {
    let mut cur = Cursor::new(s);
    let word = cur.word();
    match word {
        "import" | "from" | "global" | "pass" | "del" | "assert" => return false,
        "elif" | "else" | "except" | "finally" => return true,
        "def" | "class" => {
            cur.space();
            let name = cur.word();
            if word == "def" && name == "linkcode_resolve" {
                crate::convert::report_linkcode_resolve(m, line);
            }
            if word == "def" && name == "setup" {
                m.warn(
                    line,
                    "migrate.setup",
                    "SETUP FUNCTION NOT RUN",
                    "setup(app) extends Sphinx while it runs; Guidedog reads conf.py without \
                     running it."
                        .into(),
                    "Check what it registers: built-in features need nothing, others wait \
                     for Guidedog's plugin system.",
                );
            }
            return true;
        }
        "if" | "for" | "while" | "try" | "with" | "async" | "match" => {
            m.warn(
                line,
                "migrate.code",
                "CODE NOT RUN",
                format!(
                    "This {word} block is Python code; Guidedog reads conf.py without running \
                     it, so the block is skipped."
                ),
                "If it sets settings, write their values in conf.toml by hand.",
            );
            return true;
        }
        "" if s.starts_with('@') || cur.string_ahead() => return false, // decorator, docstring.
        _ => {}
    }
    read_assignment(m, c, &mut cur, word, s, line);
    false
}

fn read_assignment<'a>(
    m: &mut Migration<'_>,
    c: &mut ConfPy<'a>,
    cur: &mut Cursor<'a>,
    name: &str,
    s: &'a str,
    line: usize,
) {
    if cur.take("+=") {
        let result = pyconf::eval(cur.tail(), &mut c.scope).and_then(|value| {
            match c.scope.values.get(name) {
                Some(current) => combine(current.clone(), value),
                None => Err(format!("{name} is extended before it is set.")),
            }
        });
        c.bind(name, line, s, result);
    } else if cur.take("=") && !cur.peek("=") {
        assign(m, c, cur, name, s, line);
    } else if cur.take(":") {
        // An annotated assignment: name: type = value.
        let value_text = cur.tail().split_once('=').map_or("", |(_, v)| v);
        let result = pyconf::eval(value_text, &mut c.scope);
        c.bind(name, line, s, result);
    } else if cur.peek("[") || cur.peek(".") {
        mutate(m, c, cur, name, s, line);
    } else {
        m.warn(
            line,
            "migrate.code",
            "STATEMENT NOT READ",
            "This line is Python code rather than a setting, so it is skipped.".into(),
            "If it changes a setting, write the setting's value in conf.toml by hand.",
        );
    }
}

/// Reads `name = value`, including chained assignments (`version = release = "1.0"`),
/// which bind every name. A dictionary keeps its literal entries when others are
/// computed.
fn assign<'a>(
    m: &mut Migration<'_>,
    c: &mut ConfPy<'a>,
    cur: &mut Cursor<'a>,
    name: &str,
    s: &'a str,
    line: usize,
) {
    let mut targets = vec![name];
    loop {
        let start = cur.at;
        cur.space();
        let next = cur.word();
        if !next.is_empty() && cur.take("=") && !cur.peek("=") {
            targets.push(next);
            continue;
        }
        cur.at = start;
        break;
    }
    let mut result = pyconf::eval(cur.tail(), &mut c.scope);
    if result.is_err()
        && let Some((partial, skipped)) = partial_dict(cur.tail(), &mut c.scope)
    {
        result = Ok(partial);
        m.warn(
            line,
            "migrate.computed",
            "SOME ENTRIES NOT CONVERTED",
            format!(
                "{name} keeps its literal entries. Python would compute these, so they \
                 are left out: {}.",
                skipped.join(", ")
            ),
            "Add the missing entries to conf.toml by hand.",
        );
    }
    for target in targets {
        c.bind(target, line, s, result.clone());
    }
}

/// Reads `name[key] = value` and the list and dictionary methods conf.py files use to
/// add to a setting.
fn mutate<'a>(
    m: &mut Migration<'_>,
    c: &mut ConfPy<'a>,
    cur: &mut Cursor<'a>,
    name: &str,
    s: &'a str,
    line: usize,
) {
    let Some(current) = c.scope.values.get(name).cloned() else {
        unbound_mutation(m, c, name, s, line);
        return;
    };
    let result = if cur.take("[") {
        let close = group_end(cur.src, cur.at - 1);
        let key =
            pyconf::eval(&cur.src[cur.at..close.saturating_sub(1).max(cur.at)], &mut c.scope);
        cur.at = close;
        let value = if cur.take("=") {
            pyconf::eval(cur.tail(), &mut c.scope)
        } else {
            Err("A dictionary entry is assigned with a form that is not read.".into())
        };
        key.and_then(|k| value.and_then(|v| dict_set(current, k, v)))
    } else {
        cur.advance(1); // the dot.
        let method = cur.word();
        if cur.peek("(") {
            let close = group_end(cur.src, cur.at);
            let args =
                format!("[{}]", &cur.src[cur.at + 1..close.saturating_sub(1).max(cur.at + 1)]);
            match pyconf::eval(&args, &mut c.scope) {
                Ok(Value::List(args)) => apply_method(current, method, args),
                Ok(_) => Err(format!("The method .{method}() is computed by Python.")),
                Err(e) => Err(e),
            }
        } else {
            Err(format!("The attribute {name}.{method} is computed by Python."))
        }
    };
    c.bind(name, line, s, result);
}

/// A change to a name conf.py has not bound: `sys.path` changes, or a setting changed
/// before it is set.
fn unbound_mutation<'a>(
    m: &mut Migration<'_>,
    c: &mut ConfPy<'a>,
    name: &str,
    s: &'a str,
    line: usize,
) {
    if name == "sys" && sys_path(m, c, s, line) {
        return;
    }
    if name == "sys" || name == "os" {
        m.tell(
            line,
            "migrate.path",
            "PYTHON PATH NOT NEEDED",
            "This line prepares Python's environment for extensions; Guidedog does not \
             import Python modules."
                .into(),
            "Nothing to do: conf.toml needs no counterpart of this line.",
        );
    } else if crate::convert::is_setting_name(name) {
        c.bind(name, line, s, Err(format!("{name} is changed before it is set.")));
    }
}

/// Turns `sys.path.insert(0, os.path.abspath('..'))` and `sys.path.append(...)` into
/// `autodoc_source_paths`, the folders autodoc reads Python from; paths are relative to
/// conf.py's folder, where Sphinx runs it. Returns false for forms it cannot read.
fn sys_path<'a>(m: &mut Migration<'_>, c: &mut ConfPy<'a>, s: &'a str, line: usize) -> bool {
    let text = s.trim();
    let (front, args) = if let Some(inner) = call_arguments(text, "sys.path.insert") {
        let (index, rest) = inner.split_once(',').unwrap_or((inner, ""));
        (index.trim() == "0", rest)
    } else if let Some(inner) = call_arguments(text, "sys.path.append") {
        (false, inner)
    } else {
        return false;
    };
    let Some(folder) = path_expression(args.trim()) else { return false };
    let entry = Value::List(vec![Value::Str(folder.clone())]);
    let value = match c.scope.values.get("autodoc_source_paths") {
        Some(current @ Value::List(_)) if front => combine(entry, current.clone()),
        Some(current @ Value::List(_)) => combine(current.clone(), entry),
        _ => Ok(entry),
    };
    c.bind("autodoc_source_paths", line, s, value);
    m.tell(
        line,
        "migrate.path",
        "PYTHON PATH CONVERTED",
        format!(
            "This adds {folder} to Python's path; autodoc reads Python source from \
             autodoc_source_paths instead, without importing it."
        ),
        "Nothing to do: conf.toml sets autodoc_source_paths for you.",
    );
    true
}

/// The arguments of `name(...)` when `e` is exactly that call.
fn call_arguments<'e>(e: &'e str, name: &str) -> Option<&'e str> {
    Some(e.strip_prefix(name)?.strip_prefix('(')?.strip_suffix(')')?.trim())
}

/// Reads the path expressions conf.py files put on `sys.path`: a string,
/// `os.path.abspath`, `realpath` or `normpath` of one, `os.path.join` of them, and
/// `os.path.dirname` of `os.path.abspath(__file__)` (conf.py's folder, nested for
/// parents). Paths use `/`.
fn path_expression(e: &str) -> Option<String> {
    for wrapper in ["os.path.abspath", "os.path.realpath", "os.path.normpath", "str"] {
        if let Some(inner) = call_arguments(e, wrapper) {
            return path_expression(inner);
        }
    }
    if let Some(inner) = call_arguments(e, "os.path.join") {
        let mut joined = PathBuf::new();
        for part in split_top(inner, b',') {
            joined.push(path_expression(part.trim())?);
        }
        return Some(slashed(joined));
    }
    if let Some(inner) = call_arguments(e, "os.path.dirname") {
        if inner == "__file__" || inner == "os.path.abspath(__file__)" {
            return Some(".".into());
        }
        let parent = path_expression(inner)?;
        return Some(slashed(PathBuf::from(parent).join("..")));
    }
    let quote = e.chars().next().filter(|q| matches!(q, '"' | '\''))?;
    let body = e.strip_prefix(quote)?.strip_suffix(quote)?;
    Some(body.to_string())
}

/// `path` as text with `/` separators, without a leading `./` that joining kept.
fn slashed(path: PathBuf) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    match text.strip_prefix("./") {
        Some(rest) if !rest.is_empty() => rest.to_string(),
        _ => text,
    }
}

/// `dict[key] = value`.
fn dict_set(dict: Value, key: Value, value: Value) -> Result<Value, String> {
    let Value::Dict(mut entries) = dict else {
        return Err("Only dictionaries take entries by key.".into());
    };
    match entries.iter_mut().find(|(k, _)| k.same_key(&key)) {
        Some(entry) => entry.1 = value,
        None => entries.push((key, value)),
    }
    Ok(Value::Dict(entries))
}

/// The list and dictionary methods conf.py files use to add to a setting.
fn apply_method(v: Value, method: &str, args: Vec<Value>) -> Result<Value, String> {
    match (v, method, args.as_slice()) {
        (v @ Value::List(_), "append", [_]) => combine(v, Value::List(args)),
        (v @ Value::List(_), "extend", [Value::List(more)]) => {
            combine(v, Value::List(more.clone()))
        }
        (v @ Value::List(_), "insert", [Value::Int(at), item]) => {
            let head = Value::List(vec![item.clone()]);
            if *at == 0 { combine(head, v) } else { combine(v, head) }
        }
        (mut v @ Value::Dict(_), "update", [Value::Dict(entries)]) => {
            for (k, value) in entries {
                v = dict_set(v, k.clone(), value.clone())?;
            }
            Ok(v)
        }
        _ => Err(format!("The method .{method}() is computed by Python.")),
    }
}

/// Evaluates a dictionary literal entry by entry. Returns the entries that are literals
/// and the keys (as written) of those Python would compute; `None` when `src` is not a
/// dictionary literal or no entry is a literal.
fn partial_dict(src: &str, scope: &mut Scope) -> Option<(Value, Vec<String>)> {
    let text = src.trim();
    if !text.starts_with('{') || group_end(text, 0) != text.len() {
        return None;
    }
    let mut entries = Vec::new();
    let mut skipped = Vec::new();
    for entry in split_top(&text[1..text.len() - 1], b',') {
        let e = entry.trim();
        if e.is_empty() {
            continue;
        }
        let parts = split_top(e, b':');
        if parts.len() < 2 {
            return None;
        }
        let key = pyconf::eval(parts[0], scope);
        let value = pyconf::eval(&e[parts[0].len() + 1..], scope);
        match (key, value) {
            (Ok(k), Ok(v)) => entries.push((k, v)),
            _ => skipped.push(parts[0].trim().to_string()),
        }
    }
    (!entries.is_empty()).then_some((Value::Dict(entries), skipped))
}
