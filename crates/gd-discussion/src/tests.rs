//! Library tests: metadata, configuration, the index, plans, and their transactions.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

mod config;
mod transaction;

use crate::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

const RECORD: &str = ":sds-number: XXXXX
:sds-title: Cache policy
:sds-state: prediscussion
:sds-type: Standards Track
:sds-authors: [\"Ada\", \"René\"]
:sds-created: 2026-10-12
:sds-updated: 2026-10-12
:sds-discussion: Local review
:sds-labels: [\"build\", \"architecture\"]

Cache policy
============

Abstract
--------

A proposal.

References
----------

The implementation.
";

/// A folder under the system's temporary folder, removed with its contents on drop.
pub(crate) struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(name: &str) -> TempDir {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("gd-discussion-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("a temporary folder");
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn prefix(text: &str) -> Prefix {
    Prefix::new(text).expect("a valid prefix")
}

/// An RST catalog with the prefix `eds` and one draft, in a new temporary folder.
pub(crate) fn catalog_fixture(name: &str) -> (TempDir, PathBuf) {
    let root = TempDir::new(name);
    let dir = root.0.join("eds");
    let init = Init {
        dir: dir.clone(),
        prefix: prefix("eds"),
        name: "Enso Discussions".into(),
        author: "Ada".into(),
        ..Init::default()
    };
    if let Err(p) = initialize(&init) {
        panic!("{p}");
    }
    (root, dir)
}

pub(crate) fn write(path: &Path, text: &str) {
    std::fs::write(path, text).expect("a written file");
}

fn with_prefix(p: &str) -> String {
    RECORD.replace(":sds-", &format!(":{p}-"))
}

fn record_of(p: &Prefix, path: &str, source: &str) -> Result<Record> {
    read_record(&Catalog::new(p.clone()), Path::new(path), source)
}

#[test]
fn every_prefix_names_its_own_fields_and_body_fields_stay() {
    for name in ["sds", "gds", "eds", "architecture"] {
        let p = prefix(name);
        let source = format!("{}\n::\n\n   :{name}-state: accepted\n", with_prefix(name));
        let path = "records/XXXXX-cache-policy.rst";
        let catalog = Catalog {
            labels: vec!["build".into(), "architecture".into()],
            ..Catalog::new(p.clone())
        };
        let r = read_record(&catalog, Path::new(path), &source).expect("a record");
        assert_eq!(r.authors[1], "René");
        assert!(r.sections.has_abstract && r.sections.has_references);
        let edited =
            replace_field(&source, &p, "state", FieldValue::Text("abandoned")).expect("an edit");
        let r = read_record(&catalog, Path::new(path), &edited).expect("a record");
        assert_eq!(r.state, "abandoned");
        assert!(edited.contains(&format!("   :{name}-state: accepted")));
        // Only the field's line changes.
        assert_eq!(edited.len(), source.len() + "abandoned".len() - "prediscussion".len());
    }
    let p = record_of(&prefix("gds"), "records/XXXXX-cache-policy.rst", RECORD);
    assert_eq!(p.map(|_| ()).unwrap_err().code, "gds.metadata");
    assert!(!is_valid_prefix("../sds") && !is_valid_prefix("x") && is_valid_prefix("rfc"));
}

#[test]
fn field_edits_keep_every_other_byte_and_fail_out_loud() {
    let p = prefix("eds");
    let crlf = with_prefix("eds").replace('\n', "\r\n");
    let edited = replace_field(&crlf, &p, "title", FieldValue::Text("Cache rules")).unwrap();
    assert_eq!(edited, crlf.replace(":eds-title: Cache policy", ":eds-title: Cache rules"));
    let labels = ["cli".to_string()];
    let edited = replace_field(&crlf, &p, "labels", FieldValue::List(&labels)).unwrap();
    assert!(edited.contains(":eds-labels: [\"cli\"]\r\n"));
    // A value that would break the line is quoted, and reads back as itself.
    let edited = replace_field(&crlf, &p, "title", FieldValue::Text("Two\nlines")).unwrap();
    let r = read_record(
        &Catalog {
            labels: vec!["build".into(), "architecture".into()],
            ..Catalog::new(p.clone())
        },
        Path::new("records/XXXXX-cache-policy.rst"),
        &edited,
    );
    assert_eq!(r.unwrap().title, "Two\nlines");
    let missing = replace_field(&crlf, &p, "status", FieldValue::Text("Open"));
    assert_eq!(missing.unwrap_err().code, "eds.field");
}

#[test]
fn memo_fields_and_the_last_updated_name_are_kept() {
    let p = prefix("sds");
    let source = format!(
        ":sds-category: Engineering Discussion\n:sds-status: Working draft\n\
        :sds-summary: Cache safely.\n{}",
        RECORD.replace(":sds-updated:", ":sds-last-updated:")
    );
    let path = "records/XXXXX-cache-policy.rst";
    let r = record_of(&p, path, &source).expect("a record");
    assert_eq!(r.category, "Engineering Discussion");
    assert_eq!(r.status, "Working draft");
    assert_eq!(r.summary, "Cache safely.");
    let edited = replace_field(&source, &p, "updated", FieldValue::Text("2026-10-13")).unwrap();
    assert!(edited.contains(":sds-last-updated: 2026-10-13"));
    let both = format!(":sds-updated: 2026-10-12\n{source}");
    assert_eq!(record_of(&p, path, &both).unwrap_err().code, "sds.metadata");
}

#[test]
fn metadata_mistakes_are_named_with_their_fix() {
    let p = prefix("sds");
    let path = "records/XXXXX-cache-policy.rst";
    let cases = [
        (RECORD.replace("2026-10-12\n:sds-updated", "2026-02-30\n:sds-updated"), "sds.date"),
        (RECORD.replace(":sds-updated: 2026-10-12", ":sds-updated: 2026-10-11"), "sds.date-order"),
        (RECORD.replace("prediscussion", "prediscusion"), "sds.metadata"),
        (RECORD.replace("Standards Track", "Standard"), "sds.metadata"),
        (RECORD.replace("[\"build\", \"architecture\"]", "[\"Build\"]"), "sds.label"),
        (RECORD.replace("prediscussion", "accepted"), "sds.metadata"),
    ];
    for (source, code) in cases {
        let problem = record_of(&p, path, &source).unwrap_err();
        assert_eq!(problem.code, code, "{}", problem.message);
        assert!(!problem.hint.is_empty());
    }
    let state = record_of(&p, path, &RECORD.replace("prediscussion", "prediscusion"));
    assert_eq!(state.unwrap_err().suggestion.as_deref(), Some("prediscussion"));
}

#[test]
fn generated_regions_keep_prose_and_refuse_stray_markers() {
    let p = prefix("eds");
    let original = "Before\n.. eds-generated-start\nOld\n.. eds-generated-end\nAfter\n";
    let result = index::managed_text(original, "New\n", "", &p, true).unwrap();
    assert_eq!(result, "Before\n.. eds-generated-start\nNew\n.. eds-generated-end\nAfter\n");
    assert!(index::is_balanced(&result, &p, true));
    for bad in [
        ".. eds-generated-end\n".to_string(),
        format!("{original}.. eds-generated-end\n"),
        format!("{original}.. eds-generated-start\n"),
        ".. eds-generated-start\n.. eds-generated-start\n.. eds-generated-end\n".to_string(),
    ] {
        let problem = index::managed_text(&bad, "New\n", "", &p, true).unwrap_err();
        assert_eq!(problem.code, "eds.markers");
    }
}

#[test]
fn lifecycle_moves_need_their_reasons() {
    // The default workflow is GDS 0001's; promotion, not state, opens a draft.
    let gds = Process::default();
    let moves =
        |from: &str, to: &str| gds.state(from).is_some_and(|s| s.next.iter().any(|n| n == to));
    assert!(!moves("prediscussion", "discussion") && moves("prediscussion", "abandoned"));
    assert!(!moves("prediscussion", "accepted"));
    assert!(!moves("committed", "abandoned"));
    assert!(!moves("abandoned", "discussion"));
    assert!(gds.is_unnumbered("abandoned") && !gds.is_unnumbered("accepted"));
    let root = TempDir::new("reasons");
    let p = prefix("sds");
    std::fs::create_dir_all(root.0.join("records")).unwrap();
    let source = with_prefix("sds")
        .replace(":sds-number: XXXXX", ":sds-number: 0001")
        .replace(":sds-state: prediscussion", ":sds-state: accepted");
    write(&root.0.join("records/0001-cache-policy.rst"), &source);
    let catalog = load_catalog(&root.0, &p).expect("a catalog");
    let decision = Decision {
        identifier: "1".into(),
        to: "committed".into(),
        reason: "Implemented".into(),
        ..Decision::default()
    };
    assert_eq!(state_plan(&catalog, &decision).unwrap_err().code, "sds.reason");
    let decision = Decision { reason: String::new(), to: "published".into(), ..decision };
    assert_eq!(state_plan(&catalog, &decision).unwrap_err().code, "sds.reason");
    let decision = Decision { to: "publshed".into(), reason: "Yes".into(), ..decision };
    let problem = state_plan(&catalog, &decision).unwrap_err();
    assert_eq!(problem.code, "sds.transition");
    assert_eq!(problem.suggestion.as_deref(), Some("published"));
}

#[test]
fn history_wraps_at_spaces_without_breaking_words() {
    let first = "2026-10-11: accepted -> committed. Reviewed ownership and error handling.";
    let last = "Evidence: tests/typst_test.odin; native PDF and HTML builds.";
    assert_eq!(
        rst::history(&format!("{first} {last}  \n")),
        format!("\n.. rubric:: Lifecycle event\n\n::\n\n   {first}\n   {last}\n")
    );
    let word = "界".repeat(40);
    assert_eq!(rst::history(&word), format!("\n.. rubric:: Lifecycle event\n\n::\n\n   {word}\n"));
}

#[test]
fn plans_name_only_catalog_files() {
    use catalog::is_safe_change_path as safe;
    assert!(safe("records/0001-cache-policy.rst"));
    assert!(safe("records/XXXXX-cache-policy.typ"));
    assert!(safe("registry.rst"));
    assert!(!safe("records/0001-../../escape.rst"));
    assert!(!safe("records/0001-cache-policy.rst/extra"));
    assert!(!safe("records/0000-zero.rst"));
    assert!(!safe("../registry.rst"));
}

#[test]
fn slugs_come_from_titles_and_drafts_get_their_heading() {
    assert_eq!(edit::make_slug("Costs $(touch nope) `echo no`"), "costs-touch-nope-echo-no");
    assert_eq!(edit::make_slug("  Cache: policy!  "), "cache-policy");
    let source = ":eds-number: XXXXX\n\nDiscussion proposal\n===================\n\nText\n";
    let titled = rst::draft_title(source, "Use *bold* _names_", &prefix("eds"));
    assert!(titled.contains("\nUse \\*bold\\* \\_names\\_\n======================\n"), "{titled}");
}

#[test]
fn init_makes_a_whole_catalog_and_refuses_to_merge() {
    let (root, dir) = catalog_fixture("init");
    let d = definition(&dir).unwrap().expect("a configuration");
    assert!(d.prefix.as_str() == "eds" && d.labels.is_empty());
    let c = load_catalog(&dir, &prefix("eds")).expect("a catalog");
    assert_eq!(c.records.len(), 1);
    assert_eq!(c.records[0].slug, "eds-process");
    check_sources(&c).expect("sections");
    check_index(&c).expect("a current index");
    let (found, _) = discover(&dir.join("records")).unwrap().expect("a catalog");
    assert!(found.ends_with("eds"));
    let again = Init {
        dir: dir.clone(),
        prefix: prefix("eds"),
        name: "Again".into(),
        author: "Ada".into(),
        ..Init::default()
    };
    assert_eq!(initialize(&again).unwrap_err().code, "discussion.init-exists");
    assert_eq!(load_catalog(&dir, &prefix("sds")).unwrap_err().code, "discussion.prefix");
    // A chosen process and template are used, and checked like any record.
    let other = root.0.join("rfc");
    let draft = "Our process\n===========\n\nAbstract\n--------\n\nShort.\n\nReferences\n\
        ----------\n\nNone.\n";
    initialize(&Init {
        dir: other.clone(),
        prefix: prefix("rfc"),
        name: "RFCs".into(),
        author: "Ada".into(),
        labels: Some(vec!["process".into(), "api".into()]),
        draft: draft.into(),
        ..Init::default()
    })
    .expect("a catalog with a chosen process");
    let c = load_catalog(&other, &prefix("rfc")).expect("a catalog");
    let source = std::fs::read_to_string(&c.records[0].path).unwrap();
    assert!(source.contains("Our process\n==="));
    assert_eq!(c.vocabulary().map(|v| v.len()), Some(2));
    let bad = root.0.join("bad");
    let problem = initialize(&Init {
        dir: bad.clone(),
        prefix: prefix("bad"),
        name: "Bad".into(),
        author: "Ada".into(),
        draft: "No sections\n===========\n".into(),
        ..Init::default()
    })
    .unwrap_err();
    assert_eq!(problem.code, "bad.section");
    assert!(!bad.exists());
    let leftovers = std::fs::read_dir(&root.0).unwrap().count();
    assert_eq!(leftovers, 2, "a failed init leaves no stage behind");
}

#[test]
fn initialization_refuses_an_empty_vocabulary() {
    let problem = init_plan(&Init {
        dir: "unused-review-catalog".into(),
        prefix: prefix("eds"),
        name: "Enso".into(),
        author: "Ada".into(),
        labels: Some(Vec::new()),
        ..Init::default()
    })
    .unwrap_err();
    assert_eq!(problem.code, "discussion.init");
    assert!(problem.message.contains("labels list is empty"));
}

#[test]
fn metadata_line_limits_do_not_depend_on_read_boundaries() {
    let (_root, dir) = catalog_fixture("limits");
    let p = prefix("eds");
    let path = dir.join("records/0001-long.rst");
    let source = format!(":eds-summary: {}\n", "x".repeat(scan::MAX_LINE));
    write(&path, &source);
    assert_eq!(
        read_record(&Catalog::new(p.clone()), &path, &source).unwrap_err().code,
        "eds.metadata"
    );
    for chunk in [scan::SCAN_CHUNK, 127] {
        assert_eq!(scan::scan_record(&p, &path, chunk).unwrap_err().code, "eds.metadata");
    }
}

#[test]
fn records_are_resolved_by_number_slug_title_or_file() {
    let (_root, dir) = catalog_fixture("resolve");
    let c = load_catalog(&dir, &prefix("eds")).unwrap();
    for id in ["eds-process", "Enso Discussions Process", "XXXXX-eds-process.rst"] {
        assert_eq!(resolve(&c, id).expect("one record").slug, "eds-process");
    }
    let problem = resolve(&c, "eds-proces").unwrap_err();
    assert_eq!(problem.code, "eds.record");
    assert_eq!(problem.suggestion.as_deref(), Some("eds-process"));
}

#[test]
fn a_new_draft_is_planned_whole() {
    let (_root, dir) = catalog_fixture("draft");
    let p = prefix("eds");
    let lock = lock_catalog(&dir, &p).unwrap();
    let c = load_catalog(&dir, &p).unwrap();
    let draft = NewDraft {
        title: "Cache *policy*".into(),
        author: "Ada".into(),
        labels: vec!["architecture".into()],
        ..NewDraft::default()
    };
    let plan = new_plan(&c, &draft).expect("a plan");
    assert_eq!(plan.summary, "Create records/XXXXX-cache-policy.rst");
    apply(&lock, &plan).expect("applied");
    let c = load_catalog(&dir, &p).unwrap();
    let r = resolve(&c, "cache-policy").unwrap();
    assert_eq!((r.title.as_str(), r.state.as_str()), ("Cache *policy*", "prediscussion"));
    check_index(&c).expect("a current index");
    let taken = new_plan(&c, &draft).unwrap_err();
    assert_eq!(taken.code, "eds.slug-taken");
    let unknown = NewDraft { labels: vec!["architecure".into()], slug: "x".into(), ..draft };
    let c = Catalog { labels: vec!["architecture".into()], ..c };
    let problem = new_plan(&c, &unknown).unwrap_err();
    assert_eq!(problem.code, "eds.label");
    assert_eq!(problem.suggestion.as_deref(), Some("architecture"));
}

#[test]
fn a_second_command_finds_the_catalog_locked() {
    let (_root, dir) = catalog_fixture("lock");
    let p = prefix("eds");
    let lock = lock_catalog(&dir, &p).expect("the lock");
    assert_eq!(lock_catalog(&dir, &p).unwrap_err().code, "eds.busy");
    drop(lock);
    lock_catalog(&dir, &p).expect("the lock, released");
}
