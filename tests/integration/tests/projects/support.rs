//! Helpers shared by the project tests: a quickstarted project and builds in it.

use std::path::Path;
use std::process::Command;

use guidedog_tests::{Run, Scratch, program, run_command};

/// Runs `guidedog args` in `dir` with colors off, a private configuration folder, and
/// the memory available stated (8 GiB), so the budget advice a build gives is the same
/// on every machine however busy; expects `status`.
pub fn guidedog_env(dir: &Path, status: i32, args: &[&str], env: &[(&str, &str)]) -> Run {
    let mut command = Command::new(program("guidedog"));
    command
        .args(args)
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .env("GUIDEDOG_AVAILABLE_MIB", "8192")
        .env("GUIDEDOG_CONFIG_DIR", dir.join(".gd-config"));
    for (key, value) in env {
        command.env(key, value);
    }
    run_command(&mut command).expect(status)
}

/// [`guidedog_env`] with no further variables.
pub fn cli(dir: &Path, status: i32, args: &[&str]) -> Run {
    guidedog_env(dir, status, args, &[])
}

/// A fresh project made by `guidedog quickstart` in its own scratch folder, named "Demo".
pub fn project(name: &str) -> Scratch {
    let dir = Scratch::new(name);
    cli(&dir.path, 0, &["quickstart", "-q", "-p", "Demo", "-a", "Ada"]);
    dir
}

/// Runs `guidedog build <args> --color=never` in `dir` and expects `status`.
pub fn build(dir: &Path, status: i32, args: &[&str]) -> Run {
    let mut all = vec!["build"];
    all.extend_from_slice(args);
    all.push("--color=never");
    cli(dir, status, &all)
}

/// Standard output and standard error together.
pub fn both(r: &Run) -> String {
    format!("{}{}", r.out, r.err)
}

/// Panics unless `text` holds each of `parts`, in order.
pub fn in_order(text: &str, parts: &[&str]) {
    let mut at = 0;
    for part in parts {
        match text[at..].find(part) {
            Some(i) => at += i + part.len(),
            None => panic!("{part:?} not found after offset {at} in:\n{text}"),
        }
    }
}
