//! `dsc` and `dscc` as users run them: from inside a catalog, building with an installed
//! guidedog (or the one linked in), and following a catalog's configuration.

use std::path::Path;

use super::Legacy;
use guidedog_tests::{Run, Scratch, has, program, run_program, write};

/// Runs `dsc args` in `dir` and expects `status`.
fn dsc(dir: &Path, status: i32, args: &[&str]) -> Run {
    run_program("dsc", dir, args).expect(status)
}

#[test]
fn dsc_makes_and_keeps_a_catalog_and_builds_it_with_guidedog() {
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
    let guidedog = format!("--guidedog={}", program("guidedog").display());
    dsc(&dir, 0, &["build", "--target=html", &guidedog]);
    assert!(root.exists("eds/_build/html/records/0001-cache-policy.html"));
    let r = dsc(&dir, 4, &["build", "--target=html", "--guidedog=/no/such/guidedog"]);
    has(&r.err, "eds.engine-missing");
    has(&r.err, "dscc");
}

/// dscc is dsc with Guidedog linked in: it builds without an installed guidedog.
#[test]
fn dscc_builds_with_the_linked_guidedog() {
    let root = Scratch::new("dscc");
    let dscc =
        |dir: &Path, status: i32, args: &[&str]| run_program("dscc", dir, args).expect(status);
    dscc(&root.path, 0, &["init", "eds", "--prefix=eds", "--author=Ada"]);
    let dir = root.join("eds");
    dscc(&dir, 0, &["new", "Cache policy", "--labels=architecture"]);
    dscc(&dir, 0, &["promote", "cache-policy"]);
    dscc(&dir, 0, &["build", "--target=html"]);
    assert!(root.exists("eds/_build/html/records/0001-cache-policy.html"));
}

/// A catalog without a definition, such as Guidedog's own, is named by its template.
#[test]
fn dsc_manages_a_catalog_named_by_its_template() {
    let c = Legacy::new("dsc-template");
    let dir = c.dir.to_str().unwrap();
    has(&dsc(&c.scratch.path, 0, &["list", "--dir", dir]).out, "GDS");
    dsc(&c.scratch.path, 0, &["new", "Cache policy", "--dir", dir, "--author=Ada"]);
    assert!(c.path("records/XXXXX-cache-policy.typ").is_file());
    c.gds(0, &["check"]);
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
