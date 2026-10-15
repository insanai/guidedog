//! The seam between the command line and the project builder (gd-project): every
//! command that builds, cleans, serves or translates a project reaches it only through
//! these functions.

use std::path::{Path, PathBuf};

use gd_conf::LoadOptions;
use gd_env::{BuildOptions, BuildResult, BuilderKind};
use gd_host::Problem;

use crate::command::{IntlAction, IntlCommand};

/// A loaded project: its configuration, the warnings loading gave, the engine registry
/// and the linked build plugins.
pub(crate) type Project = gd_project::Project;

/// Loads the project found from `dir` with the command line's options; `linked` are
/// the build plugins this program links in.
pub(crate) fn load(
    dir: &Path,
    options: &LoadOptions,
    linked: &gd_plugins::Registry,
) -> Result<Project, Problem> {
    let options = gd_project::LoadOptions {
        config: options.clone(),
        plugins: linked.clone(),
        registry: None,
    };
    gd_project::load(dir, &options)
}

/// Builds `project` with `kind`. Never fails outright: the result carries the problem.
pub(crate) fn build(project: &Project, kind: BuilderKind, options: &BuildOptions) -> BuildResult {
    gd_project::build(project, kind, options)
}

/// Removes the contents of the project's build folder.
pub(crate) fn clean(project: &Project) -> Result<(), Problem> {
    gd_project::clean(project)
}

/// What `serve` asks of the command that runs it: to say where the site is served,
/// and to rebuild it when its sources changed.
pub(crate) trait ServeHost {
    /// The server listens: `site` is the folder served at `url`.
    fn serving(&mut self, site: &Path, url: &str);

    /// Sources changed: reloads and rebuilds the project; the reloaded project when it
    /// built and published, `None` when it did not (the previous site stays).
    fn rebuild(&mut self) -> Option<Project>;
}

/// Serves the project's HTML output (`<build_dir>/html`) on the loopback `port`,
/// rebuilding through `host` before a page is served when a watched source changed
/// (`gd_project::watch_roots`, the build folder skipped). Returns when the server stops
/// (Ctrl+C); fails with `serve.listen` when the port cannot be used.
pub(crate) fn serve(
    project: &Project,
    port: u16,
    host: &mut dyn ServeHost,
) -> Result<(), Problem> {
    let site_dir = project.config.build_dir.join(BuilderKind::Html.name());
    let options =
        gd_serve::ServeOptions { site_dir: site_dir.clone(), port, ..Default::default() };
    let server = gd_serve::Server::bind(options)?;
    host.serving(&site_dir, &server.url());
    let watch = watch_of(project);
    server.run(watch, |watch| match host.rebuild() {
        Some(next) => {
            // A reloaded configuration may watch other folders.
            *watch = watch_of(&next);
            true
        }
        None => false,
    })
}

fn watch_of(project: &Project) -> gd_serve::Watch {
    gd_serve::Watch { roots: gd_project::watch_roots(&project.config), skip: project.watch_skip() }
}

/// What `intl` did with one catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
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
/// problem that stopped it.
pub(crate) fn intl(project: &Project, command: &IntlCommand) -> (Vec<IntlFile>, Option<Problem>) {
    let options = gd_intl::IntlOptions {
        pot_dir: command.pot_dir.clone(),
        locale_dir: command.locale_dir.clone(),
        languages: command.languages.clone(),
        width: command.line_width,
        obsolete: command.obsolete,
    };
    let mut done = Vec::new();
    let result = match command.action {
        IntlAction::Update => gd_intl::intl_update(&project.config, &options, &mut done),
        IntlAction::Stat => gd_intl::intl_stat(&project.config, &options, &mut done),
    };
    let files = done
        .into_iter()
        .map(|file| IntlFile {
            outcome: match (command.action, file.action) {
                (IntlAction::Stat, _) => IntlOutcome::Stats(gd_gettext::format_stats(&file.stats)),
                (_, gd_intl::IntlAction::Create) => IntlOutcome::Created,
                (_, gd_intl::IntlAction::Update) => IntlOutcome::Updated,
                (_, gd_intl::IntlAction::NotChanged) => IntlOutcome::Unchanged,
            },
            path: file.path,
        })
        .collect();
    (files, result.err())
}
