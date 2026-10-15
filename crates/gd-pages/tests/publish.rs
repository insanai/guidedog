//! Publications against a fake GitHub whose branch is a real bare repository.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

mod support;

use std::path::Path;
use std::time::Duration;

use gd_pages::{
    Client, Journal, Settings, bind, read_config, review_target, start_device, target_key, today,
    wait_device,
};
use gd_plugins::{
    Cancel, ChangeKind, LoadOptions, Problem, PublisherSession, SnapshotOptions, State, snapshot,
};
use support::{Fake, PagesMode, PushMode, TOKEN, has_git};

fn bind_fixture(settings: &Settings, config: &Path, adopt: bool) -> Result<(), Problem> {
    let c = read_config(config)?;
    let client = settings.client(settings.load_token(&c.profile)?, Cancel::default());
    let r = review_target(&client, &c)?;
    bind(&settings.state()?, &c, &c.profile, &r, adopt)
}

fn load(settings: &Settings, config: &Path) -> Result<PublisherSession, Problem> {
    let plugin = gd_pages::plugin_with(settings.clone());
    PublisherSession::load(&plugin, &LoadOptions::new(&config.to_string_lossy()))
}

/// Loads a session, snapshots its site, plans and publishes; returns the outcome's
/// state and problem code, and the plan's changes.
fn publish_site(settings: &Settings, config: &Path) -> (State, String, Vec<(String, ChangeKind)>) {
    let mut s = match load(settings, config) {
        Ok(s) => s,
        Err(p) => return (State::Failed, p.code.clone(), Vec::new()),
    };
    let snap = snapshot(&s.source().site, &SnapshotOptions::default()).unwrap();
    let plan = match s.plan(&snap) {
        Ok(plan) => plan,
        Err(p) => return (State::Failed, p.code.clone(), Vec::new()),
    };
    let changes = plan.changes.iter().map(|c| (c.path.clone(), c.kind)).collect();
    let r = s.publish(&snap, &plan);
    let code = r.problem.as_ref().map(|p| p.code.clone()).unwrap_or_default();
    (r.state, code, changes)
}

/// A site is published to a new branch, published again unchanged, then changed: each
/// publication is one commit on the last, which git accepts and checks.
#[test]
fn sites_are_published_as_whole_commits() {
    if !has_git() {
        return;
    }
    let f = Fake::start("whole");
    let settings = f.settings();
    let site = f.write_site(
        "out",
        &[("index.html", "<p>one</p>"), ("_static/a.css", "p{}"), ("old.html", "old")],
    );
    let config = f.write_config(&site);
    assert_eq!(publish_site(&settings, &config).1, "pages.unbound");
    bind_fixture(&settings, &config, false).unwrap();
    let (state, code, changes) = publish_site(&settings, &config);
    assert_eq!(state, State::Live, "{code}");
    assert_eq!(changes.len(), 3);
    let first = f.head("gh-pages").unwrap();
    let listing = f.git(&["ls-tree", "-r", "--name-only", &first]);
    assert_eq!(listing, ".nojekyll\n_static/a.css\nindex.html\nold.html");
    let message = f.git(&["log", "-1", "--format=%an <%ae>%n%B", &first]);
    assert!(message.starts_with("Ada <42+ada@users.noreply.github.com>"), "{message}");
    assert!(message.contains(gd_pages::MARK));
    let (state, code, _) = publish_site(&settings, &config);
    assert_eq!(state, State::Unchanged, "{code}");
    assert_eq!(f.head("gh-pages").unwrap(), first);
    f.write_site("out", &[("index.html", "<p>two</p>"), ("_static/a.css", "p{}")]);
    let (state, code, changes) = publish_site(&settings, &config);
    assert_eq!(state, State::Live, "{code}");
    assert_eq!(
        changes,
        vec![
            ("index.html".to_string(), ChangeKind::Changed),
            ("old.html".to_string(), ChangeKind::Removed)
        ]
    );
    let second = f.head("gh-pages").unwrap();
    assert_eq!(f.git(&["rev-parse", &format!("{second}^")]), first);
    f.git(&["fsck", "--strict", "--no-dangling"]);
}

/// The target is refused when it is not the bound one, when Pages publishes from
/// elsewhere, and when the branch holds another's work, until that work is adopted.
#[test]
fn unauthorized_targets_are_refused() {
    if !has_git() {
        return;
    }
    let f = Fake::start("refused");
    let settings = f.settings();
    let site = f.write_site("out", &[("index.html", "<p>one</p>")]);
    let config = f.write_config(&site);
    bind_fixture(&settings, &config, false).unwrap();
    f.world().repo_id = 8;
    assert_eq!(publish_site(&settings, &config).1, "pages.identity");
    f.world().repo_id = 7;
    f.world().account_id = 43;
    assert_eq!(publish_site(&settings, &config).1, "pages.account");
    f.world().account_id = 42;
    for (mode, want) in [
        (PagesMode::None, "pages.no-site"),
        (PagesMode::Workflow, "pages.source"),
        (PagesMode::OtherBranch, "pages.source"),
    ] {
        f.world().pages = mode;
        assert_eq!(publish_site(&settings, &config).1, want, "{mode:?}");
    }
    f.world().pages = PagesMode::Branch;
    f.world().default_branch = "gh-pages".into();
    assert_eq!(publish_site(&settings, &config).1, "pages.branch");
    f.world().default_branch = "main".into();
    // A branch of someone else's work: git makes it, without the publication mark.
    let tree = String::from_utf8(support::git_with_input(
        &["--git-dir", &f.bare.to_string_lossy(), "mktree"],
        b"",
    ))
    .unwrap();
    let other = support::git_with_input(
        &[
            "-c",
            "user.name=B",
            "-c",
            "user.email=b@example.org",
            "--git-dir",
            &f.bare.to_string_lossy(),
            "commit-tree",
            tree.trim(),
        ],
        b"Hand-made\n",
    );
    let other = String::from_utf8(other).unwrap();
    f.git(&["update-ref", "refs/heads/gh-pages", other.trim()]);
    assert_eq!(publish_site(&settings, &config).1, "pages.foreign");
    assert_eq!(bind_fixture(&settings, &config, false).unwrap_err().code, "pages.foreign");
    bind_fixture(&settings, &config, true).unwrap();
    assert_eq!(publish_site(&settings, &config).0, State::Live);
}

/// A publication planned before another moved the branch is refused; one whose answer
/// was lost is settled by reading the branch again; a failed deployment is not success.
#[test]
fn publications_settle_without_guessing() {
    if !has_git() {
        return;
    }
    let f = Fake::start("settle");
    let settings = f.settings();
    let site = f.write_site("out", &[("index.html", "<p>one</p>")]);
    let config = f.write_config(&site);
    bind_fixture(&settings, &config, false).unwrap();
    assert_eq!(publish_site(&settings, &config).0, State::Live);
    {
        // Plan, then let another publisher move the branch before publishing the plan.
        let mut s = load(&settings, &config).unwrap();
        f.write_site("out", &[("index.html", "<p>stale</p>")]);
        let snap = snapshot(&site, &SnapshotOptions::default()).unwrap();
        let plan = s.plan(&snap).unwrap();
        let head = f.head("gh-pages").unwrap();
        let tree = f.git(&["rev-parse", &format!("{head}^{{tree}}")]);
        let other = support::git_with_input(
            &[
                "-c",
                "user.name=B",
                "-c",
                "user.email=b@example.org",
                "--git-dir",
                &f.bare.to_string_lossy(),
                "commit-tree",
                &tree,
                "-p",
                &head,
            ],
            b"Guidedog-Publication: other\n",
        );
        let other = String::from_utf8(other).unwrap();
        f.git(&["update-ref", "refs/heads/gh-pages", other.trim()]);
        let r = s.publish(&snap, &plan);
        assert_eq!(r.problem.unwrap().code, "pages.conflict");
        assert_eq!(f.head("gh-pages").unwrap(), other.trim());
    }
    let moved = f.head("gh-pages").unwrap();
    f.write_site("out", &[("index.html", "<p>lost</p>")]);
    f.world().push = PushMode::Lose;
    let (state, code, _) = publish_site(&settings, &config);
    assert_eq!((state, code.as_str()), (State::Failed, "pages.network"));
    assert_eq!(f.head("gh-pages").unwrap(), moved);
    f.world().push = PushMode::ApplyThenLose;
    let (state, code, _) = publish_site(&settings, &config);
    assert_eq!(state, State::Live, "{code}");
    assert_ne!(f.head("gh-pages").unwrap(), moved);
    f.world().push = PushMode::Normal;
    f.world().build = "errored".into();
    f.write_site("out", &[("index.html", "<p>broken</p>")]);
    let (state, code, _) = publish_site(&settings, &config);
    assert_eq!((state, code.as_str()), (State::Failed, "pages.deployment"));
    f.world().build = "building".into();
    f.write_site("out", &[("index.html", "<p>slow</p>")]);
    assert_eq!(publish_site(&settings, &config).0, State::Pending);
    f.world().build = String::new();
    f.write_site("out", &[("index.html", "<p>quiet</p>")]);
    assert_eq!(publish_site(&settings, &config).0, State::Accepted);
}

/// recover settles a journal left by a publication that stopped after its push.
#[test]
fn recover_settles_a_left_journal() {
    if !has_git() {
        return;
    }
    let f = Fake::start("recover");
    let settings = f.settings();
    let site = f.write_site("out", &[("index.html", "<p>one</p>")]);
    let config = f.write_config(&site);
    bind_fixture(&settings, &config, false).unwrap();
    assert_eq!(publish_site(&settings, &config).0, State::Live);
    let key = target_key(7, "gh-pages");
    let head = f.head("gh-pages").unwrap();
    let j = Journal {
        operation: head.clone(),
        repository: "octo/site".into(),
        repository_id: 7,
        branch: "gh-pages".into(),
        observed: None,
        candidate: head,
        generation: "g".into(),
        accepted: false,
        started: today(),
    };
    let state = settings.state().unwrap();
    state.write_journal(&key, &j).unwrap();
    assert_eq!(publish_site(&settings, &config).1, "pages.unsettled");
    let mut s = load(&settings, &config).unwrap();
    let r = s.recover();
    assert_eq!(r.state, State::Live, "{:?}", r.problem);
    assert_eq!(state.read_journal(&key).unwrap(), None);
    assert_eq!(s.status("").state, State::Live);
    let head = f.head("gh-pages").unwrap();
    assert_eq!(s.status(&head).operation, head);
}

/// A successful push is not rolled back or forgotten when local settlement fails. A
/// folder at the last record's path forces a real rename failure after acceptance.
#[test]
fn accepted_publication_keeps_its_journal_until_recorded() {
    if !has_git() {
        return;
    }
    let f = Fake::start("record");
    let settings = f.settings();
    let site = f.write_site("out", &[("index.html", "one")]);
    let config = f.write_config(&site);
    bind_fixture(&settings, &config, false).unwrap();
    let key = target_key(7, "gh-pages");
    let state = settings.state().unwrap();
    let last = state.dir().join(format!("last-{key}.toml"));
    std::fs::create_dir(&last).unwrap();
    let (outcome, code, _) = publish_site(&settings, &config);
    assert_eq!((outcome, code.as_str()), (State::Accepted, "pages.record"));
    let j = state.read_journal(&key).unwrap().unwrap();
    assert_eq!(Some(j.candidate), f.head("gh-pages"));
    std::fs::remove_dir(&last).unwrap();
    let mut s = load(&settings, &config).unwrap();
    assert_eq!(s.recover().state, State::Live);
    assert_eq!(state.read_journal(&key).unwrap(), None);
}

#[test]
fn dry_runs_do_not_share_a_build_folder() {
    if !has_git() {
        return;
    }
    let f = Fake::start("dry");
    let settings = f.settings();
    let site = f.write_site("out", &[("index.html", "one")]);
    let config = f.write_config(&site);
    bind_fixture(&settings, &config, false).unwrap();
    let c = read_config(&config).unwrap();
    for _ in 0..2 {
        let one = gd_pages::build_dir(&settings, &c, true).unwrap();
        let two = gd_pages::build_dir(&settings, &c, true).unwrap();
        assert_ne!(one, two);
        std::fs::write(one.join("index.html"), "one").unwrap();
        assert!(!two.join("index.html").exists());
        std::fs::remove_dir_all(one).unwrap();
        std::fs::remove_dir_all(two).unwrap();
    }
    assert!(gd_pages::build_dir(&settings, &c, false).unwrap().ends_with("site"));
}

/// The device flow waits while authorization is pending, slows down when asked, and
/// returns the token.
#[test]
fn device_login_follows_github() {
    let f = Fake::start("device");
    let mut c = Client::new(gd_pages::Secret::default(), f.settings().roots, Cancel::default());
    c.poll_unit = Duration::ZERO;
    assert_eq!(start_device(&c, "", "public_repo").unwrap_err().code, "pages.client-id");
    let d = start_device(&c, "Iv1.test", "public_repo").unwrap();
    assert_eq!(d.user_code, "ABCD-1234");
    assert_eq!(d.interval, 5);
    let token = wait_device(&c, "Iv1.test", &d).unwrap();
    assert_eq!(token.expose(), TOKEN);
    assert_eq!(f.world().polls, 3);
    c.cancel = Cancel::from_fn(|| true);
    assert_eq!(wait_device(&c, "Iv1.test", &d).unwrap_err().code, "pages.cancelled");
}
