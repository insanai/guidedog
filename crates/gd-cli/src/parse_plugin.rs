//! The options of `guidedog plugin` and `guidedog publish`.

use std::path::PathBuf;

use gd_host::Problem;

use crate::args::{Word, Words, no_value, seconds, unknown_option, usage, with_suggestion};
use crate::command::{Command, Output, PluginAction, PluginCommand, PublishCommand};
use crate::parse::output_option;

const PLUGIN_OPTIONS: &[&str] = &[
    "--project",
    "--config",
    "--timeout",
    "--yes",
    "--help",
    "-h",
    "--color",
    "--diagnostics",
    "--quiet",
    "-q",
];

const PUBLISH_OPTIONS: &[&str] = &[
    "--publisher",
    "--config",
    "--dry-run",
    "--wait",
    "--timeout",
    "--verbose",
    "--help",
    "-h",
    "--color",
    "--diagnostics",
    "--quiet",
    "-q",
];

const ACTIONS: &[&str] = &["check", "register", "list", "inspect", "unregister", "run"];

/// The words and options of one plugin or publish command line.
#[derive(Debug, Default)]
struct PluginArgs {
    words: Vec<String>,
    rest: Vec<String>,
    publisher: String,
    config: Option<String>,
    project: Option<PathBuf>,
    wait: Option<u32>,
    timeout: Option<u32>,
    dry_run: bool,
    yes: bool,
    verbose: bool,
    help: bool,
}

/// Reads `args` for `command` (`plugin` or `publish`): what follows `--` is kept for
/// `plugin run`.
fn read_args(command: &str, args: &[String], output: &mut Output) -> Result<PluginArgs, Problem> {
    let (args, rest) = match args.iter().position(|a| a == "--") {
        Some(at) => (&args[..at], args[at + 1..].to_vec()),
        None => (args, Vec::new()),
    };
    let known = if command == "plugin" { PLUGIN_OPTIONS } else { PUBLISH_OPTIONS };
    let mut a = PluginArgs { rest, ..PluginArgs::default() };
    let mut words = Words::new(args);
    while let Some(word) = words.next_word() {
        let (name, inline) = match word {
            Word::Arg(arg) => {
                a.words.push(arg.to_string());
                continue;
            }
            Word::Opt { name, inline } => (name, inline),
        };
        if name == "--project" && command == "publish" {
            return Err(usage(
                "THIS OPTION IS FOR PLUGIN CHECKS",
                "--project names the project of guidedog plugin check and register; a \
                 publisher's configuration names the project it publishes.",
                "Use --project=DIR with guidedog plugin check or register.",
            ));
        }
        if !known.contains(&name) {
            return Err(unknown_option(command, name, known));
        }
        if output_option(&mut words, output, name, inline)? {
            continue;
        }
        set_option(&mut words, &mut a, command, name, inline)?;
    }
    Ok(a)
}

fn set_option<'a>(
    words: &mut Words<'a>,
    a: &mut PluginArgs,
    command: &str,
    name: &str,
    inline: Option<&'a str>,
) -> Result<(), Problem> {
    let flag = match name {
        "--dry-run" => Some(&mut a.dry_run),
        "--yes" => Some(&mut a.yes),
        "--verbose" => Some(&mut a.verbose),
        "--help" | "-h" => Some(&mut a.help),
        _ => None,
    };
    if let Some(flag) = flag {
        no_value(name, inline)?;
        *flag = true;
        return Ok(());
    }
    let value = words.value(name, inline)?;
    match name {
        "--publisher" => a.publisher = value.to_string(),
        "--config" => a.config = Some(value.to_string()),
        "--project" => a.project = Some(PathBuf::from(value)),
        "--wait" => {
            a.wait = Some(seconds(value).ok_or_else(|| {
                usage(
                    "NOT A NUMBER OF SECONDS",
                    format!("`{value}` is not a number of seconds."),
                    "Write --wait=SECONDS, from 0 (do not wait) to 86400.",
                )
            })?);
        }
        _ => {
            let default = if command == "plugin" { 60 } else { 300 };
            a.timeout = Some(seconds(value).filter(|&n| n > 0).ok_or_else(|| {
                usage(
                    "NOT A POSITIVE TIME LIMIT",
                    format!("`{value}` is not a time limit in seconds."),
                    format!("Write --timeout=SECONDS, from 1 to 86400; the default is {default}."),
                )
            })?);
        }
    }
    Ok(())
}

/// `guidedog plugin COMMAND ...`.
pub(crate) fn parse_plugin(args: &[String], output: &mut Output) -> Result<Command, Problem> {
    let mut a = read_args("plugin", args, output)?;
    if a.help || a.words.is_empty() {
        return Ok(Command::Help(Some("plugin")));
    }
    let action = plugin_action(&mut a)?;
    Ok(Command::Plugin(PluginCommand {
        action,
        project: a.project,
        config: a.config.map(PathBuf::from),
        timeout: a.timeout,
        yes: a.yes,
    }))
}

fn plugin_action(a: &mut PluginArgs) -> Result<PluginAction, Problem> {
    let words = std::mem::take(&mut a.words);
    let count = |n: usize, title: &'static str, hint: &str| -> Result<(), Problem> {
        if words.len() == n {
            return Ok(());
        }
        Err(usage(
            title,
            format!("guidedog plugin {} was given {}.", words[0], shown(&words)),
            hint,
        ))
    };
    match words[0].as_str() {
        "check" => {
            count(
                2,
                "NAME ONE PROGRAM TO CHECK",
                "Write guidedog plugin check PATH --project=DIR --config=FILE.",
            )?;
            Ok(PluginAction::Check { program: words[1].clone() })
        }
        "register" => {
            count(
                3,
                "NAME AN ID AND A PROGRAM",
                "Write guidedog plugin register ID /absolute/path/to/program.",
            )?;
            Ok(PluginAction::Register { id: words[1].clone(), program: words[2].clone() })
        }
        "list" => {
            count(1, "LIST TAKES NO ARGUMENT", "Write guidedog plugin list.")?;
            Ok(PluginAction::List)
        }
        action @ ("inspect" | "unregister" | "run") => {
            let hint = format!("Write guidedog plugin {action} ID.");
            count(2, "NAME ONE REGISTERED ID", &hint)?;
            let id = words[1].clone();
            Ok(match action {
                "inspect" => PluginAction::Inspect { id },
                "unregister" => PluginAction::Unregister { id },
                _ => PluginAction::Run { id, args: std::mem::take(&mut a.rest) },
            })
        }
        other => Err(usage(
            "UNKNOWN PLUGIN COMMAND",
            format!("guidedog plugin has no command `{other}`."),
            with_suggestion(
                "Use check, register, list, inspect, run, or unregister; see guidedog plugin \
                 --help.",
                other,
                ACTIONS.iter().copied(),
            ),
        )),
    }
}

/// The words after the command, quoted for a message.
fn shown(words: &[String]) -> String {
    let rest: Vec<String> = words[1..].iter().map(|w| format!("`{w}`")).collect();
    if rest.is_empty() { "nothing".to_string() } else { rest.join(", ") }
}

/// `guidedog publish --publisher=ID --config=FILE [--dry-run] ...`.
pub(crate) fn parse_publish(args: &[String], output: &mut Output) -> Result<Command, Problem> {
    let a = read_args("publish", args, output)?;
    if a.help {
        return Ok(Command::Help(Some("publish")));
    }
    let (Some(config), false, false) = (a.config, a.publisher.is_empty(), !a.words.is_empty())
    else {
        return Err(usage(
            "NAME THE PUBLISHER AND ITS CONFIGURATION",
            "guidedog publish needs --publisher and --config, and takes no other argument.",
            "Write guidedog publish --publisher=pages --config=pages.toml [--dry-run], or \
             guidedog publish pages for the guided GitHub Pages publication.",
        ));
    };
    Ok(Command::Publish(PublishCommand {
        publisher: a.publisher,
        config,
        dry_run: a.dry_run,
        wait: a.wait,
        timeout: a.timeout,
        verbose: a.verbose,
    }))
}
