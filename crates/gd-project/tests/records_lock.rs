//! A build holds its records folder's lock (GDS 0020): while another holder has it, the
//! build stops with `build.busy` and publishes nothing; once released, the build runs.

#![allow(clippy::unwrap_used)]

mod common;

use common::{assert_built, build, demo, load};
use gd_project::{BuilderKind, RECORDS_LOCK};

#[test]
fn a_build_whose_records_folder_is_locked_stops_before_writing() {
    let dir = demo("records-lock");
    let project = load(&dir.0);
    // The default layout keeps its records in the build folder itself.
    let records = dir.path("_build");
    std::fs::create_dir_all(&records).unwrap();
    let held = gd_plugins::files::lock_exclusive(&records.join(RECORDS_LOCK)).unwrap();
    let refused = build(&project, BuilderKind::Html);
    let problem = refused.problem.as_ref().map(|p| (p.code, p.exit));
    assert_eq!(problem, Some(("build.busy", 4)), "{:?}", refused.problem);
    assert!(!refused.published);
    assert!(!dir.exists("_build/html/index.html"), "a refused build publishes nothing");
    drop(held);
    let built = build(&project, BuilderKind::Html);
    assert_built(&built);
    // The lock is released once the build, its commit included, has ended.
    assert!(gd_plugins::files::lock_exclusive(&records.join(RECORDS_LOCK)).is_ok());
}
