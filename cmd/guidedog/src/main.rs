//! guidedog: builds HTML sites and PDF books from reStructuredText and Markdown. The
//! whole command line is the gd-cli crate; this program adds the crash report and the
//! stack its conversions are sized for.

#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> =
        std::env::args_os().map(|a| a.to_string_lossy().into_owned()).collect();
    // A crash, which is always a bug, ends with an explanation and exit status 70.
    gd_host::install_crash_reporter(&args, &gd_cli::version_text());
    // Conversions size their stack budgets for WORKER_STACK_BYTES (see gd-host); the
    // main thread's stack is the system's choice, so the command runs on a thread of
    // its own. Ctrl+C is caught by the commands that must settle before stopping
    // (build, convert, publish); the others, serve among them, stop at once.
    let status = gd_host::catch_crash(|| {
        gd_host::run_with_stack(gd_host::WORKER_STACK_BYTES, || {
            gd_cli::run_status(args.get(1..).unwrap_or(&[]), &gd_cli::default_registry())
        })
    });
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}
