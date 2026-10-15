//! `watch --serve`: the preview serves the latest build while the watch republishes,
//! and one cancel stops both.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.
#![cfg(unix)] // the fake guidedog is a shell script.

mod support;

use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use gd_pages::command::{Program, run_with};
use gd_plugins::Cancel;
use support::{Fake, has_git};

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

/// Waits until `check` holds, for at most 30 seconds.
fn eventually(what: &str, mut check: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(30);
    while !check() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn page(url: &str) -> String {
    gd_fetch::get(url, &gd_fetch::Options::default())
        .map(|f| String::from_utf8_lossy(&f.data).into_owned())
        .unwrap_or_default()
}

#[test]
fn the_preview_serves_the_latest_build() {
    if !has_git() {
        return;
    }
    let f = Fake::start("preview");
    let project = f.root.join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(project.join("index.html"), "<p>one</p>").unwrap();
    let guidedog = f.root.join("guidedog");
    std::fs::write(
        &guidedog,
        "#!/bin/sh\nrm -rf \"$5\" && mkdir -p \"$5\" && \
        cp \"$4\"/*.html \"$5\"/\n",
    )
    .unwrap();
    std::fs::set_permissions(&guidedog, std::fs::Permissions::from_mode(0o755)).unwrap();
    let config = f.root.join("pages.toml");
    std::fs::write(
        &config,
        format!(
            "schema = 1\nrepository = \"octo/site\"\nproject = '{}'\n\
        wait_seconds = 0\n[watch]\ndebounce_ms = 50\nmax_wait_ms = 200\n",
            project.display()
        ),
    )
    .unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let program = Program {
        settings: f.settings(),
        cancel: Cancel::from_flag(stop.clone()),
        ..Program::default()
    };
    let config_flag = format!("--config={}", config.display());
    let run = |words: &[&str]| {
        let args: Vec<String> = words.iter().map(|w| w.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let status = run_with(&program, &args, &mut out, &mut err);
        (status, String::from_utf8(err).unwrap())
    };
    assert_eq!(run(&["bind", "--yes", &config_flag]).0, 0);
    let port = free_port();
    let url = format!("http://127.0.0.1:{port}/index.html");
    let guidedog_flag = format!("--guidedog={}", guidedog.display());
    let serve_flag = format!("--serve={port}");
    std::thread::scope(|scope| {
        let watching = scope.spawn(|| run(&["watch", &config_flag, &guidedog_flag, &serve_flag]));
        eventually("the first build", || page(&url) == "<p>one</p>");
        eventually("the first publication", || f.head("gh-pages").is_some());
        let first = f.head("gh-pages");
        std::fs::write(project.join("index.html"), "<p>two</p>").unwrap();
        eventually("the rebuilt preview", || page(&url) == "<p>two</p>");
        eventually("the second publication", || f.head("gh-pages") != first);
        stop.store(true, Ordering::SeqCst);
        let (status, err) = watching.join().unwrap();
        assert_eq!(status, 0, "{err}");
        assert!(
            err.contains("Previewing at http://127.0.0.1:") && err.contains("Stopped"),
            "{err}"
        );
    });
    assert!(page(&url).is_empty(), "the preview stopped with the watch");
}
