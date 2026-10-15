//! `mangle_signature`: a signature shortened for a summary table, `(a, b[, c, d])`:
//! annotations dropped, defaults hidden behind brackets, long lists cut with `...`.

/// The signature `sig` (`(a: int, b=1) -> str`) in at most about `max_chars`
/// characters.
pub(crate) fn mangle_signature(sig: &str, max_chars: usize) -> String {
    let cleaned = cleanup_signature(sig);
    let mut s = strip_return_annotation(&cleaned);
    if let Some(inner) = s.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
        s = inner.to_owned();
    }
    let s = strip_complex_values(&strip_literals(s.trim()));
    let (args, opts) = split_options(&s);
    let args: Vec<String> = args.iter().map(|a| strip_arg_typehint(a)).collect();
    let opts: Vec<String> = opts.iter().map(|o| strip_arg_typehint(o)).collect();
    let mut out = limited_join(", ", &args, max_chars.saturating_sub(2));
    if !opts.is_empty() {
        if out.is_empty() {
            out = format!("[{}]", limited_join(", ", &opts, max_chars.saturating_sub(4)));
        } else if out.len() + 9 < max_chars {
            let room = max_chars.saturating_sub(out.len() + 6);
            out = format!("{out}[, {}]", limited_join(", ", &opts, room));
        }
    }
    format!("({out})")
}

/// `strip_arg_typehint`: `x: int` becomes `x`.
fn strip_arg_typehint(s: &str) -> String {
    s.split_once(':').map_or(s, |(a, _)| a).trim().to_owned()
}

/// `re.sub(r'\)\s*->\s.*$', ')', s)`.
fn strip_return_annotation(s: &str) -> String {
    for (k, _) in s.match_indices(')') {
        let after = s[k + 1..].trim_start();
        if let Some(rest) = after.strip_prefix("->")
            && rest.starts_with(char::is_whitespace)
        {
            return s[..=k].to_owned();
        }
    }
    s.to_owned()
}

/// Escaped backslashes and quotes, then string literals, removed.
fn strip_literals(s: &str) -> String {
    let s = s.replace("\\\\", "").replace("\\'", "").replace("\\\"", "");
    let s = remove_quoted(&s, '\'');
    remove_quoted(&s, '"')
}

/// `re.sub(r"'[^']*'", '', s)` for quote `q`.
fn remove_quoted(s: &str, q: char) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(open) = rest.find(q) {
        let Some(close) = rest[open + 1..].find(q) else { break };
        out.push_str(&rest[..open]);
        rest = &rest[open + 1 + close + 1..];
    }
    out.push_str(rest);
    out
}

/// Bracketed values removed as `while re.search(r'\([^)]*\)', s): s = re.sub(...)`
/// removes them, then `<...>` and `{...}` likewise.
fn strip_complex_values(s: &str) -> String {
    let mut s = s.to_owned();
    for (open, close) in [('(', ')'), ('<', '>'), ('{', '}')] {
        loop {
            let (out, removed) = remove_spans(&s, open, close);
            s = out;
            if !removed {
                break;
            }
        }
    }
    s
}

/// One `re.sub` of `open[^close]*close` with nothing: each span from an `open` to the
/// first `close` after it, left to right; whether any was removed.
fn remove_spans(s: &str, open: char, close: char) -> (String, bool) {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    let mut removed = false;
    while let Some(a) = rest.find(open) {
        let Some(b) = rest[a + 1..].find(close) else { break };
        out.push_str(&rest[..a]);
        rest = &rest[a + 1 + b + 1..];
        removed = true;
    }
    out.push_str(rest);
    (out, removed)
}

/// `opt_re`'s loop: the parameters before the first one with a default, and the
/// names of those with defaults.
fn split_options(s: &str) -> (Vec<String>, Vec<String>) {
    let mut s = s.to_owned();
    let mut opts = Vec::new();
    while !s.is_empty() {
        let Some((head, name)) = last_option(&s) else {
            return (s.split(", ").map(str::to_owned).collect(), opts);
        };
        opts.insert(0, name);
        s = head;
    }
    (Vec::new(), opts)
}

/// `^(.*, |)([a-zA-Z0-9_*]+)\s*=\s*` searched in `s`: the text before the last
/// `, name=` (without the `, `), and the name.
fn last_option(s: &str) -> Option<(String, String)> {
    let starts = s.match_indices(", ").map(|(k, _)| k + 2).collect::<Vec<_>>();
    for start in starts.into_iter().rev().chain([0]) {
        let rest = &s[start..];
        let n = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '*'))
            .unwrap_or(rest.len());
        if n > 0 && rest[n..].trim_start().starts_with('=') {
            let head = if start == 0 { String::new() } else { s[..start - 2].to_owned() };
            return Some((head, rest[..n].to_owned()));
        }
    }
    None
}

/// `limited_join`: the items joined, the last ones replaced by `...` when the whole
/// would be `max_chars` or longer.
pub(crate) fn limited_join(sep: &str, items: &[String], max_chars: usize) -> String {
    let full = items.join(sep);
    if full.len() < max_chars {
        return full;
    }
    let mut chars = 0;
    let mut n = 0;
    for item in items {
        chars += item.len() + sep.len();
        if chars < max_chars.saturating_sub(3) {
            n += 1;
        } else {
            break;
        }
    }
    let mut kept: Vec<&str> = items[..n].iter().map(String::as_str).collect();
    kept.push("...");
    kept.join(sep)
}

/// `_cleanup_signature`: the parameters with their annotations removed and their
/// defaults replaced by `None`, without the return annotation; the signature as
/// written when it does not parse.
fn cleanup_signature(sig: &str) -> String {
    let s = sig.trim();
    let Some(close) = matching_paren(s) else { return sig.to_owned() };
    let inner = &s[1..close];
    let after = s[close + 1..].trim();
    if !after.is_empty() && !after.starts_with("->") {
        return sig.to_owned();
    }
    let mut params = Vec::new();
    for raw in split_top_level(inner) {
        let p = raw.trim();
        if p.is_empty() {
            continue;
        }
        let (name, has_default) = match split_top_level_once(p, '=') {
            Some((name, _)) => (name.trim(), true),
            None => (p, false),
        };
        let name = split_top_level_once(name, ':').map_or(name, |(n, _)| n).trim();
        if !is_parameter_name(name) {
            return sig.to_owned();
        }
        params.push(if has_default { format!("{name}=None") } else { name.to_owned() });
    }
    format!("({})", params.join(", "))
}

/// A parameter as Python's parser reads one: `name`, `*args`, `**kwargs`, `*` or
/// `/`.
fn is_parameter_name(name: &str) -> bool {
    if name == "*" || name == "/" {
        return true;
    }
    let ident = name.trim_start_matches('*');
    name.len() - ident.len() <= 2
        && ident.starts_with(|c: char| c.is_alphabetic() || c == '_')
        && ident.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// The position of the `)` that closes the `(` at the start of `s`.
fn matching_paren(s: &str) -> Option<usize> {
    if !s.starts_with('(') {
        return None;
    }
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        if std::mem::take(&mut escaped) {
            continue;
        }
        match (quote, c) {
            (Some(_), '\\') => escaped = true,
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(c),
            (None, '(' | '[' | '{') => depth += 1,
            (None, ')' | ']' | '}') => {
                depth -= 1;
                if depth == 0 {
                    return (c == ')').then_some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// The parts of `s` between commas outside brackets and strings.
fn split_top_level(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    for (i, c) in top_level_chars(s) {
        if c == ',' {
            parts.push(&s[start..i]);
            start = i + 1;
        }
    }
    parts.push(&s[start..]);
    parts
}

/// `s` split at its first `sep` outside brackets and strings.
fn split_top_level_once(s: &str, sep: char) -> Option<(&str, &str)> {
    let (i, _) = top_level_chars(s).find(|&(_, c)| c == sep)?;
    Some((&s[..i], &s[i + 1..]))
}

/// The characters of `s` outside brackets and string literals, with their offsets.
fn top_level_chars(s: &str) -> impl Iterator<Item = (usize, char)> + '_ {
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    s.char_indices().filter(move |&(_, c)| {
        if std::mem::take(&mut escaped) {
            return false;
        }
        match (quote, c) {
            (Some(_), '\\') => escaped = true,
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(c),
            (None, '(' | '[' | '{') => depth += 1,
            (None, ')' | ']' | '}') => depth -= 1,
            (None, _) => return depth == 0,
        }
        false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sphinx's `test_mangle_signature` (tests/test_ext_autosummary).
    #[test]
    fn signatures_are_mangled_as_sphinx_mangles_them() {
        let cases = [
            ("()", "()"),
            ("(a, b, c, d, e)", "(a, b, c, d, e)"),
            ("(a, b, c=1, d=2, e=3)", "(a, b[, c, d, e])"),
            (
                "(a, b, aaa=1, bbb=1, ccc=1, eee=1, fff=1, ggg=1, hhh=1, iii=1, jjj=1)",
                "(a, b[, aaa, bbb, ccc, ...])",
            ),
            ("(a, b, c=(), d=<foo>)", "(a, b[, c, d])"),
            ("(a, b, c='foobar()', d=123)", "(a, b[, c, d])"),
            ("(a, b[, c])", "(a, b[, c])"),
            ("(a, b[, cxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx])", "(a, b[, ...)"),
            ("(a, b='c=d, e=f, g=h', c=3)", "(a[, b, c])"),
            ("(a, b=\"c=d, e=f, g=h\", c=3)", "(a[, b, c])"),
            ("(a, b='c=d, \\'e=f,\\' g=h', c=3)", "(a[, b, c])"),
            ("(a, b='c=d, ', e='\\\\' g=h, c=3)", "(a[, b, e, c])"),
            ("(a, b={'c=d, ': 3, '\\\\': 3})", "(a[, b])"),
            ("(a=1, b=2, c=3)", "([a, b, c])"),
            ("(a=1, b=<SomeClass: a, b, c>, c=3)", "([a, b, c])"),
            ("(a=1, b=T(a=1, b=2), c=3)", "([a, b, c])"),
            ("(a: Tuple[int, str], b: int) -> str", "(a, b)"),
        ];
        for (sig, expected) in cases {
            assert_eq!(mangle_signature(sig, 30), expected, "{sig}");
        }
    }
}
