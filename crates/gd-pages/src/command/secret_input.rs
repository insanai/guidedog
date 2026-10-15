//! Reads a token from standard input: on a terminal after a prompt, without showing what
//! is typed; otherwise all of the input, as a secret manager pipes it. A token is never
//! taken from the command line, where other users' tools can see it.
//!
//! Echo is turned off on Unix with the `stty` program (`stty -g` saves the terminal's
//! settings, `stty -echo` hides typing, and the saved settings are restored), run on the
//! inherited standard input. `stty` is required by POSIX and present on every Unix,
//! while a hand-written `termios` binding would have to match each system's structure
//! layout (field widths and `NCCS` differ between Linux, macOS and the BSDs) in unsafe
//! code. On Windows the console mode is changed through `windows_console`.

use std::io::{IsTerminal, Read};

use gd_plugins::Problem;

use crate::credential::{MAX_TOKEN, Secret, forget_bytes, is_valid_token};
use crate::problem::problem;

/// Reads a token, showing `prompt` first on a terminal. Fails with `pages.token` when
/// the input is not a token.
pub(crate) fn read_secret(prompt: &str) -> Result<Secret, Problem> {
    let stdin = std::io::stdin();
    let terminal = stdin.is_terminal();
    let mut buffer = Vec::with_capacity(MAX_TOKEN + 8);
    if terminal {
        eprint!("  {prompt}");
        let _hidden = HiddenInput::begin();
        let mut line = String::new();
        let _ = stdin.read_line(&mut line);
        buffer.extend_from_slice(line.as_bytes());
        crate::credential::forget_string(line);
        eprintln!();
    } else {
        let _ = stdin.lock().take(MAX_TOKEN as u64 + 8).read_to_end(&mut buffer);
    }
    let token = std::str::from_utf8(&buffer).map(|t| Secret::new(t.trim())).ok();
    forget_bytes(buffer);
    match token {
        Some(t) if is_valid_token(t.expose()) => Ok(t),
        _ => Err(problem(
            "pages.token",
            "This is not a GitHub token",
            "standard input",
            "A token is one line of letters, digits, and underscores.",
            "Paste the whole token GitHub showed when you made it, and nothing else.",
        )
        .exit(2)),
    }
}

/// The terminal's echo turned off until the value is dropped.
#[cfg(unix)]
struct HiddenInput {
    saved: Option<String>,
}

#[cfg(unix)]
impl HiddenInput {
    fn begin() -> HiddenInput {
        use std::process::{Command, Stdio};
        let saved = Command::new("stty")
            .arg("-g")
            .stdin(Stdio::inherit())
            .stderr(Stdio::null())
            .output()
            .ok()
            .filter(|out| out.status.success())
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        if saved.is_some() {
            let _ = Command::new("stty").arg("-echo").stdin(Stdio::inherit()).status();
        }
        HiddenInput { saved }
    }
}

#[cfg(unix)]
impl Drop for HiddenInput {
    fn drop(&mut self) {
        if let Some(saved) = &self.saved {
            let _ = std::process::Command::new("stty")
                .arg(saved)
                .stdin(std::process::Stdio::inherit())
                .status();
        }
    }
}

/// The console's echo turned off until the value is dropped.
#[cfg(windows)]
struct HiddenInput {
    saved: Option<u32>,
}

#[cfg(windows)]
impl HiddenInput {
    fn begin() -> HiddenInput {
        HiddenInput { saved: super::windows_console::hide_input() }
    }
}

#[cfg(windows)]
impl Drop for HiddenInput {
    fn drop(&mut self) {
        if let Some(mode) = self.saved {
            super::windows_console::restore_input(mode);
        }
    }
}

/// Elsewhere input is read as it is shown.
#[cfg(not(any(unix, windows)))]
struct HiddenInput;

#[cfg(not(any(unix, windows)))]
impl HiddenInput {
    fn begin() -> HiddenInput {
        HiddenInput
    }
}
