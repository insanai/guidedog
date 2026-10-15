//! Reading the command line: options, flags, and which command each applies to.

use super::Program;
use crate::problem::usage;
use crate::{MAX_NUMBER, Problem, Result, is_valid_prefix};
use std::path::PathBuf;

/// The commands, in the order help lists them.
pub(crate) const COMMANDS: [&str; 11] = [
    "init", "new", "list", "show", "promote", "state", "index", "check", "build", "recover",
    "config",
];

/// Options that take a value, whatever the program.
const VALUE_OPTIONS: [&str; 16] = [
    "dir",
    "author",
    "slug",
    "labels",
    "state",
    "label",
    "to",
    "reason",
    "evidence",
    "discussion",
    "record",
    "target",
    "number",
    "color",
    "diagnostics",
    "format",
];
/// Options that start a catalog, which a program fixed to one catalog does not take.
const INIT_OPTIONS: [&str; 5] = ["prefix", "name", "process", "template", "config"];
/// Options that take no value.
const FLAGS: [&str; 5] = ["dry-run", "check", "include-historical", "render", "rollback"];

/// How output is styled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Color {
    /// Colors on a terminal, unless `NO_COLOR` is set or `TERM` is `dumb`.
    #[default]
    Auto,
    Always,
    Never,
}

/// A command line, read.
#[derive(Debug, Clone, Default)]
pub(crate) struct Options {
    pub command: String,
    pub identifier: String,
    pub dir: Option<PathBuf>,
    pub prefix: String,
    pub author: String,
    pub slug: String,
    pub labels: String,
    pub state: String,
    pub label: String,
    pub to: String,
    pub reason: String,
    pub evidence: String,
    pub discussion: String,
    pub record: String,
    pub target: String,
    pub number: Option<u32>,
    pub name: String,
    pub process: String,
    pub template: String,
    pub config: String,
    /// The builder's program: `--guidedog` or `--typst`.
    pub tool: Option<PathBuf>,
    pub color: Color,
    pub json_diagnostics: bool,
    pub json_format: bool,
    pub dry_run: bool,
    pub check_only: bool,
    pub historical: bool,
    pub render: bool,
    pub rollback: bool,
    pub quiet: bool,
    pub help: bool,
    pub version: bool,
    /// The options given, by name, with the argument that gave them.
    pub given: Vec<(String, String)>,
}

fn is_value_option(program: &Program<'_>, key: &str) -> bool {
    key == program.tool
        || VALUE_OPTIONS.contains(&key)
        || (program.prefix.is_none() && INIT_OPTIONS.contains(&key))
}

/// Reads `args` (without the program name). An unknown option, a missing or empty
/// value, a value for a flag, or a third positional argument is a usage problem.
pub(crate) fn parse(program: &Program<'_>, args: &[String]) -> Result<Options> {
    let mut o = Options::default();
    let mut positional: Vec<&str> = Vec::new();
    let mut options_end = false;
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        i += 1;
        if arg == "--" && !options_end {
            options_end = true;
            continue;
        }
        if options_end || !arg.starts_with('-') || arg == "-" {
            positional.push(arg);
            continue;
        }
        match arg {
            "-h" | "--help" => o.help = true,
            "-q" | "--quiet" => o.quiet = true,
            "--version" => o.version = true,
            _ => {
                let body = arg.trim_start_matches('-');
                let (key, value) = match body.split_once('=') {
                    Some((k, v)) => (k, Some(v)),
                    None => (body, None),
                };
                if !is_value_option(program, key) {
                    set_flag(program, &mut o, arg, key, value)?;
                } else {
                    let value = match value {
                        Some(v) => v,
                        None if i < args.len() && !args[i].starts_with("--") => {
                            i += 1;
                            args[i - 1].as_str()
                        }
                        None => {
                            return Err(usage(
                                "Missing option value",
                                arg,
                                format!("Write {arg}=VALUE."),
                            ));
                        }
                    };
                    if value.is_empty() {
                        return Err(usage("Empty option value", arg, "Supply a non-empty value."));
                    }
                    set_option(program, &mut o, key, value)?;
                }
                o.given.push((key.to_string(), arg.to_string()));
            }
        }
    }
    take_positional(&mut o, &positional)?;
    Ok(o)
}

fn take_positional(o: &mut Options, positional: &[&str]) -> Result<()> {
    if let Some(command) = positional.first() {
        o.command = command.to_string();
    }
    if let Some(identifier) = positional.get(1) {
        o.identifier = identifier.to_string();
    }
    if let Some(extra) = positional.get(2) {
        return Err(usage("Too many arguments", extra, "Quote multiword titles."));
    }
    Ok(())
}

fn set_flag(
    program: &Program<'_>,
    o: &mut Options,
    arg: &str,
    key: &str,
    value: Option<&str>,
) -> Result<()> {
    if !FLAGS.contains(&key) {
        if key == "typst" && program.tool == "guidedog" {
            return Err(usage(
                "Unknown option",
                arg,
                "Leave Typst to Guidedog: dsc builds with \
                the guidedog you have. Pass --guidedog=PATH to choose it.",
            ));
        }
        let mut known: Vec<&str> = VALUE_OPTIONS.iter().chain(&FLAGS).copied().collect();
        known.extend(["help", "quiet", "version", program.tool]);
        if program.prefix.is_none() {
            known.extend(INIT_OPTIONS);
        }
        let close = gd_report::suggest(key, known).map(|k| format!("--{k}"));
        return Err(usage(
            "Unknown option",
            arg,
            format!("Run {} --help to see the options.", program.name),
        )
        .suggest(close.as_deref()));
    }
    if let Some(value) = value {
        return Err(usage(
            "Unexpected option value",
            arg,
            format!("Write --{key} without ={value}; this flag takes no value."),
        ));
    }
    match key {
        "dry-run" => o.dry_run = true,
        "check" => o.check_only = true,
        "include-historical" => o.historical = true,
        "render" => o.render = true,
        _ => o.rollback = true,
    }
    Ok(())
}

fn set_option(program: &Program<'_>, o: &mut Options, key: &str, value: &str) -> Result<()> {
    let text = value.to_string();
    match key {
        "dir" => o.dir = Some(PathBuf::from(value)),
        "prefix" => o.prefix = text,
        "name" => o.name = text,
        "process" => o.process = text,
        "config" => o.config = text,
        "template" => o.template = text,
        "author" => o.author = text,
        "slug" => o.slug = text,
        "labels" => o.labels = text,
        "state" => o.state = text,
        "label" => o.label = text,
        "to" => o.to = text,
        "reason" => o.reason = text,
        "evidence" => o.evidence = text,
        "discussion" => o.discussion = text,
        "record" => o.record = text,
        "target" => o.target = text,
        "number" => o.number = Some(parse_number(value)?),
        "color" => o.color = parse_color(value)?,
        "diagnostics" => o.json_diagnostics = parse_choice(value, "text", "--diagnostics")?,
        "format" => o.json_format = parse_choice(value, "table", "--format")?,
        _ if key == program.tool => o.tool = Some(PathBuf::from(value)),
        _ => {}
    }
    Ok(())
}

fn parse_number(value: &str) -> Result<u32> {
    match value.parse::<u32>() {
        Ok(n) if (1..=MAX_NUMBER).contains(&n) => Ok(n),
        _ => Err(usage("Invalid number", value, "Use a whole number from 1 through 999999999.")),
    }
}

fn parse_color(value: &str) -> Result<Color> {
    match value {
        "auto" => Ok(Color::Auto),
        "always" => Ok(Color::Always),
        "never" => Ok(Color::Never),
        _ => Err(usage(
            "Invalid color mode",
            value,
            "Use --color=auto, --color=always, or --color=never.",
        )
        .suggest(gd_report::suggest(value, ["auto", "always", "never"]))),
    }
}

/// Reads a choice of `plain` or `json`; true for `json`.
fn parse_choice(value: &str, plain: &str, option: &str) -> Result<bool> {
    match value {
        "json" => Ok(true),
        v if v == plain => Ok(false),
        _ => Err(usage(
            "Invalid output format",
            value,
            format!("Use {option}={plain} or {option}=json."),
        )
        .suggest(gd_report::suggest(value, [plain, "json"]))),
    }
}

/// Tells whether the option `key` applies to `command`, so an option another command
/// would take is refused rather than silently ignored.
fn option_applies(program: &Program<'_>, command: &str, key: &str) -> bool {
    if key == program.tool {
        return command == "build" || command == "check";
    }
    let one_of = |commands: &[&str]| commands.contains(&command);
    match key {
        "author" | "labels" => one_of(&["new", "init"]),
        "slug" => command == "new",
        "name" | "process" | "template" | "config" => command == "init",
        "number" => one_of(&["promote", "build"]),
        "discussion" => command == "promote",
        "to" => command == "state",
        "reason" | "evidence" => one_of(&["state", "promote"]),
        "state" | "label" | "include-historical" => command == "list",
        "record" | "target" => command == "build",
        "render" => command == "check",
        "rollback" => command == "recover",
        "check" => command == "index",
        "dry-run" => one_of(&["init", "new", "promote", "state", "index", "recover"]),
        "format" => {
            one_of(&["list", "show", "new", "promote", "state", "index", "recover", "init"])
        }
        _ => true,
    }
}

/// Checks that the command exists, has the arguments it needs, and is given only
/// options that apply to it.
pub(crate) fn validate(program: &Program<'_>, o: &Options) -> Result<()> {
    let c = o.command.as_str();
    if !COMMANDS.contains(&c) || (program.prefix.is_some() && c == "init") {
        let commands = commands_text(program);
        let names = COMMANDS.iter().copied().filter(|n| program.prefix.is_none() || *n != "init");
        return Err(usage("Unknown command", c, format!("Use {commands}."))
            .suggest(gd_report::suggest(c, names)));
    }
    let needs_id = ["new", "show", "promote", "state"].contains(&c);
    if c != "build" && c != "init" && needs_id == o.identifier.is_empty() {
        return Err(usage(
            "Unexpected command arguments",
            c,
            "Give new a title, and show, state, and promote a record; build also accepts a \
             record.",
        ));
    }
    if c == "recover" && !o.rollback {
        return Err(usage(
            "Recovery needs a direction",
            c,
            format!("Use {} recover --rollback to restore the original files.", program.name),
        ));
    }
    if !o.prefix.is_empty() && !is_valid_prefix(&o.prefix) {
        return Err(usage(
            "Invalid prefix",
            &o.prefix,
            "Pass --prefix with 2 to 16 lowercase letters, such as eds.",
        ));
    }
    if let Some((_, arg)) = o.given.iter().find(|(key, _)| !option_applies(program, c, key)) {
        return Err(usage(
            "Option does not apply to this command",
            arg,
            format!("Run {} --help and use the option with its listed command.", program.name),
        ));
    }
    Ok(())
}

/// The commands a program runs, as a sentence's list.
pub(crate) fn commands_text(program: &Program<'_>) -> &'static str {
    if program.prefix.is_some() {
        "new, list, show, promote, state, index, check, build, recover, or config"
    } else {
        "init, new, list, show, promote, state, index, check, build, recover, or config"
    }
}

/// Splits `--labels=a, b` into trimmed labels.
pub(crate) fn split_labels(labels: &str) -> Vec<String> {
    labels.split(',').map(|l| l.trim().to_string()).collect()
}

/// A problem about the command line, for callers outside this module.
pub(crate) fn usage_problem(title: &str, value: &str, hint: &str) -> Problem {
    usage(title, value, hint.to_string())
}
