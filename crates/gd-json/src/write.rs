//! Writing values as JSON text.

use crate::Value;
use std::fmt::Write;

/// Writes `value` as compact JSON (no spaces or line breaks) into a new string.
pub fn to_string(value: &Value) -> String {
    let mut out = String::with_capacity(64);
    write_value(&mut out, value);
    out
}

/// Writes `value` as JSON indented by two spaces per level, without a final line break.
pub fn to_string_pretty(value: &Value) -> String {
    let mut out = String::with_capacity(128);
    write_pretty(&mut out, value, 0);
    out
}

/// Appends `value` as compact JSON to `out`. Object keys keep their order; NaN and
/// infinite floats are written as `null`, since JSON cannot hold them.
pub fn write_value(out: &mut String, value: &Value) {
    match value {
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, item);
            }
            out.push(']');
        }
        Value::Object(object) => {
            out.push('{');
            for (i, (key, item)) in object.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(out, key);
                out.push(':');
                write_value(out, item);
            }
            out.push('}');
        }
        scalar => write_scalar(out, scalar),
    }
}

/// Appends `value` as JSON to `out`, indented as if it started at nesting `level`: each
/// array item and object member on its own line, two spaces deeper. Empty arrays and
/// objects stay `[]` and `{}`.
pub fn write_pretty(out: &mut String, value: &Value, level: usize) {
    match value {
        Value::Array(items) if !items.is_empty() => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                out.push_str(if i > 0 { ",\n" } else { "\n" });
                indent(out, level + 1);
                write_pretty(out, item, level + 1);
            }
            out.push('\n');
            indent(out, level);
            out.push(']');
        }
        Value::Object(object) if !object.is_empty() => {
            out.push('{');
            for (i, (key, item)) in object.iter().enumerate() {
                out.push_str(if i > 0 { ",\n" } else { "\n" });
                indent(out, level + 1);
                write_string(out, key);
                out.push_str(": ");
                write_pretty(out, item, level + 1);
            }
            out.push('\n');
            indent(out, level);
            out.push('}');
        }
        other => write_value(out, other),
    }
}

fn indent(out: &mut String, level: usize) {
    for _ in 0..level {
        out.push_str("  ");
    }
}

fn write_scalar(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Integer(n) => {
            let _ = write!(out, "{n}");
        }
        Value::Float(f) if f.is_finite() => {
            // Debug keeps a point or an exponent (1.0, 1e300), so the number reads back
            // as a float, and never prints hundreds of digits.
            let _ = write!(out, "{f:?}");
        }
        Value::Float(_) => out.push_str("null"),
        Value::String(s) => write_string(out, s),
        Value::Array(_) | Value::Object(_) => write_value(out, value),
    }
}

/// Appends `text` to `out` as a JSON string with its quotes. Quotes, backslashes and
/// control characters are escaped, and so are `<`, `>`, `&` and U+2028/U+2029, so the
/// output can be embedded in an HTML `<script>` element unchanged.
pub fn write_string(out: &mut String, text: &str) {
    out.reserve(text.len() + 2);
    out.push('"');
    let mut run = 0;
    for (i, c) in text.char_indices() {
        if needs_escape(c) {
            out.push_str(&text[run..i]);
            write_escape(out, c);
            run = i + c.len_utf8();
        }
    }
    out.push_str(&text[run..]);
    out.push('"');
}

fn needs_escape(c: char) -> bool {
    matches!(c, '"' | '\\' | '<' | '>' | '&' | '\u{2028}' | '\u{2029}') || (c as u32) < 0x20
}

fn write_escape(out: &mut String, c: char) {
    match c {
        '"' => out.push_str("\\\""),
        '\\' => out.push_str("\\\\"),
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        _ => {
            let _ = write!(out, "\\u{:04x}", c as u32);
        }
    }
}
