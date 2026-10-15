//! The example publisher and a faulty one, run as programs through the protocol.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

mod support;

use std::time::Duration;

use gd_plugins::{
    LoadOptions, PublisherSession, SnapshotOptions, State, digest_hex, inspect_entry,
    inspect_program, process_publisher, read_machine, register, snapshot, unregister,
    verify_entry,
};
use support::{Scratch, text};

// The example publisher, registered on a machine registry of its own, publishes
// through the process protocol: a plan, a publication, the same site again
// (unchanged), and its status. A changed program is refused until registered again.
#[test]
fn an_installed_publisher_publishes_through_the_protocol() {
    let root = Scratch::new("folder");
    let program = root.program("publisher", "folder-publisher");
    let registry = root.join("state/plugins.toml");
    let entry = inspect_entry(&program).unwrap();
    assert_eq!(entry.id, "folder");
    assert!(entry.capabilities.status && !entry.capabilities.recover);
    register(&registry, &entry).unwrap();
    assert_eq!(register(&registry, &entry).unwrap_err().code, "plugins.registered");
    let machine = read_machine(&registry).unwrap();
    let saved = machine.find("folder").unwrap();
    assert_eq!(saved.sha256, entry.sha256);
    verify_entry(saved).unwrap();

    root.write("site/index.html", "<p>one</p>");
    root.write("site/_static/a.css", "p{}");
    let config = root.write("folder.toml", "site = \"site\"\ntarget = \"www\"\n");
    let plugin = process_publisher(saved).unwrap();
    let options = LoadOptions { config: config.to_string_lossy().into(), ..Default::default() };
    let mut s = PublisherSession::load(&plugin, &options).unwrap();
    assert_eq!(s.source().site, root.join("site").to_string_lossy());
    for round in 0..2 {
        let snap = snapshot(&s.source().site, &SnapshotOptions::default()).unwrap();
        let plan = s.plan(&snap).unwrap();
        assert_eq!(plan.unchanged, round == 1);
        assert_eq!(plan.changes.len(), if round == 0 { 2 } else { 0 });
        let r = s.publish(&snap, &plan);
        let want = if round == 0 { State::Live } else { State::Unchanged };
        assert_eq!(r.state, want, "round {round}: {r:?}");
    }
    assert_eq!(text(&root.join("www/index.html")), "<p>one</p>");
    assert_eq!(s.status("").state, State::Live);
    drop(s);

    // A program that changed on disk is refused until it is registered again.
    let mut bytes = std::fs::read(&program).unwrap();
    bytes.push(0);
    std::fs::remove_file(&program).unwrap();
    std::fs::write(&program, bytes).unwrap();
    assert_eq!(verify_entry(saved).unwrap_err().code, "plugins.changed");
    unregister(&registry, "folder").unwrap();
    assert_eq!(unregister(&registry, "folder").unwrap_err().code, "plugins.unregistered");
}

#[test]
fn a_dry_run_takes_no_lock_and_writes_nothing() {
    let root = Scratch::new("folder-dry");
    let program = root.program("publisher", "folder-publisher");
    let plugin = process_publisher(&inspect_entry(&program).unwrap()).unwrap();
    root.write("site/index.html", "one");
    let config = root.write("folder.toml", "site = \"site\"\ntarget = \"www\"\n");
    let options = LoadOptions {
        config: config.to_string_lossy().into(),
        dry_run: true,
        ..Default::default()
    };
    let mut s = PublisherSession::load(&plugin, &options).unwrap();
    let snap = snapshot(&s.source().site, &SnapshotOptions::default()).unwrap();
    let plan = s.plan(&snap).unwrap();
    assert_eq!(s.publish(&snap, &plan).problem.unwrap().code, "plugins.dry-run");
    assert!(!root.join("www").exists());
}

// A real write followed by silence remains Unknown with its generation and timeout,
// and is not retried.
#[test]
fn publisher_process_failures_are_bounded_and_truthful() {
    let root = Scratch::new("fixture");
    let program = root.program("protocol_fixture", "fixture");
    let mut entry = inspect_entry(&program).unwrap();
    entry.capabilities = Default::default();
    let plugin = process_publisher(&entry).unwrap();
    for mode in ["malformed", "refused", "write", "stall-unload"] {
        let site = root.write(&format!("{mode}/index.html"), "one");
        let site = site.parent().unwrap().to_string_lossy().into_owned();
        let options = LoadOptions {
            config: site.clone(),
            request_timeout: Duration::from_secs(1),
            ..Default::default()
        };
        let loaded = PublisherSession::load(&plugin, &options);
        if mode == "malformed" || mode == "refused" {
            assert_eq!(loaded.unwrap_err().code, "plugins.protocol", "{mode}");
            continue;
        }
        let mut s = loaded.unwrap();
        if mode == "write" {
            let snap = snapshot(&site, &SnapshotOptions::default()).unwrap();
            let plan = s.plan(&snap).unwrap();
            let r = s.publish(&snap, &plan);
            assert_eq!(r.state, State::Unknown);
            assert_eq!(r.problem.unwrap().code, "plugins.timeout");
            assert_eq!(r.generation, digest_hex(&snap.digest()));
            assert!(root.join("write/written").exists());
        }
        drop(s); // stall-unload: the bounded unload ends in about a second.
    }
    // Inspection cannot wait forever for a program that never sends its handshake.
    let silent = root.program("protocol_fixture", "silent");
    let p = inspect_program(&silent, Duration::from_millis(300)).unwrap_err();
    assert_eq!(p.code, "plugins.timeout");
    assert!(p.hint.contains("recover"));
}
