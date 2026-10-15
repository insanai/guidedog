//! Building a project with the installed `guidedog`, and freezing the site to publish.

use std::path::Path;
use std::process::{Command, Stdio};

use gd_plugins::{Problem, Snapshot, SnapshotMode, SnapshotOptions, snapshot};

use crate::config::{Config, SiteSource};
use crate::problem::problem;
use crate::{Settings, build_dir};

/// Names the `guidedog` that builds a project when no program is given.
pub const GUIDEDOG_VARIABLE: &str = "GUIDEDOG_PAGES_GUIDEDOG";

/// The program that builds: `given` when not empty, `$GUIDEDOG_PAGES_GUIDEDOG`, or
/// `guidedog` on `PATH`.
pub fn guidedog_program(given: &str) -> String {
    if !given.is_empty() {
        return given.to_string();
    }
    std::env::var(GUIDEDOG_VARIABLE)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "guidedog".to_string())
}

/// The first line `program --version` prints, or `None` when it cannot run.
pub fn guidedog_version(program: &str) -> Option<String> {
    let out = Command::new(program).arg("--version").stdin(Stdio::null()).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Some(text.lines().next().unwrap_or_default().trim().to_string())
}

/// `pages.guidedog` (exit 4): `program` could not be run.
pub(crate) fn guidedog_missing(program: &str) -> Problem {
    problem(
        "pages.guidedog",
        "Guidedog was not found",
        program,
        "guidedog-pages builds projects with an installed guidedog, and none could be run.",
        &format!(
            "Install Guidedog and put guidedog on PATH, pass --guidedog=PATH, or set \
             {GUIDEDOG_VARIABLE}. To publish a site you built, set site in pages.toml."
        ),
    )
    .exit(4)
}

/// Builds `project` as HTML into `out` with `program`; its report goes to standard
/// error. A build that fails publishes nothing.
fn build(program: &str, c: &Config, project: &Path, out: &Path) -> Result<(), Problem> {
    let mut command = Command::new(program);
    command.args(["build", "-b", "html"]).arg(project).arg(out);
    if c.strict {
        command.arg("-W");
    }
    command.arg("-D").arg(format!("html_baseurl={}", c.site_url));
    // Standard error, so that a script's output stays the outcome.
    command.stdin(Stdio::null()).stdout(Stdio::from(std::io::stderr()));
    let status = command.status().map_err(|_| guidedog_missing(program))?;
    if !status.success() {
        let code = status.code().unwrap_or(1);
        return Err(problem(
            "pages.build",
            "The project did not build",
            &project.display().to_string(),
            &format!(
                "guidedog stopped with status {code}; its report is above. Nothing was \
                 published."
            ),
            "Fix what Guidedog reported, then publish again.",
        ));
    }
    Ok(())
}

/// The site the configuration publishes, frozen: its project built with `program` into
/// the target's build folder ([`build_dir`]), or its built site copied. `progress`
/// hears what is happening.
pub fn freeze_site(
    settings: &Settings,
    c: &Config,
    program: &str,
    dry_run: bool,
    progress: &mut dyn FnMut(&str),
) -> Result<Snapshot, Problem> {
    let project = match &c.source {
        SiteSource::Built(site) => return snapshot(site, &SnapshotOptions::default()),
        SiteSource::Project(p) => p,
    };
    let out = build_dir(settings, c, dry_run)?;
    progress(&format!("Building {} with {program}...", project.display()));
    let built = build(program, c, project, &out);
    // A real publication holds the target's lock; dry runs use folders of their own.
    let mode = if dry_run { SnapshotMode::Adopt } else { SnapshotMode::Borrow };
    let frozen = built.and_then(|()| snapshot(&out, &SnapshotOptions::new(mode)));
    if frozen.is_err() && dry_run {
        let _ = std::fs::remove_dir_all(&out);
    }
    frozen
}
