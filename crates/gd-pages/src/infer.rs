//! What the guided publish infers from the working folder: the GitHub repository of
//! the `origin` remote, the nearest Guidedog project, and the GitHub CLI's token.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::config::{is_valid_owner, is_valid_repo};
use crate::credential::{Secret, forget_bytes, is_valid_token};

/// The owner and repository a GitHub remote URL names: `https://github.com/O/R(.git)`,
/// with or without a user, `ssh://git@github.com[:port]/O/R(.git)`, or the scp-like
/// `git@github.com:O/R(.git)`. `None` for any other host or shape.
pub fn parse_github_remote(url: &str) -> Option<(String, String)> {
    let url = url.trim();
    let (host, path) = match url.split_once("://") {
        Some((scheme, rest)) => {
            if !["https", "http", "ssh", "git"].contains(&scheme) {
                return None;
            }
            let (authority, path) = rest.split_once('/')?;
            let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
            (host.split_once(':').map_or(host, |(h, _)| h), path)
        }
        None => {
            let (authority, path) = url.split_once(':')?;
            (authority.rsplit_once('@').map_or(authority, |(_, h)| h), path)
        }
    };
    if !host.eq_ignore_ascii_case("github.com") {
        return None;
    }
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, repo) = path.split_once('/')?;
    (is_valid_owner(owner) && is_valid_repo(repo)).then(|| (owner.to_string(), repo.to_string()))
}

/// The URL of the `origin` remote of the Git repository holding `dir`, read with
/// `git remote get-url origin`; `None` without Git, a repository, or the remote.
pub fn origin_url(dir: &Path) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["remote", "get-url", "origin"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The nearest Guidedog project to `dir`: the first folder holding `conf.toml`, looking
/// in each folder and its `docs` folder from `dir` upwards, and stopping at the Git
/// repository's root (the folder holding `.git`).
pub fn find_project(dir: &Path) -> Option<PathBuf> {
    for folder in dir.ancestors() {
        for candidate in [folder.to_path_buf(), folder.join("docs")] {
            if candidate.join("conf.toml").is_file() {
                return Some(candidate);
            }
        }
        if folder.join(".git").exists() {
            break;
        }
    }
    None
}

/// The token the GitHub CLI (`program`, normally `gh`) holds, from `gh auth token`;
/// `None` when it is not installed, not signed in, or prints something else.
pub fn gh_token(program: &str) -> Option<Secret> {
    let out = Command::new(program)
        .args(["auth", "token"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let ok = out.status.success();
    let token = std::str::from_utf8(&out.stdout).ok().map(|t| Secret::new(t.trim()));
    forget_bytes(out.stdout);
    token.filter(|t| ok && is_valid_token(t.expose()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdir::TestDir;

    #[test]
    fn github_remotes_name_their_repository() {
        let want = Some(("octo".to_string(), "manual".to_string()));
        for url in [
            "https://github.com/octo/manual.git",
            "https://github.com/octo/manual",
            "https://ada@github.com/octo/manual/",
            "git@github.com:octo/manual.git",
            "ssh://git@github.com/octo/manual.git",
            "ssh://git@github.com:22/octo/manual",
        ] {
            assert_eq!(parse_github_remote(url), want, "{url}");
        }
        for url in [
            "https://gitlab.com/octo/manual",
            "git@github.com:octo",
            "file:///x/y",
            "https://github.com/octo/manual/tree",
            "",
        ] {
            assert_eq!(parse_github_remote(url), None, "{url}");
        }
    }

    #[test]
    fn the_nearest_project_is_found_upwards() {
        let dir = TestDir::new("find-project");
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        std::fs::create_dir_all(dir.path().join("src/deep")).unwrap();
        assert_eq!(find_project(&dir.path().join("src/deep")), None);
        std::fs::write(dir.path().join("docs/conf.toml"), "").unwrap();
        assert_eq!(find_project(&dir.path().join("src/deep")), Some(dir.path().join("docs")));
        std::fs::write(dir.path().join("src/conf.toml"), "").unwrap();
        assert_eq!(find_project(&dir.path().join("src/deep")), Some(dir.path().join("src")));
    }
}
