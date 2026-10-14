//! Signatures as rustdoc shows them: the declaration's source on one line, without its
//! body, comments or attributes, with whitespace collapsed the way rustfmt would write
//! the declaration on one line.

use std::ops::Range;
use tree_sitter::Node;

/// The source of `node` without the byte ranges in `cut` (bodies the caller leaves out),
/// without comments and attributes inside it, normalized to one line. A trailing `;` is
/// dropped.
pub(crate) fn signature(source: &str, node: Node, cut: &[Range<usize>]) -> String {
    let mut excluded: Vec<Range<usize>> = cut.to_vec();
    collect_noise(node, &mut excluded);
    excluded.sort_by_key(|r| r.start);
    let mut joined = String::with_capacity(node.end_byte() - node.start_byte());
    let mut at = node.start_byte();
    for range in &excluded {
        if range.start > at {
            joined.push_str(&source[at..range.start]);
            // A space keeps the pieces either side of the hole from running together;
            // normalization removes it again where it is not wanted.
            joined.push(' ');
        }
        at = at.max(range.end);
    }
    if at < node.end_byte() {
        joined.push_str(&source[at..node.end_byte()]);
    }
    let mut text = normalize(&joined);
    while text.ends_with([';', ',', ' ']) {
        text.pop();
    }
    text
}

/// Adds the ranges of comments and attributes below `node` to `out`, walking the subtree
/// with a cursor (no recursion) and skipping the insides of what it records.
fn collect_noise(node: Node, out: &mut Vec<Range<usize>>) {
    let mut cursor = node.walk();
    if !cursor.goto_first_child() {
        return;
    }
    loop {
        let current = cursor.node();
        let noise = matches!(
            current.kind(),
            "line_comment" | "block_comment" | "attribute_item" | "inner_attribute_item"
        );
        if noise {
            out.push(current.byte_range());
        } else if cursor.goto_first_child() {
            continue;
        }
        // Move to the next sibling, climbing until one exists or the walk is back at
        // `node`.
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() || cursor.node() == node {
                return;
            }
        }
    }
}

/// Collapses whitespace outside string literals to single spaces. A run that holds a
/// line break is a rustfmt line break: it disappears after an opening bracket and before
/// a closing one or a comma, and a comma left dangling before a closing bracket (the
/// trailing comma of a vertical list) is dropped.
pub(crate) fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '"' {
            let end = string_end(text, i);
            out.push_str(&text[i..end]);
            while chars.peek().is_some_and(|&(j, _)| j < end) {
                chars.next();
            }
            continue;
        }
        if !c.is_whitespace() {
            out.push(c);
            continue;
        }
        let mut broke = c == '\n';
        while let Some(&(_, next)) = chars.peek() {
            if !next.is_whitespace() {
                break;
            }
            broke |= next == '\n';
            chars.next();
        }
        let next = chars.peek().map(|&(_, n)| n);
        push_space(&mut out, next, broke);
    }
    out.trim().to_string()
}

/// Writes the single space a whitespace run becomes, or nothing, given the character
/// after the run and whether the run broke the line.
fn push_space(out: &mut String, next: Option<char>, broke: bool) {
    let Some(next) = next else { return };
    let closing = matches!(next, ')' | ']' | '>' | '}');
    if broke && closing && out.ends_with(',') {
        out.pop();
    }
    let after_open = out.ends_with(['(', '[', '<']);
    if broke && (after_open || matches!(next, ')' | ']' | '>' | ',')) {
        return;
    }
    if after_open && matches!(next, ')' | ']') {
        return;
    }
    if !out.is_empty() && !out.ends_with(' ') {
        out.push(' ');
    }
}

/// The byte index after the string literal whose opening quote is at `start`.
fn string_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    text.len()
}

/// Shortens a constant's signature the way rustdoc does: a value longer than this many
/// characters is left out.
pub(crate) const MAX_SHOWN_VALUE: usize = 48;

/// Drops `= value` from a `const` or `static` signature when the value is long, or
/// always when `always` is set (statics).
pub(crate) fn trim_value(signature: &str, always: bool) -> String {
    let Some(eq) = top_level_eq(signature) else { return signature.to_string() };
    let value = signature[eq + 1..].trim();
    if always || value.chars().count() > MAX_SHOWN_VALUE {
        return signature[..eq].trim_end().to_string();
    }
    signature.to_string()
}

/// The index of the first `=` outside brackets and strings that is not part of `==`,
/// `=>`, `<=`, `>=` or `!=`.
fn top_level_eq(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                i = string_end(text, i);
                continue;
            }
            b'(' | b'[' | b'{' | b'<' => depth += 1,
            b')' | b']' | b'}' | b'>' => depth -= 1,
            b'=' if depth == 0 => {
                let prev = if i > 0 { bytes[i - 1] } else { b' ' };
                let next = bytes.get(i + 1).copied().unwrap_or(b' ');
                if !matches!(prev, b'=' | b'!' | b'<' | b'>') && !matches!(next, b'=' | b'>') {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertical_lists_become_one_line() {
        let text = "pub fn f<\n    T,\n>(\n    a: u8,\n    b: &str,\n) -> X\nwhere\n    T: Copy,";
        assert_eq!(normalize(text), "pub fn f<T>(a: u8, b: &str) -> X where T: Copy,");
    }

    #[test]
    fn one_tuples_and_strings_are_kept() {
        assert_eq!(normalize("fn f(x: (u8,))"), "fn f(x: (u8,))");
        assert_eq!(normalize("extern  \"C  x\"   fn"), "extern \"C  x\" fn");
    }

    #[test]
    fn long_values_are_dropped() {
        assert_eq!(trim_value("pub const N: usize = 4", false), "pub const N: usize = 4");
        assert_eq!(trim_value("pub static N: usize = 4", true), "pub static N: usize");
        let long = format!("const S: &str = \"{}\"", "x".repeat(60));
        assert_eq!(trim_value(&long, false), "const S: &str");
    }
}
