//! Contributed commands of installed programs: the host starts the program with
//! [`COMMAND_FLAG`] and one JSON request, the program inherits the terminal (its
//! standard input, output and error are the host's), and its exit status is the
//! command's. There is no conversation over standard input: the command may ask the
//! person directly.

use std::io::{BufRead, Write};
use std::process::{Command, Stdio};

use gd_json::Value;

use crate::command::no_command;
use crate::problem::{self, Problem};
use crate::wire::{self, object, version_json, version_of};
use crate::{BuildRequest, CommandContext, CommandHost, CommandInfo, Plugin, Version};

/// The argument that starts a plugin program to run one contributed command; the next
/// argument is the request, one JSON object.
pub const COMMAND_FLAG: &str = "--guidedog-command";
/// The command protocol's name in a request.
pub const COMMAND_PROTOCOL_NAME: &str = "guidedog-command";
/// The command protocol's version: a handshake lists the versions it speaks in
/// `command_versions` beside its `commands`.
pub const COMMAND_PROTOCOL_VERSION: Version = Version { major: 1, minor: 0 };
/// The longest command request, in bytes: it travels as a program argument.
const MAX_COMMAND_REQUEST: usize = 64 * 1024;

/// The command protocol version a host shares with a program that speaks `versions`:
/// the same major version at the lower minor version; `None` when none is shared.
pub(crate) fn select_command_version(versions: &[Version]) -> Option<Version> {
    let host = COMMAND_PROTOCOL_VERSION;
    versions
        .iter()
        .find(|v| v.major == host.major)
        .map(|v| Version::new(v.major, v.minor.min(host.minor)))
}

/// Runs the command `info` of the installed program `executable` with `args`: starts
/// it with [`COMMAND_FLAG`] and the request, on the host's own terminal, and returns
/// its exit status. Fails when the program cannot start or the request is too long.
pub(crate) fn run_program_command(
    executable: &str,
    version: Version,
    ctx: &mut CommandContext<'_>,
    info: &CommandInfo,
    args: &[String],
) -> Result<i32, Problem> {
    let request = gd_json::to_string(&request_json(version, ctx, info, args));
    if request.len() > MAX_COMMAND_REQUEST {
        return Err(Problem::new(
            "plugins.command-request",
            "The command line is too long",
            "The arguments of a plugin command may take at most 64 KiB.",
        )
        .hint("Pass fewer or shorter arguments, or put them in the plugin's configuration.")
        .path(executable)
        .exit(2));
    }
    // What the host wrote must come before what the program writes.
    ctx.flush();
    let status = Command::new(executable)
        .arg(crate::COMMAND_FLAG)
        .arg(&request)
        .current_dir(&ctx.dir)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| {
            Problem::new("plugins.start", "The plugin program cannot start", &e.to_string())
                .hint("Install the program again, then register it again.")
                .path(executable)
                .exit(4)
        })?;
    Ok(exit_status(status))
}

/// A finished program's exit status; a program a signal stopped is 128 plus the
/// signal, as shells report it.
fn exit_status(status: std::process::ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    if let Some(signal) = std::os::unix::process::ExitStatusExt::signal(&status) {
        return 128 + signal;
    }
    1
}

fn request_json(
    version: Version,
    ctx: &CommandContext<'_>,
    info: &CommandInfo,
    args: &[String],
) -> Value {
    let strings = |list: &[String]| Value::Array(list.iter().map(|s| s.as_str().into()).collect());
    object([
        ("protocol", crate::COMMAND_PROTOCOL_NAME.into()),
        ("version", version_json(version)),
        ("command", strings(&info.path)),
        ("args", strings(args)),
        ("dir", ctx.dir.to_string_lossy().as_ref().into()),
        ("config", ctx.config.as_str().into()),
        ("host", ctx.host_program.as_str().into()),
        ("interactive", ctx.interactive.into()),
        ("color", ctx.color.into()),
        ("json", ctx.json.into()),
        ("quiet", ctx.quiet.into()),
    ])
}

/// A command request as a program reads it.
#[derive(Debug, Default)]
struct Request {
    command: Vec<String>,
    args: Vec<String>,
    dir: String,
    config: String,
    host: String,
    interactive: bool,
    color: bool,
    json: bool,
    quiet: bool,
}

fn parse_request(text: &str) -> Result<Request, Problem> {
    let o = wire::parse_message(text)?;
    if wire::str_field(&o, "protocol") != Some(crate::COMMAND_PROTOCOL_NAME) {
        return Err(problem::protocol("The request is not a Guidedog command request."));
    }
    let version = o.get("version").and_then(version_of);
    if version.is_none_or(|v| v.major != COMMAND_PROTOCOL_VERSION.major) {
        return Err(problem::protocol("The request uses another command protocol version."));
    }
    let strings = |key| -> Result<Vec<String>, Problem> {
        let items = wire::arr_field(&o, key).unwrap_or_default();
        items
            .iter()
            .map(|v| v.as_str().map(str::to_string))
            .collect::<Option<Vec<String>>>()
            .ok_or_else(|| problem::protocol("The request's words are not strings."))
    };
    let text = |key| wire::str_field(&o, key).unwrap_or_default().to_string();
    let flag = |key| wire::bool_field(&o, key).unwrap_or(false);
    Ok(Request {
        command: strings("command")?,
        args: strings("args")?,
        dir: text("dir"),
        config: text("config"),
        host: text("host"),
        interactive: flag("interactive"),
        color: flag("color"),
        json: flag("json"),
        quiet: flag("quiet"),
    })
}

/// The request text when the program was started for a command: exactly
/// [`COMMAND_FLAG`] and one more argument.
pub(crate) fn command_request() -> Option<String> {
    let mut args = std::env::args().skip(1);
    let flag = args.next()?;
    let request = args.next()?;
    (flag == COMMAND_FLAG && args.next().is_none()).then_some(request)
}

/// Runs the command a host's `request` (the argument after [`COMMAND_FLAG`]) names,
/// with `plugin`, on the program's standard streams, and returns the exit status.
/// A problem is reported on standard error, as the request asks (text or JSON). A
/// project build asked of the host runs the host program the request names.
///
/// ```no_run
/// # fn plugin() -> gd_plugins::Plugin { unimplemented!() }
/// let args: Vec<String> = std::env::args().skip(1).collect();
/// if args.len() == 2 && args[0] == gd_plugins::COMMAND_FLAG {
///     std::process::exit(i32::from(gd_plugins::serve_command(&plugin(), &args[1])));
/// }
/// ```
pub fn serve_command(plugin: &Plugin, request: &str) -> u8 {
    serve_request(request, plugin.id(), plugin.commands(), &mut |ctx, name, args| {
        plugin.run_command(ctx, name, args)
    })
}

/// The function that runs a contributed command: context, name, arguments.
pub(crate) type RunCommand<'a> =
    dyn FnMut(&mut CommandContext<'_>, &str, &[String]) -> Result<i32, Problem> + 'a;

/// Serves the command request `request` of the plugin `id`, which contributes
/// `commands`, by calling `run` on the program's standard streams.
pub(crate) fn serve_request(
    request: &str,
    id: &str,
    commands: &[CommandInfo],
    run: &mut RunCommand<'_>,
) -> u8 {
    let request = match parse_request(request) {
        Ok(request) => request,
        Err(p) => return report(&p, false, false),
    };
    let name = request.command.join(" ");
    if !commands.iter().any(|c| c.name() == name) {
        return report(&no_command(id, &name), request.color, request.json);
    }
    let mut host =
        StdHost { program: request.host.clone(), out: std::io::stdout(), err: std::io::stderr() };
    let mut ctx = CommandContext::new(&mut host);
    if !request.dir.is_empty() {
        ctx.dir = request.dir.clone().into();
    }
    ctx.config = request.config;
    ctx.host_program = request.host;
    ctx.interactive = request.interactive;
    ctx.color = request.color;
    ctx.json = request.json;
    ctx.quiet = request.quiet;
    let result = run(&mut ctx, &name, &request.args);
    ctx.flush();
    match result {
        Ok(status) => u8::try_from(status).unwrap_or(1),
        Err(p) => report(&p, request.color, request.json),
    }
}

/// Writes `p` on standard error and returns its exit status.
fn report(p: &Problem, color: bool, json: bool) -> u8 {
    let report = p.report();
    let text = if json {
        format!("{}\n", gd_report::render_json(&report))
    } else {
        let style = gd_report::Style { color, ..gd_report::Style::default() };
        gd_report::render_text(&report, &style)
    };
    let _ = std::io::stderr().write_all(text.as_bytes());
    u8::try_from(p.exit.clamp(1, 255)).unwrap_or(1)
}

/// The host of a command run as a program: the program's standard streams, and builds
/// by the host program the request names.
struct StdHost {
    program: String,
    out: std::io::Stdout,
    err: std::io::Stderr,
}

impl CommandHost for StdHost {
    fn streams(&mut self) -> (&mut dyn Write, &mut dyn Write) {
        (&mut self.out, &mut self.err)
    }

    fn read_line(&mut self) -> Option<String> {
        let mut line = String::new();
        match std::io::stdin().lock().read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim_end_matches(['\n', '\r']).to_string()),
        }
    }

    fn build_project(&mut self, request: &BuildRequest) -> Result<(), Problem> {
        build_with_program(&self.program, request)
    }
}

/// Builds `request`'s project by starting `program` (a `guidedog`) with `build -b
/// BUILDER PROJECT OUTPUT`; its report and output go to standard error, so that
/// a command's standard output stays its result. Fails when `program` is empty or
/// cannot start, or the build fails.
pub fn build_with_program(program: &str, request: &BuildRequest) -> Result<(), Problem> {
    if program.is_empty() {
        return Err(Problem::new(
            "plugins.no-build",
            "The host builds nothing",
            "This command asked for a project build, and no Guidedog program was named.",
        )
        .hint("Run the command through guidedog, which builds the project for it.")
        .exit(4));
    }
    let builder = if request.builder.is_empty() { "html" } else { request.builder.as_str() };
    let mut command = Command::new(program);
    command.args(["build", "-b", builder]).arg(&request.project).arg(&request.output);
    if request.strict {
        command.arg("-W");
    }
    if !request.now.is_empty() {
        command.arg("--now").arg(&request.now);
    }
    if request.preview {
        command.arg("--preview");
    }
    for setting in &request.overrides {
        command.arg("-D").arg(setting);
    }
    if !request.base_url.is_empty() {
        command.arg("-D").arg(format!("html_baseurl={}", request.base_url));
    }
    command.stdin(Stdio::null()).stdout(Stdio::from(std::io::stderr()));
    let shown = request.project.to_string_lossy();
    let status = command.status().map_err(|e| {
        Problem::new("plugins.start", "Guidedog cannot start", &e.to_string())
            .hint("Install Guidedog, or run the command through guidedog.")
            .path(program)
            .exit(4)
    })?;
    if status.success() {
        return Ok(());
    }
    Err(Problem::new(
        "plugins.build",
        "The project did not build",
        &format!("guidedog stopped with status {}; its report is above.", exit_status(status)),
    )
    .hint("Fix what Guidedog reported, then run the command again.")
    .path(&shown)
    .exit(1))
}

/// Runs the command `name` of `plugin`, linked or installed; see [`Plugin::run_command`].
pub(crate) fn run_command(
    plugin: &Plugin,
    ctx: &mut CommandContext<'_>,
    name: &str,
    args: &[String],
) -> Result<i32, Problem> {
    let Some(info) = plugin.commands().iter().find(|c| c.name() == name) else {
        return Err(no_command(plugin.id(), name));
    };
    if !plugin.executable().is_empty() {
        let version = plugin.command_protocol();
        return run_program_command(plugin.executable(), version, ctx, info, args);
    }
    plugin.run_linked_command(ctx, name, args)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoHost;

    impl CommandHost for NoHost {
        fn streams(&mut self) -> (&mut dyn Write, &mut dyn Write) {
            unreachable!("the test writes nothing")
        }
        fn read_line(&mut self) -> Option<String> {
            None
        }
        fn build_project(&mut self, _: &BuildRequest) -> Result<(), Problem> {
            Ok(())
        }
    }

    #[test]
    fn requests_round_trip() {
        let mut host = NoHost;
        let mut ctx = CommandContext::new(&mut host);
        ctx.config = "pages.toml".into();
        ctx.interactive = true;
        let info = CommandInfo::new("pages workflow", "W");
        let args = vec!["--write".to_string()];
        let line = gd_json::to_string(&request_json(COMMAND_PROTOCOL_VERSION, &ctx, &info, &args));
        let r = parse_request(&line).unwrap();
        assert_eq!((r.command, r.args), (info.path, args));
        assert_eq!(r.config, "pages.toml");
        assert!(r.interactive && !r.json);
        assert!(parse_request(r#"{"protocol":"other"}"#).is_err());
        let v2 = r#"{"protocol":"guidedog-command","version":{"major":2,"minor":0}}"#;
        assert!(parse_request(v2).is_err());
    }

    #[test]
    fn command_versions_share_a_major() {
        let v = select_command_version(&[Version::new(2, 0), Version::new(1, 4)]);
        assert_eq!(v, Some(COMMAND_PROTOCOL_VERSION));
        assert_eq!(select_command_version(&[Version::new(2, 0)]), None);
        assert_eq!(select_command_version(&[]), None);
    }
}
