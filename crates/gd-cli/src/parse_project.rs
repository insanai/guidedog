//! The options of quickstart, build, serve, clean, migrate, and intl. Short options
//! follow sphinx-quickstart, sphinx-build and sphinx-intl, so habits and scripts carry
//! over; each command accepts only the options it uses.

use std::path::PathBuf;

use gd_conf::LoadOptions;
use gd_env::{BuildOptions, BuilderKind, Jobs};
use gd_host::Problem;

use crate::args::{Word, Words, no_value, positive, unknown_option, usage, with_suggestion};
use crate::command::{BuildCommand, CleanCommand, Command, IntlAction, IntlCommand, MemoryMode};
use crate::command::{MigrateCommand, MigrateOutput, Output, QuickstartCommand, ServeCommand};
use crate::parse::output_option;

const COMMON: &[&str] = &["-h", "--help", "-q", "-Q", "--quiet", "--color", "--diagnostics"];

const BUILD: &[&str] = &[
    "-b",
    "-M",
    "-a",
    "-E",
    "-n",
    "-W",
    "--keep-going",
    "--untrusted",
    "-C",
    "-c",
    "-d",
    "-D",
    "-t",
    "-j",
    "--budget",
    "--memory",
    "--disk-budget",
    "--max-depth",
    "--max-nodes",
    "--stack-kib",
    "-v",
    "--typst",
];

const QUICKSTART: &[&str] = &[
    "-p",
    "--project",
    "-a",
    "--author",
    "-v",
    "--version-name",
    "-r",
    "--release",
    "-l",
    "--language",
    "--sep",
    "--no-sep",
    "--suffix",
];

const MIGRATE: &[&str] = &["-o", "--output", "--force"];

const INTL: &[&str] = &[
    "-p",
    "--pot-dir",
    "-d",
    "--locale-dir",
    "-l",
    "--language",
    "-w",
    "--line-width",
    "--no-obsolete",
    "-c",
    "-D",
];

/// The options `command` has besides the common ones.
fn options_of(command: &str) -> Vec<&'static str> {
    let own: &[&str] = match command {
        "build" => BUILD,
        "serve" => BUILD,
        "quickstart" => QUICKSTART,
        "migrate" => MIGRATE,
        "intl" => INTL,
        _ => &[],
    };
    let mut all: Vec<&str> = COMMON.iter().chain(own).copied().collect();
    if command == "serve" {
        all.retain(|o| *o != "-b" && *o != "-M");
        all.push("--port");
    }
    all
}

/// Whether `name` takes a value for `command`. As in sphinx-quickstart and
/// sphinx-build, `-a` is the author and `-v` the version for quickstart; for builds, `-a`
/// writes all outputs and `-v` is more verbose.
fn takes_value(command: &str, name: &str) -> bool {
    match name {
        "-a" | "-v" => command == "quickstart",
        "-b" | "-M" | "-j" | "-D" | "-t" | "-c" | "-d" | "--port" | "--budget" | "--memory"
        | "--disk-budget" | "--max-depth" | "--max-nodes" | "--stack-kib" | "--typst" | "-p"
        | "--project" | "--author" | "--version-name" | "-r" | "--release" | "-l"
        | "--language" | "--suffix" | "-o" | "--output" | "-w" | "--line-width" | "--pot-dir"
        | "--locale-dir" => true,
        _ => false,
    }
}

/// Everything the project commands' options say, before it becomes a typed command.
#[derive(Debug, Default)]
struct ProjectArgs {
    positional: Vec<String>,
    builder: Option<String>,
    make_mode: bool,
    verbose: u32,
    memory: MemoryMode,
    typst: Option<String>,
    port: Option<u16>,
    load: LoadOptions,
    build: BuildOptions,
    quickstart: QuickstartCommand,
    migrate_output: Option<String>,
    force: bool,
    languages: Vec<String>,
    pot_dir: Option<PathBuf>,
    locale_dir: Option<PathBuf>,
    line_width: Option<usize>,
    no_obsolete: bool,
}

/// Reads the options of the project command `command` (the command word removed).
pub(crate) fn parse_project(
    command: &str,
    args: &[String],
    output: &mut Output,
) -> Result<Command, Problem> {
    let known = options_of(command);
    let mut words = Words::new(args);
    let mut a = ProjectArgs::default();
    while let Some(word) = words.next_word() {
        let (name, inline) = match word {
            Word::Arg(arg) => {
                a.positional.push(arg.to_string());
                continue;
            }
            Word::Opt { name, inline } => (name, inline),
        };
        if !known.contains(&name) {
            return Err(unknown_option(command, name, &known));
        }
        if name == "-Q" {
            output.quiet = true;
            continue;
        }
        if output_option(&mut words, output, name, inline)? {
            continue;
        }
        if name == "-h" || name == "--help" {
            return Ok(Command::Help(Some(help_topic(command))));
        }
        if !takes_value(command, name) {
            no_value(name, inline)?;
            set_flag(&mut a, command, name);
            continue;
        }
        let value = words.value(name, inline)?;
        set_value(&mut a, command, name, value)?;
    }
    if command == "quickstart" {
        a.quickstart.quiet = output.quiet;
    }
    finish(command, a)
}

fn help_topic(command: &str) -> &'static str {
    match command {
        "quickstart" => "quickstart",
        "migrate" => "migrate",
        "intl" => "intl",
        "serve" => "serve",
        "clean" => "clean",
        _ => "build",
    }
}

fn set_flag(a: &mut ProjectArgs, command: &str, name: &str) {
    match name {
        "-a" => a.build.all = true,
        "-E" => a.build.fresh = true,
        "-n" => a.build.nitpicky = true,
        "-W" => a.build.warnings_as_errors = true,
        "--keep-going" => a.build.keep_going = true,
        "--untrusted" => a.build.untrusted = true,
        "-C" => a.load.no_config = true,
        "-v" => a.verbose += 1,
        "--sep" => a.quickstart.separate = Some(true),
        "--no-sep" => a.quickstart.separate = Some(false),
        "--force" => a.force = true,
        "--no-obsolete" => a.no_obsolete = true,
        _ => debug_assert!(false, "{command}: {name} is listed but not handled"),
    }
}

fn set_value(a: &mut ProjectArgs, command: &str, name: &str, value: &str) -> Result<(), Problem> {
    let text = value.to_string();
    match (command, name) {
        ("intl", "-p" | "--pot-dir") => a.pot_dir = Some(PathBuf::from(value)),
        ("intl", "-d" | "--locale-dir") => a.locale_dir = Some(PathBuf::from(value)),
        ("intl", "-l" | "--language") => a.languages.push(text),
        ("intl", "-w" | "--line-width") => a.line_width = Some(positive(name, value, 10_000)?),
        (_, "-b") => a.builder = Some(text),
        (_, "-M") => (a.builder, a.make_mode) = (Some(text), true),
        (_, "-c") => a.load.conf_dir = Some(PathBuf::from(value)),
        // Records use the build's own cache folder; -d is accepted for scripts.
        (_, "-d") => {}
        (_, "-D") => a.load.overrides.push(text),
        (_, "-t") => a.build.tags.push(text),
        (_, "--typst") => a.typst = Some(text),
        (_, "-o" | "--output") => a.migrate_output = Some(text),
        (_, "-p" | "--project") => a.quickstart.project = text,
        (_, "-a" | "--author") => a.quickstart.author = text,
        (_, "-v" | "--version-name") => a.quickstart.version = text,
        (_, "-r" | "--release") => a.quickstart.release = text,
        (_, "-l" | "--language") => a.quickstart.language = text,
        (_, "--suffix") => a.quickstart.suffix = text,
        _ => set_number(a, name, value)?,
    }
    Ok(())
}

/// The options with numbers and the budget's mode.
fn set_number(a: &mut ProjectArgs, name: &str, value: &str) -> Result<(), Problem> {
    match name {
        "-j" if value == "auto" => a.build.jobs = Jobs::PerCore,
        "-j" => {
            let n = value.parse::<usize>().ok().filter(|&n| n >= 1).ok_or_else(|| {
                usage(
                    "INVALID JOB COUNT",
                    format!("`{value}` is not a number of threads."),
                    "Use -j N with a positive number, or -j auto for one thread per core.",
                )
            })?;
            a.build.jobs = Jobs::Count(n);
        }
        "--port" => a.port = Some(port(value)?),
        "--budget" => a.build.budget = Some(budget(value)? << 20),
        "--memory" => a.memory = memory_mode(value)?,
        "--disk-budget" => return Err(no_disk(name)),
        "--max-depth" => a.build.max_depth = Some(positive(name, value, usize::MAX)?),
        "--max-nodes" => a.build.max_nodes = Some(positive(name, value, usize::MAX)?),
        _ => {
            let kib = positive(name, value, usize::MAX)?.min(usize::MAX >> 10);
            a.build.stack_bytes = Some(kib << 10);
        }
    }
    Ok(())
}

fn port(value: &str) -> Result<u16, Problem> {
    value.parse::<u16>().ok().filter(|&p| p >= 1).ok_or_else(|| {
        usage("INVALID PORT", format!("`{value}` is not a port."), "Use a port from 1 to 65535.")
    })
}

fn budget(value: &str) -> Result<usize, Problem> {
    positive("--budget", value, 1 << 20).map_err(|_| {
        usage(
            "INVALID BUDGET",
            format!("`{value}` is not a memory budget."),
            "Give the memory budget in MiB, from 1 to 1048576 (1 TiB).",
        )
    })
}

fn memory_mode(value: &str) -> Result<MemoryMode, Problem> {
    match value {
        "ram" => Ok(MemoryMode::Stop),
        "disk" => Err(no_disk("--memory=disk")),
        _ => Err(usage(
            "INVALID MEMORY MODE",
            format!("`{value}` is not a memory mode."),
            with_suggestion(
                "Use --memory=ram to stop at the budget without asking.",
                value,
                ["ram"],
            ),
        )),
    }
}

/// `--memory=disk` and `--disk-budget`: earlier releases could keep working memory on
/// disk; builds now keep it in memory, so the problem says what to do instead.
fn no_disk(option: &str) -> Problem {
    Problem::new(
        "cli.memory-disk",
        "WORKING MEMORY ON DISK IS NOT AVAILABLE",
        "",
        format!(
            "{option} kept a build's working memory on disk in earlier versions. This \
             version keeps it in memory, bounded by the budget, and has no disk mode."
        ),
        "Raise the budget with --budget=MIB (up to the memory this machine has free), or \
         lower the work with -j 1. Use --memory=ram to stop at the budget without asking.",
        2,
    )
}

/// Turns the options into the command, checking its arguments.
fn finish(command: &str, mut a: ProjectArgs) -> Result<Command, Problem> {
    match command {
        "build" => build_command(a).map(|b| Command::Build(Box::new(b))),
        "serve" => {
            let dir = one_folder(command, &a.positional)?;
            let port = a.port.unwrap_or(8000);
            let build = typed_build(&mut a, dir, BuilderKind::Html);
            Ok(Command::Serve(Box::new(ServeCommand { build, port })))
        }
        "clean" => Ok(Command::Clean(CleanCommand { dir: one_folder(command, &a.positional)? })),
        "quickstart" => {
            a.quickstart.dir = one_folder(command, &a.positional)?;
            Ok(Command::Quickstart(a.quickstart))
        }
        "migrate" => {
            let output = match a.migrate_output.as_deref() {
                None => MigrateOutput::Beside,
                Some("-") => MigrateOutput::Stdout,
                Some(file) => MigrateOutput::File(PathBuf::from(file)),
            };
            let input = one_folder(command, &a.positional)?;
            Ok(Command::Migrate(MigrateCommand { input, output, force: a.force }))
        }
        _ => intl_command(a).map(Command::Intl),
    }
}

/// The one optional folder of clean, serve, quickstart, and migrate (default `.`).
fn one_folder(command: &str, positional: &[String]) -> Result<PathBuf, Problem> {
    match positional {
        [] => Ok(PathBuf::from(".")),
        [one] => Ok(PathBuf::from(one)),
        more => Err(usage(
            "TOO MANY ARGUMENTS",
            format!("guidedog {command} takes one folder, and was given {}.", more.join(" ")),
            format!("Write guidedog {command} [FOLDER]; quote a path that holds spaces."),
        )),
    }
}

fn typed_build(a: &mut ProjectArgs, dir: PathBuf, kind: BuilderKind) -> BuildCommand {
    BuildCommand {
        dir,
        kind,
        load: std::mem::take(&mut a.load),
        options: std::mem::take(&mut a.build),
        memory: a.memory,
        verbose: a.verbose,
        typst: a.typst.take(),
    }
}

/// `build [BUILDER] [PROJECT]`, or the sphinx-build forms `-b BUILDER SOURCEDIR
/// OUTPUTDIR` and `-M BUILDER SOURCEDIR BUILDDIR`.
fn build_command(mut a: ProjectArgs) -> Result<BuildCommand, Problem> {
    let (builder, dir) = match a.builder.take() {
        Some(builder) => {
            let [source, out] = a.positional.as_slice() else {
                return Err(usage(
                    "SOURCE AND OUTPUT FOLDERS ARE REQUIRED",
                    "The sphinx-build form names the source folder and the output folder.",
                    "Write guidedog build -b html SOURCEDIR OUTPUTDIR, or guidedog build html.",
                ));
            };
            a.load.source = Some(PathBuf::from(source));
            a.load.output = Some(PathBuf::from(out));
            let output = PathBuf::from(out);
            a.build.output_dir = Some(if a.make_mode { output.join(&builder) } else { output });
            (builder, PathBuf::from("."))
        }
        None => builder_and_project(&a.positional)?,
    };
    let kind = builder_kind(&builder)?;
    Ok(typed_build(&mut a, dir, kind))
}

/// The builder and project of `build [BUILDER] [PROJECT]`: a first word that is no
/// builder must be a project folder.
fn builder_and_project(positional: &[String]) -> Result<(String, PathBuf), Problem> {
    let mut rest = positional;
    let mut builder = "html".to_string();
    if let Some(first) = rest.first() {
        if BuilderKind::from_name(first).is_some() {
            builder = first.clone();
            rest = &rest[1..];
        } else if !std::path::Path::new(first).is_dir() {
            return Err(unknown_builder(first, "; or name a project folder"));
        }
    }
    match rest {
        [] => Ok((builder, PathBuf::from("."))),
        [dir] => Ok((builder, PathBuf::from(dir))),
        more => Err(usage(
            "TOO MANY BUILD ARGUMENTS",
            format!("guidedog build was given `{}` after the builder.", more.join(" ")),
            "Put the builder first: guidedog build html PROJECT. For a custom output \
             folder, use guidedog build -b html SOURCEDIR OUTPUTDIR.",
        )),
    }
}

fn builder_kind(name: &str) -> Result<BuilderKind, Problem> {
    BuilderKind::from_name(name).ok_or_else(|| unknown_builder(name, ""))
}

fn unknown_builder(name: &str, more: &str) -> Problem {
    usage(
        "UNKNOWN BUILDER",
        format!("`{name}` is not a builder."),
        with_suggestion(
            &format!(
                "Use html, dirhtml, singlehtml, pdf, text, gettext, dummy, or coverage{more}."
            ),
            name,
            BuilderKind::ALL.iter().map(|k| k.name()),
        ),
    )
}

/// `intl update|stat [PROJECT]`.
fn intl_command(a: ProjectArgs) -> Result<IntlCommand, Problem> {
    let word = a.positional.first().map_or("", String::as_str);
    let action = match word {
        "update" => IntlAction::Update,
        "stat" => IntlAction::Stat,
        _ => {
            return Err(usage(
                "UNKNOWN INTL COMMAND",
                format!("guidedog intl has no command `{word}`."),
                with_suggestion(
                    "Use guidedog intl update -l LANGUAGE, or guidedog intl stat; see \
                     guidedog intl --help.",
                    word,
                    ["update", "stat"],
                ),
            ));
        }
    };
    let dir = one_folder("intl", &a.positional[1..])?;
    Ok(IntlCommand {
        action,
        dir,
        load: LoadOptions {
            conf_dir: a.load.conf_dir,
            overrides: a.load.overrides,
            ..Default::default()
        },
        languages: a.languages,
        pot_dir: a.pot_dir,
        locale_dir: a.locale_dir,
        line_width: a.line_width.unwrap_or(76),
        obsolete: !a.no_obsolete,
    })
}
