//! Applying plans: whole or not at all, and recoverable when interrupted. Rust aborts
//! when memory runs out, so the tests sweep interruptions, not allocation refusals: an
//! apply stopped after every possible step is rolled back exactly.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

use super::{catalog_fixture, prefix, write};
use crate::transaction::apply_changes;
use crate::*;
use std::path::{Path, PathBuf};

/// Every file of the catalog, by name, as one text.
fn snapshot(dir: &Path) -> String {
    let mut out = String::new();
    for folder in [dir.to_path_buf(), dir.join("records")] {
        let mut entries: Vec<PathBuf> =
            std::fs::read_dir(&folder).unwrap().map(|e| e.unwrap().path()).collect();
        entries.sort();
        for path in entries.iter().filter(|p| p.is_file() && !p.ends_with(".eds.lock")) {
            out.push_str(&path.file_name().unwrap().to_string_lossy());
            out.push('\0');
            out.push_str(&std::fs::read_to_string(path).unwrap());
            out.push('\0');
        }
    }
    out
}

fn promotion_plan(dir: &Path) -> Plan {
    let c = load_catalog(dir, &prefix("eds")).unwrap();
    let decision = Decision { identifier: "eds-process".into(), ..Decision::default() };
    promote_plan(&c, &decision).unwrap_or_else(|p| panic!("{p}"))
}

#[test]
fn an_interrupted_apply_rolls_back_at_every_step() {
    let (_root, dir) = catalog_fixture("interrupt");
    let p = prefix("eds");
    let before = snapshot(&dir);
    let plan = promotion_plan(&dir);
    assert!(plan.changes.len() >= 3);
    let lock = lock_catalog(&dir, &p).unwrap();
    for count in 0..plan.changes.len() {
        let problem = apply_changes(&lock, &plan, count).unwrap_err();
        assert_eq!(problem.code, "eds.recovery-needed");
        assert_eq!(load_catalog(&dir, &p).unwrap_err().code, "eds.recovery-needed");
        check_recovery(&dir, &p).expect("recoverable");
        recover(&lock).expect("recovered");
        assert_eq!(snapshot(&dir), before, "after {count} files");
    }
    apply(&lock, &plan).expect("applied");
    let c = load_catalog(&dir, &p).unwrap();
    assert_eq!(c.records[0].number, "0001");
    assert_eq!(c.records[0].state, "discussion");
    check_index(&c).expect("a current index");
    recover(&lock).expect("no journal is no work");
}

#[test]
fn recovery_refuses_to_overwrite_a_later_edit() {
    let (_root, dir) = catalog_fixture("conflict");
    let p = prefix("eds");
    let plan = promotion_plan(&dir);
    let lock = lock_catalog(&dir, &p).unwrap();
    let _ = apply_changes(&lock, &plan, 1);
    write(&dir.join(&plan.changes[0].path), "An edit made after the interruption.\n");
    assert_eq!(check_recovery(&dir, &p).unwrap_err().code, "eds.recovery-conflict");
    assert_eq!(recover(&lock).unwrap_err().code, "eds.recovery-conflict");
    let journal = dir.join(".eds-journal.json");
    assert!(journal.exists(), "the journal is kept for another try");
    write(&journal, "not json");
    assert_eq!(recover(&lock).unwrap_err().code, "eds.journal");
}

#[test]
fn apply_refuses_plans_that_are_not_whole() {
    let (_root, dir) = catalog_fixture("whole");
    let p = prefix("eds");
    let before = snapshot(&dir);
    let record = std::fs::read_to_string(dir.join("records/XXXXX-eds-process.rst")).unwrap();
    let registry = std::fs::read_to_string(dir.join("registry.rst")).unwrap();
    let change = |path: &str, before: &str, after: &str| Change {
        path: path.into(),
        before: Some(before.into()),
        after: Some(after.into()),
    };
    let lock = lock_catalog(&dir, &p).unwrap();
    for c in [
        change("records/XXXXX-eds-process.rst", &record, ""),
        change("records/XXXXX-eds-process.rst", &record, &record[..record.len() / 2]),
        change("registry.rst", &registry, "Registry\n"),
    ] {
        let plan = Plan { changes: vec![c], summary: String::new() };
        assert_eq!(apply(&lock, &plan).unwrap_err().code, "eds.incomplete-plan");
        assert_eq!(snapshot(&dir), before);
        assert!(!dir.join(".eds-journal.json").exists());
    }
    let escape = change("../outside.rst", "", "x");
    let plan = Plan { changes: vec![escape], summary: String::new() };
    assert_eq!(apply(&lock, &plan).unwrap_err().code, "eds.changed");
}

/// A builder that records what it was asked to make.
#[derive(Default)]
struct Recorder {
    requests: Vec<BuildRequest>,
}

impl Builder for Recorder {
    fn build(&mut self, request: &BuildRequest) -> Result<PathBuf> {
        self.requests.push(request.clone());
        Ok(request.output())
    }
}

#[test]
fn building_sends_whole_requests_for_checked_catalogs() {
    let (_root, dir) = catalog_fixture("build");
    let c = load_catalog(&dir, &prefix("eds")).unwrap();
    let mut recorder = Recorder::default();
    let outputs = build(&c, &BuildOptions::default(), &mut recorder).expect("built");
    assert_eq!(outputs, [dir.join("_build/html"), dir.join("_build/pdf")]);
    let requests = std::mem::take(&mut recorder.requests);
    assert_eq!(requests.len(), 2);
    let book = &requests[1].books[0];
    assert_eq!(book.root, "records/XXXXX-eds-process");
    assert_eq!(book.file, "eds-XXXXX-eds-process.pdf");
    assert_eq!((book.title.as_str(), book.author.as_str()), ("Enso Discussions Process", "Ada"));
    let options = BuildOptions {
        target: Some(Target::Pdf),
        record: "eds-process".into(),
        ..BuildOptions::default()
    };
    build(&c, &options, &mut recorder).expect("a preview");
    let requests = std::mem::take(&mut recorder.requests);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].selection.as_deref(), Some("eds-XXXXX-eds-process"));
    assert_eq!(requests[0].output(), dir.join("_build/selected/eds-XXXXX-eds-process/pdf"));
    // A stale index is refused before the builder runs.
    write(&dir.join("bundle.rst"), "Order\n=====\n");
    assert_eq!(
        build(&c, &BuildOptions::default(), &mut recorder).unwrap_err().code,
        "eds.index-drift"
    );
    assert!(std::mem::take(&mut recorder.requests).is_empty());
}

#[test]
fn index_plans_rewrite_only_the_generated_region() {
    let (_root, dir) = catalog_fixture("index");
    let p = prefix("eds");
    write(
        &dir.join("registry.rst"),
        ":eds-reserved-through: 0\n\nOur prose.\n\n\
        .. eds-generated-start\n.. eds-generated-end\nAfter.\n",
    );
    let c = load_catalog(&dir, &p).unwrap();
    let plan = index_plan(&c).unwrap();
    assert_eq!(plan.changes.len(), 1);
    let after = plan.changes[0].after.as_deref().unwrap();
    assert!(after.starts_with(":eds-reserved-through: 0\n\nOur prose.\n"));
    assert!(after.contains(":doc:`records/XXXXX-eds-process`"));
    assert!(after.ends_with(".. eds-generated-end\nAfter.\n"));
    apply(&lock_catalog(&dir, &p).unwrap(), &plan).unwrap();
    check_index(&load_catalog(&dir, &p).unwrap()).expect("current");
}
