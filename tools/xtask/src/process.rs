//! Running external programs with argument vectors (never a shell), streaming their
//! output, and turning a failure into a report that says what to do.

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

use crate::{Failure, Result, fail};

/// The program and its arguments as one line, for messages and for echoing a command.
pub(crate) fn describe(command: &Command) -> String {
    let mut line = command.get_program().to_string_lossy().into_owned();
    for arg in command.get_args() {
        line.push(' ');
        line.push_str(&arg.to_string_lossy());
    }
    line
}

/// Runs a command with the terminal's input and output, and fails unless it succeeds.
pub(crate) fn run(command: &mut Command) -> Result<()> {
    let status = command.status().map_err(|err| spawn_failure(command, &err))?;
    check_status(command, status)
}

/// Runs a command and returns what it wrote to standard output; its errors still go to
/// the terminal. Fails unless it succeeds or its output is not UTF-8.
pub(crate) fn output(command: &mut Command) -> Result<String> {
    command.stdin(Stdio::null()).stderr(Stdio::inherit());
    let out = command.output().map_err(|err| spawn_failure(command, &err))?;
    check_status(command, out.status)?;
    String::from_utf8(out.stdout).map_err(|_| {
        fail(
            "output",
            "UNREADABLE OUTPUT",
            &format!("`{}` wrote output that is not UTF-8.", describe(command)),
            "Run the command by hand to see what it prints.",
        )
    })
}

/// What a finished command left: whether it succeeded, and its two outputs.
#[derive(Debug, Default)]
pub(crate) struct Captured {
    /// Whether the program exited with status 0.
    pub success: bool,
    /// Its standard output, with invalid UTF-8 replaced.
    pub stdout: String,
    /// Its standard error, with invalid UTF-8 replaced.
    pub stderr: String,
}

/// Runs a command, capturing both outputs whatever its exit status; fails only when it
/// cannot start.
pub(crate) fn capture(command: &mut Command) -> Result<Captured> {
    let out = command.output().map_err(|err| spawn_failure(command, &err))?;
    Ok(Captured {
        success: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// Runs a command with `input` as its standard input and returns its standard output,
/// whatever its exit status, with whether it succeeded.
pub(crate) fn feed(command: &mut Command, input: &[u8]) -> Result<Captured> {
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|err| spawn_failure(command, &err))?;
    if let Some(mut stdin) = child.stdin.take() {
        // A program that exits without reading its input closes the pipe; its status
        // tells what happened, so a failed write is not an error of its own.
        let _ = stdin.write_all(input);
    }
    let out = child.wait_with_output().map_err(|err| spawn_failure(command, &err))?;
    Ok(Captured {
        success: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// Runs `producer | consumer > path`: the producer's output goes through the consumer
/// (such as `gzip -n -9`) into the file. Fails unless both succeed.
pub(crate) fn pipe_to_file(
    producer: &mut Command,
    consumer: &mut Command,
    path: &Path,
) -> Result<()> {
    let file =
        File::create(path).map_err(|err| crate::fsutil::io_failure("create", path, &err))?;
    producer.stdout(Stdio::piped());
    let mut first = producer.spawn().map_err(|err| spawn_failure(producer, &err))?;
    let Some(pipe) = first.stdout.take() else {
        return Err(fail(
            "pipe",
            "NO PIPE",
            "The output pipe was not opened.",
            "Run the command again.",
        ));
    };
    consumer.stdin(Stdio::from(pipe)).stdout(Stdio::from(file));
    let second = consumer.status().map_err(|err| spawn_failure(consumer, &err));
    if second.is_err() {
        // Nothing reads the pipe, so the producer would wait forever.
        let _ = first.kill();
    }
    let first = first.wait().map_err(|err| spawn_failure(producer, &err))?;
    check_status(producer, first)?;
    check_status(consumer, second?)
}

fn check_status(command: &Command, status: ExitStatus) -> Result<()> {
    if status.success() {
        return Ok(());
    }
    let program = program_name(command);
    let how = match status.code() {
        Some(code) => format!("exited with status {code}"),
        None => String::from("was stopped by a signal"),
    };
    Err(fail(
        "command",
        "COMMAND FAILED",
        &format!("`{}` {how}.", describe(command)),
        &format!(
            "Read the messages {program} printed above, fix the problem, and run the step \
             again."
        ),
    ))
}

/// The failure for a program that could not start, with how to install the usual ones.
pub(crate) fn spawn_failure(command: &Command, err: &io::Error) -> Failure {
    let program = program_name(command);
    if err.kind() != io::ErrorKind::NotFound {
        return fail(
            "command",
            "CANNOT RUN",
            &format!("I could not run `{}`: {err}.", describe(command)),
            &format!("Check that {program} is an executable file."),
        );
    }
    fail(
        "missing-program",
        "PROGRAM NOT FOUND",
        &format!("`{program}` is not installed, or it is not on PATH."),
        install_hint(&program),
    )
}

fn program_name(command: &Command) -> String {
    let program = Path::new(command.get_program());
    program.file_name().unwrap_or(program.as_os_str()).to_string_lossy().into_owned()
}

fn install_hint(program: &str) -> &'static str {
    match program {
        "cargo" | "rustc" => "Install Rust with rustup (https://rustup.rs).",
        "gh" => "Install the GitHub CLI (https://cli.github.com) and sign in with GH_TOKEN.",
        "curl" | "git" | "tar" | "gzip" => "Install it with the system's package manager.",
        "dbus-run-session" | "gnome-keyring-daemon" => {
            "Install dbus and gnome-keyring (Debian/Ubuntu: apt install dbus gnome-keyring \
             libsecret-tools)."
        }
        _ => "Install it, or pass the path to the program.",
    }
}
