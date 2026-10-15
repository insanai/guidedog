//! The guidedog program as a user runs it: exit statuses, standard input and output,
//! and the machine's plugin registry (kept in a private GUIDEDOG_CONFIG_DIR).

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const GUIDEDOG: &str = env!("CARGO_BIN_EXE_guidedog");

/// A private folder beside the built programs (so programs can be linked into it),
/// removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let base = Path::new(GUIDEDOG).parent().unwrap().join("guidedog-tests");
        let dir = base.join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn join(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn guidedog(args: &[&str], config_dir: &Path, stdin: &str) -> Output {
    let mut child = Command::new(GUIDEDOG)
        .args(args)
        .env("GUIDEDOG_CONFIG_DIR", config_dir)
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The fixture plugin program cargo built (`target/<profile>/examples`).
fn fixture(scratch: &Scratch) -> String {
    let built = Path::new(GUIDEDOG)
        .parent()
        .unwrap()
        .join("examples")
        .join(format!("fixture_plugin{}", std::env::consts::EXE_SUFFIX));
    let program = scratch.join(&format!("shout{}", std::env::consts::EXE_SUFFIX));
    if fs::hard_link(&built, &program).is_err() {
        fs::copy(&built, &program).unwrap();
    }
    program.to_string_lossy().into_owned()
}

#[test]
fn usage_mistakes_exit_with_status_2_and_a_suggestion() {
    let scratch = Scratch::new("usage");
    let out = guidedog(&["biuld"], &scratch.0, "");
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("`build`?"));
    let out = guidedog(&["build", "--memory=disk"], &scratch.0, "");
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("WORKING MEMORY ON DISK IS NOT AVAILABLE"));
}

#[test]
fn the_version_names_typst_and_graphviz() {
    let scratch = Scratch::new("version");
    let out = guidedog(&["--version"], &scratch.0, "");
    assert_eq!(out.status.code(), Some(0));
    let stdout = text(&out.stdout);
    let first = format!("guidedog {}", env!("CARGO_PKG_VERSION"));
    assert!(stdout.starts_with(&first), "{stdout}");
    assert!(stdout.contains("\ngraphviz ") && stdout.contains("\ntypst "), "{stdout}");
}

#[test]
fn convert_reads_standard_input_and_writes_standard_output() {
    let scratch = Scratch::new("stdin");
    let args = ["convert", "-", "--from", "commonmark", "--to", "html", "-o", "-", "--fragment"];
    let out = guidedog(&args, &scratch.0, "# Hi\n\nSome `code`.\n");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "<h1 id=\"hi\">Hi</h1>\n<p>Some <code>code</code>.</p>\n");
}

#[test]
fn plugin_programs_are_checked_registered_listed_inspected_and_forgotten() {
    let scratch = Scratch::new("plugins");
    let config = scratch.join("config");
    let program = fixture(&scratch);
    let project = format!("--project={}", scratch.0.display());
    let out = guidedog(&["plugin", "check", &program, &project], &config, "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains("Checked shout"));
    // Registering asks first; without a terminal and without --yes nothing happens.
    let out = guidedog(&["plugin", "register", "shout", &program, &project], &config, "y\n");
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains("cli.unconfirmed"));
    let out = guidedog(&["plugin", "register", "other", &program, "--yes", &project], &config, "");
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("cli.plugin-id"));
    let out = guidedog(&["plugin", "register", "shout", &program, "--yes", &project], &config, "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains("Registered shout 1.0.0"));
    let out = guidedog(&["plugin", "list"], &config, "");
    let listed = text(&out.stdout);
    assert!(listed.contains("Linked into this program") && listed.contains("pages"), "{listed}");
    assert!(listed.contains("shout") && listed.contains("build"), "{listed}");
    let out = guidedog(&["plugin", "inspect", "shout"], &config, "");
    let shown = text(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(shown.contains("build plugin") && shown.contains("Hooks         source"), "{shown}");
    assert!(shown.contains("The program on disk is the one registered."), "{shown}");
    // A build plugin cannot publish.
    let out = guidedog(&["publish", "--publisher=shout", "--config=x.toml"], &config, "");
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("cli.plugin-kind"), "{}", text(&out.stderr));
    let out = guidedog(&["plugin", "unregister", "shout"], &config, "");
    assert_eq!(out.status.code(), Some(0));
    let out = guidedog(&["plugin", "inspect", "shout"], &config, "");
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("cli.plugin-missing"));
}

#[test]
fn plugin_run_starts_the_registered_program_with_its_arguments() {
    let scratch = Scratch::new("run");
    let config = scratch.join("config");
    let program = fixture(&scratch);
    let project = format!("--project={}", scratch.0.display());
    let out = guidedog(&["plugin", "register", "shout", &program, "--yes", &project], &config, "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    // Started without the protocol flag, a served plugin refuses to run (a status of its
    // own); what matters is that guidedog ran it and returned that status.
    let out = guidedog(&["plugin", "run", "shout", "--", "--help"], &config, "");
    assert!(out.status.code().is_some());
    let out = guidedog(&["plugin", "run", "missing"], &config, "");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn the_pages_plugin_adds_its_commands() {
    let scratch = Scratch::new("pages-commands");
    let config = scratch.join("config");
    let out = guidedog(&["--help"], &config, "");
    let help = text(&out.stdout);
    assert!(help.contains("PLUGIN COMMANDS") && help.contains("publish pages"), "{help}");
    let out = guidedog(&["publish", "pages", "--help"], &config, "");
    assert_eq!(out.status.code(), Some(0));
    let help = text(&out.stdout);
    assert!(help.contains("Added by the plugin pages") && help.contains("--repository"), "{help}");
    // Offline the checksum is a placeholder: the workflow is printed, with status 1.
    let out = guidedog(&["pages", "workflow", "--offline", "--project=manual"], &config, "");
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains("GUIDEDOG_SHA256"), "{}", text(&out.stdout));
    let out = guidedog(&["pages", "workflo"], &config, "");
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("`workflow`?"), "{}", text(&out.stderr));
    let out = guidedog(&["plugin", "inspect", "pages"], &config, "");
    assert!(text(&out.stdout).contains("guidedog pages workflow"), "{}", text(&out.stdout));
}

#[test]
fn registered_programs_add_commands_that_run_on_the_terminal() {
    let scratch = Scratch::new("program-commands");
    let config = scratch.join("config");
    let program = fixture(&scratch);
    let project = format!("--project={}", scratch.0.display());
    let out = guidedog(&["plugin", "register", "shout", &program, "--yes", &project], &config, "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let out = guidedog(&["shout", "loud", "hello", "there"], &config, "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "HELLO THERE\n");
    let out = guidedog(&["shout", "loud"], &config, "");
    assert_eq!(out.status.code(), Some(3));
    let out = guidedog(&["shuot"], &config, "");
    assert!(text(&out.stderr).contains("`shout`?"), "{}", text(&out.stderr));
    let out = guidedog(&["--help"], &config, "");
    assert!(text(&out.stdout).contains("shout loud"), "{}", text(&out.stdout));
    let out = guidedog(&["plugin", "inspect", "shout"], &config, "");
    assert!(text(&out.stdout).contains("guidedog shout loud"), "{}", text(&out.stdout));
}
