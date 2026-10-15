//! A loaded project: its checked configuration, the engine registry its documents are
//! read and rendered with, and the build plugins linked into the program.

use std::path::{Path, PathBuf};

use gd_conf::{Config, Digest};
use gd_host::Problem;
use gd_report::Report;
use guidedoc::Registry;

/// Where a project is and what it is built with. Fill it with struct-update syntax:
/// `LoadOptions { plugins: linked, ..Default::default() }`.
#[derive(Debug, Clone, Default)]
pub struct LoadOptions {
    /// The command line's say: `-c`, `-C`, `-D`, the sphinx-build form's folders.
    pub config: gd_conf::LoadOptions,
    /// The build plugins linked into this program, looked up before the machine
    /// registry (`plugins = [...]` in conf.toml selects them).
    pub plugins: gd_plugins::Registry,
    /// The readers and renderers; `None`: `guidedoc_defaults::registry()`.
    pub registry: Option<Registry>,
}

/// A loaded documentation project. Builds borrow it and never change it (each works on
/// its own copy of the configuration), so one project may be built any number of times.
#[derive(Debug, Clone)]
pub struct Project {
    /// The checked settings.
    pub config: Config,
    /// What loading warned about; every build reports them again.
    pub warnings: Vec<Report>,
    /// The digest of conf.toml's text and the `-D` overrides: what every output uses.
    pub settings: Digest,
    /// The readers and renderers.
    pub registry: Registry,
    /// The linked build plugins.
    pub plugins: gd_plugins::Registry,
}

/// Loads the project found from `dir` (conf.toml there, in `dir/source`, or in the
/// nearest parent folder that has one) or the folders `options` names.
///
/// Fails with `core.registry.invalid` (exit 2) when the engine registry has an adapter
/// without an ID or version, or one ID twice, and with gd-conf's problems
/// (`config.invalid`, `project.missing`, ...) when the configuration cannot be read.
/// Whether the registry has the readers the documents need is known when a build finds
/// them (`build.reader`).
pub fn load(dir: &Path, options: &LoadOptions) -> Result<Project, Problem> {
    let registry = options.registry.clone().unwrap_or_else(guidedoc_defaults::registry);
    if let Err(fail) = guidedoc::validate_registry(&registry) {
        return Err(registry_problem(&fail));
    }
    let loaded = gd_conf::load_config(dir, &options.config)?;
    Ok(Project {
        config: loaded.config,
        warnings: loaded.warnings,
        settings: loaded.settings,
        registry,
        plugins: options.plugins.clone(),
    })
}

fn registry_problem(fail: &guidedoc::Fail) -> Problem {
    let ws = guidedoc::Workspace::new();
    let report = gd_host::describe(&ws, &fail.diagnostic);
    Problem::new(
        fail.diagnostic.code,
        fail.diagnostic.title,
        "",
        report.message,
        report.hints.concat(),
        gd_host::exit_code(fail.status),
    )
}

/// Removes everything in the project's build folder, as `make clean` does for Sphinx,
/// and leaves the folder itself; a missing one is nothing to clean. Fails with
/// `project.clean-root` when the build path is not a folder and `host.io` when an entry
/// cannot be removed.
pub fn clean(project: &Project) -> Result<(), Problem> {
    gd_outputs::clean(&project.config)
}

/// The paths `guidedog serve` watches for changes: the source folder, the Odin package
/// folders and Rust crates the API pages are made from, and the static, extra,
/// template, include, autodoc and locale folders outside the source folder (those
/// inside it are watched with it). The build folder is never one of them; skip it
/// while watching (see [`Project::watch_skip`]).
pub fn watch_roots(config: &Config) -> Vec<PathBuf> {
    let mut roots = vec![config.source_dir.clone()];
    let api = config.odin_api.dirs.iter().chain(config.odin_api.collections.values());
    for dir in api.chain(&config.rust_api.crates) {
        push_new(&mut roots, gd_conf::source_path(config, dir));
    }
    let outside = [
        &config.html.static_path,
        &config.html.extra_path,
        &config.templates_path,
        &config.include_roots,
        &config.autodoc.source_paths,
        &config.intl.locale_dirs,
    ];
    for dir in outside.into_iter().flatten() {
        let path = gd_conf::source_path(config, dir);
        if !path.starts_with(&config.source_dir) {
            push_new(&mut roots, path);
        }
    }
    roots.retain(|r| *r != config.build_dir);
    roots
}

fn push_new(roots: &mut Vec<PathBuf>, path: PathBuf) {
    if !roots.contains(&path) {
        roots.push(path);
    }
}

impl Project {
    /// The paths a watch skips: the build folder, whose changes are the build's own.
    pub fn watch_skip(&self) -> Vec<PathBuf> {
        vec![self.config.build_dir.clone()]
    }
}
