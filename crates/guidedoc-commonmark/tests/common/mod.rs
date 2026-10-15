//! Helpers the integration tests share: requests, the registry, and tree dumps.

#![allow(dead_code)]

use guidedoc::{
    Registry, Request, Workspace, default_request, dump_to_string, read_document, read_extension,
    trusted_policy,
};
use guidedoc_commonmark::{MYST_READER, READER, Settings};

/// The registry of the tests: both readers and the HTML renderer.
pub fn registry() -> Registry {
    Registry { readers: vec![READER, MYST_READER], renderers: vec![guidedoc_html::RENDERER] }
}

/// A request for CommonMark's reference HTML.
pub fn reference_request(markdown: &str) -> Request<'_> {
    let mut r = default_request("example.md", markdown, "commonmark", "html");
    r.render.reference_profile = true;
    r.render.standalone = false;
    r.policy = trusted_policy();
    r
}

/// The tree the "commonmark" reader builds, or the failure's code.
pub fn dump(markdown: &str) -> String {
    let mut ws = Workspace::new();
    let request = reference_request(markdown);
    match read_document(&mut ws, &READER, &request) {
        Ok(doc) => dump_to_string(doc),
        Err(fail) => fail.code().to_owned(),
    }
}

/// The tree the "myst" reader builds with `settings`, or the failure's code.
pub fn myst_dump(markdown: &str, settings: &Settings) -> String {
    let mut ws = Workspace::new();
    let mut request = default_request("doc.md", markdown, "myst", "html");
    request.policy = trusted_policy();
    request.read.extension = Some(read_extension(settings));
    match read_document(&mut ws, &MYST_READER, &request) {
        Ok(doc) => dump_to_string(doc),
        Err(fail) => fail.code().to_owned(),
    }
}
