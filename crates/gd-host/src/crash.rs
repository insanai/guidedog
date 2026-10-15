//! The last resort: a panic (a bug in Guidedog) ends the command with an explanation and
//! the exit status `CRASH_EXIT`, instead of Rust's bare panic message. Every failure
//! Guidedog knows of is a `Problem` or a report long before this.
//!
//! A program calls `install_crash_reporter` once, then runs its command inside
//! `catch_crash`. The hook only records where the panic happened; `catch_crash` prints
//! the report once the panic has unwound to it. A panic that a library catches itself
//! (the embedded Typst compiler does) is therefore never reported as a crash.

use std::panic::{self, AssertUnwindSafe};
use std::sync::Mutex;

/// The exit status of a command that crashed: EX_SOFTWARE, which no other failure uses.
pub const CRASH_EXIT: i32 = 70;

/// Where a crash is to be reported: the issue tracker of whoever distributes the build,
/// set when it is built (`GUIDEDOG_ISSUE_TRACKER=https://...`); "" names none.
pub const ISSUE_TRACKER: &str = match option_env!("GUIDEDOG_ISSUE_TRACKER") {
    Some(tracker) => tracker,
    None => "",
};

const TITLE: &str = "GUIDEDOG CRASHED (internal error)";
const CODE: &str = "host.crash";
const MAX_COMMAND: usize = 1024;

/// The report around its cause, prepared when the reporter is installed.
static REPORT: Mutex<(String, String)> = Mutex::new((String::new(), String::new()));

/// The cause of the last panic, recorded by the hook.
static CAUSE: Mutex<String> = Mutex::new(String::new());

/// Prepares the crash report for the command line `args` (the program name first) and
/// `version` (what `guidedog --version` prints), and installs a panic hook that records
/// each panic's message and location without printing it.
pub fn install_crash_reporter(args: &[String], version: &str) {
    let parts = crash_parts(args, version, ISSUE_TRACKER);
    *REPORT.lock().unwrap_or_else(|p| p.into_inner()) = parts;
    panic::set_hook(Box::new(|info| {
        let mut cause =
            format!("{} panicked", std::thread::current().name().unwrap_or("a thread"));
        if let Some(at) = info.location() {
            cause.push_str(&format!(" at {}:{}", at.file(), at.line()));
        }
        cause.push_str(&format!(": \"{}\"", payload_text(info.payload())));
        // try_lock: a panic while the cause is being written must not deadlock.
        if let Ok(mut slot) = CAUSE.try_lock() {
            *slot = cause;
        }
    }));
}

/// Runs `command` and returns its exit status; a panic that unwinds out of it prints
/// the crash report to standard error and returns `CRASH_EXIT`.
pub fn catch_crash(command: impl FnOnce() -> i32) -> i32 {
    match panic::catch_unwind(AssertUnwindSafe(command)) {
        Ok(status) => status,
        Err(payload) => {
            let recorded = std::mem::take(&mut *CAUSE.lock().unwrap_or_else(|p| p.into_inner()));
            let cause = if recorded.is_empty() {
                format!("panicked: \"{}\"", payload_text(payload.as_ref()))
            } else {
                recorded
            };
            eprint!("{}", crash_report(&cause));
            CRASH_EXIT
        }
    }
}

/// The whole report for `cause`, from the parts `install_crash_reporter` prepared.
pub fn crash_report(cause: &str) -> String {
    let parts = REPORT.lock().unwrap_or_else(|p| p.into_inner());
    if parts.0.is_empty() {
        let (head, tail) = crash_parts(&[], "", ISSUE_TRACKER);
        return format!("{head}{cause}{tail}");
    }
    format!("{}{cause}{}", parts.0, parts.1)
}

fn payload_text(payload: &(dyn std::any::Any + Send)) -> &str {
    if let Some(text) = payload.downcast_ref::<&str>() {
        text
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text
    } else {
        "no message"
    }
}

/// The report's head (title and command) and tail (what it means and where to report
/// it), around the cause; `tracker` is where to report, or "" for none.
pub(crate) fn crash_parts(args: &[String], version: &str, tracker: &str) -> (String, String) {
    let mut head = format!("\n{TITLE}");
    head.push_str(&" ".repeat(78usize.saturating_sub(TITLE.len() + CODE.len())));
    head.push_str(CODE);
    head.push_str("\n\n  ");
    head.push_str(&quoted_command(args));
    head.push_str("\n\nGuidedog stopped on a fault in its own code:\n\n    ");
    let mut tail = String::from(
        "\n\nThis is a bug in Guidedog, not in your documents. Guidedog prepares its\n\
         output in a staging folder and publishes it only when a command succeeds, so\n\
         nothing this command wrote was published: the previous output is as it was.\n\
         If a guidedog gds command crashed, run guidedog gds recover --rollback to\n\
         restore the records it was changing.\n\n\
         Run the same command again. If it crashes again, report it ",
    );
    if tracker.is_empty() {
        tail.push_str("to whoever provided\nyour Guidedog build ");
    } else {
        tail.push_str("at\n");
        tail.push_str(tracker);
        tail.push('\n');
    }
    tail.push_str(
        "with this whole message: the command, the fault, and\nthe version below (what \
         guidedog --version prints) with the platform:\n\n",
    );
    tail.push_str(version.trim_end_matches('\n'));
    tail.push_str(&format!("\nplatform {} {}\n", std::env::consts::OS, std::env::consts::ARCH));
    (head, tail)
}

/// The command line, quoting arguments that hold spaces or quotes, cut at 1 KiB.
fn quoted_command(args: &[String]) -> String {
    let mut out = String::new();
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        if out.len() + arg.len() > MAX_COMMAND - 4 {
            out.push_str("...");
            break;
        }
        let quote = arg.is_empty() || arg.contains([' ', '\t', '\'', '"']);
        if quote {
            out.push('\'');
        }
        out.push_str(arg);
        if quote {
            out.push('\'');
        }
    }
    out
}
