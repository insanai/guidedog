//! The page map of a preview PDF (`<file>.sourcemap.json` beside it): where each block
//! of the book's sources stands on its pages, so that a click in the PDF viewer maps
//! back to the source (GDS 0017).
//!
//! ```json
//! {"schema": 1, "pdf": "guide-en.pdf", "complete": true, "pages": 12,
//!  "marks": [{"page": 1, "x": 72.0, "y": 96.5, "source": "intro.rst",
//!             "start": 0, "end": 34}]}
//! ```
//!
//! Each mark is the top left corner of a block's first line, in points from the page's
//! top left corner, with the byte span of the block's markup in its source. Marks come
//! in document order. Typst places a block's mark where the block starts; a block that
//! breaks across pages has one mark, on its first page. Section titles carry no mark of
//! their own; the first block after them does. `complete` is false when the typst
//! program, which reports no positions, typeset the book: the map then has no marks,
//! and `pages` is `null`, since that program does not report its page count either.

use std::fmt::Write;

use gd_typst::Compiled;

/// The version of the page map's form.
pub const PAGE_MAP_SCHEMA: u32 = 1;

/// The file name of the page map of the PDF `file`.
pub fn page_map_name(file: &str) -> String {
    format!("{file}.sourcemap.json")
}

/// The page map of the PDF `file`, from what compiling it reported; `positions` tells
/// whether the compiler reported positions (the embedded one does).
pub(crate) fn page_map_json(file: &str, compiled: &Compiled, positions: bool) -> String {
    let pages = compiled.pages.as_ref().map_or("null".to_string(), |p| p.len().to_string());
    let mut out = format!("{{\"schema\":{PAGE_MAP_SCHEMA},\"pdf\":");
    gd_report::escape_json(&mut out, file);
    let _ = write!(out, ",\"complete\":{positions},\"pages\":{pages},\"marks\":[");
    let mut first = true;
    for mark in &compiled.marks {
        let Some((source, start, end)) = parse_position(&mark.value) else { continue };
        if !first {
            out.push(',');
        }
        first = false;
        let point = |v: i32| f64::from(v) / 100.0;
        let _ = write!(
            out,
            "{{\"page\":{},\"x\":{},\"y\":{},\"source\":",
            mark.page,
            point(mark.x),
            point(mark.y)
        );
        gd_report::escape_json(&mut out, source);
        let _ = write!(out, ",\"start\":{start},\"end\":{end}}}");
    }
    out.push_str("]}\n");
    out
}

/// Reads `<source>:<start>-<end>`, as the Typst renderer writes a block's position.
fn parse_position(value: &str) -> Option<(&str, u64, u64)> {
    let (source, span) = value.rsplit_once(':')?;
    let (start, end) = span.split_once('-')?;
    Some((source, start.parse().ok()?, end.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_become_positions_with_spans() {
        let mark = |value: &str, page, x, y| gd_typst::Mark { value: value.into(), page, x, y };
        let compiled = Compiled {
            pages: Some(vec![String::new(), String::new()]),
            marks: vec![mark("a/intro.rst:10-24", 2, 7200, 9650), mark("broken", 1, 0, 0)],
            ..Compiled::default()
        };
        let json = page_map_json("guide.pdf", &compiled, true);
        assert_eq!(
            json,
            "{\"schema\":1,\"pdf\":\"guide.pdf\",\"complete\":true,\"pages\":2,\"marks\":[\
             {\"page\":2,\"x\":72,\"y\":96.5,\"source\":\"a/intro.rst\",\
             \"start\":10,\"end\":24}]}\n"
        );
        assert_eq!(page_map_name("guide.pdf"), "guide.pdf.sourcemap.json");
        let program = page_map_json("guide.pdf", &Compiled::default(), false);
        assert_eq!(
            program,
            "{\"schema\":1,\"pdf\":\"guide.pdf\",\"complete\":false,\"pages\":null,\
             \"marks\":[]}\n"
        );
    }
}
