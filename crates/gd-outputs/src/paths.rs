//! Where a build's output and its records go (build_paths.odin, generation_paths).

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use gd_conf::Config;
use gd_env::BuilderKind;
use gd_host::Problem;

use crate::commit::JOURNAL_SUFFIX;

/// The suffix of a builder's manifest in the doctrees folder.
pub(crate) const MANIFEST_SUFFIX: &str = ".outputs.json";

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
    /// The next folder, `<build_dir>/.<name>.next`: the spare output a build writes to.
    pub next: PathBuf,
    /// Where a commit moves the previous output: `<build_dir>/.<name>.old`.
    pub old: PathBuf,
    /// The staging folder earlier versions used (`.<name>.stage`, a sibling of the
    /// output), removed when found.
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

impl OutputPaths {
    /// The paths of a generation that writes `out_dir` and keeps its records in
    /// `build_dir`, for the builder named `builder`. Both folders are taken as given.
    fn of(out_dir: PathBuf, build_dir: PathBuf, builder: &str) -> OutputPaths {
        let doctrees = build_dir.join(".doctrees");
        OutputPaths {
            next: build_dir.join(hidden_name(&out_dir, ".next")),
            old: build_dir.join(hidden_name(&out_dir, ".old")),
            legacy: sibling(&out_dir, ".stage"),
            manifest: doctrees.join(format!("{builder}{MANIFEST_SUFFIX}")),
            journal: doctrees.join(format!("{builder}{JOURNAL_SUFFIX}")),
            out_dir,
            build_dir,
            doctrees,
        }
    }

    /// Where earlier versions kept the spare output and the previous output of a custom
    /// output: `.<name>.next`, `.<name>.old` and `.<name>.old-<n>` beside it, those
    /// that exist. Empty for a default output, whose folders are still there.
    pub(crate) fn sibling_spares(&self) -> Vec<PathBuf> {
        let next = sibling(&self.out_dir, ".next");
        let old = sibling(&self.out_dir, ".old");
        if next == self.next {
            return Vec::new();
        }
        let (Some(parent), Some(old_name)) = (old.parent(), old.file_name()) else {
            return Vec::new();
        };
        let old_name = old_name.to_string_lossy();
        let numbered = |name: &str| {
            let n = name.strip_prefix(old_name.as_ref()).and_then(|rest| rest.strip_prefix('-'));
            n.is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        };
        let entries = gd_host::list_folder(parent).unwrap_or_default();
        let mut found: Vec<PathBuf> = entries
            .iter()
            .filter(|e| numbered(&e.name.to_string_lossy()))
            .map(|e| parent.join(&e.name))
            .collect();
        found.sort();
        found.insert(0, old);
        found.insert(0, next);
        found
    }

    /// The builder's name, from its journal's file name.
    pub(crate) fn builder(&self) -> String {
        let name = self.journal.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
        name.strip_suffix(JOURNAL_SUFFIX).unwrap_or(&name).to_string()
    }
}

/// `.<name><suffix>` for the output folder `out_dir`.
fn hidden_name(out_dir: &Path, suffix: &str) -> OsString {
    let mut file = OsString::from(".");
    file.push(out_dir.file_name().unwrap_or_default());
    file.push(suffix);
    file
}

/// `.<name><suffix>` beside the output folder `out_dir`.
fn sibling(out_dir: &Path, suffix: &str) -> PathBuf {
    let parent = out_dir.parent().map(Path::to_path_buf).unwrap_or_default();
    parent.join(hidden_name(out_dir, suffix))
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
    let default_dir = output_absolute(&config.build_dir.join(kind.name()))?;
    let out_dir = match output_dir {
        Some(dir) => output_absolute(dir)?,
        None => default_dir.clone(),
    };
    let source = output_absolute(&config.source_dir)?;
    if source.starts_with(&out_dir) {
        return Err(layout_problem(&out_dir));
    }
    let build_dir = if out_dir == default_dir {
        output_absolute(&config.build_dir)?
    } else {
        // A custom output keeps its records beside it, outside what a commit swaps.
        sibling(&out_dir, ".guidedog")
    };
    Ok(OutputPaths::of(out_dir, build_dir, kind.name()))
}

fn layout_problem(out_dir: &Path) -> Problem {
    Problem::new(
        "build.output.layout",
        "OUTPUT CONTAINS THE SOURCE",
        &out_dir.display().to_string(),
        "Publishing this folder would replace a directory containing the source documents.",
        "Choose a dedicated output folder below the source folder, or beside it, such as \
         _build/html.",
        2,
    )
}

/// `path` made absolute and normalized, resolving the nearest existing ancestor (so
/// `/var` and `/private/var` agree on macOS) and appending the part that does not
/// exist yet. Fails with `host.io` when no ancestor can be resolved.
pub fn output_absolute(path: &Path) -> Result<PathBuf, Problem> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        let cwd = std::env::current_dir()
            .map_err(|e| gd_host::io_problem(Path::new("working directory"), &e))?;
        cwd.join(path)
    };
    let cleaned = gd_conf::clean_path(&joined);
    let mut ancestor = cleaned.as_path();
    loop {
        let found = gd_host::path_type(ancestor).map_err(|e| gd_host::io_problem(ancestor, &e))?;
        if found.is_some() {
            let resolved =
                std::fs::canonicalize(ancestor).map_err(|e| gd_host::io_problem(ancestor, &e))?;
            let rest = cleaned.strip_prefix(ancestor).unwrap_or(Path::new(""));
            return Ok(if rest.as_os_str().is_empty() { resolved } else { resolved.join(rest) });
        }
        match ancestor.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => ancestor = parent,
            _ => {
                let error = io::Error::new(io::ErrorKind::NotFound, "no folder of it exists");
                return Err(gd_host::io_problem(ancestor, &error));
            }
        }
    }
}

/// Whether a relative path (`/` or the system's separators) stays below the folder it
/// is relative to: not empty, not absolute, and no `..` part.
pub fn contained(rel: &str) -> bool {
    if rel.is_empty() || Path::new(rel).is_absolute() || rel.starts_with(is_separator) {
        return false;
    }
    if cfg!(windows) && Path::new(rel).has_root() {
        return false;
    }
    rel.split(is_separator).all(|part| part != "..")
}

/// Whether `c` separates the parts of a path on this system: `/` everywhere, and `\`
/// on Windows.
pub(crate) fn is_separator(c: char) -> bool {
    c == '/' || (cfg!(windows) && c == '\\')
}

/// A path of a unit's file as the manifest keeps it: its parts joined with `/`, empty
/// and `.` parts left out. `None` when it is empty or leaves the output folder.
pub(crate) fn unit_path(rel: &str) -> Option<String> {
    if !contained(rel) {
        return None;
    }
    let parts: Vec<&str> =
        rel.split(is_separator).filter(|p| !p.is_empty() && *p != ".").collect();
    if parts.is_empty() { None } else { Some(parts.join("/")) }
}
