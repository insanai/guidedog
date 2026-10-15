//! `release smoke-pages` and `release credential-store`: guidedog-pages run offline the
//! way a new user starts, and gd-pages' native credential adapter tested against a real,
//! private Linux Secret Service.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::args::{self, Spec};
use crate::fsutil::{TempDir, create_dir, read_text};
use crate::{Failure, Result, fail, process, repository_root};

const SMOKE_HELP: &str = "\
Smoke-tests guidedog-pages and guidedog publish the way a new user starts, offline: a
pages.toml, the program registered with guidedog, and publications that stop before
reaching GitHub because the target is not yet authorized on this machine.

Usage: cargo xtask release smoke-pages PAGES-PROGRAM GUIDEDOG

Machine state goes to a temporary folder (GUIDEDOG_CONFIG_DIR, GUIDEDOG_CACHE_DIR), so
the user's registry, bindings and tokens are never touched.
";

const STORE_HELP: &str = "\
Runs the native credential adapter of gd-pages against an isolated, real Linux Secret
Service (gnome-keyring through secret-tool). No GitHub token is used, and no existing
desktop keyring is opened or changed.

Usage: cargo xtask release credential-store

Needs dbus-run-session, gnome-keyring and libsecret-tools.
";

/// The flag the task passes to itself inside the private D-Bus session.
const IN_SESSION: &str = "--in-session";

fn pages_failure(what: &str) -> Failure {
    fail(
        "smoke",
        "SMOKE TEST FAILED",
        &format!("pages smoke test: {what}."),
        "Run the same command by hand with --diagnostics=json to see what it reports, then \
         fix it.",
    )
}

/// Runs guidedog and guidedog-pages with machine state in a temporary folder.
struct Pages<'a> {
    work: &'a Path,
}

impl Pages<'_> {
    fn command(&self, program: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(program);
        command.current_dir(self.work).args(args);
        command.env("GUIDEDOG_CONFIG_DIR", self.work.join("state"));
        command.env("GUIDEDOG_CACHE_DIR", self.work.join("cache"));
        command
    }

    /// The command fails, reporting the problem `code` as JSON.
    fn expect(&self, code: &str, program: &Path, args: &[&str]) -> Result<()> {
        let mut command = self.command(program, args);
        command.arg("--diagnostics=json");
        let line = process::describe(&command);
        let out = process::capture(&mut command)?;
        if out.success {
            return Err(pages_failure(&format!("`{line}` succeeded; it should stop with {code}")));
        }
        if !out.stderr.contains(&format!("\"code\":\"{code}\"")) {
            eprint!("{}", out.stderr);
            return Err(pages_failure(&format!("`{line}` did not report {code}")));
        }
        Ok(())
    }
}

/// Runs `release smoke-pages`.
pub(super) fn smoke_main(args: &[String]) -> Result<()> {
    let spec = Spec { command: "release smoke-pages", flags: &[], options: &[], help: SMOKE_HELP };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(2, &spec)?;
    let program = super::given_path(parsed.positional(0, "the guidedog-pages program", &spec)?);
    let guidedog = super::given_path(parsed.positional(1, "the guidedog program", &spec)?);
    smoke_pages(&program, &guidedog)
}

/// Smoke-tests guidedog-pages with guidedog.
pub(crate) fn smoke_pages(program: &Path, guidedog: &Path) -> Result<()> {
    let temp = TempDir::new("pages")?;
    let pages = Pages { work: temp.path() };
    let version = process::output(&mut pages.command(program, &["--version"]))?;
    let first = version.lines().next().unwrap_or_default();
    if !first.starts_with("guidedog-pages ") {
        return Err(pages_failure(&format!("{} reports \"{first}\"", program.display())));
    }
    let quickstart = &["quickstart", "docs", "-q", "-p", "Demo", "-a", "Release"];
    process::output(&mut pages.command(guidedog, quickstart))?;
    let init = &["init", "--repository=octo/manual", "--project=docs", "--color=never"];
    process::run(&mut pages.command(program, init))?;
    if !read_text(&temp.path().join("pages.toml"))?.contains("repository = \"octo/manual\"") {
        return Err(pages_failure("init wrote no repository"));
    }
    pages.expect("pages.usage", program, &["publish", "--no-such-option"])?;
    let engine = format!("--guidedog={}", guidedog.display());
    pages.expect("pages.unbound", program, &["publish", "--dry-run", &engine])?;
    let registered = program.to_string_lossy().into_owned();
    process::run(
        &mut pages.command(guidedog, &["plugin", "register", "pages", &registered, "--yes"]),
    )?;
    let inspect = process::output(&mut pages.command(guidedog, &["plugin", "inspect", "pages"]))?;
    if !inspect.contains("is the one registered") {
        return Err(pages_failure("inspect failed"));
    }
    let publish = &["publish", "--publisher=pages", "--config=pages.toml"];
    pages.expect("pages.unbound", guidedog, &[publish.as_slice(), &["--dry-run"]].concat())?;
    process::run(&mut pages.command(guidedog, &["plugin", "unregister", "pages"]))?;
    // guidedog links the Pages publisher in (gd_cli's default registry), so after the
    // registered program is gone `--publisher=pages` still finds the linked one, which
    // stops safely at the same authorization check. A plugin that is neither linked
    // nor registered is reported as missing.
    pages.expect("pages.unbound", guidedog, publish)?;
    let missing = &["publish", "--publisher=no-such-publisher", "--config=pages.toml"];
    pages.expect("cli.plugin-missing", guidedog, missing)?;
    let name = program.file_name().unwrap_or_default().to_string_lossy();
    println!("pages smoke test passed: {name} registered; each publication stopped safely");
    Ok(())
}

/// Runs `release credential-store`: outside a session it starts a private D-Bus session
/// that runs this task again with [`IN_SESSION`]; inside, it unlocks a fresh keyring and
/// runs the test.
pub(super) fn credential_store_main(args: &[String]) -> Result<()> {
    let spec = Spec {
        command: "release credential-store",
        flags: &[IN_SESSION],
        options: &[],
        help: STORE_HELP,
    };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(0, &spec)?;
    if parsed.has(IN_SESSION) {
        return test_in_session();
    }
    let root = TempDir::new("keyring")?;
    let runtime = root.path().join("run");
    create_dir(&runtime)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let private = std::fs::Permissions::from_mode(0o700);
        std::fs::set_permissions(&runtime, private)
            .map_err(|err| crate::fsutil::io_failure("change", &runtime, &err))?;
    }
    let me = std::env::current_exe()
        .map_err(|err| crate::fsutil::io_failure("find", Path::new("xtask"), &err))?;
    let mut session = Command::new("dbus-run-session");
    session.arg("--").arg(me).args(["release", "credential-store", IN_SESSION]);
    session.env("XDG_DATA_HOME", root.path().join("data"));
    session.env("XDG_CONFIG_HOME", root.path().join("config"));
    session.env("XDG_CACHE_HOME", root.path().join("cache"));
    session.env("XDG_RUNTIME_DIR", &runtime);
    process::run(&mut session)
}

fn test_in_session() -> Result<()> {
    let mut keyring = Command::new("gnome-keyring-daemon");
    // The daemon forks and keeps its output open, so nothing waits on its output.
    keyring.args(["--unlock", "--components=secrets"]);
    keyring.stdin(Stdio::piped()).stdout(Stdio::null());
    let mut child = keyring.spawn().map_err(|err| process::spawn_failure(&keyring, &err))?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(b"guidedog-fixture-password\n");
    }
    let status = child.wait().map_err(|err| process::spawn_failure(&keyring, &err))?;
    if !status.success() {
        return Err(fail(
            "keyring",
            "KEYRING DID NOT START",
            "gnome-keyring-daemon could not unlock a fresh keyring.",
            "Install gnome-keyring and run the step again; its messages above say more.",
        ));
    }
    // GUIDEDOG_TEST_REQUIRE_STORE turns an unreachable store into a test failure.
    let mut cargo = Command::new("cargo");
    cargo.current_dir(repository_root()).env("GUIDEDOG_TEST_REQUIRE_STORE", "1");
    cargo.args([
        "test",
        "--locked",
        "-p",
        "gd-pages",
        "--lib",
        "the_credential_store_keeps_tokens",
    ]);
    process::run(&mut cargo)
}
