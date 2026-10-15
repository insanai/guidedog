//! Included files served from approved roots (`Files`), and the confinement checks a
//! project applies to every file it reads.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use guidedoc::{Resource, ResourceProvider, ResourceStatus};

use crate::budget::Ledger;
use crate::files::{MAX_SOURCE_BYTES, read_file, source_text};

/// Provides included files from approved roots. It refuses Docutils' `<standard>`
/// includes, names that leave the roots, and names that pass through a symbolic link:
/// links are checked on the path as written, component by component, before it is
/// resolved; then the resolved path must still lie in a root.
///
/// ```no_run
/// let ledger = gd_host::Ledger::unbounded();
/// let files = gd_host::Files::new(&["docs"]).with_ledger(&ledger);
/// let request = guidedoc::Request { resources: Some(&files), ..Default::default() };
/// # let _ = request;
/// ```
#[derive(Debug)]
pub struct Files<'l> {
    roots: Vec<PathBuf>,
    /// The largest file served, in bytes.
    pub max_bytes: usize,
    /// When set, a name starting with `/` is relative to this folder, as Sphinx reads
    /// `/path` in `include` and `literalinclude`; otherwise such a name is absolute.
    pub project: Option<PathBuf>,
    /// When set, each file's size is charged here before it is read; a file the budget
    /// cannot hold is `ResourceStatus::Limit`. The charges last as long as the ledger.
    pub ledger: Option<&'l Ledger<'l>>,
}

impl Files<'static> {
    /// Serves files below `roots`, resolved now (`canonical_roots`), up to
    /// `MAX_SOURCE_BYTES` each, charging nothing.
    pub fn new<P: AsRef<Path>>(roots: &[P]) -> Files<'static> {
        Files {
            roots: canonical_roots(roots),
            max_bytes: MAX_SOURCE_BYTES,
            project: None,
            ledger: None,
        }
    }
}

impl<'l> Files<'l> {
    /// The same provider, charging each file to `ledger`, which it borrows.
    pub fn with_ledger<'m>(self, ledger: &'m Ledger<'m>) -> Files<'m>
    where
        'l: 'm,
    {
        Files { ledger: Some(ledger), ..self }
    }

    /// The same provider, reading names that start with `/` below `project`.
    pub fn with_project(self, project: impl Into<PathBuf>) -> Files<'l> {
        Files { project: Some(project.into()), ..self }
    }

    /// The resolved roots.
    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// Resolves `name` as written in source `from`; `None` for a name never served.
    fn written_path(&self, from: &str, name: &str) -> Option<PathBuf> {
        if name.is_empty() || name.starts_with('<') {
            return None;
        }
        let base = if from == "-" {
            std::env::current_dir().ok().and_then(|d| d.canonicalize().ok())?
        } else {
            Path::new(from).parent().map(Path::to_path_buf).unwrap_or_default()
        };
        let rooted = name.strip_prefix('/').or_else(|| name.strip_prefix('\\'));
        let joined = match (&self.project, rooted) {
            (Some(project), Some(rest)) => project.join(rest),
            _ => base.join(name),
        };
        Some(clean(&joined))
    }
}

impl ResourceProvider for Files<'_> {
    fn provide(&self, from: &str, name: &str) -> Result<Resource, ResourceStatus> {
        let written = self.written_path(from, name).ok_or(ResourceStatus::Refused)?;
        let root = containing_root(&self.roots, &written).ok_or(ResourceStatus::Refused)?;
        if passes_link(root, &written) {
            return Err(ResourceStatus::Refused);
        }
        let path = written.canonicalize().map_err(|_| ResourceStatus::Missing)?;
        if containing_root(&self.roots, &path).is_none() {
            return Err(ResourceStatus::Refused);
        }
        let info = std::fs::symlink_metadata(&path).map_err(|_| ResourceStatus::Missing)?;
        if !info.is_file() || info.len() > self.max_bytes as u64 {
            return Err(ResourceStatus::Refused);
        }
        let data = match read_file(&path, self.ledger, self.max_bytes) {
            Ok(data) => data,
            Err(p) if p.code == "host.budget" => return Err(ResourceStatus::Limit),
            Err(_) => return Err(ResourceStatus::Missing),
        };
        let canonical = path.to_str().ok_or(ResourceStatus::Refused)?.to_string();
        let text = source_text(data, &canonical).map_err(|_| ResourceStatus::NotUtf8)?;
        Ok(Resource { text: Arc::from(text), canonical })
    }
}

/// Resolves `roots` to absolute paths with every link followed, as `confined` compares
/// paths with them; a root that cannot be resolved is made absolute as written (it then
/// matches nothing that exists through a link, so it never widens what is allowed).
pub fn canonical_roots<P: AsRef<Path>>(roots: &[P]) -> Vec<PathBuf> {
    roots
        .iter()
        .map(|r| {
            let r = r.as_ref();
            r.canonicalize().unwrap_or_else(|_| clean(&std::path::absolute(r).unwrap_or_default()))
        })
        .collect()
}

/// Reports whether `path` (a document, an image, a template a project reads) lies in one
/// of the canonical `roots`: written below a root with no link on the way, or below a
/// root once every link is resolved.
pub fn confined(roots: &[PathBuf], path: &Path) -> bool {
    let written = clean(path);
    if let Some(root) = containing_root(roots, &written)
        && !passes_link(root, &written)
    {
        return true;
    }
    match written.canonicalize() {
        Ok(resolved) => containing_root(roots, &resolved).is_some(),
        Err(_) => false,
    }
}

/// Where a path lies with respect to the roots a project reads from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Confinement {
    /// Confined: below a root once every link is followed.
    Inside,
    /// The path as written is below no root.
    Outside,
    /// Written below a readable folder, but a link on the way leads out of the roots.
    ThroughLink,
}

/// Tells whether `confined` accepts `path`, and when it does not, whether the path
/// itself or a link on its way leads outside, so a refusal can say which.
pub fn confinement(roots: &[PathBuf], path: &Path) -> Confinement {
    if confined(roots, path) {
        return Confinement::Inside;
    }
    let written = clean(path);
    for folder in written.ancestors().skip(1) {
        if folder.as_os_str().is_empty() {
            break;
        }
        if confined(roots, folder) {
            return Confinement::ThroughLink;
        }
    }
    Confinement::Outside
}

/// The root of `roots` that `path` is written below (or is), comparing whole
/// components and following no link; `confined` is the check that follows them.
pub fn containing_root<'r>(roots: &'r [PathBuf], path: &Path) -> Option<&'r Path> {
    roots.iter().map(PathBuf::as_path).find(|root| path.starts_with(root))
}

/// Reports whether any component of `path` below `root`, the file included, is a link.
fn passes_link(root: &Path, path: &Path) -> bool {
    let Ok(rest) = path.strip_prefix(root) else { return false };
    let mut current = root.to_path_buf();
    for part in rest.components() {
        current.push(part);
        match std::fs::symlink_metadata(&current) {
            Ok(info) if info.file_type().is_symlink() => return true,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
    false
}

/// Normalizes `path` lexically: `.` dropped, `..` removing the component before it (or
/// kept at the start of a relative path), as Go's and Odin's `clean` do.
pub(crate) fn clean(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                let last = out.components().next_back();
                match last {
                    Some(Component::Normal(_)) => {
                        out.pop();
                    }
                    Some(Component::RootDir | Component::Prefix(_)) => {}
                    _ => out.push(".."),
                }
            }
            other => out.push(other),
        }
    }
    out
}
