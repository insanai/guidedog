//! The `guidedog` program end to end, as a user runs it: conversions, autodoc and the
//! Odin domain in a project, installed build plugins, builds repeated after changes,
//! and the repository files the manual and releases depend on.
//!
//! What crate tests already prove in process (the readers, the build stages, the plugin
//! protocol, the server) is not repeated here; these tests check that the pieces meet
//! in the program.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

mod autodoc;
mod convert;
mod incremental;
mod plugins;
mod repository;

use std::path::Path;

use guidedog_tests::{Run, Scratch, guidedog};

/// A fresh quickstart project in a scratch folder, its `conf.toml` replaced by `conf`.
pub fn project(name: &str, conf: &str) -> Scratch {
    let dir = Scratch::new(name);
    guidedog(&dir.path, 0, &["quickstart", "-q", "-p", "Demo", "-a", "Ada"]);
    dir.write("conf.toml", conf);
    dir
}

/// `guidedog build <args> --color=never` in `dir`, expecting `status`.
pub fn build(dir: &Path, status: i32, args: &[&str]) -> Run {
    let mut all = vec!["build"];
    all.extend_from_slice(args);
    all.push("--color=never");
    guidedog(dir, status, &all)
}

/// `text` with every run of whitespace made one space, so that phrases a report wraps
/// across lines can be found.
pub fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
