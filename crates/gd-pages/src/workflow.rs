//! The GitHub Actions route: a `.github/workflows/docs-pages.yml` that builds the site
//! with a released, checksum-verified Guidedog and deploys it with GitHub's official
//! Pages actions, and a check of an existing one.
//!
//! The security posture is the manual's: read-only permissions by default, Pages and
//! OIDC permissions only in the deploy job, `pull_request` (never
//! `pull_request_target`), `persist-credentials: false`, deployment only from the
//! source branch, action commits pinned by SHA with their versions in comments, and
//! the release archive checked against its SHA-256.

use std::time::Duration;

use gd_plugins::Problem;

use crate::problem::problem;

/// Where the generated workflow is saved, from the repository's root.
pub const WORKFLOW_PATH: &str = ".github/workflows/docs-pages.yml";
/// Where Guidedog releases come from by default.
pub const RELEASE_REPOSITORY: &str = "insanai/guidedog";
/// The web root release files are downloaded from.
pub const RELEASE_ROOT: &str = "https://github.com";
/// The release archive the workflow installs.
pub const ARCHIVE: &str = "guidedog-linux-amd64.tar.gz";

/// What the workflow's `GUIDEDOG_SHA256` holds until a real checksum is given; the
/// workflow's checksum test fails on it, so it can never install an unchecked archive.
pub const SHA256_PLACEHOLDER: &str = "REPLACE-WITH-THE-SHA-256-OF-guidedog-linux-amd64.tar.gz";

const TEMPLATE: &str = include_str!("workflow/docs-pages.yml");

/// An action the workflow uses, pinned to a reviewed commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionPin {
    /// `owner/name`.
    pub action: &'static str,
    /// The full commit SHA.
    pub commit: &'static str,
    /// The release tag the commit is, for people.
    pub version: &'static str,
}

/// The actions' pinned commits, as the manual lists them. Updating one is a security
/// review: check the release, its commit and its changes before changing a line here.
pub const ACTION_PINS: [ActionPin; 4] = [
    ActionPin {
        action: "actions/checkout",
        commit: "3d3c42e5aac5ba805825da76410c181273ba90b1",
        version: "v7.0.1",
    },
    ActionPin {
        action: "actions/configure-pages",
        commit: "45bfe0192ca1faeb007ade9deae92b16b8254a0d",
        version: "v6.0.0",
    },
    ActionPin {
        action: "actions/upload-pages-artifact",
        commit: "fc324d3547104276b827a68afc52ff2a11cc49c9",
        version: "v5.0.0",
    },
    ActionPin {
        action: "actions/deploy-pages",
        commit: "368f82528645a54fb793d4d04e342629a3f51346",
        version: "v5.0.1",
    },
];

/// What a workflow is made for. The default builds `docs` from `main` with this
/// program's Guidedog version, without a checksum (a placeholder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowOptions {
    /// The project folder, from the repository's root.
    pub project: String,
    /// The branch that deploys.
    pub branch: String,
    /// The Guidedog release installed.
    pub version: String,
    /// The release archive's SHA-256; `None` writes [`SHA256_PLACEHOLDER`].
    pub sha256: Option<String>,
    /// The repository releases come from (`OWNER/REPO`).
    pub release_repository: String,
}

impl Default for WorkflowOptions {
    fn default() -> WorkflowOptions {
        WorkflowOptions {
            project: "docs".to_string(),
            branch: "main".to_string(),
            version: crate::VERSION.to_string(),
            sha256: None,
            release_repository: RELEASE_REPOSITORY.to_string(),
        }
    }
}

fn pin_text(action: &str) -> String {
    let pin = ACTION_PINS.iter().find(|p| p.action == action);
    pin.map(|p| format!("{}@{} # {}", p.action, p.commit, p.version)).unwrap_or_default()
}

/// The workflow for `o`. Its fields are written as they are: check paths and names
/// with [`is_safe_workflow_name`] first.
pub fn render_workflow(o: &WorkflowOptions) -> String {
    let sha = o.sha256.as_deref().unwrap_or(SHA256_PLACEHOLDER);
    let note = if o.sha256.is_some() {
        String::new()
    } else {
        "# PLACEHOLDER: set GUIDEDOG_SHA256 (from the release's SHA256SUMS) before use;\n\
         # until then the checksum test fails and nothing is installed.\n"
            .to_string()
    };
    TEMPLATE
        .replace("@NOTE@", &note)
        .replace("@BRANCH@", &o.branch)
        .replace("@PROJECT@", &o.project)
        .replace("@VERSION@", &o.version)
        .replace("@SHA256@", sha)
        .replace("@RELEASES@", &o.release_repository)
        .replace("@CHECKOUT@", &pin_text("actions/checkout"))
        .replace("@CONFIGURE@", &pin_text("actions/configure-pages"))
        .replace("@UPLOAD@", &pin_text("actions/upload-pages-artifact"))
        .replace("@DEPLOY@", &pin_text("actions/deploy-pages"))
}

/// Whether `text` can be written into the workflow as a path, branch, version or
/// repository: letters, digits, and `. _ / -`, not starting with `-` or `/`, no `..`.
pub fn is_safe_workflow_name(text: &str) -> bool {
    !text.is_empty()
        && !text.starts_with(['-', '/'])
        && !text.contains("..")
        && text.bytes().all(|c| c.is_ascii_alphanumeric() || b"._/-".contains(&c))
}

/// Reads the SHA-256 of [`ARCHIVE`] in release `version` from its `SHA256SUMS`, at
/// `ROOT/REPOSITORY/releases/download/vVERSION/SHA256SUMS` (`root` is normally
/// [`RELEASE_ROOT`]). Fails with `pages.release` when it cannot be read or does not
/// list the archive.
pub fn release_sha256(root: &str, repository: &str, version: &str) -> Result<String, Problem> {
    let url = format!("{root}/{repository}/releases/download/v{version}/SHA256SUMS");
    let options = gd_fetch::Options {
        timeout: Some(Duration::from_secs(60)),
        max_bytes: 1024 * 1024,
        ..gd_fetch::Options::default()
    };
    let hint = "Check the version, or pass --sha256 with the checksum from the release page \
                (or --offline to write a placeholder).";
    let fetched = gd_fetch::get(&url, &options).map_err(|e| {
        problem(
            "pages.release",
            "Cannot read the release checksums",
            &url,
            &format!("{} ({})", e.detail, e.code()),
            hint,
        )
        .exit(4)
    })?;
    let text = String::from_utf8_lossy(&fetched.data);
    let found = text.lines().find_map(|line| {
        let (sum, name) = line.split_once(char::is_whitespace)?;
        let name = name.trim_start().trim_start_matches('*');
        let valid = sum.len() == 64 && sum.bytes().all(|c| c.is_ascii_hexdigit());
        (name == ARCHIVE && valid).then(|| sum.to_ascii_lowercase())
    });
    found.ok_or_else(|| {
        problem(
            "pages.release",
            "The release does not list its archive",
            &url,
            &format!("SHA256SUMS has no checksum for {ARCHIVE}."),
            hint,
        )
        .exit(4)
    })
}

/// Checks the workflow `text` (read from `path`) against what `o` would write: an
/// older Guidedog, a missing or placeholder checksum, an action not pinned to a commit
/// or pinned to another commit than the reviewed one, `pull_request_target`, and a
/// checkout that keeps its credentials. Returns one problem per finding, each with
/// its line and a hint; none means the workflow is current.
pub fn check_workflow(path: &str, text: &str, o: &WorkflowOptions) -> Vec<Problem> {
    let mut found = Vec::new();
    let at = |line: usize| format!("{path}:{line}");
    let mut version = None;
    for (i, line) in text.lines().enumerate() {
        let line_no = i + 1;
        let trimmed = line.trim();
        if let Some(v) = trimmed.strip_prefix("GUIDEDOG_VERSION:") {
            version = Some((line_no, v.trim().trim_matches(['"', '\'']).to_string()));
        }
        if let Some(sum) = trimmed.strip_prefix("GUIDEDOG_SHA256:") {
            let sum = sum.trim().trim_matches(['"', '\'']);
            if sum.len() != 64 || !sum.bytes().all(|c| c.is_ascii_hexdigit()) {
                found.push(problem(
                    "pages.workflow-sha256",
                    "The archive is not verified",
                    &at(line_no),
                    "GUIDEDOG_SHA256 is not a SHA-256, so the build cannot \
                    install Guidedog.",
                    "Run guidedog-pages workflow --write --force, or \
                    pass --sha256 with the checksum from the release's SHA256SUMS.",
                ));
            }
        }
        if let Some(uses) = trimmed.strip_prefix("- uses:").or(trimmed.strip_prefix("uses:")) {
            found.extend(check_pin(&at(line_no), uses));
        }
        if trimmed.contains("pull_request_target") {
            found.push(problem(
                "pages.workflow-trigger",
                "The workflow runs pull requests \
                with secrets",
                &at(line_no),
                "pull_request_target runs with the repository's \
                permissions on code from forks.",
                "Use pull_request, as the generated \
                workflow does.",
            ));
        }
    }
    found.extend(check_version(path, version, &o.version));
    if text.contains("actions/checkout@") && !text.contains("persist-credentials: false") {
        found.push(problem(
            "pages.workflow-credentials",
            "The checkout keeps its token",
            path,
            "actions/checkout leaves the job's token in the repository's Git settings.",
            "Add persist-credentials: false under the checkout step's with:.",
        ));
    }
    found
}

fn check_version(path: &str, version: Option<(usize, String)>, current: &str) -> Vec<Problem> {
    let Some((line, version)) = version else {
        return vec![problem(
            "pages.workflow-version",
            "No Guidedog version",
            path,
            "The workflow sets no GUIDEDOG_VERSION, so it does not install a pinned Guidedog.",
            "Generate it again with guidedog-pages workflow --write --force.",
        )];
    };
    if version_key(&version) >= version_key(current) {
        return Vec::new();
    }
    vec![problem(
        "pages.workflow-outdated",
        "The workflow installs an older Guidedog",
        &format!("{path}:{line}"),
        &format!(
            "It installs Guidedog {version}; this is \
        {current}."
        ),
        &format!(
            "Run guidedog-pages workflow --write --force to update it to \
        {current} with its verified checksum."
        ),
    )]
}

/// A version's numbers, compared as numbers (`0.10.0` is after `0.9.2`).
fn version_key(version: &str) -> Vec<u64> {
    version.trim_start_matches('v').split('.').map(|n| n.parse().unwrap_or(0)).collect()
}

fn check_pin(at: &str, uses: &str) -> Option<Problem> {
    let (reference, comment) = uses.split_once('#').unwrap_or((uses, ""));
    let (action, commit) = reference.trim().split_once('@')?;
    let is_sha = commit.len() == 40 && commit.bytes().all(|c| c.is_ascii_hexdigit());
    if !is_sha {
        return Some(problem(
            "pages.workflow-unpinned",
            "An action is not pinned",
            at,
            &format!(
                "{action}@{commit} names a tag or branch, which can be moved to other \
            code."
            ),
            &format!("Pin it to a reviewed commit SHA{}.", known_pin(action)),
        ));
    }
    let pin = ACTION_PINS.iter().find(|p| p.action == action)?;
    if pin.commit == commit {
        return None;
    }
    Some(problem(
        "pages.workflow-pin",
        "An action is pinned to another commit",
        at,
        &format!(
            "{action} is pinned to {commit} ({}); this guidedog-pages was reviewed with \
        {} ({}).",
            comment.trim(),
            pin.commit,
            pin.version
        ),
        &format!(
            "Review the change, \
        then pin it{}, or regenerate with guidedog-pages workflow --write --force.",
            known_pin(action)
        ),
    ))
}

fn known_pin(action: &str) -> String {
    let text = pin_text(action);
    if text.is_empty() { String::new() } else { format!(", such as uses: {text}") }
}
