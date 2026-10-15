//! Configured processes: reading `discussion.toml`, and following its workflow and
//! numbering.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

use super::{TempDir, catalog_fixture, prefix, write};
use crate::*;
use std::path::{Path, PathBuf};

#[test]
fn the_default_configuration_reads_back_as_itself() {
    let d = Definition {
        prefix: prefix("eds"),
        name: "Enso Discussions".into(),
        author: "Ada".into(),
        ..Definition::default()
    };
    let text = render_config(&d);
    let back = read_config(CONFIG_FILE, &text).unwrap_or_else(|p| panic!("{p}"));
    assert_eq!(back, d);
    let custom = read_config(CONFIG_FILE, CUSTOM).unwrap();
    assert_eq!(read_config(CONFIG_FILE, &render_config(&custom)).unwrap(), custom);
}

#[test]
fn configuration_mistakes_name_the_line_and_the_fix() {
    let base = "prefix = \"eds\"\nname = \"Enso\"\n";
    let cases = [
        (format!("{base}colour = \"blue\"\n"), 3, "no setting colour"),
        (format!("{base}[numbering]\nwidth = \"four\"\n"), 4, "whole number"),
        (format!("{base}[numbering]\nwidth = 0\n"), 4, "1 to 9 digits"),
        ("prefix = \"eds\"\nname = \n".to_string(), 2, ""),
        (
            format!("{base}{WORKFLOW}[workflow.states.review]\nnext = [\"nowhere\"]\n"),
            0,
            "not another state",
        ),
        (
            format!(
                "{base}[workflow]\ndraft = \"draft\"\nopen = \"review\"\n\
            [workflow.states.draft]\nnext = [\"review\"]\n[workflow.states.review]\n"
            ),
            0,
            "the open state",
        ),
        ("name = \"Enso\"\n".to_string(), 0, "prefix is missing"),
        ("prefix = \"../x\"\nname = \"Enso\"\n".to_string(), 1, "is not a prefix"),
    ];
    for (text, line, says) in cases {
        let p = read_config(CONFIG_FILE, &text).unwrap_err();
        assert_eq!(p.code, "discussion.config", "accepted {text:?}");
        if line > 0 {
            assert_eq!(p.line, line, "{text:?}: {}", p.message);
            assert!(p.excerpt.is_some(), "{text:?} has no excerpt");
        }
        assert!(p.message.contains(says), "{text:?}: {}", p.message);
        assert!(!p.hint.is_empty());
    }
    let p = read_config(CONFIG_FILE, &format!("{base}[numbering]\nwidht = 3\n")).unwrap_err();
    assert_eq!(p.suggestion.as_deref(), Some("width"));
}

#[test]
fn an_explicit_empty_type_list_is_not_the_default() {
    let p = read_config(CONFIG_FILE, "prefix = \"eds\"\nname = \"Enso\"\ntypes = []\n");
    let p = p.unwrap_err();
    assert_eq!(p.code, "discussion.config");
    assert!(p.message.contains("types is empty"));
}

const WORKFLOW: &str = "[workflow]
draft = \"draft\"
open = \"review\"

[workflow.states.draft]
next = [\"withdrawn\"]
status = \"Being written\"

[workflow.states.withdrawn]

[workflow.states.approved]
evidence = true
status = \"Approved\"

";

const CUSTOM: &str = concat!(
    "prefix = \"adr\"
name = \"Architecture decisions\"
author = \"Ada\"
types = [\"Decision\"]

[numbering]
width = 3
start = 998
limit = 1001

",
    "[workflow]
draft = \"draft\"
open = \"review\"

[workflow.states.draft]
next = [\"withdrawn\"]
status = \"Being written\"

[workflow.states.withdrawn]

[workflow.states.approved]
evidence = true
status = \"Approved\"

",
    "[workflow.states.review]
next = [\"approved\", \"withdrawn\"]
status = \"In review\"
"
);

/// A catalog with the configuration `config`, in a new temporary folder.
fn make_catalog(name: &str, config: &str) -> (TempDir, PathBuf) {
    let root = TempDir::new(name);
    let d = read_config(CONFIG_FILE, config).unwrap_or_else(|p| panic!("{p}"));
    let dir = root.0.join("adr");
    let init = Init {
        dir: dir.clone(),
        prefix: d.prefix,
        name: d.name,
        author: d.author,
        process: d.process,
        ..Init::default()
    };
    initialize(&init).unwrap_or_else(|p| panic!("{p}"));
    (root, dir)
}

enum Step<'a> {
    New(&'a str),
    Promote(Decision),
    State(Decision),
}

/// Loads, plans, and applies one step under the lock, as a command does.
fn run(dir: &Path, step: Step<'_>) -> Result<()> {
    let p = prefix("adr");
    let lock = lock_catalog(dir, &p)?;
    let c = load_catalog(dir, &p)?;
    let plan = match step {
        Step::New(title) => new_plan(
            &c,
            &NewDraft {
                title: title.into(),
                author: "Ada".into(),
                labels: vec!["process".into()],
                ..NewDraft::default()
            },
        )?,
        Step::Promote(d) => promote_plan(&c, &d)?,
        Step::State(d) => state_plan(&c, &d)?,
    };
    apply(&lock, &plan)
}

fn promote(id: &str) -> Step<'_> {
    Step::Promote(Decision { identifier: id.into(), ..Decision::default() })
}

fn move_to(id: &str, to: &str, reason: &str, evidence: &str) -> Step<'static> {
    Step::State(Decision {
        identifier: id.into(),
        to: to.into(),
        reason: reason.into(),
        evidence: evidence.into(),
        ..Decision::default()
    })
}

#[test]
fn a_configured_workflow_and_numbering_are_followed() {
    let (_root, dir) = make_catalog("workflow", CUSTOM);
    for title in ["Use Rust", "Keep TOML", "Split catalogs", "One more"] {
        run(&dir, Step::New(title)).unwrap_or_else(|p| panic!("{p}"));
    }
    for slug in ["use-rust", "keep-toml", "split-catalogs"] {
        run(&dir, promote(slug)).unwrap_or_else(|p| panic!("{slug}: {p}"));
    }
    // start 998 and width 3, past the width; limit 1001 refuses a fifth number.
    let c = load_catalog(&dir, &prefix("adr")).unwrap();
    let numbers: Vec<&str> = c.records.iter().map(|r| r.number.as_str()).collect();
    assert_eq!(numbers.join(" "), "998 999 1000 XXXXX XXXXX");
    run(&dir, promote("one-more")).expect("1001");
    assert_eq!(run(&dir, promote("adr-process")).unwrap_err().code, "adr.number-taken");
    assert_eq!(resolve(&c, "1000").unwrap().slug, "split-catalogs");
    // Moves follow next; evidence follows the target state.
    let p = run(&dir, move_to("998", "draft", "x", "")).unwrap_err();
    assert_eq!(p.code, "adr.transition");
    assert!(p.message.contains("approved, withdrawn"));
    let p = run(&dir, move_to("998", "approved", "ok", "")).unwrap_err();
    assert_eq!(p.code, "adr.reason");
    run(&dir, move_to("998", "approved", "ok", "tests")).expect("approved");
    let c = load_catalog(&dir, &prefix("adr")).unwrap();
    assert_eq!(c.records[0].state, "approved");
    assert_eq!(c.records[0].kind, "Decision");
    let text = std::fs::read_to_string(&c.records[0].path).unwrap();
    assert!(text.contains(": review -> approved. ok tests\n"), "{text}");
    // Older catalogs wrote the event with a Unicode arrow; they still load and check.
    std::fs::write(&c.records[0].path, text.replace(" -> ", " \u{2192} ")).unwrap();
    let c = load_catalog(&dir, &prefix("adr")).unwrap();
    assert_eq!(c.records[0].state, "approved");
    check_sources(&c).expect("an old lifecycle event is still a valid record");
}

#[test]
fn numbers_grow_past_their_width_and_sort_by_value() {
    let gds = Process::default();
    assert_eq!(gds.format_number(7), "0007");
    assert_eq!(gds.format_number(10000), "10000");
    for good in ["0001", "9999", "10000", "123456789"] {
        assert!(gds.number_value(good).is_some(), "{good} refused");
    }
    for bad in ["1", "007", "00010", "010000", "0000", "1234567890", "XXXXX"] {
        assert!(gds.number_value(bad).is_none(), "{bad} accepted");
    }
    let record = |number: &str, slug: &str| Record {
        number: number.into(),
        slug: slug.into(),
        ..Record::default()
    };
    let mut records =
        [record("XXXXX", "a"), record("10000", "b"), record("9999", "c"), record("0002", "d")];
    catalog::sort_by_number(&mut records);
    let order: String = records.iter().map(|r| r.slug.as_str()).collect();
    assert_eq!(order, "dcba");
}

/// A 0.2.1 catalog, identified by `.discussion.json`, follows the default process.
#[test]
fn an_older_definition_still_names_its_catalog() {
    let (_root, dir) = catalog_fixture("legacy");
    std::fs::remove_file(dir.join(CONFIG_FILE)).unwrap();
    write(
        &dir.join(DEFINITION_FILE),
        "{\"schema\": 1, \"prefix\": \"eds\", \"name\": \"Enso\", \"author\": \"Ada\"}\n",
    );
    let d = definition(&dir).unwrap().expect("a definition");
    assert_eq!(d.name, "Enso");
    assert_eq!(d.process.open, "discussion");
    assert_eq!(load_catalog(&dir, &prefix("eds")).unwrap().records.len(), 1);
    write(&dir.join(DEFINITION_FILE), "{\"schema\": 2, \"prefix\": \"eds\"}");
    assert_eq!(definition(&dir).unwrap_err().code, "discussion.definition");
}

/// A catalog keeps records' metadata, never their text: loading reads each large record
/// only up to its sections.
#[test]
fn loading_a_catalog_holds_metadata_not_text() {
    let (_root, dir) = catalog_fixture("large");
    let body = "A long paragraph of the record. ".repeat(32 * 1024); // 1 MiB.
    for i in 1..=40 {
        let source = format!(
            ":eds-number: {i:04}\n:eds-title: Record {i}\n\
            :eds-state: discussion\n:eds-type: Standards Track\n:eds-authors: [\"Ada\"]\n\
            :eds-created: 2026-10-12\n:eds-updated: 2026-10-12\n:eds-discussion: Here\n\
            :eds-labels: [\"process\"]\n\nRecord\n======\n\nAbstract\n--------\n\n{body}\n\n\
            References\n----------\n\nNone.\n"
        );
        write(&dir.join(format!("records/{i:04}-record-{i}.rst")), &source);
    }
    let c = load_catalog(&dir, &prefix("eds")).unwrap_or_else(|p| panic!("{p}"));
    assert_eq!(c.records.len(), 41);
    check_sources(&c).expect("every record has its sections");
    let metadata: usize = c.records.iter().map(|r| r.title.len() + r.discussion.len()).sum();
    assert!(metadata < 2048);
}

#[test]
fn promotion_honours_the_open_states_evidence_requirement() {
    let config = CUSTOM
        .replace("[workflow.states.review]\n", "[workflow.states.review]\nevidence = true\n");
    let (_root, dir) = make_catalog("evidence", &config);
    let source = std::fs::read_to_string(dir.join("records/XXXXX-adr-process.rst")).unwrap();
    assert!(source.contains("promote, evidence"));
    assert_eq!(run(&dir, promote("adr-process")).unwrap_err().code, "adr.reason");
    run(
        &dir,
        Step::Promote(Decision {
            identifier: "adr-process".into(),
            reason: "Reviewed".into(),
            evidence: "Review notes".into(),
            ..Decision::default()
        }),
    )
    .expect("promoted with evidence");
}
