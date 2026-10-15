//! `guidedog dsc` as users run it: from inside a catalog, building it, and following a
//! catalog's configuration.

use std::path::Path;

use super::Legacy;
use guidedog_tests::{Run, Scratch, has, run_program, write};

/// Runs `guidedog dsc args` in `dir` and expects `status`.
fn dsc(dir: &Path, status: i32, args: &[&str]) -> Run {
    let mut all = vec!["dsc"];
    all.extend_from_slice(args);
    run_program("guidedog", dir, &all).expect(status)
}

/// guidedog dsc makes, keeps and builds a catalog of any prefix.
#[test]
fn dsc_makes_keeps_and_builds_a_catalog() {
    let root = Scratch::new("dsc");
    dsc(
        &root.path,
        0,
        &["init", "eds", "--prefix=eds", "--name=Enso Discussions", "--author=Ada"],
    );
    let dir = root.join("eds");
    dsc(&dir, 0, &["new", "Cache policy", "--labels=architecture"]);
    dsc(&dir, 0, &["promote", "cache-policy"]);
    dsc(&dir, 0, &["state", "1", "--to=accepted", "--reason=Reviewed"]);
    has(&dsc(&dir, 0, &["show", "1", "--format=json"]).out, "accepted");
    dsc(&dir, 0, &["check"]);
    // A folder in the catalog finds it; --prefix must agree with its definition.
    dsc(&dir.join("records"), 0, &["list"]);
    dsc(&dir, 2, &["list", "--prefix=sds"]);
    dsc(&root.path, 2, &["init", "eds", "--prefix=eds", "--author=Ada"]);
    dsc(&dir, 0, &["build", "--target=html"]);
    assert!(root.exists("eds/_build/html/records/0001-cache-policy.html"));
    // guidedog builds the catalog itself, so no other guidedog can be named.
    dsc(&dir, 2, &["build", "--target=html", "--guidedog=/no/such/guidedog"]);
    // Options that belong to another command are refused, with the command's help.
    has(&dsc(&dir, 2, &["list", "--reason=x"]).err, "guidedog dsc list --help");
}

/// A catalog without a definition, such as Guidedog's own, is named by its template.
#[test]
fn dsc_manages_a_catalog_named_by_its_template() {
    let c = Legacy::new("dsc-template");
    let dir = c.dir.to_str().unwrap();
    has(&dsc(&c.scratch.path, 0, &["list", "--dir", dir]).out, "GDS");
    dsc(&c.scratch.path, 0, &["new", "Cache policy", "--dir", dir, "--author=Ada"]);
    assert!(c.path("records/XXXXX-cache-policy.typ").is_file());
    c.dsc(0, &["check"]);
}

const ADR_CONFIG: &str = r#"prefix = "adr"
name = "Architecture decisions"
author = "Ada"
types = ["Decision"]

[numbering]
width = 2
start = 99

[workflow]
draft = "draft"
open = "review"

[workflow.states.draft]
next = ["withdrawn"]

[workflow.states.review]
next = ["approved", "withdrawn"]
evidence = true

[workflow.states.approved]
evidence = true

[workflow.states.withdrawn]
"#;

/// A catalog's configuration sets its workflow and numbering; `dsc config` prints it, and
/// `init` starts from one.
#[test]
fn dsc_follows_the_catalogs_configuration() {
    let root = Scratch::new("dsc-config");
    write(&root.join("adr.toml"), ADR_CONFIG);
    dsc(&root.path, 0, &["init", "adr", "--config=adr.toml"]);
    let dir = root.join("adr");
    let r = dsc(&dir, 0, &["config"]);
    has(&r.out, "open = \"review\"");
    has(&r.out, "width = 2");
    dsc(&dir, 0, &["new", "First", "--labels=decision"]);
    dsc(&dir, 0, &["new", "Second", "--labels=decision"]);
    has(&dsc(&dir, 2, &["promote", "first"]).err, "evidence");
    dsc(&dir, 0, &["promote", "first", "--reason=Reviewed", "--evidence=Review notes"]);
    dsc(&dir, 0, &["promote", "second", "--evidence=Review notes"]);
    assert!(root.exists("adr/records/100-second.rst"));
    has(&dsc(&dir, 0, &["list", "--format=json"]).out, "\"number\":\"99\"");
    dsc(&dir, 2, &["state", "100", "--to=approved", "--reason=Agreed"]);
    dsc(&dir, 0, &["state", "100", "--to=approved", "--reason=Agreed", "--evidence=Built"]);
    has(&dsc(&dir, 2, &["state", "99", "--to=draft", "--reason=Back"]).err, "approved, withdrawn");
    dsc(&dir, 2, &["list", "--state=accepted"]);
    dsc(&dir, 0, &["check"]);
}
