//! Where a build's output and its records go (build_paths.odin, generation_paths).

use std::path::{Path, PathBuf};

use gd_conf::Config;
use gd_env::BuilderKind;
use gd_host::Problem;

/// The folders and files one builder's generation uses. Every path is absolute and
/// normalized, so recovering an interrupted commit never depends on the working folder.
///
/// A default output (`<build_dir>/<builder>`) shares `<build_dir>/.doctrees`. A custom
/// output (`-o`, the sphinx-build form) gets a cache folder of its own beside it
/// (`.<name>.guidedog`), so swapping the output never moves its journal and another
/// output never reuses its manifest.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutputPaths {
    /// The published output folder.
    pub out_dir: PathBuf,
    /// The next folder, a sibling: `.<name>.next`, the spare output a build writes to.
    pub next: PathBuf,
    /// Where a commit moves the previous output: `.<name>.old`.
    pub old: PathBuf,
    /// The staging folder earlier versions used (`.<name>.stage`), removed when found.
    pub legacy: PathBuf,
    /// The build folder the records live under: the configuration's, or the custom
    /// output's own cache folder. The caller sets `Config::build_dir` to it.
    pub build_dir: PathBuf,
    /// `<build_dir>/.doctrees`: the environment, objects, manifests and journals.
    pub doctrees: PathBuf,
    /// The builder's manifest: `<doctrees>/<builder>.outputs.json`.
    pub manifest: PathBuf,
    /// The builder's commit journal: `<doctrees>/<builder>.commit.json`.
    pub journal: PathBuf,
}

/// The paths of `kind`'s generation for `config`, with `output_dir` (the sphinx-build
/// form's output folder) instead of `<build_dir>/<builder>` when given. Output folders
/// need not exist: the nearest existing ancestor is resolved and the rest appended.
///
/// Fails with `build.output.layout` (exit 2) when the output folder would contain the
/// source folder (publishing it would replace the sources), and `host.io` when a folder
/// cannot be resolved.
pub fn output_paths(
    config: &Config,
    kind: BuilderKind,
    output_dir: Option<&Path>,
) -> Result<OutputPaths, Problem> {
    let _ = (config, kind, output_dir);
    todo!("wave 1: port configure_output and generation_paths")
}

/// `path` made absolute and normalized, resolving the nearest existing ancestor (so
/// `/var` and `/private/var` agree on macOS) and appending the part that does not
/// exist yet. Fails with `host.io` when no ancestor can be resolved.
pub fn output_absolute(path: &Path) -> Result<PathBuf, Problem> {
    let _ = path;
    todo!("wave 1: port output_absolute")
}

/// Whether a relative path (`/` or the system's separators) stays below the folder it
/// is relative to: not empty, not absolute, and no `..` part.
pub fn contained(rel: &str) -> bool {
    let _ = rel;
    todo!("wave 1: port contained")
}
