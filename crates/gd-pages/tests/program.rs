//! The command line and the publisher program, against a fake GitHub.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

mod support;

use std::path::{Path, PathBuf};

use gd_pages::command::{Program, run_with};
use gd_plugins::{
    LoadOptions, PublisherSession, SnapshotOptions, State, inspect_entry, process_publisher,
    snapshot,
};
use support::{Fake, PagesMode, TOKEN, has_git};

fn run(f: &Fake, words: &[&str]) -> (i32, String, String) {
    let program = Program { settings: f.settings(), ..Program::default() };
    let args: Vec<String> = words.iter().map(|w| w.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status = run_with(&program, &args, &mut out, &mut err);
    (status, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// The commands review, bind, publish, set up, check, and unbind a target as a person
/// would, with `--yes` standing for their answers.
#[test]
fn the_commands_take_a_target_from_bind_to_live() {
    if !has_git() {
        return;
    }
    let f = Fake::start("commands");
    f.world().pages = PagesMode::None;
    let site = f.write_site("out", &[("index.html", "<p>one</p>")]);
    let config = f.write_config(&site).display().to_string();
    let with = |words: &[&str]| {
        let mut all: Vec<&str> = words.to_vec();
        all.extend(["--config", config.as_str(), "--color=never"]);
        run(&f, &all)
    };
    let (status, _, err) = with(&["bind"]);
    assert_eq!(status, 1, "{err}");
    assert!(err.contains("pages.unconfirmed"), "{err}");
    let (status, out, err) = with(&["bind", "--yes"]);
    assert_eq!(status, 0, "{err}");
    assert!(out.contains("Repository  octo/site (public, GitHub ID 7)"), "{out}");
    assert!(err.contains("guidedog-pages setup"), "{err}");
    let (status, _, err) = with(&["setup", "--yes"]);
    assert_eq!(status, 2, "{err}");
    assert!(err.contains("pages.no-branch"), "{err}");
    let (status, out, _) = with(&["publish", "--dry-run"]);
    assert_eq!(status, 0);
    assert!(out.contains("1 added, 0 changed, 0 removed") && out.contains("Dry run"), "{out}");
    let (status, out, err) = with(&["publish"]);
    assert_eq!(status, 6, "{out}{err}");
    assert!(err.contains("pages.no-site"), "{err}");
    let (status, out, err) = with(&["setup", "--yes"]);
    assert_eq!(status, 0, "{out}{err}");
    assert_eq!(f.world().pages, PagesMode::Branch);
    let (status, out, _) = with(&["status"]);
    assert_eq!(status, 0, "{out}");
    assert!(out.contains("live at https://octo.github.io/site/"), "{out}");
    let (status, out, _) = with(&["publish", "--diagnostics=json"]);
    assert_eq!(status, 0);
    assert!(out.contains(r#""state":"unchanged""#), "{out}");
    let (status, out, _) = with(&["recover"]);
    assert_eq!(status, 0, "{out}");
    let (status, out, err) = with(&["doctor"]);
    assert_eq!(status, 0, "{out}{err}");
    assert!(out.contains("Ready to publish to https://octo.github.io/site/"), "{out}");
    let (status, out, _) = with(&["unbind"]);
    assert_eq!(status, 0);
    assert!(out.contains("Unbound octo/site"), "{out}");
    // Unbound again: publish would bind, which needs an answer this run cannot give.
    let (status, _, err) = with(&["publish"]);
    assert_eq!(status, 2);
    assert!(err.contains("pages.guide") && err.contains("guidedog-pages bind"), "{err}");
}

/// The folder cargo builds examples into: `target/<profile>/examples`.
fn examples_dir() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    exe.parent().unwrap().parent().unwrap().join("examples")
}

/// Links (or copies) the fixture program into `dir` with its settings beside it.
fn fixture_program(f: &Fake, dir: &Path) -> String {
    let name = format!("pages_fixture{}", std::env::consts::EXE_SUFFIX);
    let target = dir.join(&name);
    if std::fs::hard_link(examples_dir().join(&name), &target).is_err() {
        std::fs::copy(examples_dir().join(&name), &target).unwrap();
    }
    let settings = format!(
        "root = {}\nstate = {}\ncache = {}\ntoken = {TOKEN}\n",
        f.url(),
        f.root.join("state").display(),
        f.root.join("cache").display()
    );
    let mut fixture = target.clone().into_os_string();
    fixture.push(".fixture");
    std::fs::write(PathBuf::from(fixture), settings).unwrap();
    target.display().to_string()
}

/// The publisher program, reached through the process protocol, publishes as the
/// linked publisher does: the same plan, outcome, and branch.
#[test]
fn the_program_publishes_through_the_protocol() {
    if !has_git() {
        return;
    }
    let f = Fake::start("protocol");
    let site = f.write_site("out", &[("index.html", "<p>one</p>"), ("_static/a.css", "p{}")]);
    let config = f.write_config(&site);
    let (status, _, err) = run(&f, &["bind", "--yes", "--config", &config.display().to_string()]);
    assert_eq!(status, 0, "{err}");
    let program = fixture_program(&f, &f.root);
    let plugin = process_publisher(&inspect_entry(&program).unwrap()).unwrap();
    for round in 0..2 {
        let options = LoadOptions::new(&config.display().to_string());
        let mut s = PublisherSession::load(&plugin, &options).unwrap();
        let snap = snapshot(&s.source().site, &SnapshotOptions::default()).unwrap();
        let plan = s.plan(&snap).unwrap();
        assert_eq!(plan.changes.len(), if round == 0 { 2 } else { 0 });
        let r = s.publish(&snap, &plan);
        let want = if round == 0 { State::Live } else { State::Unchanged };
        assert_eq!(r.state, want, "round {round}: {:?}", r.problem);
        if round == 0 {
            assert_eq!(Some(r.operation), f.head("gh-pages"));
        }
        assert_eq!(s.status("").state, State::Live);
    }
}
