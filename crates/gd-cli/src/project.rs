//! The project commands: build, serve, clean, quickstart, migrate, and intl. They read
//! options and report; the project builder is reached through `ops`.

use std::sync::Arc;

use gd_env::{BUILD_BUDGET, BuildOptions, BuildResult};
use gd_host::{Problem, RefusalChoice};
use gd_plugins::Registry;
use gd_report::Severity;

use crate::command::{BuildCommand, CleanCommand, IntlAction, IntlCommand, MemoryMode};
use crate::command::{MigrateCommand, MigrateOutput, QuickstartCommand, ServeCommand};
use crate::console::Console;
use crate::memory::{Memory, Start, ask_refusal, low_memory_report, start_question};
use crate::memory::{stopped_problem, terminal_memory};
use crate::ops::{self, IntlOutcome, Project};
use crate::summary::{build_summary, output_note};

/// `guidedog build`: loads the project, builds it, and reports.
pub(crate) fn build(console: &mut Console<'_>, command: &BuildCommand, linked: &Registry) -> i32 {
    gd_host::catch_interrupts();
    let memory = terminal_memory(console.color_err);
    match load(console, command, linked) {
        Ok(project) => build_loaded(console, &project, command, &memory),
        Err(status) => status,
    }
}

/// Loads the project `command` names, with `--typst` applied.
/// A failure is reported, and its status returned.
fn load(
    console: &mut Console<'_>,
    command: &BuildCommand,
    linked: &Registry,
) -> Result<Project, i32> {
    let mut project = match ops::load(&command.dir, &command.load, linked) {
        Ok(project) => project,
        Err(problem) => return Err(console.problem(&problem)),
    };
    // The build reports the project's warnings with its own (they count for -W).
    if let Some(typst) = &command.typst {
        project.config.pdf.typst = typst.clone();
    }
    Ok(project)
}

/// Builds a loaded project and reports what happened; returns the exit status.
fn build_loaded(
    console: &mut Console<'_>,
    project: &Project,
    command: &BuildCommand,
    memory: &Memory,
) -> i32 {
    let mut options = command.options.clone();
    if options.now.is_none() {
        options.now = source_date_epoch();
    }
    if let Err(problem) = budget_options(console, command.memory, memory, &mut options) {
        return console.problem(&problem);
    }
    let what = format!("building {} for {}", command.kind.name(), project.config.project);
    console.heading(&what);
    let result = ops::build(project, command.kind, &options);
    report_build(console, command.verbose, &result)
}

/// The moment `SOURCE_DATE_EPOCH` (seconds since 1970, UTC) fixes for `|today|` and the
/// page dates, as reproducible builds set it; `None` when it is unset or not a whole
/// number of seconds, and the build uses the clock.
fn source_date_epoch() -> Option<std::time::SystemTime> {
    let seconds: u64 = std::env::var("SOURCE_DATE_EPOCH").ok()?.trim().parse().ok()?;
    std::time::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(seconds))
}

/// Sets the build's budget and what happens at it. A budget larger than the free memory
/// is asked about at a terminal, and warned about elsewhere; the build asks again when
/// it reaches its budget, unless `--memory=ram` or nobody can answer. Fails when the
/// person chose not to build.
fn budget_options(
    console: &mut Console<'_>,
    mode: MemoryMode,
    memory: &Memory,
    options: &mut BuildOptions,
) -> Result<(), Problem> {
    let ask = console.interactive() && mode == MemoryMode::Ask;
    let mut budget = options.budget.unwrap_or(BUILD_BUDGET);
    let mut state = memory.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(raised) = state.raised {
        budget = budget.max(raised);
    }
    if ask {
        let shared = Arc::clone(memory);
        options.on_budget = Some(Arc::new(move |refusal: &gd_host::Refusal| {
            let mut state = shared.lock().unwrap_or_else(|p| p.into_inner());
            state.available = gd_host::available_memory();
            let choice = ask_refusal(&mut state, refusal);
            if let RefusalChoice::Raise(bytes) = choice {
                state.raised = Some(bytes);
            }
            choice
        }));
    }
    state.available = gd_host::available_memory();
    if let Some(free) = state.available.filter(|&free| budget > free) {
        if !ask {
            drop(state);
            console.report(&low_memory_report(budget, free, false), "");
        } else {
            match start_question(&mut state, budget) {
                Start::Continue => {}
                Start::Lower(lower) => budget = lower,
                Start::Stop => return Err(stopped_problem(free)),
            }
        }
    }
    options.budget = Some(budget);
    Ok(())
}

/// Shows a build's reports (notes only with `-v`, as Sphinx shows Docutils' info
/// messages), its summary, the problem that failed it, and the verdict.
fn report_build(console: &mut Console<'_>, verbose: u32, result: &BuildResult) -> i32 {
    let (mut errors, mut warnings, mut notes) = (0, 0, 0);
    for report in &result.reports {
        match report.severity {
            Severity::Error => errors += 1,
            Severity::Warning => warnings += 1,
            Severity::Note if verbose == 0 => {
                notes += 1;
                continue;
            }
            Severity::Note => {}
        }
        console.report(report, "");
    }
    let (read, write) = build_summary(result);
    console.step("read", &read);
    if let Some(write) = write {
        console.step("write", &write);
    }
    if notes > 0 {
        console.step("notes", &format!("{notes} not shown; -v shows them"));
    }
    if let Some(problem) = &result.problem {
        console.problem(problem);
        errors += 1;
    }
    if !console.quiet && !console.json {
        let verdict = console.verdict(result.exit == 0);
        let note = output_note(result);
        let line = format!("{verdict}, {errors} error(s), {warnings} warning(s).{note}\n");
        console.print(&line);
    }
    result.exit
}

/// `guidedog serve`: builds the site, then serves it and rebuilds on change.
pub(crate) fn serve(console: &mut Console<'_>, command: &ServeCommand, linked: &Registry) -> i32 {
    let memory = terminal_memory(console.color_err);
    let project = match load(console, &command.build, linked) {
        Ok(project) => project,
        Err(status) => return status,
    };
    let status = build_loaded(console, &project, &command.build, &memory);
    if status != 0 {
        return status;
    }
    let mut host = Preview { console, command, linked, memory: &memory };
    match ops::serve(&project, command.port, &mut host) {
        Ok(()) => 0,
        Err(problem) => host.console.problem(&problem),
    }
}

/// The preview's side of `serve`: says where the site is, and rebuilds it. A failed
/// load or build keeps the previous site; the server keeps serving it.
struct Preview<'c, 'o> {
    console: &'c mut Console<'o>,
    command: &'c ServeCommand,
    linked: &'c Registry,
    memory: &'c Memory,
}

impl ops::ServeHost for Preview<'_, '_> {
    fn serving(&mut self, site: &std::path::Path, url: &str) {
        let site = gd_host::display_path(&site.to_string_lossy());
        self.console.print(&format!("  Serving {site} at {url} (Ctrl+C to stop)\n"));
        let _ = self.console.out.flush();
    }

    fn rebuild(&mut self) -> Option<Project> {
        self.console.print("  Sources changed; rebuilding...\n");
        let next = load(self.console, &self.command.build, self.linked).ok()?;
        let built = build_loaded(self.console, &next, &self.command.build, self.memory) == 0;
        if built {
            self.console.print("  Rebuilt.\n");
        }
        let _ = self.console.out.flush();
        built.then_some(next)
    }
}

/// `guidedog clean`: removes the contents of the build folder.
pub(crate) fn clean(console: &mut Console<'_>, command: &CleanCommand, linked: &Registry) -> i32 {
    let project = match ops::load(&command.dir, &gd_conf::LoadOptions::default(), linked) {
        Ok(project) => project,
        Err(problem) => return console.problem(&problem),
    };
    if let Err(problem) = ops::clean(&project) {
        return console.problem(&problem);
    }
    let folder = gd_host::display_path(&project.config.build_dir.to_string_lossy());
    console.success(&format!("Removed the contents of {folder}"));
    0
}

/// `guidedog intl update|stat`.
pub(crate) fn intl(console: &mut Console<'_>, command: &IntlCommand, linked: &Registry) -> i32 {
    let project = match ops::load(&command.dir, &command.load, linked) {
        Ok(project) => project,
        Err(problem) => return console.problem(&problem),
    };
    let (files, problem) = ops::intl(&project, command);
    for file in &files {
        let path = gd_host::display_path(&file.path.to_string_lossy());
        match (&file.outcome, command.action) {
            (IntlOutcome::Stats(stats), _) if !console.quiet => {
                console.print(&format!("{path}: {stats}\n"));
            }
            (IntlOutcome::Created, IntlAction::Update) => console.step("create", &path),
            (IntlOutcome::Updated, IntlAction::Update) => console.step("update", &path),
            (IntlOutcome::Unchanged, IntlAction::Update) => console.step("same", &path),
            _ => {}
        }
    }
    match problem {
        Some(problem) => console.problem(&problem),
        None => 0,
    }
}

/// `guidedog migrate`: conf.py to conf.toml, without Python.
pub(crate) fn migrate(console: &mut Console<'_>, command: &MigrateCommand) -> i32 {
    let output = match &command.output {
        MigrateOutput::Beside => gd_migrate::Destination::Beside,
        MigrateOutput::File(path) => gd_migrate::Destination::File(path.clone()),
        MigrateOutput::Stdout => gd_migrate::Destination::Nowhere,
    };
    let options = gd_migrate::MigrateOptions {
        input: command.input.clone(),
        output,
        force: command.force,
        year: None,
    };
    let migrated = match gd_migrate::migrate(&options) {
        Ok(migrated) => migrated,
        Err(failure) => {
            for note in &failure.notes {
                console.report(note, "");
            }
            return console.problem(&failure.problem);
        }
    };
    let conversion = &migrated.conversion;
    for note in &conversion.notes {
        console.report(note, "");
    }
    let Some(written) = &migrated.output else {
        console.print(&conversion.toml);
        return 0;
    };
    let attention = match conversion.attention {
        0 => String::new(),
        n => format!("; {n} need attention (see above)"),
    };
    let message = format!(
        "Wrote {} from {}: {} setting(s) converted{attention}.",
        written.display(),
        migrated.input.display(),
        conversion.converted
    );
    console.success(&message);
    0
}

/// `guidedog quickstart`: asks for what the options did not give (at a terminal,
/// without `-q`), then creates the project.
pub(crate) fn quickstart(console: &mut Console<'_>, command: &QuickstartCommand) -> i32 {
    let mut q = command.clone();
    if !q.quiet && console.terminals.stdin {
        ask_quickstart(console, &mut q);
    }
    if q.project.is_empty() || q.author.is_empty() {
        return console.problem(&crate::args::usage(
            "PROJECT NAME AND AUTHOR ARE REQUIRED",
            "guidedog quickstart needs the project's name and its author.",
            "Write guidedog quickstart -q -p \"My Project\" -a \"Your Name\", or answer the \
             prompts without -q.",
        ));
    }
    let date = gd_discussion::today();
    let separate = q.separate.unwrap_or(false);
    let options = gd_migrate::QuickstartOptions {
        dir: q.dir.clone(),
        project: q.project.clone(),
        author: q.author.clone(),
        version: if q.version.is_empty() { q.release.clone() } else { q.version.clone() },
        release: q.release.clone(),
        language: q.language.clone(),
        suffix: q.suffix.clone(),
        separate,
        year: date.get(..4).and_then(|y| y.parse().ok()),
        date: Some(date.clone()),
        ..gd_migrate::QuickstartOptions::default()
    };
    let created = match gd_migrate::quickstart(&options) {
        Ok(created) => created,
        Err(problem) => return console.problem(&problem),
    };
    if !console.quiet {
        quickstart_done(console, &created, &q, separate);
    }
    0
}

fn quickstart_done(
    console: &mut Console<'_>,
    created: &[std::path::PathBuf],
    q: &QuickstartCommand,
    separate: bool,
) {
    console.print("\n");
    for file in created {
        console.step("created", &gd_host::display_path(&file.to_string_lossy()));
    }
    let build = if separate { "build" } else { "_build" };
    let dir = gd_host::display_path(&q.dir.to_string_lossy());
    console.print(&format!(
        "\nFinished: a documentation project is ready in {dir}.\nWrite your documents next to \
         index.rst, then build them with\n\n  guidedog build html   -> {build}/html\n  \
         guidedog build pdf    -> {build}/pdf\n\n"
    ));
}

/// Asks, as sphinx-quickstart does, for what the options left out.
fn ask_quickstart(console: &mut Console<'_>, q: &mut QuickstartCommand) {
    console.print("Welcome to the Guidedog quickstart.\n\n");
    console.print("Please enter values for the following settings (just press Enter to\n");
    console.print("accept a default value, if one is given in brackets).\n\n");
    if q.separate.is_none() {
        let answer = ask(console, "Separate source and build directories (y/n)", "n");
        q.separate = Some(answer.to_lowercase().starts_with('y'));
    }
    let questions: [(&str, &str, &mut String); 4] = [
        ("Project name", "", &mut q.project),
        ("Author name(s)", "", &mut q.author),
        ("Project release", "", &mut q.release),
        ("Project language", "en", &mut q.language),
    ];
    for (question, fallback, value) in questions {
        if value.is_empty() {
            *value = ask(console, question, fallback);
        }
    }
}

fn ask(console: &mut Console<'_>, question: &str, fallback: &str) -> String {
    if fallback.is_empty() {
        console.print(&format!("> {question}: "));
    } else {
        console.print(&format!("> {question} [{fallback}]: "));
    }
    let _ = console.out.flush();
    match console.read_line() {
        Some(line) if !line.is_empty() => line,
        _ => fallback.to_string(),
    }
}
