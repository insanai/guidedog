//! sphinx.ext.linkcode's URL template (GDS 0011). Sphinx calls the project's Python
//! function `linkcode_resolve(domain, info)`; Guidedog fills `linkcode_url`, a template
//! with the placeholders of [`LINKCODE_FIELDS`], from what it knows of each object. The
//! folder `{path}` is relative to and the text of `{branch}` are found once per build
//! ([`LinkcodeSite::of`]).

use std::path::{Path, PathBuf};

use gd_report::Report;

use crate::{Config, LINKCODE_FIELDS};

/// What every link of one build shares: the template, the folder `{path}` is relative
/// to, and the branch.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinkcodeSite {
    /// `linkcode_url`; empty: no links.
    pub template: String,
    /// `linkcode_root` (absolute), or the repository's top folder; `None` when neither.
    pub root: Option<PathBuf>,
    /// `linkcode_branch`, or the branch (or commit) the repository has checked out.
    pub branch: Option<String>,
}

/// One object to link: its domain and names, and where it is defined when the source
/// scan found it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinkcodeObject<'a> {
    /// `py`, `c`, `cpp`, `js`, `rust`, `odin`.
    pub domain: &'a str,
    /// The module, crate or package path.
    pub module: &'a str,
    /// The name within the module.
    pub fullname: &'a str,
    /// The source file (absolute), its first and last line; `None` when unknown.
    pub source: Option<(&'a Path, u32, u32)>,
}

impl LinkcodeSite {
    /// The template of `config`, with its root and branch looked up: `linkcode_root`
    /// relative to the source folder, else the nearest folder at or above the source
    /// folder that holds `.git`; `linkcode_branch`, else the repository's `HEAD`.
    pub fn of(config: &Config) -> LinkcodeSite {
        let l = &config.ext.linkcode;
        if l.url.is_empty() || !config.has_extension("sphinx.ext.linkcode") {
            return LinkcodeSite::default();
        }
        let repository = repository_of(&config.source_dir);
        let root = if l.root.is_empty() {
            repository.clone()
        } else {
            Some(crate::clean_path(&config.source_dir.join(&l.root)))
        };
        let branch = if l.branch.is_empty() {
            repository.as_deref().and_then(checked_out)
        } else {
            Some(l.branch.clone())
        };
        LinkcodeSite { template: l.url.clone(), root, branch }
    }

    /// Whether the template uses `{branch}` and no branch was found: the warning
    /// `linkcode.branch`, once per build.
    pub fn branch_report(&self, config: &Config) -> Option<Report> {
        if self.branch.is_some() || !uses_field(&self.template, "branch") {
            return None;
        }
        let report = Report::warning(
            "linkcode.branch",
            "NO BRANCH FOR LINKCODE",
            "linkcode_url uses {branch}, but linkcode_branch is not set and no Git \
             repository holds the source folder, so no [source] links are made.",
        )
        .hint("Set linkcode_branch in conf.toml, such as linkcode_branch = \"main\".");
        Some(Report { path: config.config_path().display().to_string(), ..report })
    }

    /// The URL of `object`, or `None` when the template uses a placeholder it has no
    /// value for. `Err` with the file when the object's file lies outside the root.
    pub fn url(&self, object: &LinkcodeObject<'_>) -> Result<Option<String>, PathBuf> {
        if self.template.is_empty() {
            return Ok(None);
        }
        let path = match (object.source, &self.root) {
            (Some((file, _, _)), Some(root)) => match file.strip_prefix(root) {
                Ok(relative) => Some(slashed(relative)),
                Err(_) if uses_field(&self.template, "path") => return Err(file.to_path_buf()),
                Err(_) => None,
            },
            _ => None,
        };
        let lines = object.source.map(|(_, line, end)| (line.to_string(), end.to_string()));
        let value = |field: &str| -> Option<String> {
            let text = |s: &str| (!s.is_empty()).then(|| s.to_owned());
            match field {
                "domain" => text(object.domain),
                "module" => text(object.module),
                "fullname" => text(object.fullname),
                "path" => path.clone(),
                "line" => lines.as_ref().map(|l| l.0.clone()),
                "end_line" => lines.as_ref().map(|l| l.1.clone()),
                "branch" => self.branch.clone(),
                _ => None,
            }
        };
        Ok(fill(&self.template, value))
    }
}

/// Whether `template` uses the placeholder `{field}`.
fn uses_field(template: &str, field: &str) -> bool {
    let mut used = false;
    fill(template, |f| {
        used |= f == field;
        Some(String::new())
    });
    used
}

/// Fills `template`: `{name}` by `value(name)`, `{{` and `}}` as single braces. `None`
/// when a placeholder has no value or a brace is not closed.
fn fill(template: &str, mut value: impl FnMut(&str) -> Option<String>) -> Option<String> {
    let mut out = String::with_capacity(template.len() + 32);
    let mut rest = template;
    while let Some(i) = rest.find(['{', '}']) {
        out.push_str(&rest[..i]);
        let brace = &rest[i..=i];
        if rest[i + 1..].starts_with(brace) || brace == "}" {
            out.push_str(brace);
            rest = &rest[i + if rest[i + 1..].starts_with(brace) { 2 } else { 1 }..];
            continue;
        }
        let close = rest[i..].find('}')?;
        let field = &rest[i + 1..i + close];
        if !LINKCODE_FIELDS.contains(&field) {
            return None;
        }
        out.push_str(&value(field)?);
        rest = &rest[i + close + 1..];
    }
    out.push_str(rest);
    Some(out)
}

/// A relative path with `/` separators.
fn slashed(path: &Path) -> String {
    let parts: Vec<String> =
        path.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    parts.join("/")
}

/// The nearest folder at or above `dir` that holds `.git`.
fn repository_of(dir: &Path) -> Option<PathBuf> {
    let start = crate::clean_path(dir);
    start.ancestors().find(|d| d.join(".git").exists()).map(Path::to_path_buf)
}

/// The branch the repository at `top` has checked out, from `.git/HEAD`, or the commit
/// when `HEAD` is detached. A `.git` file (a worktree) names the folder that holds HEAD.
fn checked_out(top: &Path) -> Option<String> {
    let git = top.join(".git");
    let folder = if git.is_file() {
        let text = std::fs::read_to_string(&git).ok()?;
        let named = text.lines().find_map(|l| l.strip_prefix("gitdir:"))?.trim();
        top.join(named)
    } else {
        git
    };
    let head = std::fs::read_to_string(folder.join("HEAD")).ok()?;
    let head = head.trim();
    let branch = match head.strip_prefix("ref:") {
        Some(reference) => {
            let reference = reference.trim();
            reference.strip_prefix("refs/heads/").unwrap_or(reference).to_owned()
        }
        None => head.to_owned(),
    };
    (!branch.is_empty()).then_some(branch)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(template: &str) -> LinkcodeSite {
        LinkcodeSite {
            template: template.into(),
            root: Some(PathBuf::from("/repo")),
            branch: Some("main".into()),
        }
    }

    fn square() -> LinkcodeObject<'static> {
        LinkcodeObject {
            domain: "py",
            module: "shapes",
            fullname: "Square.area",
            source: Some((Path::new("/repo/src/shapes.py"), 12, 15)),
        }
    }

    #[test]
    fn placeholders_take_the_objects_values() {
        let s =
            site("https://x.org/{branch}/{path}#L{line}-L{end_line}?{domain}:{module}.{fullname}");
        let url = s.url(&square()).unwrap().unwrap();
        assert_eq!(url, "https://x.org/main/src/shapes.py#L12-L15?py:shapes.Square.area");
        assert_eq!(site("{{{module}}}").url(&square()).unwrap().unwrap(), "{shapes}");
    }

    #[test]
    fn an_object_without_a_value_gets_no_link() {
        let hand_written = LinkcodeObject { source: None, ..square() };
        assert_eq!(site("{path}").url(&hand_written), Ok(None));
        assert_eq!(
            site("/{module}/{fullname}").url(&hand_written),
            Ok(Some("/shapes/Square.area".into()))
        );
        let elsewhere =
            LinkcodeObject { source: Some((Path::new("/other/a.py"), 1, 2)), ..square() };
        assert_eq!(site("{path}").url(&elsewhere), Err(PathBuf::from("/other/a.py")));
        let no_branch = LinkcodeSite { branch: None, ..site("{branch}") };
        assert_eq!(no_branch.url(&square()), Ok(None));
    }

    #[test]
    fn the_branch_is_read_from_head() {
        let dir = std::env::temp_dir().join(format!("gd-conf-linkcode-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        std::fs::write(dir.join(".git/HEAD"), "ref: refs/heads/feature/x\n").unwrap();
        assert_eq!(checked_out(&dir).as_deref(), Some("feature/x"));
        std::fs::write(dir.join(".git/HEAD"), "0123abcd\n").unwrap();
        assert_eq!(checked_out(&dir).as_deref(), Some("0123abcd"));
        assert_eq!(repository_of(&dir.join("docs/sub")), Some(dir.clone()));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
