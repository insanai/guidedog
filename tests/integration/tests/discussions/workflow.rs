//! The lifecycle on Guidedog's catalog: drafts, promotion, decisions, the index, and the
//! command line's refusals.

use super::{Legacy, exists, fixture};
use gd_discussion as ds;
use guidedog_tests::{has, lacks, write};

#[test]
fn new_promote_and_state() {
    let c = Legacy::new("gds-lifecycle");
    c.dsc(0, &["new", "Cache policy", "--author=Ada", "--labels=architecture,build"]);
    c.dsc(0, &["check"]);
    let r = c.dsc(0, &["show", "Cache policy", "--format=json"]);
    has(&r.out, "prediscussion");
    c.dsc(0, &["promote", "cache-policy", "--dry-run"]);
    assert!(exists(&c.path("records/XXXXX-cache-policy.typ")));
    c.dsc(0, &["promote", "cache-policy"]);
    assert!(!exists(&c.path("records/XXXXX-cache-policy.typ")));
    has(&c.dsc(0, &["show", "2", "--format=json"]).out, "0002");
    // A decision follows the lifecycle: committed needs accepted first, and evidence.
    c.dsc(2, &["state", "2", "--to=committed", "--reason=Done"]);
    c.dsc(0, &["state", "2", "--to=accepted", "--reason=Reviewed"]);
    c.dsc(2, &["state", "2", "--to=committed", "--reason=Done"]);
    c.dsc(0, &["state", "2", "--to=committed", "--reason=Done", "--evidence=tests"]);
    c.dsc(2, &["state", "2", "--to=abandoned", "--reason=Changed"]);
    c.dsc(0, &["check"]);
}

#[test]
fn preview_and_index_preserve_data() {
    let c = Legacy::new("gds-preview");
    let registry = c.path("registry.typ");
    let original = c.read("registry.typ");
    c.dsc(0, &["new", "Preview", "--author=Ada", "--dry-run"]);
    assert!(!exists(&c.path("records/XXXXX-preview.typ")));
    assert_eq!(c.read("registry.typ"), original);
    // Prose around the generated region is the user's, and indexing is idempotent.
    write(&registry, &format!("// Hand-written introduction\n{original}// Keep this footer\n"));
    c.dsc(0, &["index"]);
    let saved = c.read("registry.typ");
    c.dsc(0, &["index"]);
    assert_eq!(c.read("registry.typ"), saved);
    has(&saved, "Hand-written introduction");
    has(&saved, "Keep this footer");
    c.dsc(0, &["index", "--check"]);
}

#[test]
fn invalid_input_has_directions_and_no_mutation() {
    let c = Legacy::new("gds-invalid");
    let original = c.read("registry.typ");
    let r = c.dsc(2, &["promtoe", "--diagnostics=json"]);
    let report = gd_json::parse(&r.err).unwrap_or_else(|e| panic!("{e}: {}", r.err));
    let hint = report.get("hint").and_then(gd_json::Value::as_str).unwrap_or_default();
    assert!(!hint.is_empty(), "{}", r.err);
    c.dsc(2, &["list", "--author=Ada"]);
    c.dsc(2, &["new", "Bad", "--author=Ada", "--slug=../escape"]);
    c.dsc(2, &["new", "Bad", "--author=Ada", "--labels=unknown"]);
    c.dsc(2, &["build", "1", "--record=gds-process"]);
    assert_eq!(c.read("registry.typ"), original);
}

#[test]
fn metadata_is_never_evaluated() {
    let c = Legacy::new("gds-literal");
    // Written as a Typst expression by hand: replace_field writes only literals.
    let title = "#let gds-title = \"The Guidedog Discussion Process\"";
    let process = fixture("process.typ");
    let source = process.replace(title, "#let gds-title = read(\"/etc/passwd\")");
    assert_ne!(source, process);
    write(&c.path("records/0001-gds-process.typ"), &source);
    let r = c.dsc(1, &["list"]);
    has(&r.err, "not a literal");
    has(&r.err, "Hint:");
}

#[test]
fn quoted_titles_remain_data() {
    let c = Legacy::new("gds-quoted");
    let title = "Quoted \"title\" #panic(\"no\")";
    c.dsc(0, &["new", title, "--slug=quoted", "--author=Ada"]);
    has(&c.dsc(0, &["show", title, "--format=json"]).out, "Quoted");
    c.dsc(0, &["check"]);
}

#[test]
fn selectors_reject_ambiguity() {
    let c = Legacy::new("gds-ambiguous");
    c.dsc(0, &["new", "Same title", "--slug=first", "--author=Ada"]);
    c.dsc(0, &["new", "Same title", "--slug=second", "--author=Ada"]);
    let r = c.dsc(2, &["show", "Same title"]);
    has(&r.err, "first");
    has(&r.err, "second");
    c.dsc(0, &["show", "XXXXX-first.typ"]);
}

#[test]
fn color_and_json_are_separate() {
    let c = Legacy::new("gds-color");
    lacks(&c.dsc(0, &["list"]).out, "\x1b");
    has(&c.dsc(0, &["list", "--color=always"]).out, "\x1b[");
    let r = c.dsc(0, &["list", "--format=json", "--color=always"]);
    lacks(&r.out, "\x1b");
    let value = gd_json::parse(&r.out).unwrap_or_else(|e| panic!("{e}: {}", r.out));
    assert!(value.as_array().is_some_and(|records| !records.is_empty()), "{}", r.out);
}

#[test]
fn numbering_uses_guidedog_history() {
    let c = Legacy::new("gds-numbers");
    let mut catalog = ds::load_catalog(&c.dir, &super::gds()).unwrap();
    // A historical record's number does not hold the sequence; the reservation does.
    catalog.records.push(ds::Record {
        number: "0016".into(),
        historical: true,
        ..ds::Record::default()
    });
    assert_eq!(ds::next_number(&catalog, None).unwrap(), 2);
    catalog.reserved = 8;
    assert_eq!(ds::next_number(&catalog, None).unwrap(), 9);
    c.dsc(0, &["new", "First proposal", "--author=Ada"]);
    c.dsc(0, &["promote", "first-proposal"]);
    c.dsc(0, &["new", "Second proposal", "--author=Ada"]);
    c.dsc(0, &["promote", "second-proposal"]);
    has(&c.dsc(0, &["show", "3", "--format=json"]).out, "Second proposal");
}
