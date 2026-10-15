//! The pieces every command's parser shares: walking the words of a command line,
//! taking option values, reading numbers, and usage problems with "Did you mean".

use gd_host::Problem;

/// One word of a command line, as a parser sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Word<'a> {
    /// An argument: a word that is not an option, a lone `-`, or anything after `--`.
    Arg(&'a str),
    /// An option such as `--output`, `-o` or `--output=FILE`: its name with the dashes,
    /// and the value written after `=`, if any.
    Opt { name: &'a str, inline: Option<&'a str> },
}

/// Walks a command line's words. `--` ends the options: every word after it is an
/// argument. A long option takes its value as `--name=VALUE` or as the next word.
#[derive(Debug)]
pub(crate) struct Words<'a> {
    args: &'a [String],
    at: usize,
    options: bool,
}

impl<'a> Words<'a> {
    /// Starts at the first of `args`.
    pub fn new(args: &'a [String]) -> Words<'a> {
        Words { args, at: 0, options: true }
    }

    /// The next word, or `None` at the end.
    pub fn next_word(&mut self) -> Option<Word<'a>> {
        loop {
            let arg = self.args.get(self.at)?.as_str();
            self.at += 1;
            if self.options && arg == "--" {
                self.options = false;
                continue;
            }
            if !self.options || !arg.starts_with('-') || arg == "-" {
                return Some(Word::Arg(arg));
            }
            return Some(match arg.split_once('=') {
                Some((name, value)) => Word::Opt { name, inline: Some(value) },
                None => Word::Opt { name: arg, inline: None },
            });
        }
    }

    /// The value of the option `name`: the one written after `=`, else the next word.
    /// Fails when there is none, when the next word is another long option, or when the
    /// value is empty.
    pub fn value(&mut self, name: &str, inline: Option<&'a str>) -> Result<&'a str, Problem> {
        let value = match inline {
            Some(value) => value,
            None => match self.args.get(self.at) {
                Some(next) if !next.starts_with("--") => {
                    self.at += 1;
                    next.as_str()
                }
                _ => {
                    return Err(usage(
                        "MISSING OPTION VALUE",
                        format!("`{name}` needs a value, and none follows it."),
                        format!("Write {name}=VALUE, or {name} VALUE."),
                    ));
                }
            },
        };
        if value.is_empty() {
            return Err(usage(
                "EMPTY OPTION VALUE",
                format!("`{name}` was given an empty value."),
                "Supply a non-empty value.",
            ));
        }
        Ok(value)
    }
}

/// A usage problem (`cli.usage`, exit status 2).
pub(crate) fn usage(
    title: &'static str,
    message: impl Into<String>,
    hint: impl Into<String>,
) -> Problem {
    Problem::new("cli.usage", title, "", message, hint, 2)
}

/// `hint`, followed by "Did you mean `X`?" when one of `candidates` is close to `name`.
pub(crate) fn with_suggestion<'c>(
    hint: &str,
    name: &str,
    candidates: impl IntoIterator<Item = &'c str>,
) -> String {
    match gd_report::suggest(name, candidates) {
        Some(close) => format!("{hint} Did you mean `{close}`?"),
        None => hint.to_string(),
    }
}

/// The problem of an option the command does not have; `known` are the ones it has.
pub(crate) fn unknown_option(command: &str, name: &str, known: &[&str]) -> Problem {
    let shown =
        if command.is_empty() { "guidedog".to_string() } else { format!("guidedog {command}") };
    usage(
        "UNKNOWN OPTION",
        format!("`{name}` is not an option of {shown}."),
        with_suggestion(
            &format!("Run {shown} --help to see its options."),
            name,
            known.iter().copied(),
        ),
    )
}

/// Checks that the switch `name` was written without a value; `--force=yes` is a
/// usage problem.
pub(crate) fn no_value(name: &str, inline: Option<&str>) -> Result<(), Problem> {
    match inline {
        None => Ok(()),
        Some(value) => Err(usage(
            "UNEXPECTED OPTION VALUE",
            format!("`{name}` is a switch and takes no value, but was given `{value}`."),
            format!("Write {name} without ={value}."),
        )),
    }
}

/// Reads a whole number from 1 to `max` for the option `name`.
pub(crate) fn positive(name: &str, value: &str, max: usize) -> Result<usize, Problem> {
    match value.parse::<usize>() {
        Ok(n) if (1..=max).contains(&n) => Ok(n),
        _ => Err(usage(
            "INVALID NUMBER",
            format!("`{name}` needs a whole number from 1 to {max}, not `{value}`."),
            format!("Write {name}=N with a positive whole number, at most {max}."),
        )),
    }
}

/// Reads a number of seconds from 0 to a day, as `--wait` and `--timeout` take them.
pub(crate) fn seconds(value: &str) -> Option<u32> {
    if value.is_empty() || value.len() > 5 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    value.parse::<u32>().ok().filter(|&n| n <= 86_400)
}

/// The command word of a command line and its position: the first word that is not an
/// option and not the value of one of `value_options` written as the next word.
pub(crate) fn command_word<'a>(
    args: &'a [String],
    value_options: &[&str],
) -> Option<(usize, &'a str)> {
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "--" {
            return args.get(i + 1).map(|word| (i + 1, word.as_str()));
        }
        if !arg.starts_with('-') || arg == "-" {
            return Some((i, arg));
        }
        if !arg.contains('=') && value_options.contains(&arg) {
            i += 1;
        }
        i += 1;
    }
    None
}

/// `args` without the word at `at`.
pub(crate) fn without(args: &[String], at: usize) -> Vec<String> {
    let mut rest = Vec::with_capacity(args.len().saturating_sub(1));
    rest.extend_from_slice(&args[..at]);
    rest.extend_from_slice(&args[at + 1..]);
    rest
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn words_split_options_values_and_arguments() {
        let args = strings(&["a", "--x=1", "-y", "-", "--", "--z"]);
        let mut w = Words::new(&args);
        assert_eq!(w.next_word(), Some(Word::Arg("a")));
        assert_eq!(w.next_word(), Some(Word::Opt { name: "--x", inline: Some("1") }));
        assert_eq!(w.next_word(), Some(Word::Opt { name: "-y", inline: None }));
        assert_eq!(w.next_word(), Some(Word::Arg("-")));
        assert_eq!(w.next_word(), Some(Word::Arg("--z")));
        assert_eq!(w.next_word(), None);
    }

    #[test]
    fn values_come_inline_or_next_and_are_never_empty_or_options() {
        let args = strings(&["v", "--next"]);
        let mut w = Words::new(&args);
        assert_eq!(w.value("--a", None).ok(), Some("v"));
        assert_eq!(w.value("--a", None).map_err(|p| p.title), Err("MISSING OPTION VALUE"));
        assert_eq!(w.value("--a", Some("")).map_err(|p| p.title), Err("EMPTY OPTION VALUE"));
    }

    #[test]
    fn unknown_options_suggest_the_closest() {
        let p = unknown_option("convert", "--outptu", &["--output", "--force"]);
        assert_eq!(p.exit, 2);
        assert!(p.hint.contains("Did you mean `--output`?"), "{}", p.hint);
    }

    #[test]
    fn numbers_and_seconds_are_bounded() {
        assert_eq!(positive("--port", "80", 65535).ok(), Some(80));
        assert!(positive("--port", "0", 65535).is_err());
        assert!(positive("--port", "70000", 65535).is_err());
        assert_eq!(seconds("0"), Some(0));
        assert_eq!(seconds("86401"), None);
        assert_eq!(seconds("-1"), None);
    }

    #[test]
    fn the_command_word_skips_global_option_values() {
        let args = strings(&["--color", "always", "-q", "build", "html"]);
        assert_eq!(command_word(&args, &["--color"]), Some((3, "build")));
        assert_eq!(without(&args, 3), strings(&["--color", "always", "-q", "html"]));
        assert_eq!(command_word(&strings(&["--version"]), &[]), None);
    }
}
