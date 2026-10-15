//! `pages.toml`: where a site comes from and which repository and branch it goes to.

use std::path::{Path, PathBuf};

use gd_plugins::Problem;
use gd_toml::{Table, Value};

use crate::credential::ENV_PROFILE;
use crate::problem::problem;

/// The usual name of a publication's configuration.
pub const CONFIG_FILE: &str = "pages.toml";

const MAX_CONFIG_BYTES: usize = 64 * 1024;
const TOP_KEYS: &[&str] = &[
    "schema",
    "repository",
    "branch",
    "project",
    "site",
    "profile",
    "site_url",
    "strict",
    "wait_seconds",
    "watch",
];
const WATCH_KEYS: &[&str] = &["debounce_ms", "max_wait_ms"];

/// What a publication publishes: a Guidedog project, built as HTML, or a site already
/// built, published as it is. Paths are absolute once read from a configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiteSource {
    /// A project folder (the one holding `conf.toml`).
    Project(PathBuf),
    /// A built site's folder.
    Built(PathBuf),
}

impl SiteSource {
    /// The folder, whichever kind it is.
    pub fn folder(&self) -> &Path {
        match self {
            SiteSource::Project(p) | SiteSource::Built(p) => p,
        }
    }
}

/// A `pages.toml`: where a site comes from and which repository and branch it is
/// published to. It holds no secret: `profile` only names a stored token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The file's absolute path.
    pub path: PathBuf,
    /// The repository's owner, as written.
    pub owner: String,
    /// The repository's name, as written.
    pub repo: String,
    /// The branch the site is published to (`gh-pages` by default).
    pub branch: String,
    /// What is published.
    pub source: SiteSource,
    /// The credential profile (`default` by default).
    pub profile: String,
    /// The address readers open, ending in `/`.
    pub site_url: String,
    /// Build the project with warnings as errors.
    pub strict: bool,
    /// How long publish waits for the deployment, in seconds.
    pub wait_seconds: u32,
    /// `watch` publishes once sources are quiet this long, in milliseconds.
    pub debounce_ms: u32,
    /// `watch` publishes at most this long after the first change, in milliseconds.
    pub max_wait_ms: u32,
}

impl Config {
    /// How long publish waits for the deployment, in seconds: `run_wait` (the run's
    /// choice), or this configuration's when the run leaves it 0; below 0, not at all.
    pub(crate) fn wait_for(&self, run_wait: i64) -> u64 {
        match run_wait {
            w if w < 0 => 0,
            0 => u64::from(self.wait_seconds),
            w => w.unsigned_abs(),
        }
    }

    /// `OWNER/REPO`, as the configuration names the repository.
    pub fn repository(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    /// The configuration's path as problems and hints show it.
    pub fn shown_path(&self) -> String {
        self.path.display().to_string()
    }
}

/// Reads the configuration at `path`. A mistake is a problem (`pages.config`, exit 2)
/// that names its line and the fix; a project folder that does not exist is one too.
pub fn read_config(path: &Path) -> Result<Config, Problem> {
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let shown = abs.display().to_string();
    let text = std::fs::read_to_string(&abs).map_err(|e| {
        problem(
            "pages.config",
            "Cannot read the configuration",
            &path.display().to_string(),
            &e.to_string(),
            "Write one with guidedog pages init, or name it with --config.",
        )
        .exit(2)
    })?;
    if text.len() > MAX_CONFIG_BYTES {
        let hint = "Keep pages.toml to the settings guidedog pages init writes.";
        return Err(config_problem(&shown, 0, "The file is over 64 KiB.", hint));
    }
    let root = gd_toml::parse(&text).map_err(|e| {
        config_problem(&shown, e.line, &e.message, "Fix the TOML syntax on that line.")
    })?;
    known_keys(&shown, &root, "", TOP_KEYS)?;
    let schema = root.get("schema");
    if schema.and_then(Value::as_integer) != Some(1) {
        let hint = "Write schema = 1 at the top of the file.";
        return Err(config_problem(&shown, line_of(schema), "schema must be 1.", hint));
    }
    let mut c = read_settings(&abs, &root)?;
    read_watch(&mut c, &root)?;
    check_config(&c, &root)?;
    Ok(c)
}

/// Reads the top-level settings of `root`, the file at `path`.
fn read_settings(path: &Path, root: &Table) -> Result<Config, Problem> {
    let shown = path.display().to_string();
    let (owner, repo) = read_repository(&shown, root)?;
    let branch = string_setting(&shown, root, "branch")?.unwrap_or("gh-pages").to_string();
    if !is_valid_branch(&branch) {
        return Err(config_problem(
            &shown,
            line_of(root.get("branch")),
            &format!("branch = {branch:?} is not a branch name."),
            "Name a branch of letters, digits, and . _ / -, such as gh-pages.",
        ));
    }
    let source = read_source(path, root)?;
    let profile = string_setting(&shown, root, "profile")?.unwrap_or("default").to_string();
    if !is_valid_profile(&profile) {
        return Err(config_problem(
            &shown,
            line_of(root.get("profile")),
            &format!("profile = {profile:?} is not a profile name."),
            "Use letters, digits, and - _ ., such as profile = \"work\"; the profile env is \
             chosen only with --credential=env.",
        ));
    }
    let site_url = read_site_url(&shown, root, &owner, &repo)?;
    let strict = match root.get("strict") {
        None => true,
        Some(v) => v.as_bool().ok_or_else(|| {
            let hint = "Write strict = true, without quotes.";
            config_problem(&shown, v.line, "strict must be true or false.", hint)
        })?,
    };
    let wait_seconds = integer(&shown, root, "wait_seconds", 300, 0, 86_400)?;
    Ok(Config {
        path: path.to_path_buf(),
        owner,
        repo,
        branch,
        source,
        profile,
        site_url,
        strict,
        wait_seconds,
        debounce_ms: 1000,
        max_wait_ms: 10_000,
    })
}

fn read_repository(shown: &str, root: &Table) -> Result<(String, String), Problem> {
    let Some(repository) = string_setting(shown, root, "repository")? else {
        return Err(config_problem(
            shown,
            0,
            "repository is missing.",
            "Add repository = \"...\" to the file.",
        ));
    };
    match repository.split_once('/') {
        Some((owner, repo)) if is_valid_owner(owner) && is_valid_repo(repo) => {
            Ok((owner.to_string(), repo.to_string()))
        }
        _ => Err(config_problem(
            shown,
            line_of(root.get("repository")),
            &format!("repository = {repository:?} is not OWNER/REPOSITORY."),
            "Write the repository as GitHub shows it, such as repository = \"octo/manual\".",
        )),
    }
}

/// Reads `project` or `site`: exactly one of them, resolved from the file's folder.
fn read_source(path: &Path, root: &Table) -> Result<SiteSource, Problem> {
    let shown = path.display().to_string();
    let base = path.parent().unwrap_or(Path::new("."));
    let project = string_setting(&shown, root, "project")?;
    let site = string_setting(&shown, root, "site")?;
    match (project, site) {
        (Some(p), None) => Ok(SiteSource::Project(base.join(p))),
        (None, Some(s)) => Ok(SiteSource::Built(base.join(s))),
        _ => Err(config_problem(
            &shown,
            line_of(root.get("site").or(root.get("project"))),
            "Name either project or site, not both and not neither.",
            "Write project = \"docs\" to build a project, or site = \"build/site\" to publish a \
             site you built.",
        )),
    }
}

fn read_site_url(shown: &str, root: &Table, owner: &str, repo: &str) -> Result<String, Problem> {
    let Some(url) = string_setting(shown, root, "site_url")? else {
        return Ok(default_site_url(owner, repo));
    };
    if !url.starts_with("https://") {
        return Err(config_problem(
            shown,
            line_of(root.get("site_url")),
            "site_url must be an https URL.",
            "Write the address readers open, such as \"https://octo.github.io/manual/\".",
        ));
    }
    Ok(if url.ends_with('/') { url.to_string() } else { format!("{url}/") })
}

fn read_watch(c: &mut Config, root: &Table) -> Result<(), Problem> {
    let Some(watch) = root.get("watch") else {
        return Ok(());
    };
    let shown = c.shown_path();
    let Some(table) = watch.as_table() else {
        let hint = "Write it as a [watch] section.";
        return Err(config_problem(&shown, watch.line, "watch must be a table.", hint));
    };
    known_keys(&shown, table, "watch.", WATCH_KEYS)?;
    c.debounce_ms = integer(&shown, table, "debounce_ms", c.debounce_ms, 50, 600_000)?;
    c.max_wait_ms =
        integer(&shown, table, "max_wait_ms", c.max_wait_ms, c.debounce_ms, 3_600_000)?;
    Ok(())
}

fn check_config(c: &Config, root: &Table) -> Result<(), Problem> {
    if let SiteSource::Project(dir) = &c.source
        && !dir.is_dir()
    {
        return Err(config_problem(
            &c.shown_path(),
            line_of(root.get("project")),
            &format!("There is no project folder {}.", dir.display()),
            "Point project at the folder that holds conf.toml.",
        ));
    }
    Ok(())
}

/// Where GitHub serves a repository's site: the root of `OWNER.github.io` for the
/// repository of that name, and a folder of it for any other.
pub fn default_site_url(owner: &str, repo: &str) -> String {
    let host = format!("{owner}.github.io").to_lowercase();
    if repo.to_lowercase() == host {
        return format!("https://{host}/");
    }
    format!("https://{host}/{repo}/")
}

/// A new `pages.toml` for `owner/repo`, with a comment on each setting.
pub fn render_config(
    owner: &str,
    repo: &str,
    branch: &str,
    source: &SiteSource,
    profile: &str,
) -> String {
    let mut out = String::from(
        "# Where guidedog pages publishes this documentation. It holds no secret: profile\n\
         # names a token in this machine's credential store.\nschema = 1\n",
    );
    out.push_str(&format!("repository = {}\n", quoted(&format!("{owner}/{repo}"))));
    out.push_str(&format!("branch = {}\n", quoted(branch)));
    match source {
        SiteSource::Project(p) => out.push_str(&format!(
            "# The Guidedog project, built as HTML with warnings as errors.\nproject = {}\n\
             strict = true\n",
            quoted(&p.to_string_lossy())
        )),
        SiteSource::Built(p) => out.push_str(&format!(
            "# The site, already built, published as it is.\nsite = {}\n",
            quoted(&p.to_string_lossy())
        )),
    }
    out.push_str(&format!(
        "profile = {}\n# The address readers open; canonical links use it.\nsite_url = {}\n\
         # How long publish waits for GitHub Pages to deploy, in seconds.\nwait_seconds = 300\n\
         \n# guidedog pages watch publishes once sources are quiet for debounce_ms,\n\
         # and at most max_wait_ms after the first change.\n[watch]\ndebounce_ms = 1000\n\
         max_wait_ms = 10000\n",
        quoted(profile),
        quoted(&default_site_url(owner, repo))
    ));
    out
}

/// `text` as a TOML basic string.
pub(crate) fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    gd_toml::write_string(&mut out, text);
    out
}

fn known_keys(path: &str, t: &Table, prefix: &str, names: &[&str]) -> Result<(), Problem> {
    for (key, value) in t.iter() {
        if !names.contains(&key) {
            return Err(config_problem(
                path,
                value.line,
                &format!("There is no setting {prefix}{key}."),
                &format!("Use one of: {}.", names.join(", ")),
            ));
        }
    }
    Ok(())
}

/// The one-line text setting `key`, or `None` when it is not set.
fn string_setting<'a>(path: &str, t: &'a Table, key: &str) -> Result<Option<&'a str>, Problem> {
    let Some(v) = t.get(key) else {
        return Ok(None);
    };
    match v.as_str() {
        Some(text) if !text.contains(['\r', '\n', '\0']) => Ok(Some(text)),
        _ => Err(config_problem(
            path,
            v.line,
            &format!("{key} must be one line of text."),
            &format!("Write {key} = \"...\" in quotes."),
        )),
    }
}

/// The whole number `key`, from `low` to `high`, or `fallback` when it is not set.
fn integer(
    path: &str,
    t: &Table,
    key: &str,
    fallback: u32,
    low: u32,
    high: u32,
) -> Result<u32, Problem> {
    let Some(v) = t.get(key) else {
        return Ok(fallback);
    };
    match v.as_integer() {
        Some(n) if n >= i64::from(low) && n <= i64::from(high) => Ok(n as u32),
        _ => Err(config_problem(
            path,
            v.line,
            &format!("{key} must be a whole number from {low} to {high}."),
            &format!("Write {key} = {fallback}, without quotes."),
        )),
    }
}

fn line_of(v: Option<&Value>) -> usize {
    v.map_or(0, |v| v.line)
}

/// `pages.config` (exit 2) at `line` of `path` (0: the file as a whole).
fn config_problem(path: &str, line: usize, message: &str, hint: &str) -> Problem {
    let place = if line > 0 { format!("{path}:{line}") } else { path.to_string() };
    problem("pages.config", "The publication settings are not valid", &place, message, hint)
        .exit(2)
}

/// Whether `name` can be a GitHub user or organization: 1 to 39 letters, digits, and
/// single hyphens, not at either end.
pub fn is_valid_owner(name: &str) -> bool {
    let b = name.as_bytes();
    (1..=39).contains(&b.len())
        && b[0] != b'-'
        && b[b.len() - 1] != b'-'
        && !name.contains("--")
        && b.iter().all(|&c| c.is_ascii_alphanumeric() || c == b'-')
}

/// Whether `name` can be a repository: 1 to 100 letters, digits, and `. _ -`, not `.`,
/// `..`, or a name ending in `.git`.
pub fn is_valid_repo(name: &str) -> bool {
    (1..=100).contains(&name.len())
        && name != "."
        && name != ".."
        && name.bytes().all(is_name_byte)
        && !name.to_ascii_lowercase().ends_with(".git")
}

/// Whether `name` is a branch name Git accepts, from a safe alphabet.
pub fn is_valid_branch(name: &str) -> bool {
    let Some(&first) = name.as_bytes().first() else {
        return false;
    };
    name.len() <= 200
        && !b"-/.".contains(&first)
        && !["/", ".lock", "."].iter().any(|bad| name.ends_with(bad))
        && !["..", "//", "/."].iter().any(|bad| name.contains(bad))
        && name.bytes().all(|c| c.is_ascii_alphanumeric() || b"._/-".contains(&c))
}

/// Whether `name` can name a credential profile: 1 to 64 letters, digits, and `. _ -`,
/// and not `env`, which means the environment's token.
pub fn is_valid_profile(name: &str) -> bool {
    (1..=64).contains(&name.len()) && name != ENV_PROFILE && name.bytes().all(is_name_byte)
}

fn is_name_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'.' || c == b'_' || c == b'-'
}

#[cfg(test)]
mod tests;
