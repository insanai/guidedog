//! The commands end to end, in process: conversions, Guidedog's discussion catalog, and
//! publishing with a linked publisher.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

use std::fs;
use std::path::{Path, PathBuf};

use gd_cli::{Streams, Terminals, run_in};
use gd_plugins::{
    Capabilities, Change, ChangeKind, LoadOptions, Loaded, Outcome, Plan, Plugin, Problem,
    Publisher, PublisherInfo, Registry, Snapshot, Source, State,
};

/// A private folder, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let path = std::env::temp_dir().join(format!("gd-cli-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }

    fn join(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }

    fn arg(&self, rel: &str) -> String {
        self.join(rel).to_string_lossy().into_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str], linked: &Registry) -> (i32, String, String) {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut input: &[u8] = b"";
    let streams = Streams {
        out: &mut out,
        err: &mut err,
        input: &mut input,
        terminals: Terminals::default(),
    };
    let status = run_in(&args, linked, streams);
    (status, String::from_utf8_lossy(&out).into(), String::from_utf8_lossy(&err).into())
}

#[test]
fn restructuredtext_and_markdown_convert_to_html() {
    let dir = TempDir::new("html");
    fs::write(dir.join("a.rst"), "Title\n=====\n\nHello *world*.\n").unwrap();
    fs::write(dir.join("b.md"), "# Notes\n\nSome `code`.\n").unwrap();
    let none = Registry::default();
    let (status, out, err) = run(&["convert", &dir.arg("a.rst"), "-o", &dir.arg("a.html")], &none);
    assert_eq!(status, 0, "{err}");
    assert!(out.contains("Wrote"), "{out}");
    let html = fs::read_to_string(dir.join("a.html")).unwrap();
    assert!(html.contains("<title>Title</title>") && html.contains("<em>world</em>"), "{html}");
    let (status, _, err) =
        run(&["convert", &dir.arg("b.md"), "--output", &dir.arg("b.html"), "-q"], &none);
    assert_eq!(status, 0, "{err}");
    assert!(fs::read_to_string(dir.join("b.html")).unwrap().contains("<code>code</code>"));
    // An existing output stays unless --force.
    let (status, _, err) = run(&["convert", &dir.arg("b.md"), "-o", &dir.arg("b.html")], &none);
    assert_eq!(status, 2, "{err}");
    let (status, out, _) = run(
        &["convert", &dir.arg("b.md"), "-o", &dir.arg("b.html"), "--force", "--diagnostics=json"],
        &none,
    );
    assert_eq!(status, 0);
    assert!(out.starts_with("{\"status\":\"ok\""), "{out}");
}

#[test]
fn markdown_converts_to_pdf_with_the_embedded_typst() {
    if !gd_typst::EMBEDDED {
        return;
    }
    let dir = TempDir::new("pdf");
    fs::write(dir.join("b.md"), "# Notes\n\nSome *text*.\n").unwrap();
    let args =
        ["convert", &dir.arg("b.md"), "-o", &dir.arg("b.pdf"), "--emit-typst", &dir.arg("b.typ")];
    let (status, _, err) = run(&args, &Registry::default());
    assert_eq!(status, 0, "{err}");
    assert!(fs::read(dir.join("b.pdf")).unwrap().starts_with(b"%PDF"));
    assert!(fs::read_to_string(dir.join("b.typ")).unwrap().contains("Notes"));
}

#[test]
fn a_failed_conversion_says_nothing_was_written() {
    let dir = TempDir::new("raw");
    fs::write(dir.join("r.rst"), ".. raw:: html\n\n   <b>x</b>\n").unwrap();
    let (status, _, err) =
        run(&["convert", &dir.arg("r.rst"), "-o", &dir.arg("r.html")], &Registry::default());
    assert_eq!(status, 1, "{err}");
    assert!(err.contains("No output was written."), "{err}");
    assert!(!dir.join("r.html").exists());
}

/// A gds catalog made with dsc's init, then kept with guidedog gds.
#[test]
fn gds_keeps_guidedog_s_catalog() {
    let root = TempDir::new("gds");
    let dir = format!("--dir={}", root.arg("docs/gds"));
    let init: Vec<String> =
        ["init", &dir, "--prefix=gds", "--author=Ada"].iter().map(|s| s.to_string()).collect();
    let dsc = gd_discussion::command::Program::default();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(
        gd_discussion::command::run_with(&dsc, &init, &mut out, &mut err, (false, false)),
        0
    );
    let none = Registry::default();
    let (status, _, err) =
        run(&["gds", "new", "Cache policy", &dir, "--labels=architecture", "--author=Ada"], &none);
    assert_eq!(status, 0, "{err}");
    let (status, out, _) = run(&["gds", "list", &dir], &none);
    assert_eq!(status, 0);
    assert!(out.contains("Cache policy"), "{out}");
    let (status, out, _) = run(&["gds", "--version"], &none);
    assert_eq!(status, 0);
    assert!(out.starts_with("guidedog "), "{out}");
    let (status, _, err) = run(&["gds", "lsit", &dir], &none);
    assert_eq!(status, 2);
    assert!(err.contains("Did you mean `list`?"), "{err}");
    // Building goes through this program's project builder.
    let (status, _, err) = run(&["gds", "build", &dir, "--target=html"], &none);
    assert_ne!(status, 0);
    assert!(err.contains("cli.not-available") || err.contains("NOT AVAILABLE"), "{err}");
}

/// Publishes a built site to a local folder, as the gd-plugins example does.
#[derive(Debug, Clone, Default)]
struct Folder {
    target: PathBuf,
}

impl Publisher for Folder {
    fn info(&self) -> PublisherInfo {
        PublisherInfo::new("folder", "Local folder", "1.0.0", Capabilities::default())
    }

    fn load(&mut self, options: &LoadOptions) -> Result<Loaded, Problem> {
        let text = fs::read_to_string(&options.config)
            .map_err(|e| Problem::new("folder.config", "No configuration", &e.to_string()))?;
        let mut lines = text.lines();
        let site = lines.next().unwrap_or_default().to_string();
        self.target = PathBuf::from(lines.next().unwrap_or_default());
        Ok(Loaded {
            source: Source { site, ..Source::default() },
            target: self.target.to_string_lossy().into(),
        })
    }

    fn plan(&mut self, snapshot: &Snapshot, plan: &mut Plan) -> Result<(), Problem> {
        for entry in snapshot.entries() {
            if !self.target.join(&entry.path).exists() {
                plan.changes.push(Change { path: entry.path.clone(), kind: ChangeKind::Added });
            }
        }
        plan.unchanged = plan.changes.is_empty();
        Ok(())
    }

    fn publish(&mut self, snapshot: &Snapshot, plan: &Plan) -> Outcome {
        for entry in snapshot.entries() {
            let to = self.target.join(&entry.path);
            let _ = fs::create_dir_all(to.parent().unwrap_or(Path::new(".")));
            let _ = fs::copy(snapshot.root().join(&entry.path), &to);
        }
        Outcome {
            state: State::Live,
            operation: "copy".into(),
            generation: plan.generation_hex(),
            url: "file://target".into(),
            problem: None,
        }
    }
}

#[test]
fn publish_uses_a_linked_publisher_before_the_machine_registry() {
    let root = TempDir::new("publish");
    fs::create_dir_all(root.join("site")).unwrap();
    fs::write(root.join("site/index.html"), "<p>hello</p>").unwrap();
    fs::write(root.join("folder.conf"), format!("{}\n{}\n", root.arg("site"), root.arg("www")))
        .unwrap();
    let linked = Registry::new(vec![Plugin::publisher(Folder::default())]).unwrap();
    let config = format!("--config={}", root.arg("folder.conf"));
    let (status, out, err) =
        run(&["publish", "--publisher=folder", &config, "--dry-run"], &linked);
    assert_eq!(status, 0, "{err}");
    assert!(out.contains("1 added, 0 changed, 0 removed") && out.contains("Dry run"), "{out}");
    assert!(!root.join("www/index.html").exists());
    let (status, out, err) = run(&["publish", "--publisher=folder", &config], &linked);
    assert_eq!(status, 0, "{err}");
    assert!(out.contains("live at file://target"), "{out}");
    assert!(root.join("www/index.html").exists());
}

/// The machine's own registry is set up only in the process tests (cmd/guidedog), where
/// GUIDEDOG_CONFIG_DIR can be given to the child; here only the linked list is checked.
#[test]
fn plugin_list_shows_the_linked_plugins() {
    let linked = Registry::new(vec![Plugin::publisher(Folder::default())]).unwrap();
    let (status, out, err) = run(&["plugin", "list"], &linked);
    assert_eq!(status, 0, "{err}");
    assert!(out.contains("Linked into this program") && out.contains("folder"), "{out}");
}

/// quickstart and migrate run gd-migrate; building reads and checks the project, then
/// stops at the seam to the project builder until it is ported.
#[test]
fn quickstart_migrate_and_the_project_builder_seam() {
    let root = TempDir::new("project");
    let none = Registry::default();
    let docs = root.arg("docs");
    let (status, _, err) = run(&["quickstart", &docs, "-q", "-p", "Demo", "-a", "Ada"], &none);
    assert_eq!(status, 0, "{err}");
    assert!(root.join("docs/conf.toml").is_file() && root.join("docs/index.rst").is_file());
    let (status, out, err) = run(&["build", "html", &docs, "--color=never"], &none);
    assert_eq!(status, 2, "{err}");
    assert!(out.contains("building html for Demo") && out.contains("build failed"), "{out}");
    assert!(err.contains("cli.not-available"), "{err}");
    let (status, _, err) = run(&["build", "html", &root.arg("nowhere/x")], &none);
    assert_eq!(status, 2);
    assert!(err.contains("UNKNOWN BUILDER") || err.contains("project.missing"), "{err}");
    fs::write(root.join("conf.py"), "project = 'Old'\nauthor = 'Ada'\n").unwrap();
    let (status, out, err) = run(&["migrate", &root.arg("conf.py"), "-o", "-"], &none);
    assert_eq!(status, 0, "{err}");
    assert!(out.contains("project = \"Old\""), "{out}");
}
