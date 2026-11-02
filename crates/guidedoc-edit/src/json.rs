//! The JSON form of a source map, for tools in other languages.

use std::fmt::Write;

use gd_report::escape_json;

use crate::edit::block_operations;
use crate::map::{Block, BlockId, DirectiveParts, SourceMap};
use crate::{SOURCE_MAP_VERSION, Span};

/// Appends the JSON form of `map`, made from `text`, to `out`:
///
/// ```json
/// {"version": 1, "format": "rst", "length": 120, "truncated": false, "roots": [0],
///  "blocks": [{"id": 0, "kind": "section", "span": [0, 120], "content": [],
///              "text": "", "level": 0, "parent": null, "children": [1, 2],
///              "directive": null, "operations": ["insert-block", ...]}, ...]}
/// ```
///
/// Spans are `[start, end]` byte offsets. `text` is the editable text (the content
/// lines joined by line breaks). A directive is `{"name": "note", "name_span": [..],
/// "arguments": [..] | null, "options": [{"name": "class", "span": [..],
/// "name_span": [..], "value": [..] | null}], "yaml_options": false, "content": [..] |
/// null}`.
pub fn write_source_map_json(map: &SourceMap, text: &str, out: &mut String) {
    let _ = write!(
        out,
        "{{\"version\":{SOURCE_MAP_VERSION},\"format\":\"{}\",\"length\":{},\
         \"truncated\":{},\"roots\":",
        map.format.name(),
        map.len,
        map.truncated
    );
    ids(out, &map.roots);
    out.push_str(",\"blocks\":[");
    for (i, block) in map.blocks.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_block(out, map, text, BlockId(i as u32), block);
    }
    out.push_str("]}");
}

fn write_block(out: &mut String, map: &SourceMap, text: &str, id: BlockId, block: &Block) {
    let _ = write!(out, "{{\"id\":{},\"kind\":\"{}\",\"span\":", id.0, block.kind.name());
    span(out, block.span);
    out.push_str(",\"content\":[");
    for (i, s) in block.content.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        span(out, *s);
    }
    out.push_str("],\"text\":");
    escape_json(out, &map.content_text(id, text));
    let _ = write!(out, ",\"level\":{},\"parent\":", block.level);
    match block.parent {
        Some(p) => {
            let _ = write!(out, "{}", p.0);
        }
        None => out.push_str("null"),
    }
    out.push_str(",\"children\":");
    ids(out, &block.children);
    out.push_str(",\"directive\":");
    match &block.directive {
        Some(parts) => directive(out, text, parts),
        None => out.push_str("null"),
    }
    out.push_str(",\"operations\":[");
    for (i, op) in block_operations(map, text, id).iter().enumerate() {
        let lead = if i > 0 { "," } else { "" };
        let _ = write!(out, "{lead}\"{}\"", op.name());
    }
    out.push_str("]}");
}

fn directive(out: &mut String, text: &str, parts: &DirectiveParts) {
    out.push_str("{\"name\":");
    escape_json(out, parts.name.text(text));
    out.push_str(",\"name_span\":");
    span(out, parts.name);
    out.push_str(",\"arguments\":");
    optional_span(out, parts.arguments);
    out.push_str(",\"options\":[");
    for (i, o) in parts.options.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"name\":");
        escape_json(out, o.name.text(text));
        out.push_str(",\"span\":");
        span(out, o.span);
        out.push_str(",\"name_span\":");
        span(out, o.name);
        out.push_str(",\"value\":");
        optional_span(out, o.value);
        out.push('}');
    }
    let _ = write!(out, "],\"yaml_options\":{},\"content\":", parts.yaml_options);
    optional_span(out, parts.content);
    out.push('}');
}

fn span(out: &mut String, s: Span) {
    let _ = write!(out, "[{},{}]", s.start, s.end);
}

fn optional_span(out: &mut String, s: Option<Span>) {
    match s {
        Some(s) => span(out, s),
        None => out.push_str("null"),
    }
}

fn ids(out: &mut String, list: &[BlockId]) {
    out.push('[');
    for (i, id) in list.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "{}", id.0);
    }
    out.push(']');
}
