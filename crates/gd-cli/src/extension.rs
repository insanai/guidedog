//! Commands plugins add to the command line: which ones there are (linked plugins
//! first, then the programs registered on this machine), and which one a command line
//! names. `guidedog pages workflow` is such a command, contributed by the Pages plugin;
//! so is the target `pages` of `guidedog publish`. Running them is `extension_run`.

use gd_host::Problem;
use gd_plugins::{CommandInfo, Registry};

use crate::args::{usage, with_suggestion};
use crate::command::{Command, ExtensionCommand};
use crate::parse::{COMMANDS, GLOBAL_VALUES, PUBLISH_VALUES};

/// A command a plugin adds to guidedog's command line, and the plugin that runs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extension {
    /// The ID of the plugin that contributes it.
    pub plugin: String,
    /// Its name, summary, help and options, as the plugin declares them.
    pub info: CommandInfo,
    /// The plugin is linked into this program (not a registered program).
    pub linked: bool,
}

impl Extension {
    /// The command's name, such as `pages workflow`.
    pub fn name(&self) -> String {
        self.info.name()
    }
}

/// Whether a plugin may add the command named `path`: not a built-in command, but a
/// target of `publish` is welcome.
fn is_allowed(path: &[String]) -> bool {
    let Some(first) = path.first() else {
        return false;
    };
    !COMMANDS.contains(&first.as_str()) || (first == "publish" && path.len() >= 2)
}

/// The commands the plugins in `linked` add, then (when `machine`) those of the
/// programs registered on this machine. A name already taken, by guidedog or an earlier
/// plugin, is skipped. An unreadable machine registry adds nothing here; `guidedog
/// plugin list` reports it.
pub fn extensions(linked: &Registry, machine: bool) -> Vec<Extension> {
    let mut found: Vec<Extension> = Vec::new();
    let mut add = |plugin: &str, info: &CommandInfo, linked: bool| {
        if is_allowed(&info.path) && !found.iter().any(|e| e.info.path == info.path) {
            found.push(Extension { plugin: plugin.to_string(), info: info.clone(), linked });
        }
    };
    for (plugin, info) in linked.commands() {
        add(plugin.id(), info, true);
    }
    let registered = machine
        .then(|| gd_plugins::machine_path().and_then(|p| gd_plugins::read_machine(&p)).ok())
        .flatten();
    for entry in registered.iter().flat_map(|m| &m.entries) {
        for info in &entry.commands {
            add(&entry.id, info, false);
        }
    }
    found
}

/// Whether the command line `args` may name a plugin's command, so the machine's
/// registry is worth reading: no command word (the help lists them), a word guidedog
/// does not know, or `publish` with a target.
pub(crate) fn may_name_extension(args: &[String]) -> bool {
    let Some((at, word)) = crate::args::command_word(args, GLOBAL_VALUES) else {
        return true;
    };
    if word == "publish" {
        let rest = crate::args::without(args, at);
        return crate::args::command_word(&rest, PUBLISH_VALUES).is_some()
            || rest.iter().any(|a| a == "--help" || a == "-h");
    }
    !COMMANDS.contains(&word)
}

/// The words of `args` that are not options or the values of `value_options`, with
/// their positions, up to `--`.
fn positional<'a>(args: &'a [String], value_options: &[&str]) -> Vec<(usize, &'a str)> {
    let mut found = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "--" {
            break;
        }
        if !arg.starts_with('-') || arg == "-" {
            found.push((i, arg));
        } else if !arg.contains('=') && value_options.contains(&arg) {
            i += 1;
        }
        i += 1;
    }
    found
}

/// The options that take the next word as their value, of guidedog and of every
/// extension, so that a value is never taken for a command's word.
fn value_options(extensions: &[Extension]) -> Vec<&str> {
    let mut names: Vec<&str> = GLOBAL_VALUES.iter().chain(PUBLISH_VALUES).copied().collect();
    let declared = extensions.iter().flat_map(|e| &e.info.options);
    names.extend(declared.filter(|o| o.takes_value()).map(|o| o.name.as_str()));
    names
}

/// The extension the command line `args` names, whose first word is `word`; `None`
/// when it names none and guidedog's own commands decide. A word that only begins
/// plugin commands, or a `publish` target no plugin adds, is a usage problem with the
/// closest names.
pub(crate) fn parse_extension(
    args: &[String],
    word: &str,
    extensions: &[Extension],
) -> Option<Result<Command, Problem>> {
    let words = positional(args, &value_options(extensions));
    let named = |e: &&Extension| {
        e.info.path.len() <= words.len()
            && e.info.path.iter().zip(&words).all(|(p, (_, w))| p == w)
    };
    let best = extensions.iter().filter(named).max_by_key(|e| e.info.path.len());
    if let Some(e) = best {
        let skip: Vec<usize> = words[..e.info.path.len()].iter().map(|(i, _)| *i).collect();
        let rest = args.iter().enumerate().filter(|(i, _)| !skip.contains(i));
        let rest: Vec<String> = rest.map(|(_, a)| a.clone()).collect();
        let before_end = rest.iter().take_while(|a| *a != "--");
        let help = before_end.clone().any(|a| a == "--help" || a == "-h");
        let command = ExtensionCommand { extension: e.clone(), args: rest, help };
        return Some(Ok(Command::Extension(command)));
    }
    let group: Vec<&Extension> =
        extensions.iter().filter(|e| e.info.path.len() > 1 && e.info.path[0] == word).collect();
    let next = words.get(1).map(|(_, w)| *w);
    match next {
        Some(next) if word == "publish" => Some(Err(unknown_target(next, &group))),
        None if word == "publish" => None,
        _ if group.is_empty() => None,
        Some(next) => Some(Err(unknown_subcommand(word, next, &group))),
        None if args.iter().any(|a| a == "--help" || a == "-h") => {
            Some(Ok(Command::ExtensionHelp(word.to_string())))
        }
        None => Some(Err(missing_subcommand(word, &group))),
    }
}

/// The second words of `group`, for messages.
fn second_words<'a>(group: &[&'a Extension]) -> Vec<&'a str> {
    group.iter().map(|e| e.info.path[1].as_str()).collect()
}

fn unknown_target(target: &str, group: &[&Extension]) -> Problem {
    let targets = second_words(group);
    let hint = if targets.is_empty() {
        "Write guidedog publish --publisher=ID --config=FILE; no plugin adds a publish target."
            .to_string()
    } else {
        format!(
            "Use guidedog publish {}, or --publisher=ID --config=FILE.",
            targets.join(", guidedog publish ")
        )
    };
    usage(
        "UNKNOWN PUBLISH TARGET",
        format!("No plugin adds the target `{target}` to guidedog publish."),
        with_suggestion(&hint, target, targets),
    )
}

fn unknown_subcommand(word: &str, next: &str, group: &[&Extension]) -> Problem {
    let words = second_words(group);
    usage(
        "UNKNOWN COMMAND",
        format!("guidedog {word} has no command `{next}`."),
        with_suggestion(
            &format!("Use guidedog {word} {}; see guidedog {word} --help.", words.join(", ")),
            next,
            words,
        ),
    )
}

fn missing_subcommand(word: &str, group: &[&Extension]) -> Problem {
    usage(
        "NAME A COMMAND",
        format!("guidedog {word} needs a command after it."),
        format!(
            "Use guidedog {word} {}; see guidedog {word} --help.",
            second_words(group).join(", ")
        ),
    )
}

/// The first words of the extensions, for "Did you mean" across all commands.
pub(crate) fn first_words(extensions: &[Extension]) -> Vec<&str> {
    let mut words: Vec<&str> = Vec::new();
    for e in extensions {
        let first = e.info.path[0].as_str();
        if !words.contains(&first) && !COMMANDS.contains(&first) {
            words.push(first);
        }
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn values_are_not_words() {
        let args = strings(&["publish", "--config", "x.toml", "pages", "--", "z"]);
        let found = positional(&args, &["--config"]);
        assert_eq!(found, [(0, "publish"), (3, "pages")]);
    }

    #[test]
    fn built_in_commands_cannot_be_replaced() {
        assert!(is_allowed(&strings(&["pages", "workflow"])));
        assert!(is_allowed(&strings(&["publish", "pages"])));
        assert!(!is_allowed(&strings(&["publish"])));
        assert!(!is_allowed(&strings(&["build", "fast"])));
    }
}
