//! Tests of the commit, the output paths, `clean`, ownership (as `Drop` tests) and
//! robustness (for the records this crate reads),
//! with a small simulated builder (`support`) in place of the whole project build.

mod clean;
mod commit;
mod layout;
mod ownership;
mod paths;
mod reuse;
mod robust;
mod support;
