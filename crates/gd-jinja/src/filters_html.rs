//! Filters that produce or inspect HTML, URLs and JSON.

use std::fmt::Write as _;
use std::rc::Rc;

use crate::calls::Call;
use crate::convert::{MAX_VALUE_DEPTH, char_slice, quoted, to_str, type_name, write_float_repr};
use crate::environment::Environment;
use crate::error::ErrorKind;
use crate::escape::{escape, escape_string, striptags, write_escaped, write_url_quoted};
use crate::filters_text::{Builtin, Outcome, keep_markup};
use crate::runtime::{Result, State};
use crate::text::{is_space, is_word_char, split_whitespace, to_lower};
use crate::value::{Dict, DictKind, Value};

pub(crate) fn register(env: &mut Environment) {
    let filters: &[(&str, Builtin)] = &[
        ("escape", |c| Ok(escape(&c.arg(0, "value")))),
        ("e", |c| Ok(escape(&c.arg(0, "value")))),
        ("forceescape", |c| Ok(Value::markup(&escape_string(&to_str(&c.arg(0, "value")))))),
        ("safe", filter_safe),
        ("striptags", |c| Ok(Value::from(striptags(&to_str(&c.arg(0, "value")))))),
        ("tojson", filter_tojson),
        ("urlencode", filter_urlencode),
        ("urlize", filter_urlize),
        ("xmlattr", filter_xmlattr),
    ];
    for &(name, f) in filters {
        env.add_filter(name, f);
    }
}

fn filter_safe(c: &mut Call<'_, '_>) -> Outcome {
    let v = c.arg(0, "value");
    Ok(if v.is_markup() { v } else { Value::markup(&to_str(&v)) })
}

// ---------------------------------------------------------------------------------------
// tojson: json.dumps(sort_keys=True) made safe for HTML

fn filter_tojson(c: &mut Call<'_, '_>) -> Outcome {
    c.check_args("tojson", &["value", "indent"], 0)?;
    let indent = match c.arg_or(1, "indent", Value::None) {
        Value::None => None,
        Value::Str(text) | Value::Markup(text) => Some(text.to_string()),
        _ => Some(crate::limits::repeat_text(c.state_ref(), " ", c.arg_count(1, "indent", 0)?)?),
    };
    let mut w = JsonWriter { s: c.state_ref(), out: String::new(), indent };
    w.value(&c.arg(0, "value"), 0)?;
    Ok(Value::Markup(Rc::from(w.out)))
}

/// Writes JSON into out, one line or (with an indent) pretty-printed.
struct JsonWriter<'a, 'env> {
    s: &'a State<'env>,
    out: String,
    indent: Option<String>,
}

impl JsonWriter<'_, '_> {
    /// Starts an item of a list or object: a comma after the first, and its line.
    fn separator(&mut self, first: bool, depth: usize) {
        if !first {
            self.out.push_str(if self.indent.is_some() { "," } else { ", " });
        }
        self.newline(depth);
    }

    fn newline(&mut self, depth: usize) {
        if let Some(indent) = &self.indent {
            self.out.push('\n');
            (0..depth).for_each(|_| self.out.push_str(indent));
        }
    }

    fn value(&mut self, v: &Value, depth: usize) -> Result<()> {
        if depth > MAX_VALUE_DEPTH {
            let message = format!(
                "tojson: the value nests more than {MAX_VALUE_DEPTH} levels deep (a list or \
                 dict that contains itself?)"
            );
            return Err(self.s.error(ErrorKind::Limit, message));
        }
        match v {
            Value::None => self.out.push_str("null"),
            Value::Bool(b) => self.out.push_str(if *b { "true" } else { "false" }),
            Value::Int(i) => {
                let _ = write!(self.out, "{i}");
            }
            Value::Float(f) if f.is_nan() => self.out.push_str("NaN"),
            Value::Float(f) if f.is_infinite() => {
                self.out.push_str(if *f > 0.0 { "Infinity" } else { "-Infinity" });
            }
            Value::Float(f) => write_float_repr(&mut self.out, *f),
            Value::Str(text) | Value::Markup(text) => write_json_string(&mut self.out, text),
            Value::List(l) => self.list(&l.snapshot(), depth)?,
            Value::Dict(d) if d.kind() != DictKind::Module => self.object(d, depth)?,
            _ => {
                let message = format!("Object of type {} is not JSON serializable", type_name(v));
                return Err(self.s.error(ErrorKind::Type, message));
            }
        }
        Ok(())
    }

    fn list(&mut self, items: &[Value], depth: usize) -> Result<()> {
        if items.is_empty() {
            self.out.push_str("[]");
            return Ok(());
        }
        self.out.push('[');
        for (i, item) in items.iter().enumerate() {
            self.separator(i == 0, depth + 1);
            self.value(item, depth + 1)?;
        }
        self.newline(depth);
        self.out.push(']');
        Ok(())
    }

    /// An object with its keys sorted, as `sort_keys=True` writes it.
    fn object(&mut self, d: &Dict, depth: usize) -> Result<()> {
        let entries = d.entries();
        if entries.is_empty() {
            self.out.push_str("{}");
            return Ok(());
        }
        let keys: Vec<Value> = entries.iter().map(|(k, _)| k.clone()).collect();
        let order = crate::filters_seq::sort_order(self.s, &keys, false, true)?;
        self.out.push('{');
        for (i, index) in order.into_iter().enumerate() {
            self.separator(i == 0, depth + 1);
            let (key, value) = &entries[index];
            self.key(key)?;
            self.out.push_str(": ");
            self.value(value, depth + 1)?;
        }
        self.newline(depth);
        self.out.push('}');
        Ok(())
    }

    /// An object key: strings as they are, None, bools and numbers as strings.
    fn key(&mut self, key: &Value) -> Result<()> {
        match key {
            Value::Str(text) | Value::Markup(text) => write_json_string(&mut self.out, text),
            Value::None => self.out.push_str("\"null\""),
            Value::Bool(b) => self.out.push_str(if *b { "\"true\"" } else { "\"false\"" }),
            Value::Int(_) | Value::Float(_) => {
                self.out.push('"');
                self.value(key, 0)?;
                self.out.push('"');
            }
            _ => {
                let what = type_name(key);
                let message = format!("keys must be str, int, float, bool or None, not {what}");
                return Err(self.s.error(ErrorKind::Type, message));
            }
        }
        Ok(())
    }
}

/// Escapes like json.dumps(ensure_ascii=True), plus the HTML-safety replacements of
/// `< > &` and `'`.
fn write_json_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x08' => out.push_str("\\b"),
            '\x0c' => out.push_str("\\f"),
            ' '..='~' if !matches!(ch, '<' | '>' | '&' | '\'') => out.push(ch),
            _ => {
                let mut units = [0u16; 2];
                for unit in ch.encode_utf16(&mut units) {
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
        }
    }
    out.push('"');
}

// ---------------------------------------------------------------------------------------
// urlencode and xmlattr

fn filter_urlencode(c: &mut Call<'_, '_>) -> Outcome {
    let v = c.arg(0, "value");
    let pairs = match &v {
        Value::Dict(d) => d.entries(),
        Value::List(l) => {
            let mut pairs = Vec::new();
            for item in l.snapshot() {
                match item {
                    Value::List(p) if p.len() == 2 => {
                        pairs.push((p.item(0).unwrap_or_default(), p.item(1).unwrap_or_default()))
                    }
                    _ => return Err(c.error("urlencode expects pairs")),
                }
            }
            pairs
        }
        _ => {
            let mut out = String::new();
            write_url_quoted(&mut out, &to_str(&v), "/", false);
            return Ok(Value::from(out));
        }
    };
    let mut out = String::new();
    for (i, (k, val)) in pairs.iter().enumerate() {
        if i > 0 {
            out.push('&');
        }
        write_url_quoted(&mut out, &to_str(k), "", true);
        out.push('=');
        write_url_quoted(&mut out, &to_str(val), "", true);
    }
    Ok(Value::from(out))
}

fn filter_xmlattr(c: &mut Call<'_, '_>) -> Outcome {
    let Value::Dict(d) = c.arg(0, "d") else { return Err(c.error("xmlattr expects a mapping")) };
    let mut out = String::new();
    for (key, value) in d.entries() {
        if value.is_none() || value.is_undefined() {
            continue;
        }
        let name = to_str(&key);
        let invalid =
            |ch: char| ch.is_ascii_whitespace() || matches!(ch, '\x0b' | '/' | '>' | '=');
        if name.contains(invalid) {
            return Err(
                c.error(&format!("Invalid character in attribute name: {}", quoted(&name)))
            );
        }
        if !out.is_empty() {
            out.push(' ');
        }
        write_escaped(&mut out, &name);
        out.push_str("=\"");
        write_escaped(&mut out, &to_str(&value));
        out.push('"');
    }
    if c.arg_flag(1, "autospace", true) && !out.is_empty() {
        out.insert(0, ' ');
    }
    Ok(keep_markup(out, c.autoescaping()))
}

// ---------------------------------------------------------------------------------------
// urlize

/// How urlize writes links.
struct Urlize {
    trim_limit: Option<usize>,
    /// The ` rel="..." target="..."` attributes of http links.
    attrs: String,
    schemes: Vec<String>,
}

fn filter_urlize(c: &mut Call<'_, '_>) -> Outcome {
    let names = ["value", "trim_url_limit", "nofollow", "target", "rel", "extra_schemes"];
    c.check_args("urlize", &names, 0)?;
    let trim_limit = match c.arg_or(1, "trim_url_limit", Value::None) {
        Value::None => None,
        _ => Some(c.arg_count(1, "trim_url_limit", 0)?),
    };
    let mut rel: Vec<String> = match c.arg_or(4, "rel", Value::None) {
        Value::None => Vec::new(),
        v => split_whitespace(&to_str(&v)).map(String::from).collect(),
    };
    if c.arg_flag(2, "nofollow", false) {
        rel.push("nofollow".to_string());
    }
    rel.push("noopener".to_string());
    rel.sort();
    rel.dedup();
    let mut attrs = String::from(" rel=\"");
    write_escaped(&mut attrs, &rel.join(" "));
    attrs.push('"');
    match c.arg_or(3, "target", Value::None) {
        Value::None => {}
        target => {
            attrs.push_str(" target=\"");
            write_escaped(&mut attrs, &to_str(&target));
            attrs.push('"');
        }
    }
    let schemes = match c.arg_or(5, "extra_schemes", Value::None) {
        Value::List(l) => l.items().iter().map(|v| to_str(v).into_owned()).collect(),
        _ => Vec::new(),
    };
    let u = Urlize { trim_limit, attrs, schemes };
    let out = urlize_text(&to_str(&escape(&c.arg(0, "value"))), &u);
    Ok(keep_markup(out, c.autoescaping()))
}

fn urlize_text(text: &str, u: &Urlize) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(first) = rest.chars().next() {
        let space = is_space(first);
        let end = rest.find(|ch: char| is_space(ch) != space).unwrap_or(rest.len());
        if space {
            out.push_str(&rest[..end]);
        } else {
            urlize_word(&mut out, &rest[..end], u);
        }
        rest = &rest[end..];
    }
    out
}

/// Splits leading "(" "<" "&lt;" and trailing ")" ">" "." "," "&gt;" from a word, keeps
/// balanced brackets in the middle, and links the middle.
fn urlize_word(out: &mut String, word: &str, u: &Urlize) {
    let mut head_end = 0;
    loop {
        let rest = &word[head_end..];
        if rest.starts_with("&lt;") {
            head_end += 4;
        } else if rest.starts_with(['(', '<']) {
            head_end += 1;
        } else {
            break;
        }
    }
    let body = &word[head_end..];
    let mut tail_start = body.len();
    loop {
        let part = &body[..tail_start];
        if part.ends_with("&gt;") {
            tail_start -= 4;
        } else if part.ends_with([')', '>', '.', ',', '\n']) {
            tail_start -= 1;
        } else {
            break;
        }
    }
    let mut middle = body[..tail_start].to_string();
    let mut tail = &body[tail_start..];
    for (open, close) in [("(", ")"), ("<", ">"), ("&lt;", "&gt;")] {
        let opens = middle.matches(open).count();
        if opens <= middle.matches(close).count() {
            continue;
        }
        for _ in 0..opens.min(tail.matches(close).count()) {
            let end = tail.find(close).map_or(0, |i| i + close.len());
            middle.push_str(&tail[..end]);
            tail = &tail[end..];
        }
    }
    out.push_str(&word[..head_end]);
    urlize_middle(out, &middle, u);
    out.push_str(tail);
}

fn urlize_middle(out: &mut String, middle: &str, u: &Urlize) {
    let shown = match u.trim_limit {
        Some(limit) if middle.chars().count() > limit => {
            format!("{}...", char_slice(middle, 0, limit))
        }
        _ => middle.to_string(),
    };
    let lower = to_lower(middle);
    let scheme =
        u.schemes.iter().any(|sc| middle != sc.as_str() && middle.starts_with(sc.as_str()));
    let mailto = middle.strip_prefix("mailto:").filter(|a| match_email(a));
    let bare_email = middle.contains('@')
        && !middle.starts_with("www.")
        && !middle.contains(':')
        && match_email(middle);
    let _ = if match_http(middle) {
        let http = lower.starts_with("http://") || lower.starts_with("https://");
        let href = if http { middle.to_string() } else { format!("https://{middle}") };
        write!(out, "<a href=\"{href}\"{}>{shown}</a>", u.attrs)
    } else if let Some(address) = mailto {
        write!(out, "<a href=\"{middle}\">{address}</a>")
    } else if bare_email {
        write!(out, "<a href=\"mailto:{middle}\">{middle}</a>")
    } else if scheme {
        write!(out, "<a href=\"{middle}\"{}>{middle}</a>", u.attrs)
    } else {
        out.write_str(middle)
    };
}

fn is_label_char(ch: char) -> bool {
    is_word_char(ch) || ch == '%' || ch == '-'
}

/// Follows Jinja's _http_re: a scheme or www. with a host and tld, a bare domain with a
/// common tld, or a scheme with an IP address; then port and path.
fn match_http(text: &str) -> bool {
    let lower = to_lower(text);
    let scheme_len =
        ["https://", "http://"].iter().find(|p| lower.starts_with(**p)).map_or(0, |p| p.len());
    let mut host = &text[scheme_len..];
    if let Some(cut) = host.find(['/', '?', '#']) {
        host = &host[..cut];
    }
    if let Some(colon) = host.rfind(':')
        && !host.ends_with(']')
    {
        let port = &host[colon + 1..];
        if (1..=5).contains(&port.len()) && port.bytes().all(|b| b.is_ascii_digit()) {
            host = &host[..colon];
        }
    }
    if scheme_len > 0 && is_ip_host(host) {
        return true;
    }
    let mut has_scheme = scheme_len > 0;
    if !has_scheme && to_lower(host).starts_with("www.") {
        has_scheme = true;
        host = &host[4..];
    }
    let labels: Vec<&str> = host.split('.').collect();
    let (tld, domain) = labels.split_last().unwrap_or((&"", &[]));
    if has_scheme {
        let tld_ok = if to_lower(tld).starts_with("xn--") {
            (6..=63).contains(&tld.len())
        } else {
            (2..=63).contains(&tld.len()) && tld.bytes().all(|b| b.is_ascii_alphabetic())
        };
        return tld_ok && domain.iter().all(|l| !l.is_empty() && l.chars().all(is_label_char));
    }
    let common = ["com", "net", "int", "edu", "gov", "org", "info", "mil"];
    common.contains(&to_lower(tld).as_str())
        && !domain.is_empty()
        && domain
            .iter()
            .all(|l| (2..=63).contains(&l.chars().count()) && l.chars().all(is_label_char))
}

fn is_ip_host(host: &str) -> bool {
    if let Some(inner) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        return inner.matches(':').count() >= 2
            && inner.chars().all(|ch| ch == ':' || ch.is_ascii_hexdigit());
    }
    let parts: Vec<&str> = host.split('.').collect();
    parts.len() == 4
        && parts
            .iter()
            .all(|p| (1..=3).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Follows `^\S+@\w[\w.-]*\.\w+$`.
fn match_email(text: &str) -> bool {
    let Some((user, domain)) = text.rsplit_once('@') else { return false };
    let Some((name, tld)) = domain.rsplit_once('.') else { return false };
    !user.is_empty()
        && !user.contains(is_space)
        && name.chars().next().is_some_and(is_word_char)
        && name.chars().all(|ch| is_word_char(ch) || ch == '.' || ch == '-')
        && !tld.is_empty()
        && tld.chars().all(is_word_char)
}
