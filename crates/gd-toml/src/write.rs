//! Writing values back as TOML text.

use crate::{Data, Table, Value};
use std::fmt::Write;

/// Inline arrays longer than this many bytes are written one item per line at the top of
/// a table.
const INLINE_ARRAY: usize = 72;

/// Writes `table` as a TOML document: its plain keys first, then each sub-table under a
/// `[header]` and each array of tables as `[[header]]` items, in key order. Returns an
/// allocated string that [`crate::parse`] reads back to an equal table.
pub fn to_string(table: &Table) -> String {
    let mut out = String::with_capacity(256);
    let mut path = String::new();
    write_table(&mut out, table, &mut path);
    out
}

fn is_array_of_tables(value: &Value) -> bool {
    match &value.data {
        Data::Array(items) => !items.is_empty() && items.iter().all(Value::is_table),
        _ => false,
    }
}

/// Appends `table` as the body of the table named by the dotted header `path` (empty
/// for the root) to `out`. `path` is scratch: it is extended and restored.
pub fn write_table(out: &mut String, table: &Table, path: &mut String) {
    for (key, value) in table.iter() {
        if value.is_table() || is_array_of_tables(value) {
            continue;
        }
        write_key(out, key);
        out.push_str(" = ");
        write_top_value(out, value);
        out.push('\n');
    }
    for (key, value) in table.iter() {
        let mark = path.len();
        if !path.is_empty() {
            path.push('.');
        }
        write_key(path, key);
        if let Data::Table(sub) = &value.data {
            let has_plain = sub.iter().any(|(_, v)| !v.is_table() && !is_array_of_tables(v));
            if has_plain || sub.is_empty() {
                header(out, "[", path, "]");
            }
            write_table(out, sub, path);
        } else if let (true, Data::Array(items)) = (is_array_of_tables(value), &value.data) {
            for item in items.iter().filter_map(Value::as_table) {
                header(out, "[[", path, "]]");
                write_table(out, item, path);
            }
        }
        path.truncate(mark);
    }
}

fn header(out: &mut String, open: &str, path: &str, close: &str) {
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(open);
    out.push_str(path);
    out.push_str(close);
    out.push('\n');
}

/// Writes a value at the top of a table, spreading a long array over several lines.
fn write_top_value(out: &mut String, value: &Value) {
    let Data::Array(items) = &value.data else {
        return write_value(out, value);
    };
    let mark = out.len();
    write_value(out, value);
    if out.len() - mark <= INLINE_ARRAY || out[mark..].contains('\n') {
        return;
    }
    out.truncate(mark);
    out.push_str("[\n");
    for item in items {
        out.push_str("    ");
        write_value(out, item);
        out.push_str(",\n");
    }
    out.push(']');
}

/// Appends `value` in inline form to `out`: arrays as `[a, b]` and tables as
/// `{ k = v }`. Floats keep a point or exponent so they read back as floats.
pub fn write_value(out: &mut String, value: &Value) {
    match &value.data {
        Data::String(s) => write_string(out, s),
        Data::Integer(n) => {
            let _ = write!(out, "{n}");
        }
        Data::Float(f) => write_float(out, *f),
        Data::Boolean(b) => out.push_str(if *b { "true" } else { "false" }),
        Data::Datetime(d) => {
            let _ = write!(out, "{d}");
        }
        Data::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_value(out, item);
            }
            out.push(']');
        }
        Data::Table(t) => {
            out.push('{');
            for (i, (key, item)) in t.iter().enumerate() {
                out.push_str(if i > 0 { ", " } else { " " });
                write_key(out, key);
                out.push_str(" = ");
                write_value(out, item);
            }
            out.push_str(if t.is_empty() { "}" } else { " }" });
        }
    }
}

fn write_float(out: &mut String, f: f64) {
    if f.is_nan() {
        out.push_str("nan");
    } else if f.is_infinite() {
        out.push_str(if f > 0.0 { "inf" } else { "-inf" });
    } else {
        // Debug keeps a point or an exponent (1.0, 1e300), so the text stays a float.
        let _ = write!(out, "{f:?}");
    }
}

/// Appends `key` to `out`, bare when it is letters, digits, `_` and `-`, quoted otherwise.
pub fn write_key(out: &mut String, key: &str) {
    let bare = !key.is_empty()
        && key.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-');
    if bare {
        out.push_str(key);
    } else {
        write_basic(out, key);
    }
}

/// Appends `s` to `out` as a TOML string: multi-line text as a `'''` literal block so its
/// backslashes stay readable, everything else as an escaped basic string.
pub fn write_string(out: &mut String, s: &str) {
    let literal = s.contains('\n')
        && !s.contains("'''")
        && !s.ends_with('\'')
        && !s.bytes().any(|c| (c < 0x20 && c != b'\n' && c != b'\t') || c == 0x7f);
    if literal {
        out.push_str("'''\n");
        out.push_str(s);
        out.push_str("'''");
    } else {
        write_basic(out, s);
    }
}

fn write_basic(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
