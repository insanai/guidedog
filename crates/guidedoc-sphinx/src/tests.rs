//! Test helpers: read reStructuredText (or MyST Markdown) with the Sphinx layer and look
//! at the dump and the report codes. Every module's tests use these.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::field_reassign_with_default)]

mod code_blocks;
mod cpp_nesting;
mod directives;
mod docfields;
mod domains;
mod glossary;
mod markdown;
mod python;
mod reader;
mod roles;
mod toc;

use std::sync::Arc;

use guidedoc::{
    Request, Resource, ResourceProvider, ResourceStatus, Workspace, default_request,
    dump_to_string, read_document, read_extension, trusted_policy,
};

use crate::config::Config;

/// Files served to literalinclude, graphviz, and include in tests.
pub(crate) const TEST_FILES: &[(&str, &str)] = &[
    (
        "example.py",
        "import os\n\n\n@decorator\ndef hello(name):\n    \"\"\"Greet.\"\"\"\n    \
         print(name)\n\n    # done\n\nclass Greeter:\n    def greet(self):\n        \
         return 1\n\n    def other(self):\n        pass\n",
    ),
    ("old.txt", "a\nb\nc\nd\n"),
    ("new.txt", "a\nB\nc\nd\ne\n"),
    ("lines.txt", "one\ntwo\nthree\nfour\nfive\n"),
    ("tabs.txt", "\tx\n"),
    ("part.md", "---\norphan: true\n---\n## Part\n\nIncluded *text*.\n"),
    ("sub/inner.md", "![i](pic.png) [up](../top.md)\n"),
];

/// Serves `TEST_FILES`.
#[derive(Debug)]
pub(crate) struct TestFiles;

impl ResourceProvider for TestFiles {
    fn provide(&self, _from: &str, name: &str) -> Result<Resource, ResourceStatus> {
        let (n, text) =
            TEST_FILES.iter().find(|(n, _)| *n == name).ok_or(ResourceStatus::Missing)?;
        Ok(Resource { text: Arc::from(*text), canonical: (*n).to_owned() })
    }
}

/// The outcome of a test read: the dump (empty when the read failed), the report codes
/// separated by spaces, and whether the read succeeded.
#[derive(Debug, Default)]
pub(crate) struct Read {
    pub dump: String,
    pub codes: String,
    pub ok: bool,
}

/// Reads `text` as "test.rst" with the Sphinx reader, `config` (or the defaults),
/// titles not promoted, trusted, with `TEST_FILES`.
pub(crate) fn read_text(text: &str, config: Option<&Config>) -> Read {
    let mut request = default_request("test.rst", text, "sphinx", "html");
    request.read.promote_title = false;
    request.read.extension = config.map(read_extension);
    request.policy = trusted_policy();
    request.resources = Some(&TestFiles);
    read_request(&request, &crate::READER)
}

/// Reads a request with `reader`.
pub(crate) fn read_request(request: &Request<'_>, reader: &guidedoc::Reader) -> Read {
    let mut ws = Workspace::new();
    let result = read_document(&mut ws, reader, request).map(dump_to_string);
    let codes: Vec<&str> = ws.reports().items().iter().map(|d| d.code).collect();
    let codes = codes.join(" ");
    match result {
        Ok(dump) => Read { dump, codes, ok: true },
        Err(_) => Read { dump: String::new(), codes, ok: false },
    }
}

/// Checks the whole dump.
#[track_caller]
pub(crate) fn expect_dump(text: &str, want: &str, config: Option<&Config>) {
    let r = read_text(text, config);
    assert!(r.ok, "read failed: {}", r.codes);
    assert_eq!(r.dump, want, "dump differs; got:\n{}", r.dump);
}

/// Checks that the dump holds every fragment, in order.
#[track_caller]
pub(crate) fn expect_contains(text: &str, fragments: &[&str], config: Option<&Config>) {
    let r = read_text(text, config);
    assert!(r.ok, "read failed: {}", r.codes);
    contains_in_order(&r.dump, fragments);
}

/// Checks that `got` holds every fragment, in order.
#[track_caller]
pub(crate) fn contains_in_order(got: &str, fragments: &[&str]) {
    let mut at = 0;
    for f in fragments {
        match got[at..].find(f) {
            Some(k) => at += k + f.len(),
            None => panic!("missing {f:?}; got:\n{got}"),
        }
    }
}

/// Checks that every code of `want` (separated by spaces) was reported.
#[track_caller]
pub(crate) fn expect_codes(text: &str, want: &str, config: Option<&Config>) {
    let r = read_text(text, config);
    for code in want.split(' ') {
        assert!(r.codes.contains(code), "missing report {code}; got: {}", r.codes);
    }
}

/// Reads Markdown with the Sphinx layer, as a project reads .md documents: "doc.md",
/// docname "doc", `myst` settings.
pub(crate) fn md_read(text: &str, myst: guidedoc_commonmark::Settings) -> Read {
    let config = Config { docname: "doc".into(), ..Config::default() };
    let settings = crate::myst_settings(config, myst);
    let mut request = default_request("doc.md", text, "myst", "html");
    request.read.promote_title = false;
    request.read.extension = Some(read_extension(&settings));
    request.policy = trusted_policy();
    request.resources = Some(&TestFiles);
    read_request(&request, &guidedoc_commonmark::MYST_READER)
}

/// Checks that the dump of Markdown (default MyST settings) holds every fragment, in
/// order.
#[track_caller]
pub(crate) fn md_expect(text: &str, fragments: &[&str]) {
    md_expect_with(text, fragments, guidedoc_commonmark::Settings::default());
}

/// `md_expect` with MyST settings.
#[track_caller]
pub(crate) fn md_expect_with(text: &str, fragments: &[&str], myst: guidedoc_commonmark::Settings) {
    let r = md_read(text, myst);
    assert!(r.ok, "read failed: {}", r.codes);
    contains_in_order(&r.dump, fragments);
}
