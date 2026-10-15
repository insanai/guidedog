//! The guided publish against a fake GitHub: a fresh machine to a live site, the
//! configured fast path, and runs that may not ask.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.
#![cfg(unix)] // the fake guidedog and gh are shell scripts.

mod support;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use gd_pages::{Credential, CredentialStore, GuideOptions, Prompt, publish_guided};
use gd_plugins::State;
use support::{Fake, PagesMode, TOKEN, has_git};

/// Answers from a script: every confirmation yes (unless `refuse`), the choice whose
/// text holds `pick`; records what was shown and asked.
#[derive(Default)]
struct Scripted {
    pick: &'static str,
    refuse: bool,
    shown: Vec<String>,
    asked: Vec<String>,
}

impl Prompt for Scripted {
    fn show(&mut self, text: &str) {
        self.shown.push(text.to_string());
    }

    fn confirm(&mut self, question: &str) -> bool {
        self.asked.push(question.to_string());
        !self.refuse
    }

    fn choose(&mut self, question: &str, choices: &[&str]) -> Option<usize> {
        self.asked.push(question.to_string());
        choices.iter().position(|c| c.contains(self.pick))
    }
}

/// Fails the test when asked anything.
struct Silent;

impl Prompt for Silent {
    fn show(&mut self, _text: &str) {}

    fn confirm(&mut self, question: &str) -> bool {
        panic!("asked {question:?}");
    }

    fn choose(&mut self, question: &str, _choices: &[&str]) -> Option<usize> {
        panic!("asked {question:?}");
    }
}

fn script(path: &Path, text: &str) -> PathBuf {
    std::fs::write(path, format!("#!/bin/sh\n{text}\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_path_buf()
}

/// A Git repository whose origin is github.com/octo/site, with a project in `docs`, a
/// fake guidedog that copies its HTML pages, and a fake gh that holds the test token.
fn repository(f: &Fake) -> (PathBuf, PathBuf, PathBuf) {
    let repo = f.root.join("repo");
    std::fs::create_dir_all(repo.join("docs")).unwrap();
    support::git(&["init", "-q", &repo.to_string_lossy()]);
    support::git(&[
        "-C",
        &repo.to_string_lossy(),
        "remote",
        "add",
        "origin",
        "git@github.com:octo/site.git",
    ]);
    std::fs::write(repo.join("docs/conf.toml"), "[project]\n").unwrap();
    std::fs::write(repo.join("docs/index.html"), "<p>one</p>").unwrap();
    let guidedog = script(
        &f.root.join("guidedog"),
        "rm -rf \"$5\" && mkdir -p \"$5\" && \
        cp \"$4\"/*.html \"$5\"/",
    );
    let gh = script(&f.root.join("gh"), &format!("echo {TOKEN}"));
    (repo, guidedog, gh)
}

fn options(f: &Fake, repo: &Path, guidedog: &Path) -> GuideOptions {
    GuideOptions {
        settings: f.settings(),
        config: repo.join("pages.toml"),
        dir: repo.join("docs"),
        guidedog: guidedog.display().to_string(),
        gh: f.root.join("gh").display().to_string(),
        ..GuideOptions::default()
    }
}

/// A machine with nothing set up: the repository comes from the origin remote, the
/// project from conf.toml, the token from gh for this run, then bind, publish, and
/// Pages setup after the first push, each asked once.
#[test]
fn a_fresh_machine_publishes_to_a_live_site() {
    if !has_git() {
        return;
    }
    let f = Fake::start("guide-fresh");
    f.world().pages = PagesMode::None;
    let (repo, guidedog, _) = repository(&f);
    let mut o = options(&f, &repo, &guidedog);
    o.settings.credential = Credential::Stored;
    o.settings.store = CredentialStore { service: "guidedog-pages-guide-test".into() };
    o.client_id = String::new();
    o.interactive = true;
    let mut prompt = Scripted { pick: "GitHub CLI", ..Scripted::default() };
    let r = publish_guided(&o, &mut prompt).unwrap();
    assert_eq!(r.state, State::Live, "{:?} {:?}", r.problem, prompt.shown);
    assert_eq!(Some(r.operation), f.head("gh-pages"));
    assert_eq!(f.world().pages, PagesMode::Branch);
    let config = std::fs::read_to_string(repo.join("pages.toml")).unwrap();
    assert!(
        config.contains("repository = \"octo/site\"") && config.contains("project = \"docs\""),
        "{config}"
    );
    assert_eq!(prompt.asked.len(), 4, "{:?}", prompt.asked);
    assert!(prompt.asked[0].contains("github.com/octo/site"));
    assert!(prompt.shown.iter().any(|l| l.starts_with("Live at https://octo.github.io/site/")));
}

/// A configured machine publishes without a question.
#[test]
fn a_configured_machine_is_not_asked() {
    if !has_git() {
        return;
    }
    let f = Fake::start("guide-fast");
    let (repo, guidedog, _) = repository(&f);
    let mut o = options(&f, &repo, &guidedog);
    o.assume_yes = true;
    let first = publish_guided(&o, &mut Scripted::default()).unwrap();
    assert_eq!(first.state, State::Live);
    let o = GuideOptions { assume_yes: false, interactive: true, ..o };
    let again = publish_guided(&o, &mut Silent).unwrap();
    assert_eq!(again.state, State::Unchanged);
    std::fs::write(repo.join("docs/index.html"), "<p>two</p>").unwrap();
    assert_eq!(publish_guided(&o, &mut Silent).unwrap().state, State::Live);
}

/// A run that may not ask fails at the first step that needs an answer, with a hint
/// naming the flag or command.
#[test]
fn runs_that_may_not_ask_name_the_missing_step() {
    if !has_git() {
        return;
    }
    let f = Fake::start("guide-batch");
    let (repo, guidedog, _) = repository(&f);
    let o = options(&f, &repo, &guidedog);
    let p = publish_guided(&o, &mut Silent).unwrap_err();
    assert_eq!((p.code.as_str(), p.path.as_str()), ("pages.guide", "repository"));
    assert!(p.hint.contains("--repository=octo/site"), "{}", p.hint);
    let o = GuideOptions { repository: "octo/site".into(), ..o };
    let p = publish_guided(&o, &mut Silent).unwrap_err();
    assert_eq!((p.code.as_str(), p.path.as_str()), ("pages.guide", "bind"));
    assert!(p.hint.contains("guidedog-pages bind") && p.hint.contains("--yes"), "{}", p.hint);
    let stored = gd_pages::Settings {
        credential: Credential::Stored,
        store: CredentialStore { service: "guidedog-pages-guide-test".into() },
        ..o.settings.clone()
    };
    let o = GuideOptions { settings: stored, ..o };
    let p = publish_guided(&o, &mut Silent).unwrap_err();
    assert!(["pages.guide", "pages.store"].contains(&p.code.as_str()), "{}", p.code);
    assert!(p.hint.contains("--credential=env"), "{}", p.hint);
    let o = GuideOptions { settings: f.settings(), assume_yes: true, ..o };
    f.world().pages = PagesMode::OtherBranch;
    let p = publish_guided(&o, &mut Silent).unwrap_err();
    assert_eq!(p.code, "pages.source", "a missing branch cannot be set up first");
}

/// The command line runs the guided publish too, and without a terminal it never asks.
#[test]
fn the_publish_command_is_guided() {
    if !has_git() {
        return;
    }
    let f = Fake::start("guide-command");
    let (repo, guidedog, _) = repository(&f);
    let program = gd_pages::command::Program {
        settings: f.settings(),
        dir: Some(repo.clone()),
        ..Default::default()
    };
    let run = |words: &[&str]| {
        let args: Vec<String> = words.iter().map(|w| w.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let status = gd_pages::command::run_with(&program, &args, &mut out, &mut err);
        (status, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    };
    let program_flag = format!("--guidedog={}", guidedog.display());
    let (status, _, err) = run(&["publish", &program_flag, "--color=never"]);
    assert_eq!(status, 2, "{err}");
    assert!(err.contains("--repository=octo/site"), "{err}");
    let (status, out, err) =
        run(&["publish", &program_flag, "--repository=octo/site", "--project=docs", "--yes"]);
    assert_eq!(status, 0, "{out}{err}");
    assert!(out.contains("live at https://octo.github.io/site/"), "{out}");
    assert!(err.contains("Let this machine publish there with this account? yes (--yes)"));
}
