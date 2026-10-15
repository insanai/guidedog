#![allow(clippy::unwrap_used)]

//! Helpers of the reader's tests: read a text and compare the dump of its tree and the
//! codes of its reports.

#![allow(dead_code)]

use guidedoc::{
    Request, Status, Workspace, default_request, dump_to_string, read_document, read_extension,
    trusted_policy,
};
use guidedoc_rst::{Settings, SmartQuotes, reader};

/// What a read gave: the dump (empty unless it succeeded), the report codes
/// space-separated, and whether it succeeded.
pub struct Read {
    pub dump: String,
    pub codes: String,
    pub ok: bool,
    pub status: Status,
}

/// A request with title promotion off, so dumps show the block structure, and a date
/// for the date directive.
pub fn request_for(text: &str) -> Request<'_> {
    let mut request = default_request("test.rst", text, "rst", "html");
    request.read.promote_title = false;
    request.read.today = "2026-09-29";
    request.policy = trusted_policy();
    request
}

/// Reads with `request`.
pub fn read_request(request: &Request<'_>) -> Read {
    let mut ws = Workspace::new();
    let (dump, status) = match read_document(&mut ws, &reader(), request) {
        Ok(doc) => (dump_to_string(doc), Status::Ok),
        Err(fail) => (String::new(), fail.status),
    };
    let ok = status == Status::Ok;
    let codes: Vec<&str> = ws.reports().items().iter().map(|d| d.code).collect();
    Read { dump, codes: codes.join(" "), ok, status }
}

/// Reads `text` with the default settings.
pub fn read_text(text: &str) -> Read {
    read_request(&request_for(text))
}

/// Reads `text` with `settings`, title promotion as `promote` says.
pub fn read_settings(text: &str, settings: &Settings, promote: bool) -> Read {
    let mut request = request_for(text);
    request.read.promote_title = promote;
    request.read.extension = Some(read_extension(settings));
    read_request(&request)
}

#[track_caller]
pub fn expect_dump(text: &str, want: &str) {
    let r = read_text(text);
    assert!(r.ok, "read failed: {}", r.codes);
    assert_eq!(r.dump, want, "dump differs; got:\n{}", r.dump);
}

#[track_caller]
pub fn expect_dump_codes(text: &str, want: &str, codes: &str) {
    let r = read_text(text);
    assert!(r.ok, "read failed: {}", r.codes);
    assert_eq!(r.dump, want, "dump differs; got:\n{}", r.dump);
    expect_codes_in(&r.codes, codes);
}

/// Checks that reading `text` reports every code in `want` (space-separated).
#[track_caller]
pub fn expect_codes(text: &str, want: &str) {
    let r = read_text(text);
    expect_codes_in(&r.codes, want);
}

#[track_caller]
pub fn expect_codes_in(codes: &str, want: &str) {
    for code in want.split(' ').filter(|c| !c.is_empty()) {
        assert!(codes.split(' ').any(|c| c == code), "missing report {code}; got: {codes}");
    }
}

/// Checks a dump read without bibliographic fields, for tests of field lists.
#[track_caller]
pub fn expect_dump_plain(text: &str, want: &str) {
    let settings = Settings { docinfo: false, ..Settings::default() };
    let r = read_settings(text, &settings, false);
    assert_eq!(r.dump, want, "dump differs ({}); got:\n{}", r.codes, r.dump);
}

#[track_caller]
pub fn expect_dump_with(text: &str, settings: &Settings, want: &str, promote: bool) {
    let r = read_settings(text, settings, promote);
    assert_eq!(r.dump, want, "dump differs ({}); got:\n{}", r.codes, r.dump);
}

/// The reader settings the settings-behaviour tests cover.
pub fn every_setting() -> Settings {
    Settings {
        pep_references: true,
        rfc_references: true,
        trim_footnote_reference_space: true,
        smart_quotes: SmartQuotes::On,
        sectsubtitle: true,
        ..Settings::default()
    }
}
