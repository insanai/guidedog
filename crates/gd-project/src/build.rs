//! One build: prepare (output folder, plugins, documents, translations, inventories),
//! read, resolve, write with the builder, then publish or keep the previous output.

use std::path::PathBuf;
use std::time::SystemTime;

use gd_conf::{Config, Digest};
use gd_env::{
    BuildOptions, BuildResult, BuildStage, BuilderKind, CachedFile, DocInfo, Environment, Jobs,
    ObjectStore, PluginRun, PluginStart, SourceFile,
};
use gd_host::{Budget, Problem};
use gd_intl::Intl;
use gd_outputs::{Generation, GenerationOptions};
use gd_read::{ReadContext, ReadingTime};
use gd_report::Report;
use gd_resolve::{BuildView, Dates, Intersphinx, IntersphinxOptions, Tables};
use guidedoc::{Limits, Policy};

use crate::duration;
use crate::finish::finish;
use crate::limits::{build_limits, trusted_policy, untrusted_policy};
use crate::project::Project;
use crate::write::write_output;

/// Builds `project` with the builder `kind`: reads the documents whose sources changed
/// (all of them with `fresh`), resolves the project, writes the outputs whose inputs
/// changed (all of them with `all`), and publishes them whole.
///
/// The project is never changed: the build works on its own copy of the
/// configuration, so an `--untrusted` build confines only itself and any number of
/// builds may follow. It never fails outright: the result carries the problem that
/// stopped it (exit != 0), every report, and the counters. Only a build that did all
/// its work, and that `-W` does not fail, publishes; any other leaves the previous
/// output, manifest and environment as they were.
pub fn build(project: &Project, kind: BuilderKind, options: &BuildOptions) -> BuildResult {
    let setup = Setup {
        project,
        kind,
        options,
        budget: build_budget(options),
        policy: if options.untrusted { untrusted_policy() } else { trusted_policy() },
    };
    let mut config = project.config.clone();
    let mut log = Log {
        reports: project.warnings.clone(),
        result: BuildResult {
            output: options
                .output_dir
                .clone()
                .unwrap_or_else(|| config.build_dir.join(kind.name())),
            ..BuildResult::default()
        },
    };
    let outcome = run(&setup, &mut config, &mut log);
    finish(&setup, &config, log, outcome)
}

/// The budget of a build: `--budget` or `BUILD_BUDGET`, asking `on_budget` (a terminal
/// build asks the person) when it is reached.
fn build_budget(options: &BuildOptions) -> Budget {
    let limit = options.budget.unwrap_or(gd_env::BUILD_BUDGET);
    match &options.on_budget {
        Some(ask) => {
            let ask = std::sync::Arc::clone(ask);
            Budget::with_policy(limit, move |refusal| ask(refusal))
        }
        None => Budget::new(limit),
    }
}

/// What a build is given.
pub(crate) struct Setup<'p> {
    pub project: &'p Project,
    pub kind: BuilderKind,
    pub options: &'p BuildOptions,
    pub budget: Budget,
    pub policy: Policy,
}

/// What a build has done so far: its reports and the result's counters.
pub(crate) struct Log {
    pub reports: Vec<Report>,
    pub result: BuildResult,
}

/// A build that wrote its output, waiting to be published.
pub(crate) struct Written {
    pub generation: Generation,
    pub env: Environment,
    pub store: ObjectStore,
}

/// What reading and writing share once the build is prepared.
struct Parts<'c> {
    config: &'c Config,
    /// The limits the options ask for, which the reading and output settings record.
    limits: Limits,
    /// What one conversion may use: `limits` with its byte bounds lowered to the budget
    /// (`gd_host::limits_within`), so a budget bounds what a document and a page hold.
    run_limits: Limits,
    plugins: PluginRun,
    sources: Vec<SourceFile>,
    intl: Intl,
    dates: Dates,
    intersphinx: Intersphinx,
    store: ObjectStore,
    /// `gd_env::build_settings`: what the environment is saved with.
    settings: Digest,
}

fn run(s: &Setup<'_>, config: &mut Config, log: &mut Log) -> Result<Written, Problem> {
    let (kind, options) = (s.kind, s.options);
    if options.untrusted {
        log.reports.extend(gd_conf::confine_untrusted(config));
    }
    let paths = gd_outputs::output_paths(config, kind, options.output_dir.as_deref())?;
    // A custom output keeps its records in a cache folder of its own beside it; every
    // record (environment, objects, graphs, fonts) must live there, or the next build
    // would find none of them and read everything again.
    config.build_dir.clone_from(&paths.build_dir);
    log.result.output = paths.out_dir.clone();
    let generation_options = GenerationOptions { all: options.all, budget: &s.budget };
    let mut generation = Generation::begin(paths, generation_options, &mut log.reports)?;
    let limits = build_limits(options)?;
    let plugins = PluginRun::start(PluginStart {
        config,
        linked: &s.project.plugins,
        kind,
        untrusted: options.untrusted,
    })?;
    let sources = project_sources(config, &mut log.reports)?;
    let intl = gd_intl::load_intl(config, kind, &s.budget, &mut log.reports);
    if kind != BuilderKind::Gettext {
        gd_intl::localize_config(config, &intl.ui);
    }
    let config: &Config = config;
    let dates =
        gd_intl::build_dates(config, &intl.ui, options.now.unwrap_or_else(SystemTime::now));
    let network = IntersphinxOptions { network: !options.untrusted, budget: &s.budget };
    let intersphinx = gd_resolve::load_intersphinx(config, network, &mut log.reports)?;
    log.result.reached = BuildStage::Reading;
    let reading = gd_env::reading_settings(config);
    let settings =
        gd_env::build_settings(reading, intl.digest, options.untrusted, &limits, plugins.digest());
    let store = ObjectStore::new(gd_env::doctrees_dir(config));
    let run_limits = gd_host::limits_within(limits, s.budget.limit());
    let parts = Parts {
        config,
        limits,
        run_limits,
        plugins,
        sources,
        intl,
        dates,
        intersphinx,
        store,
        settings,
    };
    let (env, times) = read(s, &parts, log)?;
    duration::report_durations(config, &times, &mut log.result, &mut log.reports);
    log.reports.extend(gd_conf::LinkcodeSite::of(config).branch_report(config));
    let tables = gd_resolve::resolve(config, &env, &parts.intl.ui, &mut log.reports);
    log.result.reached = BuildStage::Writing;
    write(s, &parts, &env, &tables, &mut generation, log)?;
    duration::write_durations_json(config, &times, &mut generation)?;
    log.result.reached = BuildStage::Written;
    Ok(Written { generation, env, store: parts.store })
}

/// The project's documents: the source folder's (with `contents` as the root document
/// of an old project without `index`), and the generated Odin and Rust API pages.
fn project_sources(
    config: &mut Config,
    reports: &mut Vec<Report>,
) -> Result<Vec<SourceFile>, Problem> {
    let found = gd_env::discover(config)?;
    reports.extend(gd_env::contents_root(config, &found));
    let pages = gd_read::api_sources(config, &found, reports);
    Ok(gd_env::with_generated(found, pages))
}

fn read_context<'a>(s: &'a Setup<'_>, parts: &'a Parts<'_>) -> ReadContext<'a> {
    ReadContext {
        config: parts.config,
        registry: &s.project.registry,
        store: &parts.store,
        intl: &parts.intl,
        plugins: &parts.plugins,
        budget: &s.budget,
        policy: s.policy,
        limits: parts.run_limits,
        settings: parts.settings,
        today: &parts.dates.today,
        date: &parts.dates.date,
        jobs: match s.options.jobs {
            Jobs::Count(n) => n.max(1),
            Jobs::PerCore => 0,
        },
    }
}

/// Reads the documents (only those that changed since the saved environment, unless
/// `fresh`), counting them; the documents' reports join the build's. Returns the
/// environment and how long each document parsed took.
fn read(
    s: &Setup<'_>,
    parts: &Parts<'_>,
    log: &mut Log,
) -> Result<(Environment, Vec<ReadingTime>), Problem> {
    let ctx = read_context(s, parts);
    let path = gd_env::environment_path(parts.config);
    let previous = gd_env::load_environment(&path, ctx.settings, s.options.fresh, &s.budget)?;
    let mut outcome = gd_read::read_documents(&ctx, previous, &parts.sources)?;
    log.result.read = outcome.read;
    log.result.reused = outcome.reused;
    log.result.unread = outcome.unread;
    if gd_host::was_interrupted() {
        return Err(gd_host::interrupted_problem());
    }
    if outcome.reused > 0 {
        replace_stale_objects(&ctx, &parts.sources, &mut outcome.env, &mut log.reports)?;
    }
    for doc in &outcome.env.docs {
        log.reports.extend(doc.reports.iter().cloned());
    }
    Ok((outcome.env, outcome.reading_times))
}

/// Reads again, with a warning, each document whose cached object the engine cannot load
/// (damaged, or from another format version) although its digest matches, so every
/// builder renders from objects that load. Other failures (the budget) are left to the
/// builder, which reports them where it meets them.
fn replace_stale_objects(
    ctx: &ReadContext<'_>,
    sources: &[SourceFile],
    env: &mut Environment,
    reports: &mut Vec<Report>,
) -> Result<(), Problem> {
    for (i, stale) in stale_objects(ctx, env) {
        let doc = &mut env.docs[i];
        let Some(source) = sources.iter().find(|s| s.docname == doc.docname) else { continue };
        reports.push(gd_read::stale_warning(&stale, doc));
        *doc = gd_read::reread_document(ctx, source, doc)?;
    }
    Ok(())
}

/// The documents (by position in `env.docs`) whose cached object is stale, each with
/// why, in order. Checking an object hashes and loads all of it, so the checks share
/// the reading threads, each with its own workspace.
fn stale_objects(ctx: &ReadContext<'_>, env: &Environment) -> Vec<(usize, Problem)> {
    let threads = ctx.thread_count(env.docs.len());
    let checked = gd_host::map_ordered(threads, &env.docs, guidedoc::Workspace::new, |ws, doc| {
        let object = doc.object.as_ref()?;
        match ctx.store.load_object(ws, object, &ctx.limits) {
            Err(e) if e.stale => Some(e.problem),
            _ => None,
        }
    });
    checked.into_iter().enumerate().filter_map(|(i, p)| Some((i, p?))).collect()
}

/// Writes the output with the builder, counting the units written and kept.
fn write(
    s: &Setup<'_>,
    parts: &Parts<'_>,
    env: &Environment,
    tables: &Tables,
    generation: &mut Generation,
    log: &mut Log,
) -> Result<(), Problem> {
    let roots: Vec<PathBuf> = gd_host::canonical_roots(&gd_conf::readable_roots(parts.config));
    let options = s.options;
    let view = BuildView {
        config: parts.config,
        options,
        kind: s.kind,
        env,
        tables,
        intersphinx: &parts.intersphinx,
        objects: &parts.store,
        registry: &s.project.registry,
        policy: &s.policy,
        limits: &parts.run_limits,
        ui: &parts.intl.ui,
        dates: &parts.dates,
        plugins: &parts.plugins,
        budget: &s.budget,
        roots: &roots,
        settings: gd_env::output_settings(
            s.project.settings,
            s.kind,
            &options.tags,
            options.nitpicky,
            options.untrusted,
            &parts.limits,
            parts.plugins.digest(),
        ),
    };
    let ctx = read_context(s, parts);
    let reread = |doc: &DocInfo| reread(&ctx, &parts.sources, doc);
    let written =
        write_output(&view, &parts.sources, &parts.intl, &reread, generation, &mut log.reports);
    log.result.written = generation.written_count();
    log.result.unchanged = generation.reused_count();
    if log.result.written == 0 && log.result.unchanged == 0 && generation.is_site_written() {
        // The whole output is the site unit (singlehtml, gettext): one output written.
        log.result.written = 1;
    }
    written
}

/// Reads a document again whose cached object is stale, for the gettext builder.
fn reread(
    ctx: &ReadContext<'_>,
    sources: &[SourceFile],
    doc: &DocInfo,
) -> Result<CachedFile, Problem> {
    let missing = || {
        Problem::new(
            "build.object",
            "CACHED DOCUMENT CANNOT BE READ AGAIN",
            &doc.relative,
            "Its cached object is stale, and the document is no longer among the sources.",
            "Build again with -E to read every document anew.",
            1,
        )
    };
    let source = sources.iter().find(|s| s.docname == doc.docname).ok_or_else(missing)?;
    let fresh = gd_read::reread_document(ctx, source, doc)?;
    fresh.object.ok_or_else(missing)
}
