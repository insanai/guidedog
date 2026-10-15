//! The `guidedog-pages` command line: it signs in to GitHub, authorizes targets, and
//! publishes Guidedog sites to GitHub Pages. A project is built by the installed
//! `guidedog`; the program links no document engine. Started with
//! `gd_plugins::PROTOCOL_FLAG`, it serves the publisher protocol instead, for
//! `guidedog publish --publisher=pages`; started with `gd_plugins::COMMAND_FLAG`, it
//! runs the command it contributes to guidedog's command line (`guidedog publish pages`,
//! `guidedog pages workflow`) on the terminal guidedog lends it.

mod auth;
mod display;
pub(crate) mod extension;
mod help;
mod publish;
mod secret_input;
mod target;
#[cfg(test)]
mod tests;
mod watch;
// Console mode calls on Windows: FFI.
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows_console;
mod workflow;

use std::io::Write;

use gd_plugins::{Cancel, Problem};

use crate::config::is_valid_profile;
use crate::credential::{Credential, ENV_PROFILE, TOKEN_VARIABLE};
use crate::{CONFIG_FILE, Pages, Settings};
use display::Console;

/// The program's version, which is Guidedog's.
pub const VERSION: &str = crate::VERSION;

/// What the command line runs with: where state, credentials and GitHub are, and the
/// probe that Ctrl+C sets. The default is the real thing, with no cancellation.
#[derive(Debug, Clone, Default)]
pub struct Program {
    /// Where the run's state, credentials and GitHub are.
    pub settings: Settings,
    /// Stops transfers and waits; `guidedog-pages` sets it on Ctrl+C.
    pub cancel: Cancel,
    /// The working folder (repository and project inference, `workflow`); `None` is the
    /// current folder.
    pub dir: Option<std::path::PathBuf>,
}

/// The commands `guidedog-pages` has.
pub const COMMANDS: &[&str] = &[
    "init", "auth", "bind", "unbind", "setup", "doctor", "publish", "watch", "status", "recover",
    "workflow",
];

const VALUES: &[&str] = &[
    "config",
    "repository",
    "project",
    "site",
    "branch",
    "profile",
    "client-id",
    "scope",
    "credential",
    "guidedog",
    "operation",
    "wait",
    "color",
    "diagnostics",
    "sha256",
    "release-repository",
];
/// Options that are switches alone and take a value with `=`: `--serve[=PORT]`, and
/// `--version` (the program's version) or `--version=X` (workflow: Guidedog's).
const OPTIONAL_VALUES: &[&str] = &["serve", "version"];
const FLAGS: &[&str] = &[
    "dry-run", "adopt", "yes", "open", "replace", "force", "verbose", "help", "version", "write",
    "check", "offline",
];

/// A command line's settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Options {
    pub command: String,
    pub sub: String,
    pub config: String,
    pub repository: String,
    pub project: String,
    pub site: String,
    pub branch: String,
    pub profile: String,
    pub client_id: String,
    pub scope: String,
    /// `env`: the token in `GUIDEDOG_PAGES_TOKEN`.
    pub credential: String,
    pub guidedog: String,
    pub operation: String,
    /// publish: seconds to wait; `None` when not given.
    pub wait: Option<u32>,
    pub color: String,
    pub json: bool,
    pub dry_run: bool,
    pub adopt: bool,
    pub yes: bool,
    pub open: bool,
    pub replace: bool,
    pub force: bool,
    pub verbose: bool,
    pub help: bool,
    pub version: bool,
    /// `--branch` was given (workflow's default branch differs).
    pub branch_given: bool,
    /// watch: serve a preview on this port.
    pub serve: Option<u16>,
    /// workflow: the Guidedog release; empty for this program's version.
    pub release: String,
    /// workflow: the release archive's SHA-256.
    pub sha256: String,
    /// workflow: the repository releases come from; empty for the default.
    pub release_repository: String,
    pub write: bool,
    pub check: bool,
    pub offline: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            command: String::new(),
            sub: String::new(),
            config: CONFIG_FILE.to_string(),
            repository: String::new(),
            project: String::new(),
            site: String::new(),
            branch: "gh-pages".to_string(),
            profile: "default".to_string(),
            client_id: String::new(),
            scope: "public".to_string(),
            credential: String::new(),
            guidedog: String::new(),
            operation: String::new(),
            wait: None,
            color: "auto".to_string(),
            json: false,
            dry_run: false,
            adopt: false,
            yes: false,
            open: false,
            replace: false,
            force: false,
            verbose: false,
            help: false,
            version: false,
            branch_given: false,
            serve: None,
            release: String::new(),
            sha256: String::new(),
            release_repository: String::new(),
            write: false,
            check: false,
            offline: false,
        }
    }
}

/// Runs the command line `args` (without the program name) with standard output and
/// error, and returns the exit status: 0 published or unchanged, 1 failed (nothing
/// published), 2 usage, 6 published but not confirmed live, 7 unknown (run recover).
pub fn run(program: &Program, args: &[String]) -> i32 {
    if args.len() == 2 && args[0] == gd_plugins::COMMAND_FLAG {
        let plugin = gd_plugins::Plugin::publisher(Pages::new(program.settings.clone()));
        return i32::from(gd_plugins::serve_command(&plugin, &args[1]));
    }
    if args.len() == 1 && args[0] == gd_plugins::PROTOCOL_FLAG {
        let input = std::io::BufReader::new(std::io::stdin().lock());
        let publisher = Pages::new(program.settings.clone());
        return i32::from(gd_plugins::serve_publisher_streams(
            publisher,
            input,
            std::io::stdout().lock(),
        ));
    }
    let (mut out, mut err) = (std::io::stdout().lock(), std::io::stderr().lock());
    run_with(program, args, &mut out, &mut err)
}

/// Runs the command line `args` writing to `out` and `err`; see [`run`].
pub fn run_with(
    program: &Program,
    args: &[String],
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let (o, parsed) = parse(args);
    let mut console = Console::new(out, err, &o);
    if let Err(p) = parsed {
        return console.report(&p);
    }
    if o.version {
        console.print(&format!(
            "guidedog pages {VERSION}\npublishes to GitHub Pages with an \
            installed guidedog (--guidedog, ${}, or PATH)\n",
            crate::GUIDEDOG_VARIABLE
        ));
        return 0;
    }
    if o.help || o.command.is_empty() {
        console.print(&help::text());
        return 0;
    }
    let mut settings = program.settings.clone();
    if o.credential == ENV_PROFILE {
        settings.credential = Credential::Env;
    }
    let dir = program.dir.clone().unwrap_or_else(|| std::path::PathBuf::from("."));
    let ctx = Context { o: &o, settings: &settings, cancel: &program.cancel, dir };
    match o.command.as_str() {
        "init" => target::init(&ctx, &mut console),
        "auth" => auth::run(&ctx, &mut console),
        "bind" => target::bind(&ctx, &mut console),
        "unbind" => target::unbind(&ctx, &mut console),
        "setup" => target::setup(&ctx, &mut console),
        "doctor" => target::doctor(&ctx, &mut console),
        "publish" => publish::publish(&ctx, &mut console),
        "watch" => watch::watch(&ctx, &mut console),
        "workflow" => workflow::workflow(&ctx, &mut console),
        "status" => publish::status(&ctx, &mut console),
        _ => publish::recover(&ctx, &mut console),
    }
}

/// What every command reads: the options, the settings, and the cancel probe.
pub(crate) struct Context<'a> {
    pub o: &'a Options,
    pub settings: &'a Settings,
    pub cancel: &'a Cancel,
    pub dir: std::path::PathBuf,
}

impl Context<'_> {
    /// The configuration's path: `--config` from the working folder.
    pub fn config_path(&self) -> std::path::PathBuf {
        self.dir.join(&self.o.config)
    }
}

/// The options each command takes, beyond `--help`, `--color`, and `--diagnostics`.
fn allowed(command: &str, sub: &str) -> &'static [&'static str] {
    match (command, sub) {
        ("init", _) => &["config", "repository", "project", "site", "branch", "profile", "force"],
        ("auth", "login") => &["profile", "client-id", "scope", "open", "replace"],
        ("auth", "import") => &["profile", "replace"],
        ("auth", _) => &["profile"],
        ("bind", _) => &["config", "adopt", "yes", "credential"],
        ("unbind", _) => &["config"],
        ("setup", _) => &["config", "yes", "credential"],
        ("doctor" | "recover", _) => &["config", "credential"],
        ("publish", _) => &[
            "config",
            "dry-run",
            "wait",
            "guidedog",
            "credential",
            "verbose",
            "repository",
            "project",
            "branch",
            "profile",
            "client-id",
            "yes",
        ],
        ("watch", _) => &["config", "guidedog", "credential", "serve"],
        ("workflow", _) => &[
            "project",
            "branch",
            "version",
            "sha256",
            "offline",
            "release-repository",
            "write",
            "check",
            "force",
        ],
        ("status", _) => &["config", "operation", "credential"],
        _ => &[],
    }
}

/// Reads `args` into options; a word or option the command does not take is a problem
/// naming the ones it does. The options are returned even then, so that the problem is
/// reported as `--diagnostics` asks.
pub(crate) fn parse(args: &[String]) -> (Options, Result<(), Problem>) {
    let mut o = Options::default();
    // How to report comes first, so that a mistake later on is reported as asked.
    for arg in args {
        if let Some(v) = arg.strip_prefix("--diagnostics=") {
            o.json = v == "json";
        }
        if let Some(v) = arg.strip_prefix("--color=") {
            o.color = v.to_string();
        }
    }
    let result = parse_into(&mut o, args);
    (o, result)
}

fn parse_into(o: &mut Options, args: &[String]) -> Result<(), Problem> {
    let mut words = Vec::new();
    let mut given = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        i += 1;
        let Some(option) = arg.strip_prefix("--").filter(|rest| !rest.is_empty()) else {
            words.push(arg.as_str());
            continue;
        };
        let (key, value) = match option.split_once('=') {
            Some((k, v)) => (k, Some(v.to_string())),
            None => (option, None),
        };
        let optional = OPTIONAL_VALUES.contains(&key);
        let is_flag = FLAGS.contains(&key) || (optional && value.is_none());
        if !is_flag && !optional && !VALUES.contains(&key) {
            return Err(usage(
                "Unknown option",
                arg,
                "See guidedog pages --help for the options of each command.",
            ));
        }
        if is_flag && !optional && value.is_some() {
            return Err(usage("A switch takes no value", arg, &format!("Write --{key} alone.")));
        }
        let value = match value {
            Some(v) => v,
            None if is_flag => String::new(),
            None if i < args.len() => {
                i += 1;
                args[i - 1].clone()
            }
            None => return Err(usage("Missing value", arg, &format!("Write --{key}=VALUE."))),
        };
        set_option(o, key, &value)?;
        given.push(key);
    }
    check_words(o, &words)?;
    let takes = allowed(&o.command, &o.sub);
    for key in given {
        let global = ["help", "color", "diagnostics"].contains(&key)
            || (key == "version" && o.release.is_empty());
        if global || takes.contains(&key) {
            continue;
        }
        let name =
            if o.sub.is_empty() { o.command.clone() } else { format!("{} {}", o.command, o.sub) };
        let list: Vec<String> = takes.iter().map(|k| format!("--{k}")).collect();
        let hint = if list.is_empty() {
            "Use no options.".to_string()
        } else {
            format!("Use {}.", list.join(", "))
        };
        return Err(usage(&format!("{name} has no such option"), &format!("--{key}"), &hint));
    }
    Ok(())
}

fn check_words(o: &mut Options, words: &[&str]) -> Result<(), Problem> {
    if let Some(first) = words.first() {
        o.command = first.to_string();
    }
    let auth = o.command == "auth";
    if auth && let Some(sub) = words.get(1) {
        o.sub = sub.to_string();
    }
    let extra = if auth { 2 } else { 1 };
    if let Some(word) = words.get(extra) {
        return Err(usage(
            "Unexpected word",
            word,
            "Give options as --name=value; see guidedog pages --help.",
        ));
    }
    if !o.command.is_empty() && !COMMANDS.contains(&o.command.as_str()) {
        let hint = format!("Use one of: {}.", COMMANDS.join(", "));
        return Err(usage("Unknown command", &o.command, &hint));
    }
    if auth && !["login", "import", "status", "logout"].contains(&o.sub.as_str()) {
        return Err(usage(
            "Unknown auth command",
            &o.sub,
            "Use the auth command with login, import, status, or logout.",
        ));
    }
    Ok(())
}

fn set_option(o: &mut Options, key: &str, value: &str) -> Result<(), Problem> {
    let text = value.to_string();
    match key {
        "config" => o.config = text,
        "repository" => o.repository = text,
        "project" => o.project = text,
        "site" => o.site = text,
        "branch" => {
            o.branch = text;
            o.branch_given = true;
        }
        "profile" => o.profile = text,
        "client-id" => o.client_id = text,
        "guidedog" => o.guidedog = text,
        "operation" => o.operation = text,
        "version" => o.release = text,
        "release-repository" => o.release_repository = text,
        "sha256" | "serve" => return set_checked(o, key, value),
        "scope" if value == "public" || value == "private" => o.scope = text,
        "scope" => {
            return Err(usage(
                "Unknown scope",
                value,
                "Use --scope=public for public repositories, or --scope=private.",
            ));
        }
        "credential" if value == ENV_PROFILE || value == "profile" => o.credential = text,
        "credential" => {
            return Err(usage(
                "Unknown credential",
                value,
                &format!("Use --credential=env for the token in {TOKEN_VARIABLE}."),
            ));
        }
        "wait" => match value.parse::<u32>() {
            Ok(n) if n <= 86_400 && !value.starts_with('+') => o.wait = Some(n),
            _ => {
                return Err(usage(
                    "Not a number of seconds",
                    value,
                    "Write --wait=SECONDS, from 0 (do not wait) to 86400.",
                ));
            }
        },
        "color" if ["auto", "always", "never"].contains(&value) => o.color = text,
        "color" => {
            return Err(usage(
                "Unknown color choice",
                value,
                "Use --color=auto, always, or never.",
            ));
        }
        "diagnostics" if value == "text" || value == "json" => o.json = value == "json",
        "diagnostics" => {
            return Err(usage(
                "Unknown diagnostics format",
                value,
                "Use --diagnostics=text or json.",
            ));
        }
        _ => set_flag(o, key),
    }
    Ok(())
}

/// Sets an option whose value is checked.
fn set_checked(o: &mut Options, key: &str, value: &str) -> Result<(), Problem> {
    if key == "sha256" {
        if value.len() != 64 || !value.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(usage(
                "Not a SHA-256",
                value,
                "Write --sha256= and the 64 hexadecimal digits of the archive's SHA-256.",
            ));
        }
        o.sha256 = value.to_ascii_lowercase();
        return Ok(());
    }
    match value.parse::<u16>() {
        Ok(port) if !value.starts_with('+') => o.serve = Some(port),
        _ => {
            return Err(usage(
                "Not a port",
                value,
                "Write --serve for port 8000, or --serve=PORT with a port from 0 to 65535.",
            ));
        }
    }
    Ok(())
}

fn set_flag(o: &mut Options, key: &str) {
    match key {
        "serve" => o.serve = Some(8000),
        "write" => o.write = true,
        "check" => o.check = true,
        "offline" => o.offline = true,
        "dry-run" => o.dry_run = true,
        "adopt" => o.adopt = true,
        "yes" => o.yes = true,
        "open" => o.open = true,
        "replace" => o.replace = true,
        "force" => o.force = true,
        "verbose" => o.verbose = true,
        "help" => o.help = true,
        _ => o.version = true,
    }
}

/// `pages.usage` (exit 2): a command line that is not understood.
pub(crate) fn usage(title: &str, value: &str, hint: &str) -> Problem {
    crate::problem::problem(
        "pages.usage",
        title,
        value,
        &format!("{value:?} is not understood here."),
        hint,
    )
    .exit(2)
}

/// Refuses a profile name that is not one, for the auth commands.
pub(crate) fn check_profile(o: &Options) -> Result<(), Problem> {
    if is_valid_profile(&o.profile) {
        return Ok(());
    }
    Err(usage(
        "Not a profile name",
        &o.profile,
        "Use letters, digits, and - _ ., such as --profile=work.",
    ))
}
