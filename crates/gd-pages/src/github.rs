//! GitHub's REST API: the token's account, the repository, its Pages settings and
//! builds, and the branch's commit and files.

use std::collections::HashMap;

use gd_fetch::Method;
use gd_json::{Object, Value};
use gd_plugins::Problem;

use crate::client::{Call, Client};
use crate::problem::problem;

/// The GitHub account a token belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Account {
    /// The login, such as `octocat`.
    pub login: String,
    /// The display name; may be empty.
    pub name: String,
    /// The account's GitHub ID.
    pub id: i64,
}

impl Account {
    /// The account's private commit address at GitHub, which publication commits use:
    /// no personal address enters the published branch.
    pub fn noreply(&self) -> String {
        format!("{}+{}@users.noreply.github.com", self.id, self.login)
    }
}

/// What GitHub says of a repository.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Repository {
    /// The repository's GitHub ID.
    pub id: i64,
    /// `OWNER/REPO` as GitHub names it now.
    pub full_name: String,
    /// The branch that holds sources.
    pub default_branch: String,
    /// Whether the repository is private.
    pub private: bool,
    /// Whether the token's account may push.
    pub can_push: bool,
    /// Whether the token's account administers it.
    pub can_admin: bool,
}

/// A repository's GitHub Pages settings, as far as the token may read them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Site {
    /// `false`: the token may not read them, so they are unknown.
    pub readable: bool,
    /// The repository has a Pages site.
    pub exists: bool,
    /// `legacy` (from a branch) or `workflow` (GitHub Actions).
    pub build_type: String,
    /// The branch it publishes, for `legacy`.
    pub branch: String,
    /// `/` or `/docs`.
    pub path: String,
    /// Where GitHub serves it.
    pub html_url: String,
    /// A custom domain; may be empty.
    pub cname: String,
    /// Whether the site is public.
    pub public: bool,
}

impl Site {
    /// Whether the site publishes `branch` from its root.
    pub fn publishes(&self, branch: &str) -> bool {
        self.exists && self.build_type == "legacy" && self.branch == branch && self.path == "/"
    }
}

/// A Pages build of a commit.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Build {
    /// `queued`, `building`, `built`, or `errored`.
    pub status: String,
    /// The commit built.
    pub commit: String,
    /// The build's error message; may be empty.
    pub error: String,
}

/// What GitHub lists of a commit's Pages builds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildState {
    /// The token may not read builds.
    Unreadable,
    /// GitHub lists no build of the commit yet.
    NotListed,
    /// The most recent build of the commit.
    Listed(Build),
}

fn text(o: &Value, key: &str) -> String {
    o.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn flag(o: &Value, key: &str) -> bool {
    o.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn get<'a>(path: &'a str, doing: &'a str, needs: &'a str) -> Call<'a> {
    Call { method: Method::Get, path, doing, needs, ..Call::default() }
}

/// Reads the account of `c`'s token. Fails when GitHub refuses the token or names no
/// account.
pub fn account(c: &Client) -> Result<Account, Problem> {
    let (_, v) = c.call(&get("/user", "read the token's account", "no permission"))?;
    let a = Account {
        login: text(&v, "login"),
        name: text(&v, "name"),
        id: v.get("id").and_then(Value::as_i64).unwrap_or(0),
    };
    if a.login.is_empty() || a.id <= 0 {
        return Err(problem(
            "pages.github",
            "GitHub named no account",
            "/user",
            "GitHub's answer has no account.",
            "Try the command again in a few minutes.",
        ));
    }
    Ok(a)
}

/// Reads the repository `owner/repo`.
pub fn read_repository(c: &Client, owner: &str, repo: &str) -> Result<Repository, Problem> {
    let path = format!("/repos/{owner}/{repo}");
    let (_, v) = c.call(&get(&path, "read the repository", "Metadata: read"))?;
    let perms = v.get("permissions").cloned().unwrap_or(Value::Object(Object::new()));
    let r = Repository {
        id: v.get("id").and_then(Value::as_i64).unwrap_or(0),
        full_name: text(&v, "full_name"),
        default_branch: text(&v, "default_branch"),
        private: flag(&v, "private"),
        can_push: flag(&perms, "push"),
        can_admin: flag(&perms, "admin"),
    };
    if r.id <= 0 || r.full_name.is_empty() {
        return Err(problem(
            "pages.github",
            "GitHub described no repository",
            &format!("{owner}/{repo}"),
            "GitHub's answer has no ID.",
            "Try the command again in a few minutes.",
        ));
    }
    Ok(r)
}

/// Reads the Pages settings of `owner/repo`. A token that may not read them gives an
/// unreadable site, not a problem.
pub fn pages_site(c: &Client, owner: &str, repo: &str) -> Result<Site, Problem> {
    let path = format!("/repos/{owner}/{repo}/pages");
    let v = match c.call(&get(&path, "read the Pages settings", "Pages: read")) {
        Ok((_, v)) => v,
        Err(p) if p.code == "pages.not-found" => {
            return Ok(Site { readable: true, ..Site::default() });
        }
        Err(p) if p.code == "pages.permission" => return Ok(Site::default()),
        Err(p) => return Err(p),
    };
    let source = v.get("source").cloned().unwrap_or(Value::Null);
    Ok(Site {
        readable: true,
        exists: true,
        build_type: text(&v, "build_type"),
        branch: text(&source, "branch"),
        path: text(&source, "path"),
        html_url: text(&v, "html_url"),
        cname: text(&v, "cname"),
        public: flag(&v, "public"),
    })
}

/// Makes GitHub Pages publish `branch` of `owner/repo` from its root: it creates the
/// site when there is none (`create`), and changes its source otherwise. It needs the
/// Pages and Administration permissions.
pub fn configure_pages(
    c: &Client,
    owner: &str,
    repo: &str,
    branch: &str,
    create: bool,
) -> Result<(), Problem> {
    let mut source = Object::new();
    source.insert("branch".into(), branch.into());
    source.insert("path".into(), "/".into());
    let mut body = Object::new();
    body.insert("build_type".into(), "legacy".into());
    body.insert("source".into(), Value::Object(source));
    let path = format!("/repos/{owner}/{repo}/pages");
    c.call(&Call {
        method: if create { Method::Post } else { Method::Put },
        path: &path,
        body: Some(gd_json::to_string(&Value::Object(body))),
        doing: "set the Pages source",
        needs: "Pages: write and Administration: write",
        max_bytes: 0,
    })?;
    Ok(())
}

/// Reads the most recent Pages build of `commit`.
pub fn pages_build(
    c: &Client,
    owner: &str,
    repo: &str,
    commit: &str,
) -> Result<BuildState, Problem> {
    let path = format!("/repos/{owner}/{repo}/pages/builds?per_page=30");
    let v = match c.call(&get(&path, "read the Pages builds", "Pages: read")) {
        Ok((_, v)) => v,
        Err(p) if p.code == "pages.permission" || p.code == "pages.not-found" => {
            return Ok(BuildState::Unreadable);
        }
        Err(p) => return Err(p),
    };
    let builds = v.as_array().map(Vec::as_slice).unwrap_or(&[]);
    let Some(b) = builds.iter().find(|b| text(b, "commit") == commit) else {
        return Ok(BuildState::NotListed);
    };
    let error = b.get("error").map(|e| text(e, "message")).unwrap_or_default();
    Ok(BuildState::Listed(Build { status: text(b, "status"), commit: commit.to_string(), error }))
}

/// Reads the tree and message of the commit `id`.
pub fn commit_tree(
    c: &Client,
    owner: &str,
    repo: &str,
    id: &str,
) -> Result<(String, String), Problem> {
    let path = format!("/repos/{owner}/{repo}/git/commits/{id}");
    let (_, v) = c.call(&get(&path, "read the branch's commit", "Contents: read"))?;
    let tree = v.get("tree").map(|t| text(t, "sha")).unwrap_or_default();
    if tree.is_empty() {
        return Err(problem(
            "pages.github",
            "GitHub described no tree",
            id,
            "The commit's description has no tree.",
            "Try the command again in a few minutes.",
        ));
    }
    Ok((tree, text(&v, "message")))
}

/// The files of a tree: each blob's path and ID, and whether the list is complete
/// (GitHub cuts the list of a very large tree short).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RemoteFiles {
    /// Path to blob ID.
    pub files: HashMap<String, String>,
    /// Whether GitHub listed every file.
    pub complete: bool,
}

/// Reads the files of the tree `id`.
pub fn remote_files(
    c: &Client,
    owner: &str,
    repo: &str,
    id: &str,
) -> Result<RemoteFiles, Problem> {
    let path = format!("/repos/{owner}/{repo}/git/trees/{id}?recursive=1");
    let call = Call {
        max_bytes: 64 * 1024 * 1024,
        ..get(&path, "read the branch's files", "Contents: read")
    };
    let (_, v) = c.call(&call)?;
    let entries = v.get("tree").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let mut files = HashMap::with_capacity(entries.len());
    for e in entries.iter().filter(|e| text(e, "type") == "blob") {
        files.insert(text(e, "path"), text(e, "sha"));
    }
    Ok(RemoteFiles { files, complete: !flag(&v, "truncated") })
}
