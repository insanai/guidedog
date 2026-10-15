//! The engine's capacity results through the real readers and renderers (GDS 0003): the
//! output limit that exactly fits succeeds, one byte less fails with a `Capacity` result
//! naming the output and the exact size, and documents whose anchors outgrow their nodes
//! (long and wide headings, many headings) convert.
//!
//! The engine grows owned buffers, so only the limits a caller sets (output, nodes, text)
//! can be one short.

use guidedoc::{Region, Status, Workspace, convert, convert_text, default_request};

const RST: &str = "Title\n=====\n\nIntro with *emphasis*, ``code``, a link_ and [#]_.\n\n\
    .. _link: https://example.org\n\n.. [#] A footnote.\n\n- one\n- two\n\n  1. nested\n  \
    2. list\n\n+-----+-----+\n| a   | b   |\n+=====+=====+\n| c   | d   |\n+-----+-----+\n\n\
    .. note:: Careful.\n\n.. code-block:: rust\n   :class: example\n\n   fn main() {}\n";

const MARKDOWN: &str = "# Title\n\nSome *text* with [a link](https://example.org \"t\") and \
    `code`.\n\n1. one\n2. two\n\n> quote\n\n```rust\nfn main() {}\n```\n";

/// Converts `text` with the output limited to `limit` bytes.
fn convert_limited(
    ws: &mut Workspace,
    reader: &str,
    renderer: &str,
    text: &str,
    limit: usize,
) -> Result<usize, guidedoc::Fail> {
    let registry = guidedoc_defaults::registry();
    let mut request = default_request("capacity", text, reader, renderer);
    request.render.standalone = true;
    request.limits.max_output_bytes = limit;
    convert(ws, &registry, &request).map(str::len)
}

#[test]
fn the_output_limit_has_an_exact_and_a_one_short_capacity() {
    let mut ws = Workspace::new();
    for (reader, text) in [("rst", RST), ("commonmark", MARKDOWN)] {
        for renderer in ["html", "typst", "text"] {
            let size = convert_limited(&mut ws, reader, renderer, text, usize::MAX)
                .unwrap_or_else(|f| panic!("{reader} to {renderer}: {:?}", f.status));
            assert!(size > 0);
            let exact = convert_limited(&mut ws, reader, renderer, text, size);
            assert_eq!(exact.as_ref().ok(), Some(&size), "{reader}/{renderer} exact");
            let short = convert_limited(&mut ws, reader, renderer, text, size - 1).unwrap_err();
            assert_eq!(short.status, Status::Capacity, "{reader}/{renderer} one short");
            assert_eq!(short.need.region, Region::Output);
            assert!(short.need.exact);
            assert_eq!(short.need.minimum, size as u64, "{reader}/{renderer} needs");
        }
    }
}

/// Node and text limits one short of a document fail with a result, never a panic.
#[test]
fn node_and_text_limits_fail_with_a_result() {
    let registry = guidedoc_defaults::registry();
    let mut ws = Workspace::new();
    for (reader, text) in [("rst", RST), ("commonmark", MARKDOWN)] {
        let mut request = default_request("capacity", text, reader, "html");
        request.limits.max_nodes = 3;
        let fail = convert(&mut ws, &registry, &request).unwrap_err();
        assert!(matches!(fail.status, Status::Limit | Status::Capacity), "{reader} nodes");
        let mut request = default_request("capacity", text, reader, "html");
        request.limits.max_text_bytes = 8;
        let fail = convert(&mut ws, &registry, &request).unwrap_err();
        assert!(matches!(fail.status, Status::Limit | Status::Capacity), "{reader} text");
    }
}

/// Anchor storage depends on heading text, not on the number of nodes: long headings and
/// heading-only documents once failed with an unrecoverable scratch shortfall.
#[test]
fn long_headings_convert() {
    let registry = guidedoc_defaults::registry();
    let many: String = (0..50)
        .map(|i| format!("## Release notes for version {i} of the product and its many parts\n"))
        .collect();
    let cases = [
        ("commonmark", format!("# {}\n", "a".repeat(400))),
        ("commonmark", format!("# {}\n", "中".repeat(120))),
        ("commonmark", many),
        ("rst", format!("{}\n{}\n", "T".repeat(300), "=".repeat(300))),
    ];
    let mut ws = Workspace::new();
    for (reader, text) in &cases {
        for renderer in ["html", "typst"] {
            let result = convert_text(&mut ws, &registry, reader, renderer, text);
            assert!(result.is_ok(), "{reader} to {renderer}: {:?}", result.err());
        }
    }
}
