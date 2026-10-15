//! Helpers shared by the reader-dependent tests: read a document with a real reader,
//! translate it with a PO catalog, and look at the result.

#![allow(dead_code)] // each test file uses part of the helpers.

use gd_gettext::parse_po;
use gd_i18n::{
    ExtractOptions, Progress, Targets, Template, Translation, TranslationCounts,
    translate_document,
};
use guidedoc::{
    Document, ReadOptions, Reader, Request, Source, Workspace, default_request, dump_to_string,
    read_document, render_document, trusted_policy,
};

/// Returns the reader with this id.
pub fn reader(id: &str) -> Reader {
    match id {
        "rst" => guidedoc_rst::reader(),
        "commonmark" => guidedoc_commonmark::READER,
        "myst" => guidedoc_commonmark::MYST_READER,
        other => panic!("no reader {other}"),
    }
}

/// Reads `text` with reader `id` into `ws`.
pub fn read<'w>(
    ws: &'w mut Workspace,
    id: &str,
    text: &str,
    options: ReadOptions<'_>,
) -> Document<'w> {
    let request = Request {
        source: Source { name: "index.rst", text },
        read: options,
        policy: trusted_policy(),
        ..default_request("index.rst", text, id, "html")
    };
    match read_document(ws, &reader(id), &request) {
        Ok(doc) => doc,
        Err(fail) => panic!("read failed: {}", fail.diagnostic.formatted_message()),
    }
}

/// Extracts a document's msgids, in order.
pub fn messages(id: &str, text: &str, targets: Targets) -> Vec<String> {
    let mut ws = Workspace::new();
    let doc = read(&mut ws, id, text, ReadOptions::default());
    let mut pot = Template::new("x");
    pot.extract(
        doc,
        &ExtractOptions { targets, location: true, path: "index.rst", ..Default::default() },
    );
    pot.message_ids().map(str::to_owned).collect()
}

/// What translating a document gave.
#[derive(Debug)]
pub struct Translated {
    pub html: String,
    pub tree: String,
    pub codes: Vec<&'static str>,
    pub counts: TranslationCounts,
}

/// The settings of one translation test.
#[derive(Debug, Clone, Copy, Default)]
pub struct Case<'a> {
    pub reader: &'a str,
    pub text: &'a str,
    pub po: &'a str,
    pub targets: Targets,
    pub progress: Progress,
}

/// Reads a document, translates it with a PO catalog, and renders it as HTML and as a
/// tree dump; also returns the codes of the diagnostics the translation reported.
pub fn translate(case: Case<'_>) -> Translated {
    let mut ws = Workspace::new();
    read(&mut ws, case.reader, case.text, ReadOptions::default());
    let catalog = parse_po(case.po, "de.po").unwrap_or_else(|e| panic!("catalog: {e}"));
    let before = ws.reports().items().len();
    let translation = Translation {
        catalog: Some(&catalog),
        read_inline: reader(case.reader).read_inline,
        policy: trusted_policy(),
        targets: case.targets,
        progress: case.progress,
        source_name: "de.po",
        ..Translation::default()
    };
    let (doc, counts) = match translate_document(&mut ws, translation) {
        Ok(result) => result,
        Err(fail) => panic!("pass failed: {}", fail.diagnostic.formatted_message()),
    };
    let tree = dump_to_string(doc);
    let codes = ws.reports().items()[before..].iter().map(|d| d.code).collect();
    let request =
        Request { policy: trusted_policy(), ..default_request("index.rst", "", "", "html") };
    let html = match render_document(&mut ws, &guidedoc_html::renderer(), &request) {
        Ok(html) => html.to_owned(),
        Err(fail) => panic!("render failed: {}", fail.diagnostic.formatted_message()),
    };
    Translated { html, tree, codes, counts }
}

#[track_caller]
pub fn has(haystack: &str, needle: &str) {
    assert!(haystack.contains(needle), "missing {needle:?} in:\n{haystack}");
}

#[track_caller]
pub fn lacks(haystack: &str, needle: &str) {
    assert!(!haystack.contains(needle), "unexpected {needle:?} in:\n{haystack}");
}
