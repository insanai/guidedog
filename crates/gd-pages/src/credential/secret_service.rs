//! Linux and other Unix desktops: tokens are passwords in the Secret Service (GNOME
//! Keyring, KWallet, KeePassXC), reached through the `secret-tool` program of
//! libsecret. The token goes to `secret-tool` on its standard input and comes back on
//! its standard output, never on a command line. A system without `secret-tool` can
//! still publish with the environment's token.
//!
//! Items carry fixed attributes (`xdg:schema`, `service`, `profile`), the same in every
//! release, so tokens stored by an earlier release are found.

use std::io::Write;
use std::process::{Command, Output, Stdio};

use gd_plugins::Problem;

use super::{Secret, forget_bytes, missing_token, store_problem, stored_secret};

/// The store's name in messages.
pub(super) const STORE_NAME: &str = "the Secret Service (secret-tool)";

const SCHEMA: &str = "org.guidedog.Pages";
const LABEL: &str = "--label=Guidedog Pages publisher token";

fn command(action: &str, service: &str, profile: &str) -> Command {
    let mut c = Command::new("secret-tool");
    c.arg(action);
    if action == "store" {
        c.arg(LABEL);
    }
    c.args(["xdg:schema", SCHEMA, "service", service, "profile", profile]);
    c.stdout(Stdio::piped()).stderr(Stdio::piped());
    c
}

/// Runs `c` with `input` on its standard input (none when empty).
fn run(mut c: Command, input: &str) -> Result<Output, Problem> {
    c.stdin(if input.is_empty() { Stdio::null() } else { Stdio::piped() });
    let mut child = c.spawn().map_err(|e| unavailable(&e))?;
    if let Some(mut stdin) = child.stdin.take() {
        // A failed write shows in secret-tool's status.
        let _ = stdin.write_all(input.as_bytes());
    }
    child.wait_with_output().map_err(|e| unavailable(&e))
}

fn unavailable(e: &std::io::Error) -> Problem {
    let mut p = store_problem("secret-tool could not be run", &format!(" ({e})."));
    if e.kind() == std::io::ErrorKind::NotFound {
        p.message = "secret-tool is not installed, so no desktop credential store can be \
                     reached."
            .to_string();
        p.hint = "Install secret-tool (the libsecret-tools or libsecret package), or give a \
                  token to one command with GUIDEDOG_PAGES_TOKEN and --credential=env."
            .to_string();
    }
    p
}

fn failure(what: &str, out: &Output) -> Problem {
    let said = String::from_utf8_lossy(&out.stderr);
    let said = said.trim();
    let detail = if said.is_empty() { ".".to_string() } else { format!(": {said}.") };
    store_problem(what, &detail)
}

pub(super) fn store(service: &str, profile: &str, token: &str) -> Result<(), Problem> {
    let out = run(command("store", service, profile), token)?;
    if !out.status.success() {
        return Err(failure("The Secret Service refused to store the token", &out));
    }
    Ok(())
}

pub(super) fn load(service: &str, profile: &str) -> Result<Secret, Problem> {
    let mut out = run(command("lookup", service, profile), "")?;
    let stdout = std::mem::take(&mut out.stdout);
    if !out.status.success() || stdout.is_empty() {
        forget_bytes(stdout);
        // secret-tool says nothing when there is no such item.
        if out.stderr.iter().all(u8::is_ascii_whitespace) {
            return Err(missing_token(profile));
        }
        return Err(failure("The Secret Service refused to read the token", &out));
    }
    let end = stdout.iter().rposition(|c| !c.is_ascii_whitespace()).map_or(0, |i| i + 1);
    let secret = stored_secret(&stdout[..end]);
    forget_bytes(stdout);
    secret
}

pub(super) fn delete(service: &str, profile: &str) -> Result<bool, Problem> {
    let found = match load(service, profile) {
        Ok(_) => true,
        Err(p) if p.code == "pages.no-token" => false,
        Err(p) => return Err(p),
    };
    if !found {
        return Ok(false);
    }
    let out = run(command("clear", service, profile), "")?;
    if !out.status.success() {
        return Err(failure("The Secret Service refused to delete the token", &out));
    }
    Ok(true)
}
