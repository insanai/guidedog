//! Older Typst records (`#let gds-number = "0007"`), as the first GDS catalog wrote them.
//! They are read, changed, and indexed; building such a catalog is a [`crate::Builder`]'s
//! optional work.

use crate::metadata::{
    FieldValue, REQUIRED, Record, literal_end, metadata_problem, parse_literal, quote, set_value,
    take_slug,
};
use crate::{Prefix, Result};
use std::path::Path;

/// Reads the `#let PREFIX-name = literal` lines before the first heading or `#show:`.
pub(crate) fn parse_record(path: &Path, source: &str, prefix: &Prefix) -> Result<Record> {
    let mut r = Record { path: path.to_path_buf(), ..Record::default() };
    let marker = format!("#let {prefix}-");
    let mut fields: Vec<(&str, &str)> = Vec::with_capacity(REQUIRED.len());
    for line in source.lines() {
        let s = line.trim();
        if s.starts_with("#show:") || s.starts_with("= ") {
            break;
        }
        let Some(field) = s.strip_prefix(&marker) else { continue };
        let Some((key, raw)) = field.split_once('=') else {
            return Err(metadata_problem(prefix, path, field.trim()));
        };
        let key = key.trim();
        if fields.iter().any(|(k, _)| *k == key) || !REQUIRED.contains(&key) {
            return Err(metadata_problem(prefix, path, key));
        }
        fields.push((key, raw.trim()));
    }
    for key in REQUIRED {
        let raw = fields.iter().find(|(k, _)| *k == key).map_or("", |(_, v)| v);
        if key == "authors" || key == "labels" {
            let values = parse_tuple(raw).filter(|v| !v.is_empty());
            let Some(values) = values else { return Err(metadata_problem(prefix, path, key)) };
            if key == "authors" {
                r.authors = values;
            } else {
                r.labels = values;
            }
        } else {
            match parse_literal(raw) {
                Some((value, tail)) if literal_end(tail) => set_value(&mut r, key, value),
                _ => return Err(metadata_problem(prefix, path, key)),
            }
        }
    }
    take_slug(&mut r, prefix)?;
    Ok(r)
}

/// A tuple of string literals, `("Ada", "René",)`, with an optional trailing comment.
fn parse_tuple(input: &str) -> Option<Vec<String>> {
    let mut s = input.trim().strip_prefix('(')?.trim_start();
    let mut values = Vec::new();
    while !s.is_empty() && !s.starts_with(')') {
        let (value, rest) = parse_literal(s)?;
        values.push(value);
        s = rest.trim_start();
        if let Some(rest) = s.strip_prefix(',') {
            s = rest.trim_start();
        } else if !s.starts_with(')') {
            return None;
        }
    }
    let rest = s.strip_prefix(')')?;
    literal_end(rest).then_some(values)
}

/// `value` as a Typst record writes it: a string literal, or a tuple of them.
pub(crate) fn field_text(value: FieldValue<'_>) -> String {
    match value {
        FieldValue::Text(text) => quote(text),
        FieldValue::List(values) => {
            let mut out = String::from("(");
            for v in values {
                out.push_str(&quote(v));
                out.push(',');
            }
            out.push(')');
            out
        }
    }
}

/// The lifecycle event `text` as a Typst paragraph of short string literals.
pub(crate) fn history(text: &str) -> String {
    let mut out = String::from("\n// Lifecycle event recorded by guidedog.\n");
    out.push_str("\n#par()[#text(\n  \"\"");
    let mut chunk = String::new();
    // Chunks are limited before quoting, leaving room for escaped punctuation.
    for c in text.chars() {
        chunk.push(c);
        if chunk.len() >= 36 {
            out.push_str(" +\n  ");
            out.push_str(&quote(&chunk));
            chunk.clear();
        }
    }
    if !chunk.is_empty() {
        out.push_str(" +\n  ");
        out.push_str(&quote(&chunk));
    }
    out.push_str("\n)]\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tuples_hold_only_string_literals() {
        assert_eq!(
            parse_tuple("(\"Ada\", \"René\",) // two"),
            Some(vec!["Ada".to_string(), "René".to_string()])
        );
        assert_eq!(parse_tuple("()"), Some(vec![]));
        assert_eq!(parse_tuple("(read(\"x\"),)"), None);
        assert_eq!(parse_tuple("(\"a\""), None);
    }
}
