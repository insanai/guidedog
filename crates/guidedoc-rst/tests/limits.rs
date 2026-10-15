//! Limits: repeated reads in one workspace, and depth, stack, work and node limits reported as
//! results.

#![allow(clippy::unwrap_used)]

mod common;

use std::sync::Arc;

use common::*;
use guidedoc::{
    NodeKind, Resource, ResourceProvider, ResourceStatus, Status, Workspace, read_document,
};
use guidedoc_rst::reader;

/// One block of every body construct and most directives, without names, so it can be
/// repeated.
const MIXED: &str = r#"Section
=======

A paragraph with *emphasis*, **strong**, ``literal`` and a footnote [#]_.

* bullet one
* bullet two

  continued

1. enumerated
2. second

term : classifier
   Definition.

:field: body
:other: more

-a, --all  Option description.

Literal follows::

    code	with tab

| line one
|    nested line

    Block quote.

    -- Attribution

>>> doctest

+-----+-----+
| a   | b   |
+=====+=====+
| c spanning|
+-----+-----+

=====  =====
x      y
=====  =====
1      2
=====  =====

.. [#] Footnote body.

.. note:: Admonition text.

.. figure:: picture.png
   :alt: alt

   Caption.

.. code:: odin

   main :: proc() {}

.. csv-table:: CSV
   :header: "h1", "h2"

   1, "two, quoted"

.. list-table::

   * - l1
     - l2

.. math::

   x^2

.. container:: box

   Inside.

.. class:: special

Classed paragraph.

.. A comment.

----------

After the transition.

"#;

const MIXED_ONCE: &str = r#"
.. _target: http://example.com

Reference to target_ and |sub|.

.. role:: custom

.. |sub| unicode:: U+2014

.. include:: part.rst

.. include:: code.py
   :code: python
"#;

struct Files;

impl ResourceProvider for Files {
    fn provide(&self, _from: &str, name: &str) -> Result<Resource, ResourceStatus> {
        let text = match name {
            "part.rst" => "Included *text*.\n",
            "code.py" => "x = 1\n",
            _ => return Err(ResourceStatus::Missing),
        };
        Ok(Resource { text: Arc::from(text), canonical: name.to_owned() })
    }
}

/// Runs `f` on a thread with a large stack, as deep debug-build recursion needs.
fn with_big_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().stack_size(256 << 20).spawn(f).unwrap().join().unwrap();
}

#[test]
fn every_construct_reads_repeatedly() {
    let text = format!("{MIXED_ONCE}{}", MIXED.repeat(40));
    let mut request = request_for(&text);
    request.resources = Some(&Files);
    let mut ws = Workspace::new();
    let doc = read_document(&mut ws, &reader(), &request).unwrap();
    assert!(doc.node_count() > 40 * 100);
    // A second read in the same workspace gives the same tree.
    let first = guidedoc::dump_to_string(doc);
    let again = read_document(&mut ws, &reader(), &request).unwrap();
    assert_eq!(guidedoc::dump_to_string(again), first);
}

/// Reads `text` with a stack budget beyond `max_depth`'s needs even in debug builds,
/// whose frames are larger, so the depth limit is the one reached.
fn read_deep(text: &str) -> Read {
    let mut r = request_for(text);
    r.limits.stack_bytes = 128 << 20;
    read_request(&r)
}

#[test]
fn deep_block_quotes_hit_the_depth_limit() {
    let mut text = String::new();
    for k in 0..2000 {
        text.push_str(&" ".repeat(k));
        text.push_str("level\n\n");
    }
    with_big_stack(move || {
        let r = read_deep(&text);
        assert_eq!(r.status, Status::Limit);
        expect_codes_in(&r.codes, "core.limit.depth");
    });
}

#[test]
fn deep_lists_hit_the_depth_limit() {
    with_big_stack(|| {
        let r = read_deep(&format!("{}x\n", "- ".repeat(5000)));
        assert_eq!(r.status, Status::Limit);
        expect_codes_in(&r.codes, "core.limit.depth");
    });
}

// With max_depth far beyond what any thread's stack holds, the stack guard still turns
// deep nesting into a Limit; the recursion never overflows the stack.
#[test]
fn deep_lists_past_a_huge_depth_limit_hit_the_stack_limit() {
    with_big_stack(|| {
        let text = format!("{}x\n", "- ".repeat(200_000));
        let mut r = request_for(&text);
        r.limits.max_depth = 10_000_000;
        let got = read_request(&r);
        assert_eq!(got.status, Status::Limit);
        expect_codes_in(&got.codes, "core.limit.stack");
    });
}

// A smaller stack budget stops the same nesting sooner; the default holds max_depth.
#[test]
fn stack_budget_bounds_nesting_below_max_depth() {
    with_big_stack(|| {
        let text = format!("{}x\n", "- ".repeat(100));
        assert_eq!(read_text(&text).status, Status::Ok);
        let mut r = request_for(&text);
        r.limits.stack_bytes = 16 << 10;
        let got = read_request(&r);
        assert_eq!(got.status, Status::Limit);
        expect_codes_in(&got.codes, "core.limit.stack");
    });
}

#[test]
fn large_grid_table_respects_the_work_limit() {
    let border = "+---".repeat(200);
    let row = "| x ".repeat(200);
    let mut text = String::new();
    for _ in 0..100 {
        text.push_str(&format!("{border}+\n{row}|\n"));
    }
    text.push_str(&format!("{border}+\n"));
    let r = read_text(&text);
    assert_eq!(r.status, Status::Ok);
    assert_eq!(r.dump.matches("cell span=1x1").count(), 200 * 100);
    let mut request = request_for(&text);
    request.limits.max_work = 20_000;
    let got = read_request(&request);
    assert_eq!(got.status, Status::Limit);
    expect_codes_in(&got.codes, "core.limit.work");
}

#[test]
fn node_limit_is_a_result() {
    let text = "para\n\n".repeat(100);
    let mut r = request_for(&text);
    r.limits.max_nodes = 50;
    let got = read_request(&r);
    assert_eq!(got.status, Status::Limit);
    expect_codes_in(&got.codes, "core.limit.nodes");
}

/// The source text of the `n`-th node of `kind`.
fn node_text_span(text: &str, kind: NodeKind, n: usize) -> String {
    let mut ws = Workspace::new();
    let request = request_for(text);
    let Ok(doc) = read_document(&mut ws, &reader(), &request) else {
        return "<no document>".into();
    };
    let found = doc.nodes().iter().filter(|node| node.kind == kind).nth(n);
    found.map_or_else(|| "<missing>".into(), |node| doc.source_text(node.span).to_owned())
}

#[test]
fn spans_are_exact_through_tabs_and_nesting() {
    let text = "-\titem\n\tcontinuation\n\n+-----+\n| cell|\n+-----+\n\n* a\n\n  * nested\n";
    assert_eq!(node_text_span(text, NodeKind::Text, 1), "continuation");
    assert_eq!(node_text_span(text, NodeKind::Paragraph, 0), "item\n\tcontinuation");
    assert_eq!(node_text_span(text, NodeKind::Cell, 0), "+-----+\n| cell|\n+-----+");
    assert_eq!(node_text_span(text, NodeKind::Text, 2), "cell");
    assert_eq!(node_text_span(text, NodeKind::List, 2), "* nested");
    assert_eq!(node_text_span(text, NodeKind::Text, 4), "nested");
    assert_eq!(node_text_span("Para::\n\n\tx\ty\n", NodeKind::CodeBlock, 0), "x\ty");
    let title = "Title\n=====\n\nBody.\n\nNext\n====\n";
    assert_eq!(node_text_span(title, NodeKind::Section, 0), "Title\n=====\n\nBody.");
    assert_eq!(node_text_span(title, NodeKind::Heading, 0), "Title");
}

// Random documents built from the characters that matter to the block parser must
// read to some result without crashing or hanging.
#[test]
fn random_markup_terminates() {
    let alphabet: Vec<char> = " \n\n\n  -=+|*:._`#>()[]\\ab1iI\t".chars().collect();
    let pieces = [
        ".. ",
        "::",
        "\n\n",
        "+---+",
        "| ",
        "===  ===",
        ".. |x| ",
        "replace:: ",
        "__ ",
        ".. _a: ",
        "- ",
        "1. ",
        ":f: ",
        "-a  ",
        "   ",
        "\n   ",
        ".. note::",
        "\n----\n",
        ".. [#] ",
        ".. csv-table::\n   ",
        "\"",
        ",",
        ".. list-table::\n   * - ",
    ];
    let mut state: u64 = 7;
    let mut next = |n: usize| {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((state >> 33) as usize) % n
    };
    for _ in 0..1500 {
        let mut text = String::new();
        for _ in 0..next(300) {
            if next(4) == 0 {
                text.push_str(pieces[next(pieces.len())]);
            } else {
                text.push(alphabet[next(alphabet.len())]);
            }
        }
        let r = read_text(&text);
        assert!(!matches!(r.status, Status::Internal | Status::Capacity), "{text:?}: {}", r.codes);
    }
}

// In optimized builds the default stack budget holds the default depth limit.
#[cfg(not(debug_assertions))]
#[test]
fn default_stack_budget_holds_the_default_depth() {
    with_big_stack(|| {
        let r = read_text(&format!("{}x\n", "- ".repeat(5000)));
        assert_eq!(r.status, Status::Limit);
        expect_codes_in(&r.codes, "core.limit.depth");
    });
}
