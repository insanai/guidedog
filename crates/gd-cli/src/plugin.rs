//! `guidedog plugin`: checks, registers, lists, inspects, runs, and unregisters plugin
//! programs; lists and inspects the plugins linked into this program too.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gd_host::Problem as HostProblem;
use gd_plugins::{MachineEntry, Plugin, PluginOptions, Problem, Registry};

use crate::command::{PluginAction, PluginCommand};
use crate::console::Console;

/// Runs `command`; `linked` are the plugins built into this program.
pub(crate) fn plugin(
    console: &mut Console<'_>,
    command: &PluginCommand,
    linked: &Registry,
) -> i32 {
    match run(console, command, linked) {
        Ok(status) => status,
        Err(problem) => plugin_problem(console, &problem),
    }
}

fn run(
    console: &mut Console<'_>,
    command: &PluginCommand,
    linked: &Registry,
) -> Result<i32, Problem> {
    let path = gd_plugins::machine_path()?;
    match &command.action {
        PluginAction::Check { program } => check(console, command, program),
        PluginAction::Register { id, program } => register(console, command, &path, id, program),
        PluginAction::List => list(console, &path, linked),
        PluginAction::Inspect { id } => inspect(console, &path, id, linked),
        PluginAction::Unregister { id } => {
            gd_plugins::unregister(&path, id)?;
            let text = format!(
                "  Unregistered {id}; the program itself and anything it published are \
                 unchanged.\n"
            );
            console.print(&text);
            Ok(0)
        }
        PluginAction::Run { id, args } => run_program(&path, id, args),
    }
}

/// Reports a plugin problem and returns its exit status.
pub(crate) fn plugin_problem(console: &mut Console<'_>, problem: &Problem) -> i32 {
    console.report(&problem.report(), "");
    problem.exit
}

/// `path` made absolute against the working folder.
fn absolute(path: &str) -> String {
    let p = Path::new(path);
    if p.is_absolute() {
        return path.to_string();
    }
    std::path::absolute(p).map_or_else(|_| path.to_string(), |a| a.to_string_lossy().into_owned())
}

/// `plugin check PATH`: qualifies a build plugin without changing the registry.
fn check(
    console: &mut Console<'_>,
    command: &PluginCommand,
    program: &str,
) -> Result<i32, Problem> {
    let entry = gd_plugins::inspect_entry(&absolute(program))?;
    if !entry.is_build() {
        return Err(Problem::new(
            "cli.plugin-check-kind",
            "This program is a publisher",
            "Generic build checks never invoke publication or access a remote destination.",
        )
        .hint("Test this publisher with its own test suite and guidedog publish --dry-run.")
        .path(&entry.path)
        .exit(2));
    }
    qualify(command, &entry)?;
    let text = format!(
        "  Checked {}: loading, declared hooks, repeated results and cleanup passed.\n  The \
         registry is unchanged. Native code still requires trust and process isolation.\n",
        entry.id
    );
    console.print(&gd_report::escape_terminal(&text));
    Ok(0)
}

/// Runs the qualification checks on a build plugin's program; its SHA-256 must be the
/// same before and after, so the registry never records a program changed meanwhile.
fn qualify(command: &PluginCommand, entry: &MachineEntry) -> Result<(), Problem> {
    let given = command.project.clone().unwrap_or_else(|| PathBuf::from("."));
    let root = std::path::absolute(&given).unwrap_or(given);
    if !root.is_dir() {
        return Err(Problem::new(
            "cli.plugin-check-project",
            "The plugin check needs a project directory",
            "The project path cannot be opened as a directory.",
        )
        .hint("Pass --project=DIR with an existing project folder.")
        .path(&root.to_string_lossy())
        .exit(2));
    }
    let config = command.config.as_ref().map(|c| root.join(c)).unwrap_or_default();
    gd_plugins::verify_entry(entry)?;
    let plugin = gd_plugins::process_plugin(entry)?;
    gd_host::catch_interrupts();
    let mut options =
        PluginOptions::new(&config.to_string_lossy(), &root.to_string_lossy(), "html", "en");
    options.request_timeout = Duration::from_secs(u64::from(command.timeout.unwrap_or(60)));
    options.cancel = gd_plugins::Cancel::from_fn(gd_host::was_interrupted);
    gd_plugins::check_plugin(&plugin, &options)?;
    gd_plugins::verify_entry(entry)
}

/// `plugin register ID PATH`: after confirmation, inspects the program, qualifies a
/// build plugin, and records its SHA-256.
fn register(
    console: &mut Console<'_>,
    command: &PluginCommand,
    registry: &Path,
    id: &str,
    program: &str,
) -> Result<i32, Problem> {
    let path = absolute(program);
    console.print(&format!(
        "\n  Registering inspects and tests {}. It runs with your permissions, as any program \
         you start does; register only programs you trust.\n\n",
        gd_report::escape_terminal(&path)
    ));
    if !command.yes && !confirm(console, "Run it and register it?") {
        return Err(Problem::new(
            "cli.unconfirmed",
            "Not confirmed",
            "Nothing was registered: registering needs your confirmation.",
        )
        .hint("Run the command in a terminal and answer y, or add --yes.")
        .path(&path)
        .exit(1));
    }
    let entry = gd_plugins::inspect_entry(&path)?;
    if entry.id != id {
        return Err(Problem::new(
            "cli.plugin-id",
            "The program is another plugin",
            &format!("The program says it is {:?}, not {id:?}.", entry.id),
        )
        .hint(&format!(
            "Register it as {}, or check that this is the program you meant.",
            entry.id
        ))
        .path(&path)
        .exit(2));
    }
    if entry.is_build() {
        qualify(command, &entry)?;
    }
    gd_plugins::register(registry, &entry)?;
    let pin = entry.sha256.get(..16).unwrap_or(&entry.sha256);
    let text = format!(
        "  Registered {} {} ({}), pinned to SHA-256 {pin}.\n",
        entry.id, entry.version, entry.name
    );
    console.print(&gd_report::escape_terminal(&text));
    Ok(0)
}

/// Asks a yes-or-no question at a terminal; anything but yes, or no terminal, is no.
fn confirm(console: &mut Console<'_>, question: &str) -> bool {
    if !console.terminals.stdin {
        return false;
    }
    let answer = console.prompt(&format!("  {question} [y/N] ")).unwrap_or_default();
    let answer = answer.to_lowercase();
    answer == "y" || answer == "yes"
}

/// `plugin list`: the plugins linked into this program, then the registered programs.
fn list(console: &mut Console<'_>, registry: &Path, linked: &Registry) -> Result<i32, Problem> {
    let machine = gd_plugins::read_machine(registry)?;
    let mut out = String::new();
    if !linked.plugins().is_empty() {
        out.push_str("  Linked into this program:\n");
        for plugin in linked.plugins() {
            let (version, name) = linked_identity(plugin);
            out.push_str(&format!(
                "  {:<14} {version:<10} {:<10} {name}\n",
                plugin.id(),
                kind(plugin)
            ));
        }
    }
    if machine.entries.is_empty() {
        out.push_str("  No plugin program is registered on this machine.\n");
    } else {
        out.push_str("  Registered on this machine:\n");
        for e in &machine.entries {
            let kind = if e.is_build() { "build" } else { "publisher" };
            out.push_str(&format!("  {:<14} {:<10} {kind:<10} {}\n", e.id, e.version, e.path));
        }
    }
    console.print(&gd_report::escape_terminal(&out));
    Ok(0)
}

fn kind(plugin: &Plugin) -> &'static str {
    if plugin.is_publisher() { "publisher" } else { "build" }
}

/// A linked plugin's version and name.
fn linked_identity(plugin: &Plugin) -> (String, String) {
    match (plugin.build_info(), plugin.publisher_info()) {
        (Some(info), _) => (info.version.clone(), info.name.clone()),
        (None, Some(info)) => (info.version.clone(), info.name.clone()),
        (None, None) => (String::new(), String::new()),
    }
}

/// `plugin inspect ID`: a registered entry, and whether the program on disk is still
/// the one registered; or a linked plugin's identity.
fn inspect(
    console: &mut Console<'_>,
    registry: &Path,
    id: &str,
    linked: &Registry,
) -> Result<i32, Problem> {
    let machine = gd_plugins::read_machine(registry)?;
    let Some(e) = machine.find(id) else {
        if let Some(plugin) = linked.find(id) {
            let (version, name) = linked_identity(plugin);
            let mut text = format!(
                "  Kind          {} plugin, linked into this program\n  ID            {id}\n  \
                 Name          {name}\n  Version       {version}\n",
                kind(plugin)
            );
            text.push_str(&commands_text(plugin.commands()));
            console.print(&gd_report::escape_terminal(&text));
            return Ok(0);
        }
        return Err(missing(id));
    };
    let mut text = if e.is_build() {
        format!("  Kind          build plugin\n  Hooks         {}\n", e.hooks.join(", "))
    } else {
        "  Kind          publisher plugin\n".to_string()
    };
    let capabilities: Vec<&str> = e.capabilities.names().collect();
    text.push_str(&format!(
        "  ID            {}\n  Name          {}\n  Version       {}\n  Program       {}\n  \
         SHA-256       {}\n  Protocol      {}.{}\n  Capabilities  {}\n  Registered    {}\n",
        e.id,
        e.name,
        e.version,
        e.path,
        e.sha256,
        e.protocol.major,
        e.protocol.minor,
        capabilities.join(", "),
        e.registered
    ));
    text.push_str(&commands_text(&e.commands));
    console.print(&gd_report::escape_terminal(&text));
    gd_plugins::verify_entry(e)?;
    console.print("  The program on disk is the one registered.\n");
    Ok(0)
}

/// The `Commands` lines of an inspection: each command the plugin adds to the command
/// line, with its summary; nothing when it adds none.
fn commands_text(commands: &[gd_plugins::CommandInfo]) -> String {
    let mut text = String::new();
    for (i, c) in commands.iter().enumerate() {
        let label = if i == 0 { "Commands" } else { "" };
        text.push_str(&format!("  {label:<13} guidedog {}: {}\n", c.name(), c.summary));
    }
    text
}

/// The problem of an ID nothing is registered as.
pub(crate) fn missing(id: &str) -> Problem {
    Problem::new(
        "cli.plugin-missing",
        "No such plugin is registered",
        &format!("No program is registered as {id} on this machine."),
    )
    .hint(&format!("Install it, then run guidedog plugin register {id} /path/to/program."))
    .path(id)
    .exit(2)
}

/// The registered entry `id`.
pub(crate) fn registered(registry: &Path, id: &str) -> Result<MachineEntry, Problem> {
    let machine = gd_plugins::read_machine(registry)?;
    machine.find(id).cloned().ok_or_else(|| missing(id))
}

/// `plugin run ID -- ARGS`: runs a registered program with `args`, after checking its
/// SHA-256, and returns its exit status.
fn run_program(registry: &Path, id: &str, args: &[String]) -> Result<i32, Problem> {
    let entry = registered(registry, id)?;
    gd_plugins::verify_entry(&entry)?;
    let status = std::process::Command::new(&entry.path).args(args).status().map_err(|e| {
        Problem::new("cli.plugin-start", "The program cannot start", &e.to_string())
            .hint("Install it again, then register it again.")
            .path(&entry.path)
            .exit(4)
    })?;
    Ok(status.code().unwrap_or(1))
}

/// A host problem as a plugin problem, for the steps of `publish` that share both.
pub(crate) fn from_host(p: &HostProblem) -> Problem {
    Problem::new(p.code, p.title, &p.message).hint(&p.hint).path(&p.path).exit(p.exit)
}
