//! Tests ported from lib/project: commit_test, build_paths_test, clean_test,
//! ownership_test (as `Drop` tests) and robust_test (for the records this crate reads),
//! with a small simulated builder (`support`) in place of the whole project build.

mod clean;
mod commit;
mod ownership;
mod paths;
mod reuse;
mod robust;
mod support;
