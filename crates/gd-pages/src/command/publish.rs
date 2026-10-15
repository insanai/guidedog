//! `publish`, `status`, and `recover`.

use std::io::IsTerminal;
use std::path::PathBuf;

use gd_plugins::{BuildRequest, LoadOptions, Plugin, Problem, PublisherSession, Snapshot, State};

use super::display::{Console, ConsolePrompt};
use super::{Context, Options};
use crate::build::{build_with_guidedog, freeze_site, guidedog_program};
use crate::config::read_config;
use crate::{GuideOptions, Pages, publish_guided};

/// The program that builds for this command line.
fn guidedog(o: &Options) -> String {
    guidedog_program(&o.guidedog)
}

/// The first line `guidedog --version` prints, or `None` when it cannot run.
pub(crate) fn guidedog_version(o: &Options) -> Option<String> {
    crate::build::guidedog_version(&guidedog(o))
}

/// `pages.guidedog` (exit 4): no `guidedog` could be run.
pub(crate) fn guidedog_missing(o: &Options) -> Problem {
    crate::build::guidedog_missing(&guidedog(o))
}

/// `--wait` as `LoadOptions::wait_seconds`: none given is the configuration's, 0 is
/// not at all.
fn wait_seconds(o: &Options) -> i64 {
    match o.wait {
        None => 0,
        Some(0) => -1,
        Some(n) => i64::from(n),
    }
}

/// The publisher's load options for a run.
fn load_options(ctx: &Context<'_>, dry_run: bool) -> LoadOptions {
    let config = ctx.config_path();
    let mut options = LoadOptions::new(&config.to_string_lossy());
    options.dry_run = dry_run;
    options.wait_seconds = wait_seconds(ctx.o);
    options.cancel = ctx.cancel.clone();
    options
}

fn load(ctx: &Context<'_>, dry_run: bool) -> Result<PublisherSession, Problem> {
    let plugin = Plugin::publisher(Pages::new(ctx.settings.clone()));
    PublisherSession::load(&plugin, &load_options(ctx, dry_run))
}

/// Publishes the site, doing first whatever is missing (see `crate::guide`); a dry run
/// only plans, and needs a configured target.
pub(crate) fn publish(ctx: &Context<'_>, console: &mut Console<'_>) -> i32 {
    if ctx.o.dry_run {
        return publish_once(ctx, console, true).0;
    }
    let interactive = std::io::stdin().is_terminal() && !ctx.o.yes;
    let options = guide_options(ctx, interactive);
    let result = publish_guided(&options, &mut ConsolePrompt { console: &mut *console });
    match result {
        Ok(r) => console.show_result(&r),
        Err(p) => console.report(&p),
    }
}

/// The guided publication's options for a command line; `interactive` when a person
/// may be asked.
pub(crate) fn guide_options(ctx: &Context<'_>, interactive: bool) -> GuideOptions {
    let o = ctx.o;
    GuideOptions {
        settings: ctx.settings.clone(),
        cancel: ctx.cancel.clone(),
        config: ctx.config_path(),
        dir: ctx.dir.clone(),
        repository: o.repository.clone(),
        project: (!o.project.is_empty()).then(|| PathBuf::from(&o.project)),
        branch: o.branch.clone(),
        profile: o.profile.clone(),
        client_id: o.client_id.clone(),
        guidedog: o.guidedog.clone(),
        wait_seconds: wait_seconds(o),
        interactive,
        assume_yes: o.yes,
        verbose: o.verbose,
        ..GuideOptions::default()
    }
}

/// One publication; returns the exit status and the problem's code (empty for none).
pub(crate) fn publish_once(
    ctx: &Context<'_>,
    console: &mut Console<'_>,
    dry_run: bool,
) -> (i32, String) {
    let program = guidedog(ctx.o);
    let staged = stage(ctx, dry_run, &mut |request| {
        console.say(&format!("Building {} with {program}...", request.project.display()));
        build_with_guidedog(&program, request)
    });
    finish(ctx, console, staged, dry_run)
}

/// The loaded session and the frozen site of a publication, its project built by
/// `build`.
pub(crate) fn stage(
    ctx: &Context<'_>,
    dry_run: bool,
    build: &mut dyn FnMut(&BuildRequest) -> Result<(), Problem>,
) -> Result<(PublisherSession, Snapshot), Problem> {
    let session = load(ctx, dry_run)?;
    let c = read_config(&ctx.config_path())?;
    let snap = freeze_site(ctx.settings, &c, dry_run, build)?;
    Ok((session, snap))
}

/// Plans and (unless `dry_run`) publishes what [`stage`] gave, showing the plan and
/// the outcome; returns the exit status and the problem's code (empty for none).
pub(crate) fn finish(
    ctx: &Context<'_>,
    console: &mut Console<'_>,
    staged: Result<(PublisherSession, Snapshot), Problem>,
    dry_run: bool,
) -> (i32, String) {
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
