//! The top of the command line: which command it names, how to report, and the options
//! of the commands that need no project (`convert`, `formats`, `--version`, `--help`).
//! Project commands are read in `parse_project`, plugin commands in `parse_plugin`.

use std::path::{Path, PathBuf};

use gd_host::{Format, Problem, Target};
use guidedoc::ContentPolicy;

use crate::args::{Word, Words, command_word, no_value, positive, unknown_option, usage};
use crate::args::{with_suggestion, without};
use crate::command::Parsed;
use crate::command::{ColorMode, Command, ConvertCommand, Diagnostics, FormatsCommand, Output};
use crate::extension::{Extension, first_words, parse_extension};

/// Every command word, for the help text and "Did you mean".
pub(crate) const COMMANDS: &[&str] = &[
    "quickstart",
    "build",
    "serve",
    "clean",
    "migrate",
    "intl",
    "convert",
    "formats",
    "publish",
    "plugin",
];

/// The options that may come before the command word and take the next word as value.
pub(crate) const GLOBAL_VALUES: &[&str] = &["--color", "--diagnostics", "--typst", "--format"];

/// The options of `publish` that take the next word as value, so that `--publisher
/// pages` is not taken for the target `publish pages` (targets declare their own).
pub(crate) const PUBLISH_VALUES: &[&str] =
    &["--publisher", "--config", "--wait", "--timeout", "--color", "--diagnostics"];

/// The largest `--stack-kib` of a conversion: what a thread of the command's stack
/// (`gd_host::WORKER_STACK_BYTES`, the stack cmd/guidedog runs on) holds; 6144 KiB.
pub(crate) fn max_stack_kib() -> usize {
    gd_host::stack_budget_limit(gd_host::WORKER_STACK_BYTES) >> 10
}

/// Reads a command line (without the program name) into the command it asks for, with
/// guidedog's own commands only. Never fails: a mistake is returned as the command's
/// problem, beside the reporting options read so far, so that it is reported as
/// `--diagnostics` asks.
pub fn parse_command(args: &[String]) -> Parsed {
    parse_command_with(args, &[])
}

/// [`parse_command`] knowing the commands plugins add ([`crate::extensions`]): a word
/// guidedog does not know, or a `publish` target, names one of them.
pub fn parse_command_with(args: &[String], extensions: &[Extension]) -> Parsed {
    let mut output = prescan_output(args);
    let command = parse_into(args, extensions, &mut output);
    Parsed { output, command }
}

fn parse_into(
    args: &[String],
    extensions: &[Extension],
    output: &mut Output,
) -> Result<Command, Problem> {
    let Some((at, word)) = command_word(args, GLOBAL_VALUES) else {
        return parse_global(args, output);
    };
    if (word == "publish" || !COMMANDS.contains(&word))
        && let Some(found) = parse_extension(args, word, extensions)
    {
        return found;
    }
    let rest = without(args, at);
    match word {
        "plugin" => crate::parse_plugin::parse_plugin(&rest, output),
        "publish" => crate::parse_plugin::parse_publish(&rest, output),
        "convert" => parse_convert(&rest, output),
        "formats" => parse_formats(&rest, output),
        "quickstart" | "build" | "serve" | "clean" | "migrate" | "intl" => {
            crate::parse_project::parse_project(word, &rest, output)
        }
        _ => Err(unknown_command(word, extensions)),
    }
}

/// The problem of a command word neither guidedog nor a plugin knows, with the closest
/// of both.
fn unknown_command(word: &str, extensions: &[Extension]) -> Problem {
    let added = first_words(extensions);
    let hint = if added.is_empty() {
        "Use quickstart, build, serve, clean, migrate, intl, convert, formats, publish, \
         or plugin; see guidedog --help."
            .to_string()
    } else {
        format!(
            "See guidedog --help for its commands and the ones plugins add ({}).",
            added.join(", ")
        )
    };
    let candidates = COMMANDS.iter().copied().chain(added.iter().copied());
    usage(
        "UNKNOWN COMMAND",
        format!("`{word}` is not a guidedog command."),
        with_suggestion(&hint, word, candidates),
    )
}

/// `--color` and `--diagnostics` wherever they are, so that a mistake found before them
/// is still reported as they ask. Values are checked by the command's own parser.
fn prescan_output(args: &[String]) -> Output {
    let mut output = Output::default();
    for (i, arg) in args.iter().enumerate() {
        if arg == "--" {
            break;
        }
        let (name, value) = match arg.split_once('=') {
            Some((name, value)) => (name, Some(value)),
            None => (arg.as_str(), args.get(i + 1).map(String::as_str)),
        };
        match (name, value) {
            ("--color", Some(v)) => output.color = color_mode(v).unwrap_or_default(),
            ("--diagnostics", Some(v)) => output.diagnostics = diagnostics(v).unwrap_or_default(),
            ("-q" | "--quiet", _) => output.quiet = true,
            _ => {}
        }
    }
    output
}

fn color_mode(value: &str) -> Option<ColorMode> {
    match value {
        "auto" => Some(ColorMode::Auto),
        "always" => Some(ColorMode::Always),
        "never" => Some(ColorMode::Never),
        _ => None,
    }
}

fn diagnostics(value: &str) -> Option<Diagnostics> {
    match value {
        "text" => Some(Diagnostics::Text),
        "json" => Some(Diagnostics::Json),
        _ => None,
    }
}

/// Sets the reporting option `name` (`--color`, `--diagnostics`, `-q`) and returns
/// `true`, or returns `false` for any other option. Shared by every parser.
pub(crate) fn output_option<'a>(
    words: &mut Words<'a>,
    output: &mut Output,
    name: &str,
    inline: Option<&'a str>,
) -> Result<bool, Problem> {
    match name {
        "-q" | "--quiet" => {
            no_value(name, inline)?;
            output.quiet = true;
        }
        "--color" => {
            let value = words.value(name, inline)?;
            output.color = color_mode(value).ok_or_else(|| {
                usage(
                    "INVALID COLOR MODE",
                    format!("`{value}` is not a color mode."),
                    "Use --color=auto, --color=always, or --color=never.",
                )
            })?;
        }
        "--diagnostics" => {
            let value = words.value(name, inline)?;
            output.diagnostics = diagnostics(value).ok_or_else(|| {
                usage(
                    "INVALID OUTPUT FORMAT",
                    format!("`{value}` is not a diagnostics format."),
                    "Use --diagnostics=text or --diagnostics=json.",
                )
            })?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// A command line without a command word: `--version`, `--licenses`, `--help`, or
/// nothing (the help).
fn parse_global(args: &[String], output: &mut Output) -> Result<Command, Problem> {
    const KNOWN: &[&str] = &[
        "--version",
        "--licenses",
        "--help",
        "-h",
        "--typst",
        "--color",
        "--diagnostics",
        "--quiet",
        "-q",
    ];
    let mut words = Words::new(args);
    let (mut version, mut licenses, mut typst) = (false, false, "typst".to_string());
    while let Some(word) = words.next_word() {
        let Word::Opt { name, inline } = word else { continue };
        if output_option(&mut words, output, name, inline)? {
            continue;
        }
        match name {
            "--version" => version = true,
            "--licenses" => licenses = true,
            "-h" | "--help" => {}
            "--typst" => typst = words.value(name, inline)?.to_string(),
            _ => return Err(unknown_option("", name, KNOWN)),
        }
    }
    Ok(if version {
        Command::Version { typst }
    } else if licenses {
        Command::Licenses
    } else {
        Command::Help(None)
    })
}

/// `guidedog formats [--format=table|json] [--typst=PATH]`.
fn parse_formats(args: &[String], output: &mut Output) -> Result<Command, Problem> {
    const KNOWN: &[&str] =
        &["--format", "--typst", "--help", "-h", "--color", "--diagnostics", "--quiet", "-q"];
    let mut words = Words::new(args);
    let mut command = FormatsCommand { json: false, typst: "typst".to_string() };
    while let Some(word) = words.next_word() {
        let (name, inline) = match word {
            Word::Arg(arg) => return Err(unexpected_argument("formats", arg)),
            Word::Opt { name, inline } => (name, inline),
        };
        if output_option(&mut words, output, name, inline)? {
            continue;
        }
        match name {
            "-h" | "--help" => return Ok(Command::Help(Some("formats"))),
            "--typst" => command.typst = words.value(name, inline)?.to_string(),
            "--format" => command.json = table_or_json(words.value(name, inline)?)?,
            _ => return Err(unknown_option("formats", name, KNOWN)),
        }
    }
    Ok(Command::Formats(command))
}

fn table_or_json(value: &str) -> Result<bool, Problem> {
    match value {
        "table" => Ok(false),
        "json" => Ok(true),
        _ => Err(usage(
            "INVALID OUTPUT FORMAT",
            format!("`{value}` is not an output format."),
            "Use --format=table or --format=json.",
        )),
    }
}

/// The problem of an argument a command does not take.
pub(crate) fn unexpected_argument(command: &str, arg: &str) -> Problem {
    usage(
        "UNEXPECTED ARGUMENT",
        format!("guidedog {command} takes no argument `{arg}` here."),
        format!("Run guidedog {command} --help to see what it takes."),
    )
}

/// The options of `convert`.
const CONVERT_OPTIONS: &[&str] = &[
    "--from",
    "--to",
    "--output",
    "-o",
    "--emit-typst",
    "--force",
    "--raw",
    "--strict",
    "--title",
    "--lang",
    "--fragment",
    "--number-sections",
    "--typst",
    "--budget",
    "--max-depth",
    "--max-nodes",
    "--stack-kib",
    "--color",
    "--diagnostics",
    "--offline",
    "--font-path",
    "--embedded-fonts-only",
    "--package-cache",
    "--quiet",
    "-q",
    "--help",
    "-h",
];

/// What `convert`'s options say before the formats are resolved.
#[derive(Debug, Default)]
struct ConvertArgs {
    inputs: Vec<String>,
    from: String,
    to: String,
    output: String,
    raw: String,
    command: ConvertCommand,
}

/// `guidedog convert INPUT --output FILE [options]`.
fn parse_convert(args: &[String], output: &mut Output) -> Result<Command, Problem> {
    let mut words = Words::new(args);
    let command = ConvertCommand { typst: "typst".to_string(), ..Default::default() };
    let mut a = ConvertArgs { command, ..Default::default() };
    while let Some(word) = words.next_word() {
        let (name, inline) = match word {
            Word::Arg(arg) => {
                a.inputs.push(arg.to_string());
                continue;
            }
            Word::Opt { name, inline } => (name, inline),
        };
        if output_option(&mut words, output, name, inline)? {
            continue;
        }
        if name == "-h" || name == "--help" {
            return Ok(Command::Help(Some("convert")));
        }
        convert_option(&mut words, &mut a, name, inline)?;
    }
    finish_convert(a).map(|c| Command::Convert(Box::new(c)))
}

fn convert_option<'a>(
    words: &mut Words<'a>,
    a: &mut ConvertArgs,
    name: &str,
    inline: Option<&'a str>,
) -> Result<(), Problem> {
    let c = &mut a.command;
    let flag = match name {
        "--force" => Some(&mut c.force),
        "--strict" => Some(&mut c.strict),
        "--fragment" => Some(&mut c.fragment),
        "--number-sections" => Some(&mut c.number_sections),
        "--offline" => Some(&mut c.offline),
        "--embedded-fonts-only" => Some(&mut c.embedded_fonts_only),
        _ => None,
    };
    if let Some(flag) = flag {
        no_value(name, inline)?;
        *flag = true;
        return Ok(());
    }
    if !CONVERT_OPTIONS.contains(&name) {
        return Err(unknown_option("convert", name, CONVERT_OPTIONS));
    }
    let value = words.value(name, inline)?;
    match name {
        "--from" => a.from = value.to_string(),
        "--to" => a.to = value.to_string(),
        "-o" | "--output" => a.output = value.to_string(),
        "--raw" => a.raw = value.to_string(),
        "--emit-typst" => c.emit_typst = Some(PathBuf::from(value)),
        "--title" => c.title = value.to_string(),
        "--lang" => c.lang = value.to_string(),
        "--typst" => c.typst = value.to_string(),
        "--font-path" => c.font_paths.push(PathBuf::from(value)),
        "--package-cache" => c.package_cache = Some(PathBuf::from(value)),
        _ => convert_number(c, name, value)?,
    }
    Ok(())
}

fn convert_number(c: &mut ConvertCommand, name: &str, value: &str) -> Result<(), Problem> {
    match name {
        "--budget" => {
            c.budget_mib = Some(positive(name, value, 1 << 20).map_err(|_| {
                usage(
                    "BUDGET IS TOO LARGE",
                    format!("`{value}` is not a budget from 1 to 1048576 MiB."),
                    "Use --budget=N with N at most 1048576 (MiB, which is 1 TiB).",
                )
            })?);
        }
        "--max-depth" => c.max_depth = Some(positive(name, value, usize::MAX)?),
        "--max-nodes" => c.max_nodes = Some(positive(name, value, usize::MAX)?),
        _ => {
            let n = positive(name, value, usize::MAX)?;
            let max = max_stack_kib();
            if n > max {
                return Err(usage(
                    "STACK BUDGET IS TOO LARGE",
                    format!("A stack budget of {n} KiB does not fit the command's stack."),
                    format!("Use at most {max} KiB, which leaves the thread room."),
                ));
            }
            c.stack_kib = Some(n);
        }
    }
    Ok(())
}

/// Checks the input count and resolves the formats from the options and suffixes.
fn finish_convert(a: ConvertArgs) -> Result<ConvertCommand, Problem> {
    let mut c = a.command;
    if a.inputs.len() != 1 {
        return Err(usage(
            "CONVERT NEEDS ONE INPUT",
            format!("guidedog convert takes one input, and was given {}.", a.inputs.len()),
            "Write guidedog convert guide.rst --to html --output guide.html.",
        ));
    }
    c.input = a.inputs[0].clone();
    c.from = input_format(&a.from, &c.input)?;
    c.to = output_target(&a.to, &a.output)?;
    if a.output.is_empty() {
        return Err(usage(
            "MISSING OUTPUT",
            format!("No output was named for `{}`.", c.input),
            "Add --output FILE, or --output - to write to standard output.",
        ));
    }
    c.output = a.output;
    if c.from == Format::Typst && c.emit_typst.is_some() {
        return Err(usage(
            "NOTHING TO EMIT",
            "--emit-typst writes the generated Typst, and the input is already Typst.",
            "Remove --emit-typst.",
        ));
    }
    if c.from == Format::Typst && c.to == Target::Rst {
        return Err(usage(
            "TYPST CANNOT BE WRITTEN AS RESTRUCTUREDTEXT",
            "Typst input is compiled by Typst, not read into a document tree.",
            "Convert Typst input with --to html or --to pdf.",
        ));
    }
    c.raw = raw_policy(&a.raw)?;
    Ok(c)
}

fn suffix(path: &str) -> String {
    Path::new(path)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default()
}

/// The input format `--from` names, or the input's suffix tells.
pub(crate) fn input_format(from: &str, input: &str) -> Result<Format, Problem> {
    let name = if from.is_empty() {
        match suffix(input).as_str() {
            ".rst" | ".rest" => "rst",
            ".md" | ".markdown" => "commonmark",
            ".typ" => "typst",
            _ => {
                return Err(usage(
                    "CANNOT TELL THE INPUT FORMAT",
                    format!("The suffix of `{input}` names no input format."),
                    "Add --from rst, --from commonmark, or --from typst.",
                ));
            }
        }
    } else {
        from
    };
    match name {
        "rst" | "restructuredtext" => Ok(Format::Rst),
        "sphinx" => Ok(Format::Sphinx),
        "md" | "markdown" | "commonmark" => Ok(Format::Commonmark),
        "myst" => Ok(Format::Myst),
        "typ" | "typst" => Ok(Format::Typst),
        _ => Err(usage(
            "UNKNOWN INPUT FORMAT",
            format!("`{name}` is not an input format."),
            with_suggestion(
                "Use rst, sphinx, commonmark, myst, or typst.",
                name,
                ["rst", "sphinx", "commonmark", "myst", "typst"],
            ),
        )),
    }
}

/// The output format `--to` names, or the output's suffix tells.
pub(crate) fn output_target(to: &str, output: &str) -> Result<Target, Problem> {
    let name = if to.is_empty() {
        match suffix(output).as_str() {
            ".html" | ".htm" => "html",
            ".pdf" => "pdf",
            ".rst" | ".rest" => "rst",
            _ => {
                return Err(usage(
                    "CANNOT TELL THE OUTPUT FORMAT",
                    format!("The suffix of `{output}` names no output format."),
                    "Add --to html, pdf, or rst. Use --emit-typst to keep generated Typst.",
                ));
            }
        }
    } else {
        to
    };
    match name {
        "html" => Ok(Target::Html),
        "pdf" => Ok(Target::Pdf),
        "rst" | "restructuredtext" => Ok(Target::Rst),
        _ => Err(usage(
            "UNKNOWN OUTPUT FORMAT",
            format!("`{name}` is not an output format."),
            with_suggestion(
                "Use --to html, pdf, or rst; --emit-typst writes the intermediate Typst.",
                name,
                ["html", "pdf", "rst"],
            ),
        )),
    }
}

fn raw_policy(raw: &str) -> Result<ContentPolicy, Problem> {
    match raw {
        "" | "refuse" => Ok(ContentPolicy::Refuse),
        "omit" => Ok(ContentPolicy::Omit),
        "allow" => Ok(ContentPolicy::Allow),
        _ => Err(usage(
            "INVALID RAW-CONTENT POLICY",
            format!("`{raw}` is not a raw-content policy."),
            with_suggestion(
                "Use --raw=refuse, --raw=omit, or --raw=allow.",
                raw,
                ["refuse", "omit", "allow"],
            ),
        )),
    }
}
