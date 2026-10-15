//! The discussion command line, which `dsc` and `dscc` run on any catalog and
//! `guidedog gds` on Guidedog's own. A [`Program`] says which: its name, version, how it
//! builds, and, for `guidedog gds`, the prefix and folder it is fixed to.
//!
//! ```no_run
//! use gd_discussion::command::{self, Program};
//! let args: Vec<String> = std::env::args().skip(1).collect();
//! let program = Program::default(); // dsc: builds with the installed guidedog
//! std::process::exit(command::run(&program, &args));
//! ```

mod display;
mod help;
mod options;
#[cfg(test)]
mod tests;

use crate::edit::git_author;
use crate::problem::usage;
use crate::{BuildOptions, Decision, NewDraft, Target};
use crate::{Builder, Catalog, Definition, GuidedogProgram, Init, Lock, Prefix, Problem, Result};
use display::Console;
use options::{Color, Options, split_labels, usage_problem};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

/// The version of `dsc` and `dscc`, which is Guidedog's.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What runs the command line: `dsc`, `dscc`, or `guidedog gds`.
#[derive(Debug, Clone)]
pub struct Program<'a> {
    /// The name as the user types it: `"dsc"`, `"dscc"`, `"guidedog gds"`.
    pub name: &'a str,
    /// What `--version` prints.
    pub version: &'a str,
    /// The option that names the builder's program, without dashes: `"guidedog"` or
    /// `"typst"`.
    pub tool: &'a str,
    /// Makes the builder, given the program that option names.
    pub make_builder: fn(Option<PathBuf>) -> Box<dyn Builder>,
    /// The only prefix it manages (`gds`), or `None` for any.
    pub prefix: Option<Prefix>,
    /// The catalog when `--dir` is not given, or `None` to find it.
    pub dir: Option<PathBuf>,
}

impl Default for Program<'_> {
    /// `dsc`: any catalog, built with the installed `guidedog`.
    fn default() -> Self {
        Program {
            name: "dsc",
            version: concat!("dsc ", env!("CARGO_PKG_VERSION"), "\n"),
            tool: "guidedog",
            make_builder: external_builder,
            prefix: None,
            dir: None,
        }
    }
}

/// The builder `dsc` uses: the installed `guidedog`, or the one `--guidedog` names.
pub fn external_builder(program: Option<PathBuf>) -> Box<dyn Builder> {
    Box::new(GuidedogProgram { program })
}

/// Runs the command line `args` (without the program name) on the terminal and returns
/// the exit status: 0 on success, and on failure the status the problem names, after
/// reporting it on standard error.
pub fn run(program: &Program<'_>, args: &[String]) -> i32 {
    let (stdout, stderr) = (std::io::stdout(), std::io::stderr());
    let terminals = (stdout.is_terminal(), stderr.is_terminal());
    let (mut out, mut err) = (stdout.lock(), stderr.lock());
    let status = run_with(program, args, &mut out, &mut err, terminals);
    let _ = out.flush();
    status
}

/// Runs the command line, writing to `out` and `err`; `terminals` says whether each is a
/// terminal, for `--color=auto`.
pub fn run_with(
    program: &Program<'_>,
    args: &[String],
    out: &mut dyn Write,
    err: &mut dyn Write,
    terminals: (bool, bool),
) -> i32 {
    let parsed = options::parse(program, args);
    let o = parsed.as_ref().ok().cloned().unwrap_or_default();
    let color = |tty: bool| match o.color {
        Color::Always => true,
        Color::Never => false,
        Color::Auto => gd_report::ansi::terminal_color(tty),
    };
    let mut console = Console {
        out,
        err,
        color_out: color(terminals.0),
        color_err: color(terminals.1),
        json: o.json_diagnostics,
        quiet: o.quiet,
    };
    match parsed.and_then(|mut o| run_command(program, &mut o, &mut console)) {
        Ok(status) => status,
        Err(p) => console.report(&p),
    }
}

fn run_command(program: &Program<'_>, o: &mut Options, console: &mut Console<'_>) -> Result<i32> {
    if o.version {
        console.print(program.version);
        return Ok(0);
    }
    if o.help || o.command.is_empty() {
        let text = help::help_text(program, console.color_out);
        console.print(&text);
        return Ok(0);
    }
    options::validate(program, o)?;
    if o.command == "init" {
        return init_catalog(program, o, console);
    }
    let (dir, prefix) = resolve_catalog(program, o)?;
    let mutating = ["new", "promote", "state", "index", "recover"].contains(&o.command.as_str())
        && !o.dry_run
        && !o.check_only;
    let lock = if mutating { Some(crate::lock_catalog(&dir, &prefix)?) } else { None };
    if o.command == "recover" {
        return recover(&dir, &prefix, lock.as_ref(), console);
    }
    let catalog = crate::load_catalog(&dir, &prefix)?;
    dispatch(program, o, &catalog, lock.as_ref(), console)
}

fn recover(
    dir: &Path,
    prefix: &Prefix,
    lock: Option<&Lock>,
    console: &mut Console<'_>,
) -> Result<i32> {
    match lock {
        Some(lock) => {
            crate::recover(lock)?;
            console.success("Recovery complete; original files restored");
        }
        None => {
            crate::check_recovery(dir, prefix)?;
            console.success("Recovery checked");
        }
    }
    Ok(0)
}

fn dispatch(
    program: &Program<'_>,
    o: &Options,
    c: &Catalog,
    lock: Option<&Lock>,
    console: &mut Console<'_>,
) -> Result<i32> {
    let plan = match o.command.as_str() {
        "list" => {
            check_state_filter(o, c)?;
            console.list(o, c, &catalog_title(c));
            return Ok(0);
        }
        "config" => {
            console.print(&crate::render_config(&catalog_definition(c)));
            return Ok(0);
        }
        "show" => {
            console.show(c, crate::resolve(c, &o.identifier)?, o.json_format);
            return Ok(0);
        }
        "check" => {
            crate::check_sources(c)?;
            crate::check_index(c)?;
            if o.render {
                return build_records(program, o, c, console);
            }
            let count = c.records.len();
            console.success(&format!("Checked {count} records; active metadata and index agree"));
            return Ok(0);
        }
        "build" => return build_records(program, o, c, console),
        "new" => crate::new_plan(c, &new_draft(o))?,
        "promote" => crate::promote_plan(c, &decision(o))?,
        "state" => crate::state_plan(c, &decision(o))?,
        _ => crate::index_plan(c)?,
    };
    if o.check_only {
        crate::check_index(c)?;
        console.success("Index is up to date");
        return Ok(0);
    }
    match lock {
        Some(lock) if !o.dry_run => {
            crate::apply(lock, &plan)?;
            console.success(&plan.summary);
            Ok(0)
        }
        _ => Ok(console.preview(&plan, o.json_format)),
    }
}

fn new_draft(o: &Options) -> NewDraft {
    NewDraft {
        title: o.identifier.clone(),
        slug: o.slug.clone(),
        author: o.author.clone(),
        labels: if o.labels.is_empty() { Vec::new() } else { split_labels(&o.labels) },
    }
}

fn decision(o: &Options) -> Decision {
    Decision {
        identifier: o.identifier.clone(),
        to: o.to.clone(),
        number: o.number,
        discussion: o.discussion.clone(),
        reason: o.reason.clone(),
        evidence: o.evidence.clone(),
    }
}

fn build_records(
    program: &Program<'_>,
    o: &Options,
    c: &Catalog,
    console: &mut Console<'_>,
) -> Result<i32> {
    let mut record = o.record.clone();
    if !o.identifier.is_empty() {
        if !record.is_empty() || o.number.is_some() {
            return Err(usage_problem(
                "Use one record selector",
                &o.identifier,
                "Choose a positional selector, --record, or --number.",
            ));
        }
        record = o.identifier.clone();
    }
    if let Some(n) = o.number {
        if !record.is_empty() {
            return Err(usage_problem(
                "Use one selector",
                &record,
                "Choose --record or --number, not both.",
            ));
        }
        record = n.to_string();
    }
    let target = match o.target.as_str() {
        "" | "both" => None,
        "html" => Some(Target::Html),
        "pdf" => Some(Target::Pdf),
        other => {
            return Err(Problem::new(
                c.prefix.code("target"),
                "Unsupported build target",
                other,
                "The available targets are pdf, html, and both.",
                "Use --target=html or --target=pdf.",
            )
            .exit(2)
            .suggest(gd_report::suggest(other, ["pdf", "html", "both"])));
        }
    };
    let builder = (program.make_builder)(o.tool.clone());
    let outputs = crate::build(c, &BuildOptions { target, record }, builder.as_ref())?;
    let shown: Vec<String> = outputs.iter().map(|p| p.display().to_string()).collect();
    console.success(&format!("Built {}", shown.join(", ")));
    Ok(0)
}

/// Refuses a `--state` the catalog's workflow does not have.
fn check_state_filter(o: &Options, c: &Catalog) -> Result<()> {
    if o.state.is_empty() || c.process.state(&o.state).is_some() {
        return Ok(());
    }
    let names: Vec<&str> = c.process.state_names().collect();
    Err(usage(
        "Unknown discussion state",
        &o.state,
        format!("Pass --state one of: {}.", names.join(", ")),
    )
    .suggest(gd_report::suggest(&o.state, names)))
}

/// The heading of a list: the catalog's name, in capitals.
fn catalog_title(c: &Catalog) -> String {
    if c.prefix.as_str() == crate::DEFAULT_PREFIX {
        return "GUIDEDOG DISCUSSIONS".to_string();
    }
    match crate::definition(&c.dir) {
        Ok(Some(d)) => d.name.to_uppercase(),
        _ => format!("{} DISCUSSIONS", c.prefix.upper()),
    }
}

/// The catalog's configuration as `config` prints it: what its configuration file says,
/// or what the catalog does without one.
fn catalog_definition(c: &Catalog) -> Definition {
    let mut d = crate::definition(&c.dir).ok().flatten().unwrap_or_else(|| Definition {
        prefix: c.prefix.clone(),
        name: if c.prefix.as_str() == crate::DEFAULT_PREFIX {
            "Guidedog Discussions".to_string()
        } else {
            format!("{} Discussions", c.prefix.upper())
        },
        ..Definition::default()
    });
    d.process = c.process.clone();
    d.labels = c.vocabulary().map_or_else(Vec::new, |v| v.iter().map(|l| l.to_string()).collect());
    d
}

/// Settles the catalog's folder and prefix: `--dir`, or the catalog the working folder
/// is in; `--prefix`, or the catalog's configuration, or its template's fields.
fn resolve_catalog(program: &Program<'_>, o: &mut Options) -> Result<(PathBuf, Prefix)> {
    if let Some(prefix) = &program.prefix {
        let dir = o.dir.clone().or_else(|| program.dir.clone()).unwrap_or_default();
        return Ok((dir, prefix.clone()));
    }
    let dir = match &o.dir {
        Some(dir) => dir.clone(),
        None => crate::discover(Path::new("."))?.map_or_else(|| PathBuf::from("."), |(d, _)| d),
    };
    if let Some(d) = crate::definition(&dir)? {
        if !o.prefix.is_empty() && o.prefix != d.prefix.as_str() {
            return Err(Problem::new(
                "discussion.prefix",
                "The prefix differs from the catalog's",
                dir.display().to_string(),
                format!("The catalog's configuration names prefix {}.", d.prefix),
                format!("Pass --prefix={}, or leave --prefix out.", d.prefix),
            )
            .exit(2));
        }
        if o.author.is_empty() {
            o.author = d.author;
        }
        return Ok((dir, d.prefix));
    }
    let text = if o.prefix.is_empty() { template_prefix(&dir) } else { o.prefix.clone() };
    if let Ok(prefix) = Prefix::new(&text) {
        return Ok((dir, prefix));
    }
    Err(Problem::new(
        "discussion.catalog",
        "No discussion catalog here",
        dir.display().to_string(),
        "This folder has no discussion.toml and no record template to name its prefix.",
        format!(
            "Run {0} in a catalog, pass --dir=PATH and --prefix=NAME, or create one with \
            {0} init DIR --prefix=NAME.",
            program.name
        ),
    )
    .exit(2))
}

/// The prefix of the number field of the catalog's record template, as in
/// `:gds-number:` or `#let gds-number`, or "".
pub(crate) fn template_prefix(dir: &Path) -> String {
    for name in ["template/rfc-template.rst", "template/rfc-template.typ"] {
        let Ok(text) = std::fs::read_to_string(dir.join(name)) else { continue };
        for line in text.lines() {
            let line = line.trim();
            let line = line.strip_prefix(':').unwrap_or(line);
            let line = line.strip_prefix("#let ").unwrap_or(line);
            if let Some((prefix, field)) = line.split_once('-')
                && field.starts_with("number")
                && crate::is_valid_prefix(prefix)
            {
                return prefix.to_string();
            }
        }
    }
    String::new()
}

fn init_catalog(program: &Program<'_>, o: &Options, console: &mut Console<'_>) -> Result<i32> {
    let dir = match (&o.dir, o.identifier.is_empty()) {
        (Some(_), false) => {
            return Err(usage_problem(
                "Name the folder once",
                &o.identifier,
                "Give the new catalog's folder as DIR or as --dir, not both.",
            ));
        }
        (None, true) => {
            return Err(usage(
                "Name the new catalog's folder",
                "init",
                format!("Write {} init DIR --prefix=NAME.", program.name),
            ));
        }
        (Some(dir), true) => dir.clone(),
        (None, false) => PathBuf::from(&o.identifier),
    };
    let init = init_settings(o, dir)?;
    if o.dry_run {
        let plan = crate::init_plan(&init)?;
        return Ok(console.preview(&plan, o.json_format));
    }
    let plan = crate::init_plan(&init)?;
    crate::initialize(&init)?;
    console.success(&plan.summary);
    Ok(0)
}

/// The new catalog's settings: the command line's, then `--config`'s, then defaults.
fn init_settings(o: &Options, dir: PathBuf) -> Result<Init> {
    let mut init = Init {
        dir,
        name: o.name.clone(),
        author: o.author.clone(),
        labels: (!o.labels.is_empty()).then(|| split_labels(&o.labels)),
        ..Init::default()
    };
    let mut prefix = o.prefix.clone();
    if !o.config.is_empty() {
        let text = read_file(&o.config, "--config")?;
        let d = crate::read_config(&o.config, &text)?;
        init.process = d.process;
        if prefix.is_empty() {
            prefix = d.prefix.to_string();
        }
        if init.name.is_empty() {
            init.name = d.name;
        }
        if init.author.is_empty() {
            init.author = d.author;
        }
        if init.labels.is_none() && !d.labels.is_empty() {
            init.labels = Some(d.labels);
        }
    }
    if prefix.is_empty() {
        return Err(usage(
            "Choose the catalog's prefix",
            "init",
            "Pass --prefix with 2 to 16 lowercase letters, such as eds; records use \
             :eds-number:.",
        ));
    }
    init.prefix = Prefix::new(&prefix)?;
    if init.name.is_empty() {
        init.name = format!("{} Discussions", init.prefix.upper());
    }
    if init.author.is_empty() {
        init.author = git_author(Path::new("."));
    }
    if init.author.is_empty() {
        return Err(usage(
            "An author is needed",
            "init",
            "Pass --author='Your name'; it is the catalog's default author.",
        ));
    }
    if !o.process.is_empty() {
        init.draft = read_file(&o.process, "--process")?;
    }
    if !o.template.is_empty() {
        init.template = read_file(&o.template, "--template")?;
    }
    Ok(init)
}

fn read_file(path: &str, option: &str) -> Result<String> {
    std::fs::read_to_string(path).map_err(|e| {
        Problem::new(
            "discussion.io",
            "Cannot read this file",
            path,
            e.to_string(),
            format!("Check the path given to {option}."),
        )
        .exit(4)
    })
}
