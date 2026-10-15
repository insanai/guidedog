//! Ownership, as `Drop` tests: what a build holds is released by dropping
//! it, and dropping never changes the published output.

use gd_env::{BUILD_BUDGET, Digest, Environment, ObjectStore};
use gd_host::Budget;

use super::support::{Options, Project, expect_settled, snapshot, text_of};
use crate::hook::{StepAction, set_hook};
use crate::{CommitRecords, CommitState, Generation, GenerationOptions, UnitRecord};

fn built() -> Project {
    let p = Project::new("ownership");
    p.put("index.rst", "Demo\n\nFirst.\n");
    p.put("a.rst", "A\n\nA page.\n");
    assert!(p.build(&Options::default()).ok());
    p.put("index.rst", "Demo\n\nSecond.\n");
    assert!(p.build(&Options::default()).ok());
    p
}

fn begin(p: &Project, budget: &Budget) -> Generation {
    let paths = p.paths(&Options::default()).unwrap();
    let mut reports = Vec::new();
    Generation::begin(paths, GenerationOptions { all: false, budget }, &mut reports).unwrap()
}

/// A unit dropped unfinished counts as failed, and a generation with a failed unit
/// publishes nothing: the previous output and records stay.
#[test]
fn a_unit_dropped_unfinished_fails_the_generation() {
    let p = built();
    let kept = snapshot(&p, true);
    let budget = Budget::new(BUILD_BUDGET);
    let mut generation = begin(&p, &budget);
    {
        let mut unit = generation.unit("index");
        unit.emit_text("index.html", "half written").unwrap();
    }
    assert_eq!(generation.failed_count(), 1);
    generation.unit("a").fail();
    assert_eq!(generation.failed_count(), 2);
    let store = ObjectStore::new(&generation.paths().doctrees);
    let env = Environment::default();
    let outcome = generation.commit(CommitRecords { environment: &env, store: &store });
    assert_eq!(outcome.state, CommitState::Kept);
    assert_eq!(outcome.problem.map(|p| p.code), Some("build.output.failed"));
    assert_eq!(kept, snapshot(&p, true));
    expect_settled(&p);
}

/// A generation dropped after writing (a build that failed or was cancelled) leaves
/// the previous output and records as they were, and nothing beside them but the spare
/// output; the next build publishes.
#[test]
fn a_dropped_generation_changes_nothing_published() {
    let p = built();
    let kept = snapshot(&p, true);
    let budget = Budget::new(BUILD_BUDGET);
    let mut generation = begin(&p, &budget);
    let mut unit = generation.unit("index");
    unit.emit_text("index.html", "never published").unwrap();
    unit.emit_text("extra/new.html", "never published").unwrap();
    unit.finish(UnitRecord { inputs: Digest::of_text("x"), ..UnitRecord::default() });
    generation.abandon();
    assert_eq!(kept, snapshot(&p, true));
    expect_settled(&p);
    p.put("index.rst", "Demo\n\nThird.\n");
    let r = p.build(&Options::default());
    assert!(r.ok() && r.published);
    assert!(text_of(&p.out().join("index.html")).contains("Third."));
    assert!(!p.out().join("extra").exists());
}

/// A generation whose decided commit stopped is unsettled: dropping it touches
/// nothing, so the journal and what it names stay for the next build.
#[test]
fn an_unsettled_generation_is_left_for_the_next_build() {
    let p = built();
    p.put("index.rst", "Demo\n\nStopped.\n");
    set_hook(Some(Box::new(|step| if step == 1 { StepAction::Crash } else { StepAction::Go })));
    let stopped = p.build(&Options::default());
    set_hook(None);
    assert!(stopped.unsettled);
    let journal = p.dir.join("_build/.doctrees/html.commit.json");
    assert!(journal.is_file());
    assert!(p.dir.join("_build/.doctrees/html.outputs.json.next").is_file());
    let r = p.build(&Options::default());
    assert!(r.ok() && !journal.exists());
    assert!(text_of(&p.out().join("index.html")).contains("Stopped."));
    expect_settled(&p);
}

/// Repeated builds and drops leave no stale files behind: the file system is what a
/// build can leak.
#[test]
fn builds_leave_nothing_behind() {
    let p = built();
    for i in 0..3 {
        p.put("a.rst", &format!("A\n\nVersion {i}.\n"));
        assert!(p.build(&Options::default()).ok());
        expect_settled(&p);
    }
    let mut names: Vec<String> = std::fs::read_dir(p.dir.join("_build"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, [".doctrees", ".html.next", "html"]);
}
