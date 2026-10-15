//! Attributes and doc comments: what rustdoc reads from the `///`, `//!`, `/** */`,
//! `#[doc = ...]`, `#[cfg(...)]`, `#[deprecated]` and similar nodes around an item.

use crate::sig::normalize;
use tree_sitter::Node;

/// What the attributes and doc comments of one item (or the inner ones of a module)
/// say. Filled node by node, then [`Attrs::docs`] joins the doc fragments.
#[derive(Debug, Default)]
pub(crate) struct Attrs {
    /// Doc fragments in order: each is unindented on its own, as rustdoc does.
    fragments: Vec<String>,
    /// Consecutive `///` or `//!` lines form one fragment; they gather here first.
    sugared: Vec<String>,
    pub cfg: Vec<String>,
    pub attributes: Vec<String>,
    pub deprecated: Option<String>,
    pub is_hidden: bool,
    pub is_test: bool,
    pub inline: Option<bool>,
    pub path: Option<String>,
    pub is_exported: bool,
}

/// Reads a file named by `#[doc = include_str!("...")]`, relative to the source file;
/// `None` when it cannot be read or is outside the crate.
pub(crate) type Include<'a> = dyn FnMut(&str) -> Option<String> + 'a;

impl Attrs {
    /// Records a `line_comment` or `block_comment` node when it is a doc comment of the
    /// wanted kind (`inner` for `//!`, otherwise `///`). Plain comments are ignored.
    pub fn add_comment(&mut self, node: Node, source: &str, inner: bool) {
        let marker = if inner { "inner" } else { "outer" };
        if node.child_by_field_name(marker).is_none() {
            return;
        }
        let Some(doc) = node.child_by_field_name("doc") else {
            // `///` with nothing after it is an empty doc line.
            if node.kind() == "line_comment" {
                self.sugared.push(String::new());
            }
            return;
        };
        let text = &source[doc.byte_range()];
        if node.kind() == "line_comment" {
            self.sugared.push(text.trim_end_matches(['\n', '\r']).to_string());
        } else {
            self.flush_sugared();
            self.fragments.push(unindent(&strip_stars(text)));
        }
    }

    /// Records an `attribute` node (the inside of `#[...]` or `#![...]`).
    pub fn add_attribute(&mut self, attr: Node, source: &str, include: &mut Include<'_>) {
        let Some(path) = attr.named_child(0) else { return };
        let name = &source[path.byte_range()];
        let arguments = attr
            .child_by_field_name("arguments")
            .map(|a| inside_brackets(&source[a.byte_range()]))
            .unwrap_or("");
        let value = attr.child_by_field_name("value");
        match name {
            "doc" => self.add_doc_attribute(arguments, value, source, include),
            "cfg" if arguments.trim() == "test" => self.is_test = true,
            "cfg" => self.cfg.push(normalize(arguments)),
            "test" => self.is_test = true,
            "deprecated" => self.deprecated = Some(deprecation_note(arguments, value, source)),
            "path" => self.path = value.map(|v| string_value(&source[v.byte_range()])),
            "macro_export" => self.is_exported = true,
            "non_exhaustive" | "repr" | "no_mangle" | "export_name" | "link_section" => {
                self.attributes.push(format!("#[{}]", normalize(&source[attr.byte_range()])));
            }
            _ => {}
        }
    }

    fn add_doc_attribute(
        &mut self,
        arguments: &str,
        value: Option<Node>,
        source: &str,
        include: &mut Include<'_>,
    ) {
        if let Some(value) = value {
            let text = match value.kind() {
                "string_literal" | "raw_string_literal" => {
                    Some(string_value(&source[value.byte_range()]))
                }
                "macro_invocation" => included_text(value, source, include),
                _ => None,
            };
            if let Some(text) = text {
                self.flush_sugared();
                self.fragments.push(unindent(&text));
            }
            return;
        }
        for part in split_top_level(arguments) {
            match part.trim() {
                "hidden" => self.is_hidden = true,
                "inline" => self.inline = Some(true),
                "no_inline" => self.inline = Some(false),
                other => {
                    if let Some(cfg) = other.strip_prefix("cfg") {
                        self.cfg.push(normalize(inside_brackets(cfg.trim())));
                    }
                }
            }
        }
    }

    fn flush_sugared(&mut self) {
        if !self.sugared.is_empty() {
            let joined = self.sugared.join("\n");
            self.fragments.push(unindent(&joined));
            self.sugared.clear();
        }
    }

    /// Joins the doc fragments into the item's Markdown, trimmed of blank edges.
    pub fn docs(&mut self) -> String {
        self.flush_sugared();
        let joined = self.fragments.join("\n");
        joined.trim_matches('\n').trim_end().to_string()
    }
}

/// The text of a file named by `include_str!("path")`, through `include`.
fn included_text(macro_node: Node, source: &str, include: &mut Include<'_>) -> Option<String> {
    let name = macro_node.child_by_field_name("macro")?;
    if &source[name.byte_range()] != "include_str" {
        return None;
    }
    let mut cursor = macro_node.walk();
    let tokens = macro_node.named_children(&mut cursor).find(|n| n.kind() == "token_tree")?;
    let mut inner = tokens.walk();
    let literal = tokens.named_children(&mut inner).find(|n| n.kind() == "string_literal")?;
    include(&string_value(&source[literal.byte_range()]))
}

/// The note of `#[deprecated]`, `#[deprecated = "note"]` or
/// `#[deprecated(since = "1.2", note = "note")]`; empty when there is none.
fn deprecation_note(arguments: &str, value: Option<Node>, source: &str) -> String {
    if let Some(value) = value {
        return string_value(&source[value.byte_range()]);
    }
    let mut since = String::new();
    for part in split_top_level(arguments) {
        let Some((key, val)) = part.split_once('=') else { continue };
        match key.trim() {
            "note" => return string_value(val.trim()),
            "since" => since = format!("since {}", string_value(val.trim())),
            _ => {}
        }
    }
    since
}

/// The text between the outer brackets of `(...)`, `[...]` or `{...}`; the text itself
/// when it is not bracketed.
pub(crate) fn inside_brackets(text: &str) -> &str {
    let text = text.trim();
    let bracketed = text.len() >= 2
        && matches!(text.as_bytes()[0], b'(' | b'[' | b'{')
        && matches!(text.as_bytes()[text.len() - 1], b')' | b']' | b'}');
    if bracketed { &text[1..text.len() - 1] } else { text }
}

/// Splits on commas outside brackets and strings.
pub(crate) fn split_top_level(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut start = 0;
    for (i, c) in text.char_indices() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' | '[' | '{' | '<' => depth += 1,
            ')' | ']' | '}' | '>' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if !text[start..].trim().is_empty() {
        parts.push(&text[start..]);
    }
    parts
}

/// The value of a Rust string literal (`"a\nb"`, `r#"raw"#`), escapes resolved. Text
/// that is not a literal comes back unchanged.
pub(crate) fn string_value(literal: &str) -> String {
    let literal = literal.trim();
    if let Some(raw) = literal.strip_prefix('r') {
        let hashes = raw.len() - raw.trim_start_matches('#').len();
        let body = &raw[hashes..raw.len().saturating_sub(hashes)];
        let body = body.strip_prefix('"').unwrap_or(body);
        return body.strip_suffix('"').unwrap_or(body).to_string();
    }
    let Some(body) = literal.strip_prefix('"').and_then(|l| l.strip_suffix('"')) else {
        return literal.to_string();
    };
    unescape(body)
}

fn unescape(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some('u') => {
                let code: String = chars.by_ref().skip(1).take_while(|&c| c != '}').collect();
                let decoded = u32::from_str_radix(&code, 16).ok().and_then(char::from_u32);
                out.push(decoded.unwrap_or('\u{fffd}'));
            }
            // A backslash at the end of a line continues the string on the next one,
            // without the line break and the next line's indentation.
            Some('\n') => {
                while chars.peek().is_some_and(|c| c.is_whitespace()) {
                    chars.next();
                }
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// Removes the ` * ` decoration of a `/** */` comment's lines when every line after the
/// first has it.
fn strip_stars(text: &str) -> String {
    let text = text.trim_end().trim_end_matches('*');
    let mut lines = text.lines();
    let first = lines.next().unwrap_or("");
    let rest: Vec<&str> = lines.collect();
    let decorated = rest.iter().all(|l| l.trim().is_empty() || l.trim_start().starts_with('*'));
    let mut out = String::from(first);
    for line in rest {
        out.push('\n');
        if decorated {
            let trimmed = line.trim_start();
            out.push_str(trimmed.strip_prefix('*').unwrap_or(trimmed));
        } else {
            out.push_str(line);
        }
    }
    out
}

/// Removes the indentation every non-blank line shares, as rustdoc does to the leading
/// space of `/// text`. Trailing spaces stay: two of them are a hard line break.
pub(crate) fn unindent(text: &str) -> String {
    let indent = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    let mut out = String::with_capacity(text.len());
    for (i, line) in text.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if line.len() >= indent && line.is_char_boundary(indent) {
            out.push_str(line[indent..].trim_end_matches('\r'));
        } else {
            out.push_str(line.trim());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_unescape() {
        assert_eq!(string_value(r#""a\nb \"q\" \u{41}""#), "a\nb \"q\" A");
        assert_eq!(string_value(r###"r#"raw "x""#"###), "raw \"x\"");
        assert_eq!(string_value("\"one \\\n    two\""), "one two");
    }

    #[test]
    fn unindent_keeps_relative_indentation() {
        assert_eq!(unindent(" a\n\n   b"), "a\n\n  b");
        assert_eq!(strip_stars("\n * one\n *   two\n "), "\n one\n   two");
    }

    #[test]
    fn top_level_split_skips_nested_commas() {
        assert_eq!(split_top_level("a(b, c), \"d,e\", f"), vec!["a(b, c)", " \"d,e\"", " f"]);
    }
}
