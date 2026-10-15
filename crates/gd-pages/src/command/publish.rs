//! `publish`, `status`, and `recover`, and building a project with the installed
//! `guidedog`.

use std::path::Path;
use std::process::{Command, Stdio};

use gd_plugins::{
    LoadOptions, Plugin, Problem, PublisherSession, Snapshot, SnapshotMode, SnapshotOptions,
    State, snapshot,
};

use super::display::Console;
use super::{Context, Options};
use crate::config::{Config, SiteSource, read_config};
use crate::problem::problem;
use crate::{Pages, build_dir};

/// Names the `guidedog` that builds a project when `--guidedog` does not.
pub const GUIDEDOG_VARIABLE: &str = "GUIDEDOG_PAGES_GUIDEDOG";

/// The program that builds: `--guidedog`, `$GUIDEDOG_PAGES_GUIDEDOG`, or `guidedog` on
/// `PATH`.
fn guidedog(o: &Options) -> String {
    if !o.guidedog.is_empty() {
        return o.guidedog.clone();
    }
    std::env::var(GUIDEDOG_VARIABLE)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "guidedog".to_string())
}

/// The first line `guidedog --version` prints, or `None` when it cannot run.
pub(crate) fn guidedog_version(o: &Options) -> Option<String> {
    let out = Command::new(guidedog(o)).arg("--version").stdin(Stdio::null()).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Some(text.lines().next().unwrap_or_default().trim().to_string())
}

/// `pages.guidedog` (exit 4): no `guidedog` could be run.
pub(crate) fn guidedog_missing(o: &Options) -> Problem {
    problem(
        "pages.guidedog",
        "Guidedog was not found",
        &guidedog(o),
        "guidedog-pages builds projects with an installed guidedog, and none could be run.",
        &format!(
            "Install Guidedog and put guidedog on PATH, pass --guidedog=PATH, or set \
            {GUIDEDOG_VARIABLE}. To publish a site you built, set site in pages.toml."
        ),
    )
    .exit(4)
}

/// Builds `project` as HTML into `out` with the installed `guidedog`. A build that
/// fails publishes nothing.
fn build(
    o: &Options,
    console: &mut Console<'_>,
    c: &Config,
    project: &Path,
    out: &Path,
) -> Result<(), Problem> {
    let program = guidedog(o);
    let mut command = Command::new(&program);
    command.args(["build", "-b", "html"]).arg(project).arg(out);
    if c.strict {
        command.arg("-W");
    }
    command.arg("-D").arg(format!("html_baseurl={}", c.site_url));
    // Its report goes to standard error, so that a script's output stays the outcome.
    command.stdin(Stdio::inherit()).stdout(Stdio::from(std::io::stderr()));
    console.say(&format!("Building {} with {program}...", project.display()));
    let status = command.status().map_err(|_| guidedog_missing(o))?;
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

/// The site a session publishes: its project built, or its site copied.
fn freeze(
    ctx: &Context<'_>,
    console: &mut Console<'_>,
    c: &Config,
    dry_run: bool,
) -> Result<Snapshot, Problem> {
    let project = match &c.source {
        SiteSource::Built(site) => return snapshot(site, &SnapshotOptions::default()),
        SiteSource::Project(p) => p,
    };
    let out = build_dir(ctx.settings, c, dry_run)?;
    let built = build(ctx.o, console, c, project, &out);
    // A real publication holds the target's lock; dry runs use folders of their own.
    let mode = if dry_run { SnapshotMode::Adopt } else { SnapshotMode::Borrow };
    let frozen = built.and_then(|()| snapshot(&out, &SnapshotOptions::new(mode)));
    if frozen.is_err() && dry_run {
        let _ = std::fs::remove_dir_all(&out);
    }
    frozen
}

/// The publisher's load options for a run.
fn load_options(ctx: &Context<'_>, dry_run: bool) -> LoadOptions {
    let wait_seconds = match ctx.o.wait {
        None => 0,
        Some(0) => -1,
        Some(n) => i64::from(n),
    };
    let mut options = LoadOptions::new(&ctx.o.config);
    options.dry_run = dry_run;
    options.wait_seconds = wait_seconds;
    options.cancel = ctx.cancel.clone();
    options
}

fn load(ctx: &Context<'_>, dry_run: bool) -> Result<PublisherSession, Problem> {
    let plugin = Plugin::publisher(Pages::new(ctx.settings.clone()));
    PublisherSession::load(&plugin, &load_options(ctx, dry_run))
}

/// Builds or copies the site, plans, shows the plan, and publishes it.
pub(crate) fn publish(ctx: &Context<'_>, console: &mut Console<'_>) -> i32 {
    publish_once(ctx, console, ctx.o.dry_run).0
}

/// One publication; returns the exit status and the problem's code (empty for none).
pub(crate) fn publish_once(
    ctx: &Context<'_>,
    console: &mut Console<'_>,
    dry_run: bool,
) -> (i32, String) {
    let staged = load(ctx, dry_run).and_then(|s| {
        let c = read_config(Path::new(&ctx.o.config))?;
        let snap = freeze(ctx, console, &c, dry_run)?;
        Ok((s, snap))
    });
    let (mut session, snap) = match staged {
        Ok(found) => found,
        Err(p) => return (console.report(&p), p.code.clone()),
    };
    let plan = match session.plan(&snap) {
        Ok(plan) => plan,
        Err(p) => return (console.report(&p), p.code.clone()),
    };
    console.show_plan(&plan, ctx.o.verbose);
    if dry_run {
        console.success("Dry run: nothing was published");
        return (0, String::new());
    }
    if !plan.unchanged {
        console.say("Publishing...");
    }
    let r = session.publish(&snap, &plan);
    let code = r.problem.as_ref().map(|p| p.code.clone()).unwrap_or_default();
    (console.show_result(&r), code)
}

/// Reads the deployment of the last publication, or of `--operation`.
pub(crate) fn status(ctx: &Context<'_>, console: &mut Console<'_>) -> i32 {
    match load(ctx, true) {
        Ok(mut s) => {
            let r = s.status(&ctx.o.operation);
            console.show_result(&r)
        }
        Err(p) => console.report(&p),
    }
}

/// Settles a publication whose answer was lost.
pub(crate) fn recover(ctx: &Context<'_>, console: &mut Console<'_>) -> i32 {
    let mut s = match load(ctx, false) {
        Ok(s) => s,
        Err(p) => return console.report(&p),
    };
    let r = s.recover();
    if r.state == State::Unchanged && r.operation.is_empty() {
        console.success("Nothing to recover: no publication of this target is unsettled");
        return 0;
    }
    console.show_result(&r)
}
