//! Paths conf.toml names: made absolute against the source folder and cleaned, never
//! resolved through links (that is `gd_host::confined`'s job).

use std::path::{Component, Path, PathBuf};

use crate::config::Config;

/// Removes `.` components and resolves `..` against the components before it,
/// lexically: it never touches the file system.
pub fn clean_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let climbs =
                    matches!(out.components().next_back(), None | Some(Component::ParentDir));
                if climbs && !out.has_root() {
                    out.push("..");
                } else {
                    out.pop();
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Makes `path` absolute against the working directory without resolving it, so it
/// works for folders a build has not created yet. A path that cannot be made absolute
/// (no working directory) is returned as it is.
pub fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return clean_path(path);
    }
    match std::env::current_dir() {
        Ok(cwd) => clean_path(&cwd.join(path)),
        Err(_) => path.to_path_buf(),
    }
}

/// A path conf.toml gives, relative to the source folder unless absolute, cleaned.
pub fn source_path(config: &Config, path: &str) -> PathBuf {
    clean_path(&config.source_dir.join(path))
}

/// The folders documents may read files from (include, literalinclude, graphs, images):
/// the source folder, then the `include_roots` conf.toml opts into, relative to the
/// source folder, then the `rust_autoapi_crates` folders (so a page may quote the code
/// it documents). Sphinx reads any path; Guidedog reads only these, so a project states
/// what it shares with its documents. The paths are as written, cleaned; pass them to
/// `gd_host::canonical_roots` before `gd_host::confined`. An `--untrusted` build drops
/// the include roots and the crates outside the source folder (`confine_untrusted`).
pub fn readable_roots(config: &Config) -> Vec<PathBuf> {
    let extra = config.include_roots.iter().chain(&config.rust_api.crates);
    let mut roots =
        Vec::with_capacity(1 + config.include_roots.len() + config.rust_api.crates.len());
    roots.push(config.source_dir.clone());
    roots.extend(extra.map(|r| source_path(config, r)));
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_cleaned_without_the_file_system() {
        assert_eq!(clean_path(Path::new("/a/b/../c/./d")), PathBuf::from("/a/c/d"));
        assert_eq!(clean_path(Path::new("/a/../../b")), PathBuf::from("/b"));
        assert_eq!(clean_path(Path::new("../a/../../b")), PathBuf::from("../../b"));
        let mut c = Config::new("/p/src", "/p/_build");
        c.include_roots = vec!["..".into(), "/abs/x/".into()];
        let roots = readable_roots(&c);
        assert_eq!(roots, [PathBuf::from("/p/src"), PathBuf::from("/p"), PathBuf::from("/abs/x")]);
    }
}
