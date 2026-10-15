//! The include directive: in-place parsing, source spans, literal and code inclusion, line and
//! text ranges.

#![allow(clippy::unwrap_used)]

mod common;

use std::sync::Arc;

use common::*;
use guidedoc::{
    NodeKind, Request, Resource, ResourceProvider, ResourceStatus, SourceId, Status, Workspace,
    read_document,
};
use guidedoc_rst::reader;

/// An in-test resource provider; "refused.rst" is refused.
struct Files(Vec<(&'static str, &'static str)>);

impl ResourceProvider for Files {
    fn provide(&self, _from: &str, name: &str) -> Result<Resource, ResourceStatus> {
        if name == "refused.rst" {
            return Err(ResourceStatus::Refused);
        }
        let (n, text) = self.0.iter().find(|(n, _)| *n == name).ok_or(ResourceStatus::Missing)?;
        Ok(Resource { text: Arc::from(*text), canonical: (*n).to_owned() })
    }
}

fn include_request<'a>(text: &'a str, files: &'a Files) -> Request<'a> {
    let mut r = request_for(text);
    r.resources = Some(files);
    r
}

#[track_caller]
fn expect_included(text: &str, files: &[(&'static str, &'static str)], want: &str) {
    let files = Files(files.to_vec());
    let r = read_request(&include_request(text, &files));
    assert!(r.ok, "{:?}: {}", r.status, r.codes);
    assert_eq!(r.dump, want, "got:\n{}", r.dump);
}

#[test]
fn include_parses_in_place() {
    expect_included(
        "Before.\n\n.. include:: part.rst\n\nAfter.\n",
        &[("part.rst", "Included paragraph.\n\n- item\n")],
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"Before.\"\n",
            "  paragraph\n",
            "    text \"Included paragraph.\"\n",
            "  list +tight list=bullet,-,0\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"item\"\n",
            "  paragraph\n",
            "    text \"After.\"\n",
        ),
    );
}

#[test]
fn included_nodes_have_spans_in_their_source() {
    let files = Files(vec![("part.rst", "Inside text.\n")]);
    let mut ws = Workspace::new();
    let request = include_request(".. include:: part.rst\n", &files);
    let doc = read_document(&mut ws, &reader(), &request).unwrap();
    let texts: Vec<_> = doc.nodes().iter().filter(|n| n.kind == NodeKind::Text).collect();
    assert!(!texts.is_empty());
    for n in texts {
        assert_eq!(n.span.source, SourceId(1));
        assert_eq!(doc.source_text(n.span), "Inside text.");
    }
}

#[test]
fn included_sections_continue_the_hierarchy() {
    expect_included(
        "Top\n===\n\n.. include:: sub.rst\n\nNext text.\n",
        &[("sub.rst", "Sub\n---\n\nSub text.\n\nNext\n====\n")],
        concat!(
            "document\n",
            "  section value=1\n",
            "    heading value=1\n",
            "      text \"Top\"\n",
            "    section value=2\n",
            "      heading value=2\n",
            "        text \"Sub\"\n",
            "      paragraph\n",
            "        text \"Sub text.\"\n",
            "  section value=1\n",
            "    heading value=1\n",
            "      text \"Next\"\n",
            "    paragraph\n",
            "      text \"Next text.\"\n",
        ),
    );
}

#[test]
fn include_literal_and_code() {
    expect_included(
        concat!(
            ".. include:: code.py\n   :literal:\n   :tab-width: 4\n\n",
            ".. include:: code.py\n   :code: python\n   :number-lines: 10\n   :class: src\n",
        ),
        &[("code.py", "def f():\n\treturn 1\n\n")],
        concat!(
            "document\n",
            "  code-block \"def f():\\n    return 1\\n\"\n",
            "  code-block value=10 \"def f():\\n        return 1\\n\" @language=\"python\"",
            " @class=\"src\"\n",
        ),
    );
}

#[test]
fn include_line_and_text_ranges() {
    expect_included(
        concat!(
            ".. include:: doc.rst\n   :start-line: 2\n   :end-line: 5\n\n",
            ".. include:: doc.rst\n   :start-after: START\n   :end-before: END\n\n",
            ".. include:: doc.rst\n   :start-line: -1\n",
        ),
        &[("doc.rst", "zero\n\none\n\ntwo\n\nSTART\n\nkept\n\nEND\n\nlast\n")],
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"one\"\n",
            "  paragraph\n",
            "    text \"two\"\n",
            "  paragraph\n",
            "    text \"kept\"\n",
            "  paragraph\n",
            "    text \"last\"\n",
        ),
    );
}

#[test]
fn include_problems_are_reported() {
    let files = Files(vec![("a.rst", "text\n")]);
    let cases = [
        (".. include:: missing.rst\n", "core.include.missing"),
        (".. include:: refused.rst\n", "core.include.refused"),
        (".. include:: a.rst\n   :parser: markdown\n", "rst.directive.parser"),
        (".. include:: a.rst\n   :encoding: latin-1\n", "rst.directive.encoding"),
        (".. include:: a.rst\n   :start-after: nowhere\n", "rst.include.text-not-found"),
        (".. include::\n", "rst.directive.argument.missing"),
    ];
    for (text, code) in cases {
        let r = read_request(&include_request(text, &files));
        assert!(!r.ok, "{text}");
        expect_codes_in(&r.codes, code);
    }
}

#[test]
fn include_without_provider_or_policy_is_refused() {
    let r = read_text(".. include:: a.rst\n");
    assert_eq!(r.status, Status::Policy);
    expect_codes_in(&r.codes, "core.include.no-provider");
    let files = Files(vec![("a.rst", "text\n")]);
    let mut request = include_request(".. include:: a.rst\n", &files);
    request.policy.includes = false;
    let r = read_request(&request);
    assert_eq!(r.status, Status::Policy);
    expect_codes_in(&r.codes, "core.include.disabled");
}

#[test]
fn include_cycles_are_errors() {
    let files = Files(vec![
        ("a.rst", "A.\n\n.. include:: b.rst\n"),
        ("b.rst", "B.\n\n.. include:: a.rst\n"),
    ]);
    let mut request = include_request(".. include:: a.rst\n", &files);
    request.source.name = "main.rst";
    let r = read_request(&request);
    assert!(!r.ok);
    expect_codes_in(&r.codes, "core.include.cycle");
}

#[test]
fn include_depth_is_bounded() {
    let files = Files(vec![
        ("1.rst", ".. include:: 2.rst\n"),
        ("2.rst", ".. include:: 3.rst\n"),
        ("3.rst", ".. include:: 4.rst\n"),
        ("4.rst", "deep\n"),
    ]);
    let mut request = include_request(".. include:: 1.rst\n", &files);
    request.limits.max_include_depth = 2;
    let r = read_request(&request);
    assert_eq!(r.status, Status::Limit);
    expect_codes_in(&r.codes, "core.include.depth");
}

#[test]
fn include_inside_a_body_element() {
    expect_included(
        "- .. include:: item.rst\n",
        &[("item.rst", "Nested text.\n")],
        concat!(
            "document\n",
            "  list +tight list=bullet,-,0\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"Nested text.\"\n",
        ),
    );
}

#[test]
fn csv_table_and_raw_read_files() {
    expect_included(
        ".. csv-table::\n   :file: data.csv\n\n.. raw:: html\n   :file: x.html\n",
        &[("data.csv", "a, b\nc, d\n"), ("x.html", "<hr/>\n")],
        concat!(
            "document\n",
            "  table value=2 @widths=\"1 1\"\n",
            "    table-body\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"a\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"b\"\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"c\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"d\"\n",
            "  raw-block \"<hr/>\\n\" @format=\"html\"\n",
        ),
    );
}

#[test]
fn file_and_content_together_is_error() {
    let files = Files(vec![("x.html", "<hr/>\n")]);
    let r =
        read_request(&include_request(".. raw:: html\n   :file: x.html\n\n   <br/>\n", &files));
    expect_codes_in(&r.codes, "rst.directive.file-and-content");
}
