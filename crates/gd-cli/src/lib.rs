//! The `guidedog` command line as a library: it reads a command line, does what it asks,
//! reports on standard error, and returns the exit status. `cmd/guidedog` is a call to
//! [`run`]; a program with plugins linked in calls [`run_with`] with its registry.
//!
//! ```no_run
//! fn main() -> std::process::ExitCode {
//!     let linked = gd_plugins::Registry::new(Vec::new()).unwrap_or_default();
//!     gd_cli::run_with(linked)
//! }
//! ```
//!
//! The staged path: [`parse_command`] turns words into a typed [`Command`] (plain data,
//! checked, with Elm-style usage problems), and [`run_in`] runs one command line with
//! the streams given, which is how tests capture what a command prints.
//!
//! Exit statuses: 0 success, 1 invalid content or a failed build, 2 usage or
//! configuration, 3 limits and memory, 4 files and programs, 5 internal, 6 and 7 for
//! publications not confirmed or unknown, 130 cancelled.

#![forbid(unsafe_code)]

mod args;
mod command;
mod console;
mod convert;
mod extension;
mod extension_run;
mod gds;
mod help;
mod legacy;
mod memory;
mod ops;
mod parse;
mod parse_plugin;
mod parse_project;
mod plugin;
mod project;
mod publish;
mod summary;
mod version;

use std::process::ExitCode;

use gd_plugins::Registry;

pub use command::{BuildCommand, CleanCommand, ColorMode, Command, ConvertCommand, Diagnostics};
pub use command::{ExtensionCommand, QuickstartCommand, ServeCommand};
pub use command::{FormatsCommand, IntlAction, IntlCommand, MemoryMode, MigrateCommand};
pub use command::{MigrateOutput, Output, Parsed, PluginAction, PluginCommand, PublishCommand};
pub use console::{Streams, Terminals};
pub use extension::{Extension, extensions};
pub use gds::discussion_builder;
pub use parse::{parse_command, parse_command_with};
pub use version::{VERSION, VERSION_SUFFIX, full_version, version_text};

use console::Console;

/// Runs the process's command line with no linked plugin (registered programs are still
/// found) and returns its exit status.
pub fn run() -> ExitCode {
    run_with(Registry::default())
}

/// Runs the process's command line (its arguments after the program name) with the
/// plugins in `linked`, which are looked up before the machine's registered programs.
/// Returns the exit status.
pub fn run_with(linked: Registry) -> ExitCode {
    let args: Vec<String> =
        std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned()).collect();
    run_args(&args, linked)
}

/// Runs the command line `args` (without the program name) with `linked` on the
/// process's standard streams; returns the exit status.
pub fn run_args(args: &[String], linked: Registry) -> ExitCode {
    ExitCode::from(u8::try_from(run_status(args, &linked)).unwrap_or(1))
}

/// [`run_args`] returning the exit status as a number, for a `main` that wraps it (in
/// `gd_host::catch_crash`, which takes a number).
pub fn run_status(args: &[String], linked: &Registry) -> i32 {
    let (stdout, stderr) = (std::io::stdout(), std::io::stderr());
    let (mut out, mut err) = (stdout.lock(), stderr.lock());
    // Not a lock on standard input: `convert -` reads it through the standard library,
    // whose lock is not reentrant. `Stdin` locks only for the length of each read.
    let mut input = std::io::BufReader::new(std::io::stdin());
    let streams = Streams {
        out: &mut out,
        err: &mut err,
        input: &mut input,
        terminals: Terminals::detect(),
    };
    run_in(args, linked, streams)
}

/// Runs the command line `args` (without the program name) with `linked` and the
/// streams given, and returns the exit status. Commands that run other programs
/// (`plugin run`) or ask questions during a build use the process's own streams.
pub fn run_in(args: &[String], linked: &Registry, streams: Streams<'_>) -> i32 {
    let added = extensions(linked, extension::may_name_extension(args));
    let parsed = parse_command_with(args, &added);
    let mut console = Console::new(streams, parsed.output);
    let status = match &parsed.command {
        Ok(command) => execute(&mut console, command, linked, &added),
        Err(problem) => console.problem(problem),
    };
    let _ = console.out.flush();
    let _ = console.err.flush();
    status
}

fn execute(
    console: &mut Console<'_>,
    command: &Command,
    linked: &Registry,
    added: &[Extension],
) -> i32 {
    match command {
        Command::Help(topic) => {
            let text = help::help_text(*topic, console.color_out, added);
            console.print(&text);
            0
        }
        Command::ExtensionHelp(word) => {
            console.print(&help::group_help(word, console.color_out, added));
            0
        }
        Command::Extension(e) if e.help => {
            console.print(&help::extension_help(&e.extension, console.color_out));
            0
        }
        Command::Extension(e) => extension_run::run_extension(console, e, linked),
        Command::Version { typst } => {
            let text = format!("{}{}", version_text(), version::typst_line(typst));
            console.print(&text);
            0
        }
        Command::Licenses => {
            console.print(&version::licenses_text());
            0
        }
        Command::Formats(f) => convert::formats(console, f),
        Command::Convert(c) => convert::convert(console, c),
        Command::Quickstart(q) => project::quickstart(console, q),
        Command::Build(b) => project::build(console, b, linked),
        Command::Serve(s) => project::serve(console, s, linked),
        Command::Clean(c) => project::clean(console, c, linked),
        Command::Migrate(m) => project::migrate(console, m),
        Command::Intl(i) => project::intl(console, i, linked),
        Command::Gds(words) => gds::gds(console, words),
        Command::Plugin(p) => plugin::plugin(console, p, linked),
        Command::Publish(p) => publish::publish(console, p, linked),
    }
}

#[cfg(test)]
mod extension_tests;
#[cfg(test)]
mod tests;
