//! The `guidedog dsc` commands, which the discussion plugin adds to guidedog's command
//! line: `dsc init`, `dsc new`, `dsc promote`, `dsc state`, and the rest, on the catalog
//! the working folder is in (or `--dir`). The host lends a [`CommandContext`]: its
//! streams, and its build of a Guidedog project, which publishes a catalog.

mod declare;
mod display;
mod host;
mod options;
#[cfg(test)]
mod tests;

pub(crate) use declare::commands;

use crate::edit::git_author;
use crate::problem::usage;
use crate::{BuildOptions, Decision, NewDraft, Target};
use crate::{Catalog, Definition, Init, Lock, Prefix, Problem, Result};
use display::Console;
use gd_plugins::CommandContext;
use options::{Color, Options, split_labels, usage_problem};
use std::path::{Path, PathBuf};

/// The command as people type it, for hints.
pub(crate) const NAME: &str = "guidedog dsc";
/// The option naming the Typst compiler of an older Typst catalog, without dashes.
pub(crate) const TOOL: &str = "typst";

/// Runs `guidedog dsc WORD ARGS` (`word` is `init`, `new`, ...; `args` the words after
/// it) with what `ctx` offers, and returns the exit status: 0 on success, and on
/// failure the status the problem names, after reporting it on the error stream.
pub(crate) fn run(ctx: &mut CommandContext<'_>, word: &str, args: &[String]) -> i32 {
    let mut words = Vec::with_capacity(args.len() + 1);
    words.push(word.to_string());
    words.extend_from_slice(args);
    let parsed = options::parse(&words);
    let o = parsed.as_ref().ok().cloned().unwrap_or_default();
    let color = match o.color {
        Color::Always => true,
        Color::Never => false,
        Color::Auto => ctx.color,
    };
    let (json, quiet) = (o.json_diagnostics || ctx.json, o.quiet || ctx.quiet);
    let mut console = Console { ctx, color, json, quiet };
    match parsed.and_then(|mut o| run_command(&mut o, &mut console)) {
        Ok(status) => status,
        Err(p) => console.report(&p),
    }
}

fn run_command(o: &mut Options, console: &mut Console<'_, '_>) -> Result<i32> {
    options::validate(o)?;
    if o.command == "init" {
        return init_catalog(o, console);
    }
    let (dir, prefix) = resolve_catalog(o)?;
    let mutating = ["new", "promote", "state", "index", "recover"].contains(&o.command.as_str())
        && !o.dry_run
        && !o.check_only;
    let lock = if mutating { Some(crate::lock_catalog(&dir, &prefix)?) } else { None };
    if o.command == "recover" {
        return recover(&dir, &prefix, lock.as_ref(), console);
    }
    let catalog = crate::load_catalog(&dir, &prefix)?;
    dispatch(o, &catalog, lock.as_ref(), console)
}

fn recover(
    dir: &Path,
    prefix: &Prefix,
    lock: Option<&Lock>,
    console: &mut Console<'_, '_>,
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
    o: &Options,
    c: &Catalog,
    lock: Option<&Lock>,
    console: &mut Console<'_, '_>,
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
                return build_records(o, c, console);
            }
            let count = c.records.len();
            console.success(&format!("Checked {count} records; active metadata and index agree"));
            return Ok(0);
        }
        "build" => return build_records(o, c, console),
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

fn build_records(o: &Options, c: &Catalog, console: &mut Console<'_, '_>) -> Result<i32> {
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
    let options = BuildOptions { target, record, typst: o.tool.clone() };
    let mut builder = host::HostBuilder { ctx: &mut *console.ctx, typst: o.tool.clone() };
    let outputs = crate::build(c, &options, &mut builder)?;
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
fn resolve_catalog(o: &mut Options) -> Result<(PathBuf, Prefix)> {
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
            "Run {NAME} in a catalog, pass --dir=PATH and --prefix=NAME, or create one with \
            {NAME} init DIR --prefix=NAME."
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

fn init_catalog(o: &Options, console: &mut Console<'_, '_>) -> Result<i32> {
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
                format!("Write {NAME} init DIR --prefix=NAME."),
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
