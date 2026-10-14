//! The JSON form of a report, one object per report.

use crate::{Report, SCHEMA};
use std::fmt::Write;

/// Appends `text` to `out` as a JSON string literal with its quotes, escaping quotes,
/// backslashes and control characters (and U+2028/U+2029 for embedding in scripts).
pub fn escape_json(out: &mut String, text: &str) {
    out.push('"');
    escape_body(out, text);
    out.push('"');
}

fn escape_body(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || c == '\u{2028}' || c == '\u{2029}' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
}

fn field_str(out: &mut String, name: &str, value: &str) {
    out.push(',');
    escape_json(out, name);
    out.push(':');
    escape_json(out, value);
}

fn field_num(out: &mut String, name: &str, value: usize) {
    out.push(',');
    escape_json(out, name);
    let _ = write!(out, ":{value}");
}

/// Writes `report` as one JSON object on one line (schema [`SCHEMA`]): `schema`, `code`,
/// `severity`, `title`, `message`, `hint` (the hints joined by line breaks), `hints`,
/// `path`, `line`, `column`, `end_line`, `end_column`, `source` (the complete source line
/// of the excerpt, or `""`) and `category`. The terminal window and carets are left out;
/// columns count Unicode scalars. Returns an allocated string without a line break;
/// [`write_json`] appends to a reused buffer instead.
pub fn render_json(report: &Report) -> String {
    let mut out = String::with_capacity(256);
    write_json(&mut out, report);
    out
}

/// Appends the object [`render_json`] returns to `out`.
pub fn write_json(out: &mut String, report: &Report) {
    let _ = write!(out, "{{\"schema\":{SCHEMA}");
    field_str(out, "code", &report.code);
    field_str(out, "severity", report.severity.name());
    field_str(out, "title", &report.title);
    field_str(out, "message", &report.message);
    out.push_str(",\"hint\":\"");
    for (i, hint) in report.hints.iter().enumerate() {
        if i > 0 {
            out.push_str("\\n");
        }
        escape_body(out, hint);
    }
    out.push('"');
    out.push_str(",\"hints\":[");
    for (i, hint) in report.hints.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        escape_json(out, hint);
    }
    out.push(']');
    field_str(out, "path", &report.path);
    field_num(out, "line", report.line);
    field_num(out, "column", report.column);
    field_num(out, "end_line", report.end_line);
    field_num(out, "end_column", report.end_column);
    let source = report.excerpt.as_ref().map_or("", |e| e.text.as_str());
    field_str(out, "source", source);
    field_str(out, "category", &report.category);
    out.push('}');
}
