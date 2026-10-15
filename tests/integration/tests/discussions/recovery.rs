//! Interrupted updates, the catalog's lock across processes, and stale plans.

use super::{Legacy, exists, gds};
use gd_discussion as ds;
use guidedog_tests::{has, read, write};

/// Leaves the journal an interrupted update would leave, naming `changes`.
fn journal(c: &Legacy, changes: &[ds::Change]) {
    let items: Vec<String> = changes
        .iter()
        .map(|change| {
            let text = |value: &Option<String>| {
                gd_json::Value::from(value.clone().unwrap_or_default()).to_string()
            };
            format!(
                "{{\"path\":{},\"before\":{},\"after\":{},\"existed\":{},\"present\":{}}}",
                gd_json::Value::from(change.path.clone()),
                text(&change.before),
                text(&change.after),
                change.before.is_some(),
                change.after.is_some()
            )
        })
        .collect();
    write(&c.path(".gds-journal.json"), &format!("[{}]", items.join(",")));
}

fn change(path: &str, before: &str, after: &str) -> ds::Change {
    ds::Change { path: path.into(), before: Some(before.into()), after: Some(after.into()) }
}

#[test]
fn recovery_preserves_later_edits() {
    let c = Legacy::new("gds-recover");
    let original = c.read("registry.typ");
    journal(&c, &[change("registry.typ", &original, "interrupted")]);
    // A file edited after the interruption is neither the original nor the update.
    write(&c.path("registry.typ"), "later user edit");
    has(&c.dsc(4, &["recover", "--rollback"]).err, "LATER EDIT");
    assert_eq!(c.read("registry.typ"), "later user edit");
    write(&c.path("registry.typ"), "interrupted");
    c.dsc(0, &["recover", "--rollback", "--dry-run"]);
    assert_eq!(c.read("registry.typ"), "interrupted");
    c.dsc(0, &["recover", "--rollback"]);
    assert_eq!(c.read("registry.typ"), original);
    c.dsc(0, &["check"]);
}

#[test]
fn recovery_rejects_escaping_paths() {
    let c = Legacy::new("gds-escape");
    let outside = c.path("../outside.txt");
    write(&outside, "keep");
    journal(&c, &[change("../outside.txt", "remove", "keep")]);
    c.dsc(4, &["recover", "--rollback"]);
    assert_eq!(read(&outside), "keep");
}

#[test]
fn lock_blocks_another_process() {
    let c = Legacy::new("gds-lock");
    let _lock = ds::lock_catalog(&c.dir, &gds()).unwrap();
    has(&c.dsc(4, &["new", "Blocked", "--author=Ada"]).err, "locked");
    assert!(!exists(&c.path("records/XXXXX-blocked.typ")));
}

#[test]
fn changed_snapshot_refuses_write() {
    let c = Legacy::new("gds-stale");
    let plan = ds::Plan {
        changes: vec![change("registry.typ", "old", "replacement")],
        summary: "stale update".into(),
    };
    let old = c.read("registry.typ");
    let lock = ds::lock_catalog(&c.dir, &gds()).unwrap();
    let problem = ds::apply(&lock, &plan).unwrap_err();
    assert_eq!((problem.code.as_str(), problem.exit), ("gds.changed", 4), "{problem}");
    assert_eq!(c.read("registry.typ"), old);
    assert!(!exists(&c.path(".gds-journal.json")));
}
