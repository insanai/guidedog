//! The generated GitHub Actions workflow: the manual's workflow exactly, its checks,
//! and the release checksum read from SHA256SUMS.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

mod support;

use gd_fetch::testserver::{self, Reply};
use gd_pages::command::{Program, run_with};
use gd_pages::{
    SHA256_PLACEHOLDER, WORKFLOW_PATH, WorkflowOptions, check_workflow, release_sha256,
    render_workflow,
};

const MANUAL_SHA: &str = "35eca388ac7a4061076bfa688102127acdb2f1db30e183c8a5b8002362e359d0";
const MANUAL: &str = include_str!("data/docs-pages-0.6.3.yml");

fn manual_options() -> WorkflowOptions {
    WorkflowOptions {
        version: "0.6.3".into(),
        sha256: Some(MANUAL_SHA.into()),
        ..WorkflowOptions::default()
    }
}

#[test]
fn the_workflow_is_the_manuals() {
    assert_eq!(render_workflow(&manual_options()), MANUAL);
    let other = WorkflowOptions {
        project: "site/docs".into(),
        branch: "trunk".into(),
        ..manual_options()
    };
    let text = render_workflow(&other);
    assert!(text.contains("branches: [trunk]") && text.contains("refs/heads/trunk'"));
    assert!(text.contains("guidedog build -b html site/docs build/site"));
    assert!(!text.contains("pull_request_target") && text.contains("persist-credentials: false"));
    let placeholder = render_workflow(&WorkflowOptions::default());
    assert!(placeholder.contains(SHA256_PLACEHOLDER) && placeholder.contains("# PLACEHOLDER"));
}

#[test]
fn checks_find_old_versions_and_moved_pins() {
    let current = WorkflowOptions { version: "0.7.0".into(), ..manual_options() };
    assert!(check_workflow("w.yml", &render_workflow(&current), &current).is_empty());
    let codes = |text: &str| -> Vec<String> {
        check_workflow("w.yml", text, &current).into_iter().map(|p| p.code.clone()).collect()
    };
    assert_eq!(codes(MANUAL), ["pages.workflow-outdated"]);
    let newer = WorkflowOptions { version: "0.10.0".into(), ..manual_options() };
    assert!(check_workflow("w.yml", &render_workflow(&newer), &current).is_empty());
    let moved = MANUAL.replace("368f82528645a54fb793d4d04e342629a3f51346", &"a".repeat(40));
    assert!(codes(&moved).contains(&"pages.workflow-pin".to_string()));
    let tagged = MANUAL.replace(
        "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
        "actions/checkout@v4",
    );
    assert!(codes(&tagged).contains(&"pages.workflow-unpinned".to_string()));
    let target = MANUAL.replace("  pull_request:\n", "  pull_request_target:\n");
    assert!(codes(&target).contains(&"pages.workflow-trigger".to_string()));
    let keeps = MANUAL.replace("          persist-credentials: false\n", "");
    assert!(codes(&keeps).contains(&"pages.workflow-credentials".to_string()));
    let unchecked = render_workflow(&WorkflowOptions { sha256: None, ..current.clone() });
    assert_eq!(codes(&unchecked), ["pages.workflow-sha256"]);
    let p = &check_workflow("w.yml", MANUAL, &current)[0];
    assert!(p.path.starts_with("w.yml:") && p.hint.contains("--write"), "{p}");
}

#[test]
fn checksums_come_from_the_release() {
    let server = testserver::start(|r| {
        if r.path == "/insanai/guidedog/releases/download/v0.6.3/SHA256SUMS" {
            let body = format!(
                "{}  guidedog-darwin-arm64.tar.gz\n{MANUAL_SHA}  \
                guidedog-linux-amd64.tar.gz\n",
                "b".repeat(64)
            );
            return Reply { status: 200, body: body.into(), ..Reply::default() };
        }
        Reply { status: 404, ..Reply::default() }
    })
    .unwrap();
    let root = server.url("");
    let root = root.trim_end_matches('/');
    assert_eq!(release_sha256(root, "insanai/guidedog", "0.6.3").unwrap(), MANUAL_SHA);
    let p = release_sha256(root, "insanai/guidedog", "9.9.9").unwrap_err();
    assert_eq!(p.code, "pages.release");
    assert!(p.hint.contains("--sha256"));
}

#[test]
fn the_workflow_command_writes_and_checks() {
    let dir = support::scratch("workflow-command");
    let program = Program { dir: Some(dir.clone()), ..Program::default() };
    let run = |words: &[&str]| {
        let args: Vec<String> = words.iter().map(|w| w.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let status = run_with(&program, &args, &mut out, &mut err);
        (status, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    };
    let (status, out, err) = run(&["workflow", "--offline", "--color=never"]);
    assert_eq!(status, 1);
    assert!(out.contains(SHA256_PLACEHOLDER) && err.contains("pages.workflow-sha256"), "{err}");
    let sha = format!("--sha256={MANUAL_SHA}");
    let write = ["workflow", "--write", "--version=0.6.3", sha.as_str()];
    let (status, _, err) = run(&write);
    assert_eq!(status, 0, "{err}");
    assert_eq!(std::fs::read_to_string(dir.join(WORKFLOW_PATH)).unwrap(), MANUAL);
    assert_eq!(run(&["workflow", "--check", "--version=0.6.3"]).0, 0);
    let (status, _, err) = run(&["workflow", "--check", "--color=never"]);
    assert_eq!(status, 1);
    assert!(err.contains("pages.workflow-outdated"), "{err}");
    let (status, _, err) = run(&["workflow", "--write", sha.as_str(), "--branch=trunk"]);
    assert_eq!(status, 2, "{err}");
    assert!(err.contains("pages.exists"), "{err}");
    let mut forced = write.to_vec();
    forced.push("--force");
    assert_eq!(run(&forced).0, 0);
    assert_eq!(run(&["workflow", "--project=../x", "--offline"]).0, 2);
    let _ = std::fs::remove_dir_all(&dir);
}
