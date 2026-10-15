//! The typed form of a command line: what [`crate::parse_command`] returns and the
//! commands run. Every field is plain data, so a test (or an embedder) can build one
//! with struct-update syntax and check what a command line means without running it.

use std::path::PathBuf;

use gd_conf::LoadOptions;
use gd_env::{BuildOptions, BuilderKind};
use gd_host::{Format, Target};
use guidedoc::ContentPolicy;

/// When the command styles its output with colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    /// Only on a terminal, unless `NO_COLOR` is set or `TERM` is `dumb`.
    #[default]
    Auto,
    /// Always.
    Always,
    /// Never.
    Never,
}

/// How problems and reports are written on standard error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Diagnostics {
    /// Explained, Elm-style reports.
    #[default]
    Text,
    /// One JSON object per report.
    Json,
}

/// How the command reports, whatever it runs: `--color`, `--diagnostics`, `-q`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Output {
    /// `--color`.
    pub color: ColorMode,
    /// `--diagnostics`.
    pub diagnostics: Diagnostics,
    /// `-q`: no success messages; warnings and errors still show.
    pub quiet: bool,
}

/// A parsed command line: how to report, and the command, or the usage problem that
/// stopped parsing (reported as `output` asks).
#[derive(Debug)]
pub struct Parsed {
    /// How to report.
    pub output: Output,
    /// The command to run, or why the command line is not one.
    pub command: Result<Command, gd_host::Problem>,
}

/// What a command line asks for.
#[derive(Debug, Clone)]
pub enum Command {
    /// Show help: the overview (`None`) or a command's.
    Help(Option<&'static str>),
    /// `--version`, with the Typst program to describe.
    Version {
        /// `--typst`.
        typst: String,
    },
    /// `--licenses`.
    Licenses,
    /// `formats`.
    Formats(FormatsCommand),
    /// `convert`.
    Convert(Box<ConvertCommand>),
    /// `quickstart`.
    Quickstart(QuickstartCommand),
    /// `build` (and the sphinx-build forms).
    Build(Box<BuildCommand>),
    /// `serve`.
    Serve(Box<ServeCommand>),
    /// `clean`.
    Clean(CleanCommand),
    /// `migrate`.
    Migrate(MigrateCommand),
    /// `intl update` and `intl stat`.
    Intl(IntlCommand),
    /// `plugin ...`.
    Plugin(PluginCommand),
    /// `publish --publisher=ID`.
    Publish(PublishCommand),
    /// A command a plugin adds, such as `pages workflow` or `publish pages`.
    Extension(ExtensionCommand),
    /// `WORD --help`, where WORD begins only commands plugins add: their list.
    ExtensionHelp(String),
}

/// A command a plugin adds, as a command line names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionCommand {
    /// The command and the plugin that contributes it.
    pub extension: crate::Extension,
    /// The command line without the command's words, for the plugin to read.
    pub args: Vec<String>,
    /// `--help` or `-h` was given (before `--`): guidedog shows the command's help.
    pub help: bool,
}

/// `guidedog formats`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatsCommand {
    /// `--format=json` instead of the table.
    pub json: bool,
    /// `--typst`: the Typst program to describe.
    pub typst: String,
}

/// `guidedog convert INPUT`: one file to HTML or PDF, without a project. The formats
/// are resolved from the options and the file suffixes while parsing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConvertCommand {
    /// The input path, or `-` for standard input.
    pub input: String,
    /// The output path, or `-` for standard output.
    pub output: String,
    /// The input's markup.
    pub from: Format,
    /// The artifact to write.
    pub to: Target,
    /// `--emit-typst`: where to write the generated Typst too.
    pub emit_typst: Option<PathBuf>,
    /// `--force`: replace an existing output.
    pub force: bool,
    /// `--raw`: what happens to raw HTML or Typst content.
    pub raw: ContentPolicy,
    /// `--strict`: warnings fail the conversion.
    pub strict: bool,
    /// `--title`.
    pub title: String,
    /// `--lang`.
    pub lang: String,
    /// `--fragment`: the body only.
    pub fragment: bool,
    /// `--number-sections`.
    pub number_sections: bool,
    /// `--typst`: the Typst program, `typst` for the embedded compiler.
    pub typst: String,
    /// `--font-path`, repeatable.
    pub font_paths: Vec<PathBuf>,
    /// `--package-cache`.
    pub package_cache: Option<PathBuf>,
    /// `--offline`.
    pub offline: bool,
    /// `--embedded-fonts-only`.
    pub embedded_fonts_only: bool,
    /// `--budget`, in MiB.
    pub budget_mib: Option<usize>,
    /// `--max-depth`.
    pub max_depth: Option<usize>,
    /// `--max-nodes`.
    pub max_nodes: Option<usize>,
    /// `--stack-kib`.
    pub stack_kib: Option<usize>,
}

/// What a build does when it reaches its memory budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemoryMode {
    /// A terminal asks (raise the budget, or stop); elsewhere the build stops.
    #[default]
    Ask,
    /// `--memory=ram`: stop at the budget without asking.
    Stop,
}

/// `guidedog build`: what to load, what to build, and how.
#[derive(Debug, Clone)]
pub struct BuildCommand {
    /// The project folder (the sphinx-build forms: the source folder).
    pub dir: PathBuf,
    /// The builder.
    pub kind: BuilderKind,
    /// `-c`, `-C`, `-D`, and the sphinx-build forms' folders.
    pub load: LoadOptions,
    /// `-a`, `-E`, `-n`, `-W`, `--keep-going`, `--untrusted`, `-j`, `-t`, the limits, the
    /// budget, and the output folder of the sphinx-build forms. `on_budget` and `now` are
    /// set when the build runs.
    pub options: BuildOptions,
    /// `--memory`.
    pub memory: MemoryMode,
    /// `-v`, repeatable: show notes too.
    pub verbose: u32,
    /// `--typst`: overrides `typst` in conf.toml.
    pub typst: Option<String>,
}

/// `guidedog serve`: the HTML build it repeats, and the port.
#[derive(Debug, Clone)]
pub struct ServeCommand {
    /// The build to run first and on every change (always `html`).
    pub build: BuildCommand,
    /// `--port` (default 8000).
    pub port: u16,
}

/// `guidedog clean [PROJECT]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanCommand {
    /// The project folder.
    pub dir: PathBuf,
}

/// `guidedog quickstart [DIR]`: the answers given as options; a terminal asks for the
/// rest unless `quiet`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuickstartCommand {
    /// Where to create the project.
    pub dir: PathBuf,
    /// `-q`: ask nothing.
    pub quiet: bool,
    /// `-p`.
    pub project: String,
    /// `-a`.
    pub author: String,
    /// `-v`: the short version; the release when empty.
    pub version: String,
    /// `-r`.
    pub release: String,
    /// `-l`; empty for `en` (or the answer).
    pub language: String,
    /// `--suffix`; empty for `.rst`.
    pub suffix: String,
    /// `--sep` (`Some(true)`) or `--no-sep` (`Some(false)`); `None` asks.
    pub separate: Option<bool>,
}

/// Where `migrate` writes conf.toml.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MigrateOutput {
    /// Beside the conf.py.
    #[default]
    Beside,
    /// `-o FILE`.
    File(PathBuf),
    /// `-o -`: standard output.
    Stdout,
}

/// `guidedog migrate [conf.py|DIR]`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MigrateCommand {
    /// The conf.py, or the folder holding it.
    pub input: PathBuf,
    /// `-o`.
    pub output: MigrateOutput,
    /// `--force`: replace an existing conf.toml.
    pub force: bool,
}

/// The two `intl` commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntlAction {
    /// Create or merge the catalogs from the templates.
    Update,
    /// Count what is translated.
    Stat,
}

/// `guidedog intl update|stat [PROJECT]`, with sphinx-intl's options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntlCommand {
    /// update or stat.
    pub action: IntlAction,
    /// The project folder.
    pub dir: PathBuf,
    /// `-c` and `-D`.
    pub load: LoadOptions,
    /// `-l`, repeatable.
    pub languages: Vec<String>,
    /// `-p`: the templates; `None` for `_build/gettext`.
    pub pot_dir: Option<PathBuf>,
    /// `-d`: the catalogs; `None` for the first of `locale_dirs`.
    pub locale_dir: Option<PathBuf>,
    /// `-w` (default 76).
    pub line_width: usize,
    /// Keep messages the templates no longer have (`--no-obsolete` clears it).
    pub obsolete: bool,
}

/// What `guidedog plugin` does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginAction {
    /// `check PATH`.
    Check {
        /// The program.
        program: String,
    },
    /// `register ID PATH`.
    Register {
        /// The ID the program must have.
        id: String,
        /// The program.
        program: String,
    },
    /// `list`.
    List,
    /// `inspect ID`.
    Inspect {
        /// The registered ID.
        id: String,
    },
    /// `unregister ID`.
    Unregister {
        /// The registered ID.
        id: String,
    },
    /// `run ID -- ARGS`.
    Run {
        /// The registered ID.
        id: String,
        /// What follows `--`.
        args: Vec<String>,
    },
}

/// `guidedog plugin ...`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginCommand {
    /// The subcommand.
    pub action: PluginAction,
    /// `--project`: the project the checks use (default: the current folder).
    pub project: Option<PathBuf>,
    /// `--config`: the plugin's configuration, relative to the project.
    pub config: Option<PathBuf>,
    /// `--timeout`, in seconds (default 60).
    pub timeout: Option<u32>,
    /// `--yes`: register without asking.
    pub yes: bool,
}

/// `guidedog publish --publisher=ID --config=FILE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishCommand {
    /// `--publisher`.
    pub publisher: String,
    /// `--config`.
    pub config: String,
    /// `--dry-run`: build and plan, publish nothing.
    pub dry_run: bool,
    /// `--wait`, in seconds; `None` for the publisher's default.
    pub wait: Option<u32>,
    /// `--timeout`, in seconds; `None` for 300.
    pub timeout: Option<u32>,
    /// `--verbose`: list every change of the plan.
    pub verbose: bool,
}
