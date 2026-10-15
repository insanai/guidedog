//! The commands end to end, in process: conversions, and publishing with a linked
//! publisher.

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

/// An unresolved reference is reported once, as the error Docutils and Sphinx report
/// ("Unknown target name"): the build still succeeds and says so, and -W fails it.
#[test]
fn an_unresolved_reference_is_one_error() {
    let root = TempDir::new("unresolved");
    let none = Registry::default();
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(root.join("docs/conf.toml"), "project = \"T\"\n").unwrap();
    fs::write(root.join("docs/index.rst"), "Title\n=====\n\nSee missing_ here.\n").unwrap();
    let docs = root.arg("docs");
    let (status, out, err) = run(&["build", "html", &docs, "--color=never"], &none);
    let all = format!("{out}{err}");
    assert_eq!(status, 0, "{all}");
    assert_eq!(all.matches("missing_").count(), 2, "one excerpt and its message: {all}");
    assert!(
        all.contains("build succeeded with 1 error(s) and 0 warning(s) in the documents"),
        "{all}"
    );
    let (status, out, err) = run(&["build", "html", &docs, "--color=never", "-W"], &none);
    assert_ne!(status, 0, "{out}{err}");
    assert!(format!("{out}{err}").contains("build failed"), "{out}{err}");
}

/// An error in a document's content keeps its severity, as Docutils and Sphinx report
/// it ("ERROR"), and the build still succeeds; -W fails it.
#[test]
fn a_content_error_stays_an_error_and_the_build_succeeds() {
    let root = TempDir::new("content-error");
    let none = Registry::default();
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(root.join("docs/conf.toml"), "project = \"T\"\n").unwrap();
    let page = "Title\n=====\n\n.. rubric:: Heading\n   :heading-level: 7\n\nText.\n";
    fs::write(root.join("docs/index.rst"), page).unwrap();
    let docs = root.arg("docs");
    let args = ["build", "html", &docs, "--color=never", "--diagnostics=json"];
    let (status, out, err) = run(&args, &none);
    let all = format!("{out}{err}");
    assert_eq!(status, 0, "{all}");
    assert!(
        all.contains("\"code\":\"rst.directive.option.invalid\",\"severity\":\"error\""),
        "{all}"
    );
    let (status, out, err) = run(&["build", "html", &docs, "--color=never", "-W"], &none);
    assert_ne!(status, 0, "{out}{err}");
}

/// quickstart and migrate run gd-migrate; build, intl and clean run the project builder.
#[test]
fn quickstart_migrate_and_the_project_builder() {
    let root = TempDir::new("project");
    let none = Registry::default();
    let docs = root.arg("docs");
    let (status, _, err) = run(&["quickstart", &docs, "-q", "-p", "Demo", "-a", "Ada"], &none);
    assert_eq!(status, 0, "{err}");
    assert!(root.join("docs/conf.toml").is_file() && root.join("docs/index.rst").is_file());
    let (status, out, err) = run(&["build", "html", &docs, "--color=never", "-W"], &none);
    assert_eq!(status, 0, "{err}");
    assert!(out.contains("building html for Demo") && out.contains("build succeeded"), "{out}");
    assert!(root.join("docs/_build/html/index.html").is_file());
    let (status, out, _) = run(&["build", "html", &docs, "--color=never"], &none);
    assert_eq!(status, 0);
    assert!(out.contains("0 document(s) parsed, 1 unchanged"), "{out}");
    // The sphinx-build form writes to the folder given.
    let site = root.arg("site");
    let (status, _, err) = run(&["build", "-b", "text", &docs, &site], &none);
    assert_eq!(status, 0, "{err}");
    assert!(root.join("site/index.txt").is_file());
    let (status, _, err) = run(&["build", "gettext", &docs], &none);
    assert_eq!(status, 0, "{err}");
    let (status, out, err) = run(&["intl", "update", "-l", "de", &docs], &none);
    assert_eq!(status, 0, "{err}");
    assert!(out.contains("create") && out.contains("index.po"), "{out}");
    let (status, out, _) = run(&["intl", "stat", "-l", "de", &docs], &none);
    assert_eq!(status, 0);
    assert!(out.contains("0 translated"), "{out}");
    let (status, _, err) = run(&["clean", &docs], &none);
    assert_eq!(status, 0, "{err}");
    assert!(!root.join("docs/_build/html").exists());
    let (status, _, err) = run(&["build", "html", &root.arg("nowhere/x")], &none);
    assert_eq!(status, 2);
    assert!(err.contains("UNKNOWN BUILDER") || err.contains("project.missing"), "{err}");
    fs::write(root.join("conf.py"), "project = 'Old'\nauthor = 'Ada'\n").unwrap();
    let (status, out, err) = run(&["migrate", &root.arg("conf.py"), "-o", "-"], &none);
    assert_eq!(status, 0, "{err}");
    assert!(out.contains("project = \"Old\""), "{out}");
}
