//! What a build reads for its own bookkeeping is counted against its budget, as the
//! documents are (Odin budget_test). Each test makes one such thing larger than the
//! budget, which the rest of the build fits, and expects the budget's problem.

#![allow(clippy::unwrap_used)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use common::{TempDir, assert_built, load};
use gd_host::RefusalChoice;
use gd_project::{BuildOptions, BuildResult, BuilderKind};

const BUDGET: usize = 4 << 20;
const BIG: usize = 6 << 20;

fn project_dir(name: &str, conf: &str) -> TempDir {
    let dir = TempDir::new(name);
    dir.put("conf.toml", conf);
    dir.put("index.rst", "Demo\n====\n\nA short page.\n");
    dir
}

fn budgeted() -> BuildOptions {
    BuildOptions { budget: Some(BUDGET), ..Default::default() }
}

fn is_budget(r: &BuildResult) -> bool {
    r.problem.as_ref().is_some_and(|p| p.code == "host.budget")
        || r.reports.iter().any(|rep| rep.code == "host.budget")
}

/// The saved environment and the previous output's manifest are read within the budget,
/// and the problem names the file and a budget that fits.
#[test]
fn saved_state_is_read_within_the_budget() {
    let dir = project_dir("saved", "project = \"Demo\"\n");
    let project = load(&dir.0);
    assert_built(&gd_project::build(&project, BuilderKind::Html, &budgeted()));
    let padding = "x".repeat(BIG);
    for name in ["environment.json", "html.outputs.json"] {
        let path = format!("_build/.doctrees/{name}");
        let saved = dir.read(&path);
        dir.put(&path, &format!("{{\"padding\":\"{padding}\",{}", &saved[1..]));
        let r = gd_project::build(&project, BuilderKind::Html, &budgeted());
        let problem = r.problem.as_ref().unwrap();
        assert_eq!(problem.code, "host.budget", "{name}: {problem:?}");
        assert!(problem.path.contains(name), "{name}: {problem:?}");
        assert!(problem.hint.contains("--budget="), "{}", problem.hint);
        assert_eq!(r.exit, 3);
        dir.put(&path, &saved);
    }
    assert_built(&gd_project::build(&project, BuilderKind::Html, &budgeted()));
}

/// Catalogs are read within the budget.
#[test]
fn catalogs_are_read_within_the_budget() {
    let dir = project_dir("catalogs", "project = \"Demo\"\nlanguage = \"de\"\n");
    let mut po = String::new();
    for i in 0..BIG / 40 {
        po.push_str(&format!("msgid \"message {i}\"\nmsgstr \"Nachricht\"\n\n"));
    }
    dir.put("locales/de/LC_MESSAGES/index.po", &po);
    let r = gd_project::build(&load(&dir.0), BuilderKind::Html, &budgeted());
    assert!(is_budget(&r), "{:?}", r.problem);
}

/// At its budget a build asks what to do: raising the budget lets it finish, and the
/// result says the budget it ended with; stopping fails it with the budget's problem.
#[test]
fn the_budget_policy_raises_or_stops() {
    let dir = project_dir("policy", "project = \"Demo\"\n");
    let project = load(&dir.0);
    assert_built(&gd_project::build(&project, BuilderKind::Html, &budgeted()));
    let path = "_build/.doctrees/environment.json";
    let saved = dir.read(path);
    dir.put(path, &format!("{{\"padding\":\"{}\",{}", "x".repeat(BIG), &saved[1..]));
    let asked = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&asked);
    let raise = BuildOptions {
        on_budget: Some(Arc::new(move |refusal: &gd_host::Refusal| {
            counter.fetch_add(1, Ordering::SeqCst);
            RefusalChoice::Raise(refusal.limit + refusal.need + (64 << 20))
        })),
        ..budgeted()
    };
    let r = gd_project::build(&project, BuilderKind::Html, &raise);
    assert_built(&r);
    assert!(asked.load(Ordering::SeqCst) >= 1);
    assert!(r.budget > BUDGET && r.peak > 0, "{} {}", r.budget, r.peak);
    dir.put(path, &format!("{{\"padding\":\"{}\",{}", "x".repeat(BIG), &saved[1..]));
    let stop = BuildOptions {
        on_budget: Some(Arc::new(|_: &gd_host::Refusal| RefusalChoice::Stop)),
        ..budgeted()
    };
    let r = gd_project::build(&project, BuilderKind::Html, &stop);
    assert_eq!(r.problem.as_ref().map(|p| p.code), Some("host.budget"));
    assert!(!r.published);
}

/// A stack budget the reading threads cannot hold is refused before anything is read.
#[test]
fn a_stack_budget_too_large_is_refused() {
    let dir = project_dir("stack", "project = \"Demo\"\n");
    let options = BuildOptions { stack_bytes: Some(1 << 30), ..Default::default() };
    assert_eq!(gd_project::build_limits(&options).unwrap_err().code, "build.stack");
    let r = gd_project::build(&load(&dir.0), BuilderKind::Html, &options);
    assert_eq!(r.problem.as_ref().map(|p| (p.code, p.exit)), Some(("build.stack", 2)));
    assert_eq!(r.read, 0);
    let fits = BuildOptions { stack_bytes: Some(1 << 20), ..Default::default() };
    assert_eq!(gd_project::build_limits(&fits).unwrap().stack_bytes, 1 << 20);
}
