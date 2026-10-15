//! `guidedog publish --publisher=ID`: the publisher's configuration names a project,
//! which this program builds, or a site; the publisher plans and publishes the frozen
//! site.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use gd_env::BuilderKind;
use gd_plugins::{ChangeKind, LoadOptions, Outcome, Plan, Plugin, Problem, PublisherSession};
use gd_plugins::{Registry, Snapshot, SnapshotMode, SnapshotOptions, State};

use crate::command::{BuildCommand, PublishCommand};
use crate::console::Console;
use crate::plugin::{plugin_problem, registered};

/// Runs `command` with the publisher found in `linked`, then in the machine's registry.
pub(crate) fn publish(
    console: &mut Console<'_>,
    command: &PublishCommand,
    linked: &Registry,
) -> i32 {
    gd_host::catch_interrupts();
    match run(console, command, linked) {
        Ok(status) => status,
        Err(problem) => plugin_problem(console, &problem),
    }
}

fn run(
    console: &mut Console<'_>,
    command: &PublishCommand,
    linked: &Registry,
) -> Result<i32, Problem> {
    let plugin = select_publisher(&command.publisher, linked)?;
    let mut options = LoadOptions::new(&command.config);
    options.dry_run = command.dry_run;
    // 0 is the publisher's own wait; below 0, none (--wait=0).
    options.wait_seconds = match command.wait {
        None => 0,
        Some(0) => -1,
        Some(n) => i64::from(n),
    };
    options.request_timeout = Duration::from_secs(u64::from(command.timeout.unwrap_or(300)));
    options.cancel = gd_plugins::Cancel::from_fn(gd_host::was_interrupted);
    let mut session = PublisherSession::load(&plugin, &options)?;
    let snapshot = freeze_site(console, &session, command, linked)?;
    let plan = session.plan(&snapshot)?;
    show_plan(console, &plan, command.verbose);
    if command.dry_run {
        console.print("  Dry run: nothing was published.\n");
        return Ok(0);
    }
    let outcome = session.publish(&snapshot, &plan);
    Ok(show_outcome(console, &outcome))
}

/// The publisher `id`: linked into this program, or registered on this machine (its
/// program checked against its SHA-256). Build plugins are refused: only publishers
/// publish.
fn select_publisher(id: &str, linked: &Registry) -> Result<Plugin, Problem> {
    if let Some(plugin) = linked.find(id) {
        if !plugin.is_publisher() {
            return Err(build_plugin_problem(id));
        }
        return Ok(plugin.clone());
    }
    let entry = registered(&gd_plugins::machine_path()?, id)?;
    if entry.is_build() {
        return Err(build_plugin_problem(id));
    }
    gd_plugins::verify_entry(&entry)?;
    gd_plugins::process_publisher(&entry)
}

fn build_plugin_problem(id: &str) -> Problem {
    Problem::new(
        "cli.plugin-kind",
        "This plugin extends builds",
        "It has build hooks and no publisher service.",
    )
    .hint(&format!("Select plugins = [\"{id}\"] in conf.toml and run guidedog build."))
    .path(id)
    .exit(2)
}

/// The site to publish, frozen: the configuration's site copied, or its project built
/// as HTML with this program's engine into a private folder the snapshot adopts.
fn freeze_site(
    console: &mut Console<'_>,
    session: &PublisherSession,
    command: &PublishCommand,
    linked: &Registry,
) -> Result<Snapshot, Problem> {
    let source = session.source().clone();
    if !source.site.is_empty() {
        return gd_plugins::snapshot(&source.site, &SnapshotOptions::new(SnapshotMode::Copy));
    }
    let folder = BuildFolder::make(&session.info().id, &command.config, command.dry_run)?;
    let build = BuildCommand {
        dir: PathBuf::from("."),
        kind: BuilderKind::Html,
        load: gd_conf::LoadOptions {
            source: Some(PathBuf::from(&source.project)),
            output: Some(folder.path.clone()),
            overrides: vec![format!("html_baseurl={}", source.base_url)],
            ..Default::default()
        },
        options: gd_env::BuildOptions {
            warnings_as_errors: source.strict,
            output_dir: Some(folder.path.clone()),
            ..Default::default()
        },
        memory: crate::command::MemoryMode::Ask,
        verbose: 0,
        typst: None,
    };
    let status = crate::project::build(console, &build, linked);
    if status != 0 {
        return Err(Problem::new(
            "cli.publish-build",
            "The project did not build",
            "The build failed; its report is above. Nothing was published.",
        )
        .hint("Fix what the build reported, then publish again.")
        .path(&source.project)
        .exit(status));
    }
    let snapshot = gd_plugins::snapshot(&folder.path, &SnapshotOptions::new(SnapshotMode::Adopt))?;
    folder.adopted();
    Ok(snapshot)
}

/// One private build folder per invocation, dry runs included: two hosts using the same
/// configuration never build or publish each other's files. Removed on drop unless a
/// snapshot adopted it.
#[derive(Debug)]
struct BuildFolder {
    path: PathBuf,
    keep: bool,
}

impl BuildFolder {
    /// A new folder: in the temporary folder for a dry run, else in the user's cache
    /// folder under `guidedog/publish`, named after the publisher and configuration.
    fn make(publisher: &str, config: &str, dry_run: bool) -> Result<BuildFolder, Problem> {
        let (parent, stem) = if dry_run {
            (std::env::temp_dir(), "guidedog-publish".to_string())
        } else {
            let digest = gd_sha::sha256(config.as_bytes());
            let name = format!("{publisher}-{}", gd_sha::to_hex(&digest[..6]));
            (cache_dir()?.join("guidedog").join("publish"), name)
        };
        std::fs::create_dir_all(&parent).map_err(|e| io_problem(&parent, &e))?;
        static NEXT: AtomicU32 = AtomicU32::new(0);
        for _ in 0..100 {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.subsec_nanos());
            let path = parent.join(format!("{stem}-{}-{nanos:08x}-{n}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(BuildFolder { path, keep: false }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(io_problem(&path, &e)),
            }
        }
        let e = std::io::Error::from(std::io::ErrorKind::AlreadyExists);
        Err(io_problem(&parent, &e))
    }

    /// The snapshot owns the folder now.
    fn adopted(mut self) {
        self.keep = true;
    }
}

impl Drop for BuildFolder {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

/// The user's cache folder: `$XDG_CACHE_HOME`, or the platform's (`~/.cache`,
/// `~/Library/Caches`, `%LOCALAPPDATA%`).
fn cache_dir() -> Result<PathBuf, Problem> {
    let env = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let found = if cfg!(windows) {
        env("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library").join("Caches"))
    } else {
        env("XDG_CACHE_HOME")
            .filter(|p| p.is_absolute())
            .or_else(|| env("HOME").map(|h| h.join(".cache")))
    };
    found.ok_or_else(|| {
        Problem::new(
            "cli.io",
            "No cache folder",
            "The system names no cache folder for this user.",
        )
        .hint("Set the user's cache folder (XDG_CACHE_HOME on Linux, HOME elsewhere).")
        .exit(4)
    })
}

fn io_problem(path: &Path, error: &std::io::Error) -> Problem {
    crate::plugin::from_host(&gd_host::io_problem(path, error))
}

/// Shows the plan: its summary, the counts, and the first 20 changes (all with
/// `--verbose`).
fn show_plan(console: &mut Console<'_>, plan: &Plan, verbose: bool) {
    let head = if plan.summary.is_empty() { &plan.target } else { &plan.summary };
    let mut out = format!("\n  {}\n", gd_report::escape_terminal(head));
    if plan.unchanged {
        out.push_str("  The target already holds this site: nothing to publish.\n\n");
        console.print(&out);
        return;
    }
    let count = |kind| plan.changes.iter().filter(|c| c.kind == kind).count();
    out.push_str(&format!(
        "  {} added, {} changed, {} removed\n",
        count(ChangeKind::Added),
        count(ChangeKind::Changed),
        count(ChangeKind::Removed)
    ));
    for (i, change) in plan.changes.iter().enumerate() {
        if i == 20 && !verbose {
            let more = plan.changes.len() - 20;
            out.push_str(&format!(
                "    {} and {more} more (--verbose lists them all)\n",
                gd_report::terminal_symbols().ellipsis
            ));
            break;
        }
        let verb = match change.kind {
            ChangeKind::Added => "add",
            ChangeKind::Changed => "change",
            ChangeKind::Removed => "remove",
        };
        out.push_str(&format!("    {verb:<6} {}\n", gd_report::escape_terminal(&change.path)));
    }
    out.push('\n');
    console.print(&out);
}

/// Shows what publishing did and returns the exit status the outcome names.
fn show_outcome(console: &mut Console<'_>, outcome: &Outcome) -> i32 {
    let operation = gd_report::escape_terminal(&outcome.operation).into_owned();
    let url = gd_report::escape_terminal(&outcome.url).into_owned();
    match outcome.state {
        State::Live => {
            console.print(&format!("  Published (operation {operation}): live at {url}\n"))
        }
        State::Unchanged => {
            console.print(&format!("  Already published: {url} holds this site.\n"))
        }
        State::Accepted | State::Pending => console.print(&format!(
            "  Published (operation {operation}); the host has not confirmed it live yet.\n"
        )),
        State::Failed | State::Unknown | State::NotStarted => {}
    }
    if let Some(problem) = &outcome.problem {
        plugin_problem(console, problem);
    }
    outcome.exit_status()
}
