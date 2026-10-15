//! Command-line extensions: commands a plugin adds to the host's command line, such as
//! `guidedog pages workflow` or a target of `guidedog publish`.
//!
//! A plugin declares its commands ([`CommandInfo`]: the words that name it, a summary,
//! help, and the options it takes) and runs them with a [`CommandContext`], which is
//! everything the host offers a command: its output streams, questions to the person,
//! a build of a project, the plugin's configuration, and cancellation. The plugin reads
//! its own arguments; the declared options are for help and suggestions.

use std::fmt;
use std::io::Write;
use std::path::PathBuf;

use gd_json::{Object, Value};

use crate::paths::is_bounded;
use crate::problem::{self, Problem};
use crate::wire::{arr_field, object, str_field};
use crate::{Cancel, is_valid_id};

/// The most commands one plugin may contribute.
pub const MAX_COMMANDS: usize = 32;
/// The most words a command's name may have.
const MAX_WORDS: usize = 4;
/// The longest summary of a command or an option, in bytes.
const MAX_SUMMARY: usize = 200;
/// The longest help text of a command, in bytes (16 KiB).
const MAX_HELP: usize = 16 * 1024;
/// The most options one command may declare.
const MAX_OPTIONS: usize = 64;

/// An option a contributed command takes, as its help shows it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandOption {
    /// The option as written, such as `--write` or `-o`.
    pub name: String,
    /// The placeholder of its value, such as `DIR`; empty for a switch.
    pub value: String,
    /// One line saying what it does.
    pub summary: String,
}

impl CommandOption {
    /// A switch, such as `--write`.
    pub fn switch(name: &str, summary: &str) -> CommandOption {
        CommandOption {
            name: name.to_string(),
            value: String::new(),
            summary: summary.to_string(),
        }
    }

    /// An option that takes a value, such as `--project DIR`.
    pub fn with_value(name: &str, value: &str, summary: &str) -> CommandOption {
        CommandOption {
            name: name.to_string(),
            value: value.to_string(),
            summary: summary.to_string(),
        }
    }

    /// Whether the option takes a value.
    pub fn takes_value(&self) -> bool {
        !self.value.is_empty()
    }
}

/// A command a plugin adds to the host's command line. Its `path` is the words that
/// name it: `["pages", "workflow"]` is `guidedog pages workflow`, and `["publish",
/// "pages"]` is the target `pages` of `guidedog publish`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandInfo {
    /// The words that name the command: 1 to 4 lowercase words.
    pub path: Vec<String>,
    /// One line saying what it does, for the host's command list.
    pub summary: String,
    /// What `--help` shows after the usage line: plain text, lines of at most 80
    /// characters.
    pub help: String,
    /// The options it takes, for its help and the host's suggestions.
    pub options: Vec<CommandOption>,
}

impl CommandInfo {
    /// A command named by the words of `name` (`"pages workflow"`), with its summary.
    pub fn new(name: &str, summary: &str) -> CommandInfo {
        CommandInfo {
            path: name.split_whitespace().map(str::to_string).collect(),
            summary: summary.to_string(),
            ..CommandInfo::default()
        }
    }

    /// The command with `help` as its help text.
    pub fn with_help(mut self, help: &str) -> CommandInfo {
        self.help = help.to_string();
        self
    }

    /// The command with `option` added to its options.
    pub fn with_option(mut self, option: CommandOption) -> CommandInfo {
        self.options.push(option);
        self
    }

    /// The command's name: its words joined by spaces, such as `pages workflow`.
    pub fn name(&self) -> String {
        self.path.join(" ")
    }
}

/// What a contributed command asks the host to build: a Guidedog project with one
/// builder into a folder, with settings overridden as `-D` does, the site's public URL,
/// and warnings as errors when `strict`. The default builds HTML.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuildRequest {
    /// The project folder (where conf.toml is).
    pub project: PathBuf,
    /// The folder the output is written to.
    pub output: PathBuf,
    /// The builder, as `guidedog build` names it (`html`, `dirhtml`, `singlehtml`,
    /// `pdf`, `text`); empty for `html`.
    pub builder: String,
    /// Settings that replace conf.toml's, each `name=value` as `-D` takes them.
    pub overrides: Vec<String>,
    /// The site's public URL (`html_baseurl`); empty to keep the project's.
    pub base_url: String,
    /// Build with warnings as errors.
    pub strict: bool,
}

/// What a host offers a contributed command. `guidedog` implements it over its own
/// terminal and engine; the SDK implements it over a program's standard streams when a
/// command runs as an installed program ([`crate::serve_command`]).
pub trait CommandHost {
    /// The command's standard output and standard error, together, since a command
    /// often writes to both at once.
    fn streams(&mut self) -> (&mut dyn Write, &mut dyn Write);

    /// Reads one line the person typed, without its line break; `None` at the end of
    /// the input.
    fn read_line(&mut self) -> Option<String>;

    /// Builds `request`'s project, as `guidedog build` does; the build's report goes to
    /// the host's error stream. Fails when the build fails or the host builds
    /// nothing.
    fn build_project(&mut self, request: &BuildRequest) -> Result<(), Problem>;
}

/// Everything a contributed command may use: the host's streams, questions to the
/// person, the host's build, and plain facts about the run in public fields.
pub struct CommandContext<'h> {
    host: &'h mut dyn CommandHost,
    /// The working folder the command was started in.
    pub dir: PathBuf,
    /// The plugin's configuration file, when the project names one (conf.toml's
    /// `plugin_configs`); empty for the plugin's own default.
    pub config: String,
    /// The host program, for a command that must start it (such as a watch loop).
    pub host_program: String,
    /// A person may be asked: input and error output are a terminal.
    pub interactive: bool,
    /// The person asked for colors on the terminal.
    pub color: bool,
    /// The person asked for JSON reports (`--diagnostics=json`).
    pub json: bool,
    /// The person asked for no success messages (`-q`).
    pub quiet: bool,
    /// Set when the person asks the command to stop (Ctrl+C).
    pub cancel: Cancel,
}

impl<'h> CommandContext<'h> {
    /// A context over `host`, in the current folder, not interactive, with no colors,
    /// no configuration and no cancellation; the host sets the fields it knows.
    pub fn new(host: &'h mut dyn CommandHost) -> CommandContext<'h> {
        CommandContext {
            host,
            dir: PathBuf::from("."),
            config: String::new(),
            host_program: String::new(),
            interactive: false,
            color: false,
            json: false,
            quiet: false,
            cancel: Cancel::default(),
        }
    }

    /// The host's standard output and standard error.
    pub fn streams(&mut self) -> (&mut dyn Write, &mut dyn Write) {
        self.host.streams()
    }

    /// Writes `text` to standard output. A closed output is not the command's problem.
    pub fn print(&mut self, text: &str) {
        let _ = self.host.streams().0.write_all(text.as_bytes());
    }

    /// Writes `text` to standard error.
    pub fn eprint(&mut self, text: &str) {
        let _ = self.host.streams().1.write_all(text.as_bytes());
    }

    /// Flushes both streams, before another program writes to the same terminal.
    pub fn flush(&mut self) {
        let (out, err) = self.host.streams();
        let _ = out.flush();
        let _ = err.flush();
    }

    /// Shows `question` on standard error and returns the person's answer, trimmed;
    /// `None` when no person may be asked or the input ended.
    pub fn ask(&mut self, question: &str) -> Option<String> {
        if !self.interactive {
            return None;
        }
        self.eprint(question);
        self.flush();
        self.host.read_line().map(|line| line.trim().to_string())
    }

    /// Asks a yes-or-no question; anything but `y` or `yes`, or no person, is no.
    pub fn confirm(&mut self, question: &str) -> bool {
        let answer = self.ask(&format!("{question} [y/N] ")).unwrap_or_default();
        matches!(answer.to_lowercase().as_str(), "y" | "yes")
    }

    /// Has the host build `request`'s project; see [`CommandHost::build_project`].
    pub fn build_project(&mut self, request: &BuildRequest) -> Result<(), Problem> {
        self.host.build_project(request)
    }
}

impl fmt::Debug for CommandContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandContext")
            .field("dir", &self.dir)
            .field("config", &self.config)
            .field("host_program", &self.host_program)
            .field("interactive", &self.interactive)
            .field("json", &self.json)
            .finish_non_exhaustive()
    }
}

/// `plugins.command` (exit 2): the plugin has no command of that name.
pub(crate) fn no_command(id: &str, name: &str) -> Problem {
    Problem::new(
        "plugins.command",
        "The plugin has no such command",
        &format!("Plugin {id:?} contributes no command {name:?}."),
    )
    .hint(&format!("Run guidedog plugin inspect {id} to see the commands it contributes."))
    .path(id)
    .exit(2)
}

/// Checks a plugin's command declarations: at most [`MAX_COMMANDS`], each named by 1
/// to 4 lowercase words no other command of the plugin uses, with a one-line summary,
/// bounded help, and well-formed options.
pub(crate) fn validate_commands(id: &str, commands: &[CommandInfo]) -> Result<(), Problem> {
    if commands.len() > MAX_COMMANDS {
        return Err(problem::contract(id, "Contribute at most 32 commands."));
    }
    for (i, c) in commands.iter().enumerate() {
        let words_ok =
            (1..=MAX_WORDS).contains(&c.path.len()) && c.path.iter().all(|w| is_command_word(w));
        if !words_ok {
            let hint = "Name each command with 1 to 4 lowercase words, such as pages workflow.";
            return Err(problem::contract(id, hint));
        }
        if commands[..i].iter().any(|d| d.path == c.path) {
            return Err(problem::contract(id, "Give every command a different name."));
        }
        if c.summary.is_empty()
            || !is_line(&c.summary, MAX_SUMMARY)
            || !is_bounded(&c.help, MAX_HELP)
        {
            let hint = "Give each command a one-line summary of at most 200 bytes and help of \
                        at most 16 KiB.";
            return Err(problem::contract(id, hint));
        }
        if c.options.len() > MAX_OPTIONS || !c.options.iter().all(is_valid_option) {
            let hint = "Declare at most 64 options, each named like --name or -n, with a \
                        one-line summary and a short value placeholder.";
            return Err(problem::contract(id, hint));
        }
    }
    Ok(())
}

/// Whether `word` can be a word of a command's name: a short ID-like word (one letter
/// is enough).
fn is_command_word(word: &str) -> bool {
    (word.len() == 1 && word.as_bytes()[0].is_ascii_lowercase()) || is_valid_id(word)
}

/// Whether `text` is one line of at most `max` bytes.
fn is_line(text: &str, max: usize) -> bool {
    is_bounded(text, max) && !text.contains(['\n', '\r'])
}

fn is_valid_option(o: &CommandOption) -> bool {
    let name_ok = o.name.len() >= 2
        && o.name.len() <= 64
        && o.name.starts_with('-')
        && o.name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    name_ok && is_line(&o.value, 32) && !o.value.contains(' ') && is_line(&o.summary, MAX_SUMMARY)
}

/// A command declaration as the process protocols write it.
pub(crate) fn command_json(c: &CommandInfo) -> Value {
    let words = Value::Array(c.path.iter().map(|w| w.as_str().into()).collect());
    let options = c.options.iter().map(|o| {
        object([
            ("name", o.name.as_str().into()),
            ("value", o.value.as_str().into()),
            ("summary", o.summary.as_str().into()),
        ])
    });
    object([
        ("path", words),
        ("summary", c.summary.as_str().into()),
        ("help", c.help.as_str().into()),
        ("options", Value::Array(options.collect())),
    ])
}

/// The command declaration a protocol object holds; `None` when it is malformed.
pub(crate) fn command_of(o: &Object) -> Option<CommandInfo> {
    let mut path = Vec::new();
    for word in arr_field(o, "path")? {
        path.push(word.as_str()?.to_string());
    }
    let mut options = Vec::new();
    for item in arr_field(o, "options").unwrap_or_default() {
        let item = item.as_object()?;
        let text = |key| str_field(item, key).map(str::to_string);
        options.push(CommandOption {
            name: text("name")?,
            value: text("value").unwrap_or_default(),
            summary: text("summary").unwrap_or_default(),
        });
    }
    Some(CommandInfo {
        path,
        summary: str_field(o, "summary")?.to_string(),
        help: str_field(o, "help").unwrap_or_default().to_string(),
        options,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workflow() -> CommandInfo {
        CommandInfo::new("pages workflow", "Write the GitHub Actions workflow")
            .with_help("Prints the workflow.")
            .with_option(CommandOption::switch("--write", "Write it"))
            .with_option(CommandOption::with_value("--project", "DIR", "The project"))
    }

    #[test]
    fn commands_are_named_by_their_words() {
        let c = workflow();
        assert_eq!(c.path, ["pages", "workflow"]);
        assert_eq!(c.name(), "pages workflow");
        assert!(c.options[1].takes_value() && !c.options[0].takes_value());
    }

    #[test]
    fn declarations_are_checked() {
        assert!(validate_commands("pages", &[workflow()]).is_ok());
        let bad = [
            CommandInfo::new("", "Nothing"),
            CommandInfo::new("Pages", "Upper case"),
            CommandInfo::new("a b c d e", "Too many words"),
            CommandInfo::new("pages", ""),
            CommandInfo::new("pages", "Two\nlines"),
            workflow().with_option(CommandOption::switch("write", "No dash")),
        ];
        for c in bad {
            assert!(validate_commands("pages", std::slice::from_ref(&c)).is_err(), "{c:?}");
        }
        assert!(validate_commands("pages", &[workflow(), workflow()]).is_err());
        let many = vec![CommandInfo::new("a", "A"); MAX_COMMANDS + 1];
        assert!(validate_commands("pages", &many).is_err());
    }

    #[test]
    fn declarations_cross_the_wire() {
        let c = workflow();
        let json = command_json(&c);
        assert_eq!(command_of(json.as_object().unwrap()), Some(c));
        let no_path = object([("summary", "S".into())]);
        assert_eq!(command_of(no_path.as_object().unwrap()), None);
    }
}
