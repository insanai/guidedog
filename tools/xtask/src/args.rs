//! A small command-line reader shared by the tasks: flags (`--no-pdf`), options with a
//! value (`--out DIR` or `--out=DIR`), and positional arguments.

use crate::{Result, usage};

/// What a task accepts: its flags and options (with their dashes) and its help text.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Spec<'a> {
    /// The task's name as typed after `cargo xtask`, for hints.
    pub command: &'a str,
    /// Options without a value.
    pub flags: &'a [&'a str],
    /// Options that take a value.
    pub options: &'a [&'a str],
    /// The text `--help` prints.
    pub help: &'a str,
}

/// The arguments of one task, read with a [`Spec`].
#[derive(Debug, Default)]
pub(crate) struct Parsed {
    flags: Vec<String>,
    options: Vec<(String, String)>,
    /// The arguments that are not options, in order.
    pub positionals: Vec<String>,
}

impl Parsed {
    /// Whether the flag was given.
    pub fn has(&self, flag: &str) -> bool {
        self.flags.iter().any(|f| f == flag)
    }

    /// The value of the option given last, if it was given.
    pub fn option(&self, name: &str) -> Option<&str> {
        self.options.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }

    /// The positional argument at `index`, or a usage failure naming `what` is missing.
    pub fn positional(&self, index: usize, what: &str, spec: &Spec<'_>) -> Result<&str> {
        self.positionals.get(index).map(String::as_str).ok_or_else(|| {
            usage(
                &format!("`cargo xtask {}` needs {what}.", spec.command),
                &format!("`cargo xtask {} --help` shows how to call it.", spec.command),
            )
        })
    }

    /// Fails when more than `most` positional arguments were given.
    pub fn at_most(&self, most: usize, spec: &Spec<'_>) -> Result<()> {
        match self.positionals.get(most) {
            None => Ok(()),
            Some(extra) => Err(usage(
                &format!("`cargo xtask {}` does not take `{extra}`.", spec.command),
                &format!("`cargo xtask {} --help` shows how to call it.", spec.command),
            )),
        }
    }
}

/// Reads `args` with `spec`. Prints the help and returns `None` for `-h` or `--help`;
/// fails for an unknown option or an option without its value.
pub(crate) fn parse(args: &[String], spec: &Spec<'_>) -> Result<Option<Parsed>> {
    let mut parsed = Parsed::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == "-h" || arg == "--help" {
            print!("{}", spec.help);
            return Ok(None);
        }
        if !arg.starts_with("--") || arg == "--" {
            parsed.positionals.push(arg.clone());
            continue;
        }
        let (name, inline) = match arg.split_once('=') {
            Some((name, value)) => (name, Some(value.to_string())),
            None => (arg.as_str(), None),
        };
        if spec.flags.contains(&name) && inline.is_none() {
            parsed.flags.push(name.to_string());
        } else if spec.options.contains(&name) {
            let value = match inline {
                Some(value) => value,
                None => rest.next().cloned().ok_or_else(|| missing_value(name))?,
            };
            parsed.options.push((name.to_string(), value));
        } else {
            return Err(unknown_option(name, spec));
        }
    }
    Ok(Some(parsed))
}

fn missing_value(name: &str) -> crate::Failure {
    usage(
        &format!("The option {name} needs a value."),
        &format!("Write the value after it, such as `{name} build/site`."),
    )
}

fn unknown_option(name: &str, spec: &Spec<'_>) -> crate::Failure {
    let known = spec.flags.iter().chain(spec.options).copied();
    let mut hint = format!("`cargo xtask {} --help` lists the options.", spec.command);
    if let Some(near) = gd_report::suggest(name, known) {
        hint.push_str(&format!(" Did you mean `{near}`?"));
    }
    usage(&format!("`cargo xtask {}` has no option {name}.", spec.command), &hint)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: Spec<'static> =
        Spec { command: "demo", flags: &["--quick"], options: &["--out"], help: "" };

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn reads_flags_options_and_positionals() {
        let args = strings(&["a", "--quick", "--out", "x", "--out=y", "b"]);
        let parsed = parse(&args, &SPEC).ok().flatten().unwrap_or_default();
        assert!(parsed.has("--quick"));
        assert_eq!(parsed.option("--out"), Some("y"));
        assert_eq!(parsed.positionals, ["a", "b"]);
    }

    #[test]
    fn rejects_unknown_options_and_missing_values() {
        assert!(parse(&strings(&["--quik"]), &SPEC).is_err());
        assert!(parse(&strings(&["--out"]), &SPEC).is_err());
    }
}
