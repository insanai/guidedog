//! Python's text operations that conf.py files use on settings: escapes, `%`
//! formatting, `str.format`, and a few `str` methods.

use super::{Value, lookup};

/// Writes the body of a string literal to `out`, turning escapes into characters unless
/// the literal is raw. Unknown escapes stay as written, as in Python.
pub(crate) fn unescape(out: &mut String, s: &str, raw: bool) {
    if raw {
        out.push_str(s);
        return;
    }
    let mut i = 0;
    while i < s.len() {
        let Some(c) = s[i..].chars().next() else { return };
        if c != '\\' || i + 1 >= s.len() {
            out.push(c);
            i += c.len_utf8();
            continue;
        }
        i += 1;
        let Some(n) = s[i..].chars().next() else { return };
        i += n.len_utf8();
        i = escape(out, s, i, n);
    }
}

/// Writes the escape `\n` (`n` already read, ending at `i`) and returns where the text
/// continues.
fn escape(out: &mut String, s: &str, i: usize, n: char) -> usize {
    let simple = match n {
        '\n' => Some(None),
        '\r' => return if s[i..].starts_with('\n') { i + 1 } else { i },
        '\\' | '\'' | '"' => Some(Some(n)),
        'n' => Some(Some('\n')),
        't' => Some(Some('\t')),
        'r' => Some(Some('\r')),
        'a' => Some(Some('\u{7}')),
        'b' => Some(Some('\u{8}')),
        'f' => Some(Some('\u{c}')),
        'v' => Some(Some('\u{b}')),
        _ => None,
    };
    if let Some(c) = simple {
        out.extend(c);
        return i;
    }
    let digits = match n {
        '0'..='7' => {
            // Up to three octal digits, the first of which is n.
            let start = i - 1;
            let end = s[start..].bytes().take(3).take_while(|b| (b'0'..=b'7').contains(b)).count();
            Some((start, start + end, 8))
        }
        'x' => Some((i, i + 2, 16)),
        'u' => Some((i, i + 4, 16)),
        'U' => Some((i, i + 8, 16)),
        _ => None,
    };
    let decoded = digits.and_then(|(start, end, radix)| {
        let code = u32::from_str_radix(s.get(start..end)?, radix).ok()?;
        Some((char::from_u32(code)?, end))
    });
    match decoded {
        Some((c, end)) => {
            out.push(c);
            end
        }
        None => {
            out.push('\\');
            out.push(n);
            i
        }
    }
}

/// `format % args` for `%s`, `%d`, `%i`, `%r` and `%(name)s`.
pub(crate) fn percent(format: &Value, args: &Value) -> Result<Value, String> {
    let Value::Str(s) = format else {
        return Err("The % operator is only read for formatting text.".into());
    };
    let values = match args {
        Value::List(items) => items.as_slice(),
        single => std::slice::from_ref(single),
    };
    let mut out = String::with_capacity(s.len());
    let mut next = 0;
    let mut rest = s.as_str();
    while let Some(at) = rest.find('%') {
        out.push_str(&rest[..at]);
        rest = &rest[at + 1..];
        let mut key = None;
        if let Some(named) = rest.strip_prefix('(') {
            let Some(close) = named.find(')') else {
                return Ok(Value::Str(out));
            };
            key = Some(&named[..close]);
            rest = &named[close + 1..];
        }
        let conversion = rest.chars().next();
        rest = rest.get(1..).unwrap_or("");
        if conversion == Some('%') {
            out.push('%');
            continue;
        }
        let Some(conversion) = conversion.filter(|c| "sdir".contains(*c)) else {
            return Err("This % format is computed by Python.".into());
        };
        let v = match key {
            Some(k) => lookup(args, &Value::Str(k.to_string()))?,
            None => {
                let v = values.get(next).ok_or("The % format has more fields than values.")?;
                next += 1;
                v.clone()
            }
        };
        out.push_str(&percent_field(&v, conversion)?);
    }
    out.push_str(rest);
    Ok(Value::Str(out))
}

fn percent_field(v: &Value, conversion: char) -> Result<String, String> {
    let unusable = || "A % field holds a value that is not text or a number.".to_string();
    let text = v.to_text().ok_or_else(unusable)?;
    if matches!(conversion, 'd' | 'i') && !matches!(v, Value::Int(_)) {
        return Err(unusable());
    }
    if conversion == 'r' && matches!(v, Value::Str(_)) {
        return Ok(format!("'{text}'"));
    }
    Ok(text)
}

/// The `str` methods conf.py files call on settings.
pub(crate) fn method(v: &Value, name: &str, args: &[Value]) -> Result<Value, String> {
    let computed = || format!("The method .{name}() is computed by Python.");
    let Value::Str(s) = v else { return Err(computed()) };
    let text = match (name, args) {
        ("join", [Value::List(items)]) => {
            let parts: Option<Vec<&str>> = items.iter().map(Value::as_str).collect();
            parts.ok_or("str.join is only read for lists of text.")?.join(s)
        }
        ("format", _) => return format_fields(s, args),
        ("upper", []) => s.to_uppercase(),
        ("lower", []) => s.to_lowercase(),
        ("strip", []) => s.trim().to_string(),
        ("replace", [Value::Str(from), Value::Str(to)]) => s.replace(from.as_str(), to),
        _ => return Err(computed()),
    };
    Ok(Value::Str(text))
}

/// `str.format` with positional fields: `{}` and `{0}`.
fn format_fields(format: &str, args: &[Value]) -> Result<Value, String> {
    let mut out = String::with_capacity(format.len());
    let mut next = 0;
    let mut rest = format;
    while let Some(at) = rest.find(['{', '}']) {
        out.push_str(&rest[..at]);
        let c = &rest[at..at + 1];
        rest = &rest[at + 1..];
        if let Some(after) = rest.strip_prefix(c) {
            out.push_str(c);
            rest = after;
            continue;
        }
        if c == "}" {
            out.push('}');
            continue;
        }
        let close = rest.find('}');
        let field = close.map_or("?", |n| &rest[..n]);
        let n = if field.is_empty() {
            next += 1;
            Some(next - 1)
        } else {
            field.parse::<usize>().ok()
        };
        let Some(v) = n.and_then(|n| args.get(n)) else {
            return Err(format!("The format field {{{field}}} is computed by Python."));
        };
        let text =
            v.to_text().ok_or("A format field holds a value that is not text or a number.")?;
        out.push_str(&text);
        rest = &rest[close.map_or(rest.len(), |n| n + 1)..];
    }
    out.push_str(rest);
    Ok(Value::Str(out))
}
