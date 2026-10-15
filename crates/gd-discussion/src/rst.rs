//! reStructuredText records: the leading `:PREFIX-name:` field list, sections, the
//! lifecycle event a decision appends, and a new draft's heading.

use crate::metadata::{
    FieldValue, OPTIONAL, REQUIRED, Record, metadata_problem, parse_literal, quote, set_value,
    take_slug,
};
use crate::{Prefix, Result};
use std::fmt::Write;
use std::path::Path;

/// Reads the leading `:PREFIX-name:` fields of `source`, the record at `path`.
/// `last-updated` is another name for `updated`, as the Sanin memos write it; a record
/// may not have both.
pub(crate) fn parse_record(path: &Path, source: &str, prefix: &Prefix) -> Result<Record> {
    let mut r = Record { path: path.to_path_buf(), ..Record::default() };
    let marker = format!(":{prefix}-");
    let mut seen: Vec<&str> = Vec::with_capacity(REQUIRED.len());
    for line in source.lines() {
        let s = line.trim();
        if s.is_empty() {
            continue;
        }
        let Some(field) = s.strip_prefix(&marker) else { break };
        let (key, raw) = field.split_once(':').unwrap_or((field, ""));
        let key = if key == "last-updated" { "updated" } else { key };
        let known = REQUIRED.iter().chain(&OPTIONAL).find(|k| **k == key);
        let Some(&key) = known.filter(|_| field.contains(':') && !seen.contains(&key)) else {
            return Err(metadata_problem(prefix, path, key));
        };
        seen.push(key);
        set_field(&mut r, prefix, key, raw.trim())?;
    }
    if let Some(key) = REQUIRED.iter().find(|k| !seen.contains(k)) {
        return Err(metadata_problem(prefix, path, key));
    }
    take_slug(&mut r, prefix)?;
    Ok(r)
}

/// Sets one field from its raw text: JSON string arrays for authors and labels; plain
/// text, or a quoted literal, for the others.
fn set_field(r: &mut Record, prefix: &Prefix, key: &str, raw: &str) -> Result<()> {
    if key == "authors" || key == "labels" {
        let values = json_list(raw).filter(|v| !v.is_empty());
        let Some(values) = values else { return Err(metadata_problem(prefix, &r.path, key)) };
        if key == "authors" {
            r.authors = values;
        } else {
            r.labels = values;
        }
        return Ok(());
    }
    let value = if raw.starts_with('"') {
        match parse_literal(raw) {
            Some((value, rest)) if rest.trim().is_empty() => value,
            _ => return Err(metadata_problem(prefix, &r.path, key)),
        }
    } else {
        raw.to_string()
    };
    if value.is_empty() {
        return Err(metadata_problem(prefix, &r.path, key));
    }
    set_value(r, key, value);
    Ok(())
}

/// A JSON array of strings, or `None`.
fn json_list(raw: &str) -> Option<Vec<String>> {
    let value = gd_json::parse(raw).ok()?;
    value.as_array()?.iter().map(|v| v.as_str().map(str::to_string)).collect()
}

/// `value` as an RST record writes it: plain text (quoted when it has a line break or
/// starts with a quote), or a compact JSON array.
pub(crate) fn field_text(value: FieldValue<'_>) -> String {
    match value {
        FieldValue::Text(text) if text.contains(['\n', '\r', '\t']) || text.starts_with('"') => {
            quote(text)
        }
        FieldValue::Text(text) => text.to_string(),
        FieldValue::List(values) => {
            let mut out = String::from("[");
            for (i, v) in values.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json_string(&mut out, v);
            }
            out.push(']');
            out
        }
    }
}

/// Appends `text` as a JSON string, escaping only what JSON requires, so names such as
/// `R&D` stay readable in the record.
fn write_json_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => _ = write!(out, "\\u{:04x}", c as u32),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Tells whether `source` has a section titled `name`: the title, then an underline of
/// one repeated `=` or `-` at least as long.
pub(crate) fn has_section(source: &str, name: &str) -> bool {
    let mut previous = "";
    for line in source.lines() {
        let underline = line.strip_suffix('\r').unwrap_or(line);
        let title = std::mem::replace(&mut previous, underline);
        if title == name && is_underline(underline, title.len()) {
            return true;
        }
    }
    false
}

/// Tells whether `line` underlines a title of `width` bytes: one repeated `=` or `-`.
pub(crate) fn is_underline(line: &str, width: usize) -> bool {
    let Some(first) = line.bytes().next() else { return false };
    (first == b'=' || first == b'-') && line.len() >= width && line.bytes().all(|c| c == first)
}

/// The lifecycle event `text` as a literal block after a rubric. Literal blocks keep
/// their spacing and cannot reflow like prose, so long lines split at spaces, never
/// inside a word, an escape, or a UTF-8 character.
pub(crate) fn history(text: &str) -> String {
    let mut out = String::from("\n.. rubric:: Lifecycle event\n\n::\n\n");
    for line in text.lines() {
        let mut rest = line.trim_end_matches([' ', '\t', '\r']);
        while rest.len() > 78 {
            let cut = gd_report::floor_boundary(rest, 78);
            match rest[..cut].rfind(' ') {
                Some(split) if split > 0 => {
                    let _ = writeln!(out, "   {}", &rest[..split]);
                    rest = &rest[split + 1..];
                }
                _ => break,
            }
        }
        if rest.is_empty() {
            out.push('\n');
        } else {
            let _ = writeln!(out, "   {rest}");
        }
    }
    out
}

/// Gives a new draft `title` as its heading, escaped for RST. The metadata stays the
/// identity, so a number assigned later cannot leave a stale number in the heading.
pub(crate) fn draft_title(source: &str, title: &str, prefix: &Prefix) -> String {
    let mut escaped = String::with_capacity(title.len() + 8);
    for c in title.chars() {
        match c {
            '\\' | '*' | '`' | '_' | '|' => {
                escaped.push('\\');
                escaped.push(c);
            }
            '\n' | '\r' => escaped.push(' '),
            c => escaped.push(c),
        }
    }
    let marker = format!(":{prefix}-");
    let mut lines: Vec<&str> = source.split('\n').collect();
    let Some(i) = lines.iter().position(|l| !l.is_empty() && !l.starts_with(&marker)) else {
        return source.to_string();
    };
    if i + 1 >= lines.len() || !lines[i + 1].starts_with("===") {
        return source.to_string();
    }
    // A byte count is never shorter than the title's width, even in wide scripts.
    let underline = "=".repeat(escaped.len());
    lines[i] = &escaped;
    lines[i + 1] = &underline;
    lines.join("\n")
}

/// The leading field list of a record or template, borrowed from `source`.
pub(crate) fn metadata_block<'s>(source: &'s str, prefix: &Prefix) -> &'s str {
    let marker = format!(":{prefix}-");
    let mut end = 0;
    for line in source.split_inclusive('\n') {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with(&marker) {
            break;
        }
        end += line.len();
    }
    &source[..end]
}
