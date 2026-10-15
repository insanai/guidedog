//! The seam between the command line and the project builder.
//!
//! Every command that builds, cleans, serves or translates a project reaches the
//! project builder only through these functions, so the command line (options, prompts,
//! reports, summaries) is complete before the builder is. `load` is real: it reads and
//! checks conf.toml with gd-conf. The project builder itself (gd-project) arrives in
//! wave 4 of the port; until then `build`, `clean`, `serve` and `intl` return
//! [`not_available`]. The README lists what wave 4 fills in.

use std::path::{Path, PathBuf};

use gd_conf::{Config, LoadOptions};
use gd_env::{BuildOptions, BuildResult, BuilderKind};
use gd_host::Problem;
use gd_report::Report;

use crate::command::IntlCommand;

/// A loaded project. Wave 4: becomes (or wraps) `gd_project::Project`, which also owns
/// the linked plugin registry.
#[derive(Debug)]
pub(crate) struct Project {
    /// The checked settings.
    pub config: Config,
    /// What loading warned about.
    pub warnings: Vec<Report>,
}

/// Loads the project found from `dir` with the command line's options. Wave 4:
/// `gd_project::load(dir, options, linked)`.
pub(crate) fn load(
    dir: &Path,
    options: &LoadOptions,
    _linked: &gd_plugins::Registry,
) -> Result<Project, Problem> {
    let loaded = gd_conf::load_config(dir, options)?;
    Ok(Project { config: loaded.config, warnings: loaded.warnings })
}

/// Builds `project` with `kind`. Never fails outright: the result carries the problem.
/// Wave 4: `gd_project::build(project, kind, options)`.
pub(crate) fn build(project: &Project, kind: BuilderKind, options: &BuildOptions) -> BuildResult {
    let _ = (options, &project.config.project);
    let problem = not_available(&format!("build {}", kind.name()));
    BuildResult {
        exit: problem.exit,
        problem: Some(problem),
        output: options
            .output_dir
            .clone()
            .unwrap_or_else(|| project.config.build_dir.join(kind.name())),
        ..BuildResult::default()
    }
}

/// Removes the contents of the project's build folder. Wave 4: `gd_project::clean`.
pub(crate) fn clean(project: &Project) -> Result<(), Problem> {
    let _ = &project.config.build_dir;
    Err(not_available("clean"))
}

/// Serves the project's HTML output on `port`, calling `rebuild` before answering a
/// request when sources changed (it returns whether the rebuild published); returns
/// when the server stops. Wave 4: gd-project's watch set and `gd_serve`.
pub(crate) fn serve(
    project: &Project,
    port: u16,
    rebuild: &mut dyn FnMut() -> bool,
) -> Result<(), Problem> {
    let _ = (&project.config.build_dir, port, rebuild);
    Err(not_available("serve"))
}

/// What `intl` did with one catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(dead_code, reason = "wave 4 (gd-intl through gd-project) produces these")]
pub(crate) enum IntlOutcome {
    /// `update` created it.
    Created,
    /// `update` merged new templates into it.
    Updated,
    /// `update` found nothing to change.
    Unchanged,
    /// `stat`: what is translated, as `gd_gettext` formats the counts.
    Stats(String),
}

/// One catalog `intl` touched or counted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IntlFile {
    /// The `.po` file.
    pub path: PathBuf,
    /// What happened to it.
    pub outcome: IntlOutcome,
}

/// Runs `intl update` or `intl stat` on `project`: the catalogs it got to, and the
/// problem that stopped it. Wave 4: gd-intl's update and stat.
pub(crate) fn intl(project: &Project, command: &IntlCommand) -> (Vec<IntlFile>, Option<Problem>) {
    let _ = (&project.config.intl.locale_dirs, command);
    (Vec::new(), Some(not_available("intl")))
}

/// The problem of a command whose project builder is not in this version yet.
pub(crate) fn not_available(command: &str) -> Problem {
    Problem::new(
        "cli.not-available",
        "NOT AVAILABLE IN THIS BUILD",
        "",
        format!(
            "guidedog {command} needs the project builder, which this build of Guidedog \
             does not include yet. The project's conf.toml was read and checked."
        ),
        "Convert single files with guidedog convert meanwhile, or use a Guidedog release \
         that includes the project builder.",
        2,
    )
}
