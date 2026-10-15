//! A fake GitHub for the tests: the REST calls the publisher makes, and Git's smart
//! HTTP, served from a real bare repository by `git` itself, which is the oracle here;
//! the publisher never runs git. Tests that need it are skipped where git is missing.

// Each test file uses a different part of this module.
#![allow(dead_code)]
#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};

use gd_fetch::testserver::{self, Reply, Request, Server};
use gd_pages::{Credential, CredentialStore, Roots, Secret, Settings};

pub const TOKEN: &str = "ghp_testtokentesttokentesttoken0001";

/// What the fake's Pages settings say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagesMode {
    Branch,
    None,
    Workflow,
    OtherBranch,
}

/// How the fake answers a push.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushMode {
    Normal,
    /// The push is applied, and its answer lost.
    ApplyThenLose,
    /// The connection is lost before the push is applied.
    Lose,
}

/// What a test may change while the fake runs.
#[derive(Debug, Clone)]
pub struct World {
    pub repo_id: i64,
    pub account_id: i64,
    pub default_branch: String,
    pub pages: PagesMode,
    /// The status of a build of the branch's commit; empty for none.
    pub build: String,
    pub push: PushMode,
    pub polls: u32,
}

/// A bare repository served as `octo/site`, with its folder removed when dropped.
pub struct Fake {
    pub root: PathBuf,
    pub bare: PathBuf,
    world: Arc<Mutex<World>>,
    server: Option<Server>,
}

pub fn has_git() -> bool {
    Command::new("git").arg("--version").output().is_ok_and(|o| o.status.success())
}

impl Fake {
    pub fn start(name: &str) -> Fake {
        let root = scratch(name);
        let bare = root.join("site.git");
        git(&["init", "--bare", "-q", &bare.to_string_lossy()]);
        let world = Arc::new(Mutex::new(World {
            repo_id: 7,
            account_id: 42,
            default_branch: "main".into(),
            pages: PagesMode::Branch,
            build: "built".into(),
            push: PushMode::Normal,
            polls: 0,
        }));
        let (w, b) = (world.clone(), bare.clone());
        let server = testserver::start(move |r| answer(&w, &b, r)).unwrap();
        Fake { root, bare, world, server: Some(server) }
    }

    /// The world, to read or change.
    pub fn world(&self) -> MutexGuard<'_, World> {
        self.world.lock().unwrap()
    }

    /// The server's root URL.
    pub fn url(&self) -> String {
        self.server.as_ref().unwrap().url("")
    }

    /// Settings that reach the fake with the test token, and keep state and cache in
    /// the fixture.
    pub fn settings(&self) -> Settings {
        Settings {
            roots: Roots::at(&self.url()),
            credential: Credential::Token(Secret::new(TOKEN)),
            store: CredentialStore { service: "guidedog-pages-test".into() },
            state_dir: Some(self.root.join("state")),
            cache_dir: Some(self.root.join("cache")),
        }
    }

    /// The bare repository's commit of `branch`, `None` when there is none.
    pub fn head(&self, branch: &str) -> Option<String> {
        let out = Command::new("git")
            .args(["--git-dir", &self.bare.to_string_lossy(), "rev-parse", "--verify", "-q"])
            .arg(format!("refs/heads/{branch}"))
            .output()
            .unwrap();
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// Runs git on the bare repository.
    pub fn git(&self, args: &[&str]) -> String {
        let bare = self.bare.to_string_lossy().into_owned();
        let mut all = vec!["--git-dir", bare.as_str()];
        all.extend_from_slice(args);
        git(&all)
    }

    /// Writes a site of `files` (path, text) into a new folder `name` of the fixture.
    pub fn write_site(&self, name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = self.root.join(name);
        let _ = std::fs::remove_dir_all(&dir);
        for (path, text) in files {
            let path = dir.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
        }
        dir
    }

    /// Writes a `pages.toml` publishing `site` to `octo/site`.
    pub fn write_config(&self, site: &Path) -> PathBuf {
        let path = self.root.join("pages.toml");
        // A literal string: a Windows path's backslashes are not escapes there.
        let text = format!(
            "schema = 1\nrepository = \"octo/site\"\nsite = '{}'\n\
            wait_seconds = 0\n",
            site.display()
        );
        std::fs::write(&path, text).unwrap();
        path
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        drop(self.server.take());
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A new, empty folder for one test.
pub fn scratch(name: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir()
        .join("gd-pages-tests")
        .join(format!("{name}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs git with `args` and returns its output, trimmed.
pub fn git(args: &[&str]) -> String {
    let out = Command::new("git").args(args).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Runs git with `input` on its standard input and returns its output.
pub fn git_with_input(args: &[&str], input: &[u8]) -> Vec<u8> {
    let mut child = Command::new("git")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let input = input.to_vec();
    let feeder = std::thread::spawn(move || {
        let _ = stdin.write_all(&input);
    });
    let out = child.wait_with_output().unwrap();
    let _ = feeder.join();
    out.stdout
}

fn reply(status: u16, body: impl Into<Vec<u8>>) -> Reply {
    Reply { status, body: body.into(), ..Reply::default() }
}

fn answer(world: &Mutex<World>, bare: &Path, r: &Request) -> Reply {
    let auth = r.headers.get("authorization").map(String::as_str).unwrap_or("");
    let basic =
        format!("Basic {}", gd_json::base64::encode(format!("x-access-token:{TOKEN}").as_bytes()));
    let (path, query) = r.path.split_once('?').unwrap_or((&r.path, ""));
    let bare_arg = bare.to_string_lossy().into_owned();
    if path.starts_with("/login/") {
        return device(world, path);
    }
    if path.ends_with(".git/info/refs") {
        if auth != basic {
            return reply(401, "");
        }
        let out = git_with_input(
            &["receive-pack", "--stateless-rpc", "--advertise-refs", &bare_arg],
            &[],
        );
        let mut body = b"001f# service=git-receive-pack\n0000".to_vec();
        body.extend_from_slice(&out);
        return reply(200, body);
    }
    if path.ends_with(".git/git-receive-pack") {
        if auth != basic {
            return reply(401, "");
        }
        let mode = world.lock().unwrap().push;
        if mode == PushMode::Lose {
            return Reply { close: true, ..Reply::default() };
        }
        let out = git_with_input(&["receive-pack", "--stateless-rpc", &bare_arg], &r.body);
        if mode == PushMode::ApplyThenLose {
            return Reply { close: true, ..Reply::default() };
        }
        return reply(200, out);
    }
    if auth != format!("Bearer {TOKEN}") {
        return reply(401, r#"{"message":"Bad credentials"}"#);
    }
    rest(world, bare, &r.method, path, query)
}

fn rest(world: &Mutex<World>, bare: &Path, method: &str, path: &str, query: &str) -> Reply {
    let w = world.lock().unwrap().clone();
    let bare_arg = bare.to_string_lossy().into_owned();
    match path {
        "/user" => {
            return reply(200, format!(r#"{{"login":"ada","id":{},"name":"Ada"}}"#, w.account_id));
        }
        "/repos/octo/site" => {
            return reply(
                200,
                format!(
                    concat!(
                        r#"{{"id":{},"full_name":"octo/site","default_branch":"{}","#,
                        r#""private":false,"permissions":{{"push":true,"admin":true}}}}"#
                    ),
                    w.repo_id, w.default_branch
                ),
            );
        }
        "/repos/octo/site/pages" => return pages_reply(world, method),
        "/repos/octo/site/pages/builds" => {
            let head = git_with_input(
                &["--git-dir", &bare_arg, "rev-parse", "--verify", "-q", "refs/heads/gh-pages"],
                &[],
            );
            let head = String::from_utf8_lossy(&head).trim().to_string();
            if head.is_empty() || w.build.is_empty() {
                return reply(200, "[]");
            }
            return reply(
                200,
                format!(
                    r#"[{{"commit":"{head}","status":"{}","error":{{"message":null}}}}]"#,
                    w.build
                ),
            );
        }
        _ => {}
    }
    if let Some(id) = path.strip_prefix("/repos/octo/site/git/commits/") {
        let text = git_with_input(&["--git-dir", &bare_arg, "cat-file", "commit", id], &[]);
        let text = String::from_utf8_lossy(&text);
        let (head, message) = text.split_once("\n\n").unwrap_or((&text, ""));
        let tree = head.get(5..45).unwrap_or("");
        let mut o = gd_json::Object::new();
        o.insert("sha".into(), id.into());
        o.insert("message".into(), message.into());
        let mut t = gd_json::Object::new();
        t.insert("sha".into(), tree.into());
        o.insert("tree".into(), gd_json::Value::Object(t));
        return reply(200, gd_json::to_string(&gd_json::Value::Object(o)));
    }
    if let Some(id) = path.strip_prefix("/repos/octo/site/git/trees/")
        && query == "recursive=1"
    {
        return reply(200, tree_listing(&bare_arg, id));
    }
    reply(404, r#"{"message":"Not Found"}"#)
}

fn tree_listing(bare: &str, id: &str) -> String {
    let listing = git_with_input(&["--git-dir", bare, "ls-tree", "-r", id], &[]);
    let listing = String::from_utf8_lossy(&listing);
    let entries = listing.lines().filter_map(|line| {
        let (meta, name) = line.split_once('\t')?;
        let fields: Vec<&str> = meta.split_whitespace().collect();
        let mut o = gd_json::Object::new();
        o.insert("path".into(), name.into());
        o.insert("mode".into(), fields.first()?.to_string().into());
        o.insert("type".into(), fields.get(1)?.to_string().into());
        o.insert("sha".into(), fields.get(2)?.to_string().into());
        Some(gd_json::Value::Object(o))
    });
    let mut o = gd_json::Object::new();
    o.insert("sha".into(), id.into());
    o.insert("truncated".into(), false.into());
    o.insert("tree".into(), gd_json::Value::Array(entries.collect()));
    gd_json::to_string(&gd_json::Value::Object(o))
}

fn pages_reply(world: &Mutex<World>, method: &str) -> Reply {
    let mut w = world.lock().unwrap();
    if method == "POST" || method == "PUT" {
        w.pages = PagesMode::Branch;
        return if method == "POST" { reply(201, "{}") } else { reply(204, "") };
    }
    match w.pages {
        PagesMode::None => reply(404, r#"{"message":"Not Found"}"#),
        PagesMode::Workflow => reply(
            200,
            concat!(
                r#"{"build_type":"workflow","source":{"branch":"main","path":"/"},"#,
                r#""html_url":"https://octo.github.io/site/"}"#
            ),
        ),
        PagesMode::OtherBranch => reply(
            200,
            concat!(
                r#"{"build_type":"legacy","source":{"branch":"main","path":"/docs"},"#,
                r#""html_url":"https://octo.github.io/site/"}"#
            ),
        ),
        PagesMode::Branch => reply(
            200,
            concat!(
                r#"{"build_type":"legacy","source":{"branch":"gh-pages","path":"/"},"#,
                r#""html_url":"https://octo.github.io/site/","public":true}"#
            ),
        ),
    }
}

fn device(world: &Mutex<World>, path: &str) -> Reply {
    if path == "/login/device/code" {
        return reply(
            200,
            concat!(
                r#"{"device_code":"dc","user_code":"ABCD-1234","#,
                r#""verification_uri":"https://github.com/login/device","#,
                r#""expires_in":900,"interval":5}"#
            ),
        );
    }
    let mut w = world.lock().unwrap();
    w.polls += 1;
    match w.polls {
        1 => reply(200, r#"{"error":"authorization_pending"}"#),
        2 => reply(200, r#"{"error":"slow_down","interval":10}"#),
        _ => reply(200, format!(r#"{{"access_token":"{TOKEN}","token_type":"bearer"}}"#)),
    }
}
