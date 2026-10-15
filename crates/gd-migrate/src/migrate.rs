//! `guidedog migrate`: the options, the result, and the one-call path that finds the
//! input, converts it, checks the result loads, and writes it.

use std::fmt;
use std::path::{Path, PathBuf};

use gd_conf::CONFIG_FILE;
use gd_host::Problem;
use gd_report::{Report, Severity};

use crate::convert::{check_conf_toml, convert_binding, finish};
use crate::extensions::harvest_extensions;
use crate::legacy::convert_guidedog_toml_in;
use crate::read::read_conf_py;

/// The largest conf.py or guidedog.toml read (1 MiB).
pub const MAX_INPUT_BYTES: usize = 1 << 20;

/// Where [`migrate`] writes conf.toml.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Destination {
    /// `conf.toml` beside conf.py (or in a guidedog.toml's first source root).
    #[default]
    Beside,
    /// This file.
    File(PathBuf),
    /// Nowhere: the converted text is only returned (`-o -`).
    Nowhere,
}

/// What to migrate and how. Fill it with struct-update syntax:
/// `MigrateOptions { force: true, ..Default::default() }`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrateOptions {
    /// conf.py, guidedog.toml, or a folder holding one (directly or in `source/`);
    /// empty means the working folder.
    pub input: PathBuf,
    /// Where conf.toml goes.
    pub output: Destination,
    /// Replace an existing conf.toml.
    pub force: bool,
    /// What `datetime`'s current year evaluates to; `None` takes the system clock's.
    pub year: Option<i64>,
}

/// A converted configuration: the conf.toml text, notes on what was not converted as
/// written, and counts for the summary line.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Conversion {
    /// The conf.toml text.
    pub toml: String,
    /// Warnings (settings left out that may matter) and notes, with source excerpts.
    pub notes: Vec<Report>,
    /// How many settings conf.toml sets.
    pub converted: usize,
    /// How many of the notes need attention (the warnings).
    pub attention: usize,
}

/// What [`migrate`] did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Migrated {
    /// The file that was converted.
    pub input: PathBuf,
    /// The conf.toml written, or `None` for [`Destination::Nowhere`].
    pub output: Option<PathBuf>,
    /// The conversion itself.
    pub conversion: Conversion,
}

/// Why [`migrate`] wrote nothing, with the notes gathered before it stopped (so the
/// command can still show them).
#[derive(Debug, Clone, PartialEq)]
pub struct MigrateFailure {
    /// The failure.
    pub problem: Problem,
    /// Notes on the conversion, when it got that far.
    pub notes: Vec<Report>,
}

impl fmt::Display for MigrateFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.problem.fmt(f)
    }
}

impl std::error::Error for MigrateFailure {}

impl From<Problem> for Box<MigrateFailure> {
    fn from(problem: Problem) -> Box<MigrateFailure> {
        Box::new(MigrateFailure { problem, notes: Vec::new() })
    }
}

/// Converts the Sphinx conf.py (or the guidedog.toml of an earlier `guidedog init`) that
/// `options.input` names into conf.toml without running Python: literal assignments
/// become settings, and what Python would have computed is explained in the notes. The
/// result is loaded as a build would before it is written, so a migrated conf.toml
/// always builds.
///
/// Fails, writing nothing, with `migrate.missing` (no input found), the problem of
/// reading the input, `config.invalid` (a guidedog.toml that does not parse),
/// `migrate.internal` (the result does not load: a bug), `migrate.exists` (conf.toml is
/// already there and `force` is not set), or `host.io` when writing fails.
pub fn migrate(options: &MigrateOptions) -> Result<Migrated, Box<MigrateFailure>> {
    let given = if options.input.as_os_str().is_empty() { Path::new(".") } else { &options.input };
    let Some(input) = migration_input(given) else {
        return Err(Problem::new(
            "migrate.missing",
            "NOTHING TO MIGRATE",
            &given.display().to_string(),
            "No conf.py or guidedog.toml was found here or in a source/ folder.",
            "Pass the path of the conf.py to convert.",
            2,
        )
        .into());
    };
    let shown = input.display().to_string();
    let text = gd_host::read_source(&shown, MAX_INPUT_BYTES)?.text;
    let (conversion, dir) = if input.file_name().is_some_and(|n| n == "guidedog.toml") {
        convert_guidedog_toml_in(&text, &input)?
    } else {
        let dir = input.parent().map(Path::to_path_buf).unwrap_or_default();
        (convert_conf_py_in(&text, &input, options.year), dir)
    };
    let fail = |problem: Problem, conversion: Conversion| {
        Box::new(MigrateFailure { problem, notes: conversion.notes })
    };
    if let Err(p) = check_conf_toml(&conversion.toml) {
        return Err(fail(p, conversion));
    }
    let output = match &options.output {
        Destination::Nowhere => return Ok(Migrated { input, output: None, conversion }),
        Destination::File(path) => path.clone(),
        Destination::Beside => dir.join(CONFIG_FILE),
    };
    if output.exists() && !options.force {
        let p = Problem::new(
            "migrate.exists",
            "CONF.TOML ALREADY EXISTS",
            &output.display().to_string(),
            "Migrating would replace a conf.toml that is already there.",
            "Pass --force to replace it, or -o FILE to write somewhere else.",
            2,
        );
        return Err(fail(p, conversion));
    }
    if let Err(e) = std::fs::write(&output, &conversion.toml) {
        return Err(fail(gd_host::io_problem(&output, &e), conversion));
    }
    Ok(Migrated { input, output: Some(output), conversion })
}

/// The file to migrate: `path` itself, or conf.py, source/conf.py or guidedog.toml in it.
fn migration_input(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    ["conf.py", "source/conf.py", "guidedog.toml"]
        .iter()
        .map(|candidate| path.join(candidate))
        .find(|full| full.is_file())
}

/// Converts the text of a conf.py; `path` names it in notes and in conf.toml's header,
/// and its folder is where the project's own extensions are looked for. `year` is what
/// `datetime`'s current year evaluates to (`None`: the system clock's). Never fails:
/// what cannot be converted is explained in the notes and kept as comments.
pub fn convert_conf_py(text: &str, path: &Path, year: Option<i64>) -> Conversion {
    convert_conf_py_in(text, path, year)
}

fn convert_conf_py_in(text: &str, path: &Path, year: Option<i64>) -> Conversion {
    let year = year.unwrap_or_else(|| gd_clock::now().utc().date.year);
    let mut m = Migration::new(text, path);
    let c = read_conf_py(&mut m, text, year);
    for b in &c.bindings {
        convert_binding(&mut m, &c.scope, b);
    }
    harvest_extensions(&mut m, path.parent().unwrap_or(Path::new("")));
    finish(m)
}

/// The conversion in progress: conf.toml's three parts and what is reported.
pub(crate) struct Migration<'a> {
    pub(crate) path: PathBuf,
    /// The input, for excerpts.
    pub(crate) source: &'a str,
    /// Top-level keys.
    pub(crate) plain: String,
    /// `[tables]` and `[[arrays of tables]]`, after the keys.
    pub(crate) tables: String,
    /// Settings kept for reference only.
    pub(crate) comments: String,
    /// Sphinx-only settings among the comments.
    pub(crate) sphinx_only: Vec<String>,
    /// Extensions conf.py lists that are not built in.
    pub(crate) left_out: Vec<String>,
    pub(crate) notes: Vec<Report>,
    pub(crate) converted: usize,
    pub(crate) attention: usize,
}

/// A note's severity, code, title, message and hint.
pub(crate) struct Note<'t> {
    pub(crate) severity: Severity,
    pub(crate) code: &'t str,
    pub(crate) title: &'t str,
    pub(crate) message: String,
    pub(crate) hint: &'t str,
}

impl<'a> Migration<'a> {
    pub(crate) fn new(source: &'a str, path: &Path) -> Migration<'a> {
        Migration {
            path: path.to_path_buf(),
            source,
            plain: String::new(),
            tables: String::new(),
            comments: String::new(),
            sphinx_only: Vec::new(),
            left_out: Vec::new(),
            notes: Vec::new(),
            converted: 0,
            attention: 0,
        }
    }

    /// Adds a note at 1-based `line` of the input (0: the whole file); a warning counts
    /// as needing attention.
    pub(crate) fn note(&mut self, line: usize, note: Note<'_>) {
        let mut r = Report {
            code: note.code.to_string(),
            severity: note.severity,
            title: note.title.to_string(),
            message: note.message,
            hints: vec![note.hint.to_string()],
            path: self.path.display().to_string(),
            ..Report::default()
        };
        if line > 0 {
            gd_report::set_line_column(&mut r, self.source, line, 1);
        }
        if note.severity == Severity::Warning {
            self.attention += 1;
        }
        self.notes.push(r);
    }

    /// Adds a warning (see [`Migration::note`]).
    pub(crate) fn warn(
        &mut self,
        line: usize,
        code: &str,
        title: &str,
        message: String,
        hint: &str,
    ) {
        let severity = Severity::Warning;
        self.note(line, Note { severity, code, title, message, hint });
    }

    /// Adds a plain note (see [`Migration::note`]).
    pub(crate) fn tell(
        &mut self,
        line: usize,
        code: &str,
        title: &str,
        message: String,
        hint: &str,
    ) {
        let severity = Severity::Note;
        self.note(line, Note { severity, code, title, message, hint });
    }
}
