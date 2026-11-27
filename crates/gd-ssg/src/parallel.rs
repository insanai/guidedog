//! Editions built at the same time, in child processes, as many as fit in memory
//! (GDS 0020).
//!
//! Each edition is a job. A job's weight is the memory reserved for it while it runs: its
//! peak at its last build, recorded in `memory.toml` of the work folder, with a quarter
//! more. The capacity is nine tenths of the memory available when the build begins.
//! `gd_host::Schedule` decides which job starts; this module starts the children, waits
//! for them, copies their reports to the host in plan order, and records their peaks.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, ExitStatus, Stdio};
use std::sync::mpsc;

use gd_conf::{Config, LoadOptions};
use gd_host::{JobPhase, Problem, Schedule};

use crate::mounts::{Job, SiteBuildOptions, SiteHost};

const MIB: u64 = 1 << 20;
/// A weight is a whole number of these.
const WEIGHT_STEP: u64 = 64 * MIB;
/// The weight of an HTML edition that was never measured.
const DEFAULT_HTML: u64 = 1 << 30;
/// The weight of a PDF edition that was never measured.
const DEFAULT_PDF: u64 = 3 << 30;
/// The file of the work folder that keeps each job's last peak.
const RECORD: &str = "memory.toml";

/// How a parallel build runs: the weight of each job, the capacity and the slots.
#[derive(Debug, Clone)]
pub(crate) struct Setup {
    pub weights: Vec<u64>,
    pub capacity: u64,
    pub slots: usize,
}

/// Decides whether the editions are built at once (conditions 1 to 5 of GDS 0020), and
/// how. Fails with the reason they are built one at a time.
pub(crate) fn setup(
    jobs: &[Job],
    work: &Path,
    options: &SiteBuildOptions,
) -> Result<Setup, String> {
    if options.jobs == 1 {
        return Err("--jobs 1 asks for that".into());
    }
    if options.program.is_empty() {
        return Err("this host builds in its own process".into());
    }
    if jobs.len() < 2 {
        return Err("there is one edition".into());
    }
    let Some(available) = gd_host::available_memory() else {
        return Err(format!(
            "the memory available is unknown; set {} to build at once",
            gd_host::AVAILABLE_OVERRIDE
        ));
    };
    if let Some(reason) = reads_work(jobs, work) {
        return Err(reason);
    }
    let processors = std::thread::available_parallelism().map_or(1, |n| n.get());
    let slots = if options.jobs == 0 { processors } else { options.jobs };
    Ok(Setup {
        weights: weights(jobs, &read_record(work)),
        capacity: gd_host::fitting_budget(available) as u64,
        slots: slots.min(jobs.len()),
    })
}

/// Condition 5: the reason, if a job reads below the work folder, where other jobs
/// write, or walks a folder that holds it.
fn reads_work(jobs: &[Job], work: &Path) -> Option<String> {
    let work = work.canonicalize().unwrap_or_else(|_| work.to_path_buf());
    for job in jobs {
        let options = LoadOptions {
            source: Some(job.request.project.clone()),
            output: Some(job.request.output.clone()),
            overrides: job.request.overrides.clone(),
            ..Default::default()
        };
        let Ok(loaded) = gd_conf::load_config(&job.request.project, &options) else {
            // The build that follows reports why, as it always did.
            return Some(format!("the settings of {} do not load", job.mounted.key));
        };
        for (input, walked) in inputs(&loaded.config) {
            let input = input.canonicalize().unwrap_or(input);
            if input.starts_with(&work) || (walked && work.starts_with(&input)) {
                return Some(format!(
                    "{} reads {}, which shares the build folder {}",
                    job.mounted.key,
                    input.display(),
                    work.display()
                ));
            }
        }
    }
    None
}

/// The folders and files a build of `config` reads besides the user's caches, each
/// with whether the build walks it whole: the source folder, the templates, the static
/// and extra files, the catalogs and the fonts are walked; the include roots, the
/// documented crates and local intersphinx inventories are read file by file.
fn inputs(config: &Config) -> Vec<(PathBuf, bool)> {
    let roots = gd_conf::readable_roots(config).into_iter().enumerate();
    // The first readable root is the source folder.
    let mut paths: Vec<(PathBuf, bool)> = roots.map(|(i, root)| (root, i == 0)).collect();
    let walked = config
        .templates_path
        .iter()
        .chain(&config.html.static_path)
        .chain(&config.html.extra_path)
        .chain(&config.intl.locale_dirs)
        .chain(&config.pdf.font_paths);
    paths.extend(walked.map(|p| (gd_conf::source_path(config, p), true)));
    let inventories = config.intersphinx.mapping.iter().flat_map(|p| &p.locations);
    let local = inventories.filter(|l| !l.is_empty() && !l.contains("://"));
    paths.extend(local.map(|l| (gd_conf::source_path(config, l), false)));
    paths
}

/// Each job's weight: its recorded peak with a quarter more, in steps of 64 MiB; for a
/// job never measured, the heaviest recorded job of the same builder, or the default.
fn weights(jobs: &[Job], record: &BTreeMap<String, u64>) -> Vec<u64> {
    let heaviest = |pdf: bool| {
        jobs.iter()
            .filter(|j| j.mounted.pdf == pdf)
            .filter_map(|j| record.get(&j.mounted.key).copied())
            .max()
    };
    jobs.iter()
        .map(|job| {
            match record.get(&job.mounted.key).copied().or_else(|| heaviest(job.mounted.pdf)) {
                Some(peak) => weight_of(peak),
                None if job.mounted.pdf => DEFAULT_PDF,
                None => DEFAULT_HTML,
            }
        })
        .collect()
}

/// A peak with a quarter more, rounded up to a whole step; at least one step.
fn weight_of(peak: u64) -> u64 {
    let margin = peak.saturating_add(peak / 4);
    margin.div_ceil(WEIGHT_STEP).max(1).saturating_mul(WEIGHT_STEP)
}

/// The recorded peaks; empty when the record is missing or unreadable.
fn read_record(work: &Path) -> BTreeMap<String, u64> {
    let Ok(text) = std::fs::read_to_string(work.join(RECORD)) else {
        return BTreeMap::new();
    };
    let Ok(table) = gd_toml::parse(&text) else {
        return BTreeMap::new();
    };
    table
        .iter()
        .filter_map(|(key, value)| {
            let bytes = value.as_integer().and_then(|n| u64::try_from(n).ok())?;
            (bytes > 0).then(|| (key.to_string(), bytes))
        })
        .collect()
}

/// Writes the peaks of this plan's jobs, dropping jobs of other plans. A job that
/// succeeded records the peak its child reported; a job that failed or was stopped may
/// have ended before its peak, so it keeps the larger of its old and new peaks. A record
/// that cannot be written stays as it was: it is a cache, and the build's own outcome
/// is what the caller must see.
fn write_record(work: &Path, jobs: &[Job], files: &[JobFiles], schedule: &Schedule) {
    let old = read_record(work);
    let mut text = String::from("# The peak memory of each edition build, in bytes (GDS 0020).\n");
    for (j, (job, file)) in jobs.iter().zip(files).enumerate() {
        let key = &job.mounted.key;
        let reported = std::fs::read_to_string(&file.peak).ok();
        let new = reported.and_then(|t| t.trim().parse::<u64>().ok());
        let peak = match (new, old.get(key).copied()) {
            (Some(n), Some(o)) if schedule.phase(j) != JobPhase::Succeeded => Some(n.max(o)),
            (new, old) => new.or(old),
        };
        if let Some(peak) = peak {
            gd_toml::write_basic_string(&mut text, key);
            text.push_str(&format!(" = {}\n", peak.min(i64::MAX as u64)));
        }
    }
    let _ = gd_host::publish(&work.join(RECORD), text.as_bytes(), true);
}

/// Where a job's child writes its report and its peak: beside its output folder.
struct JobFiles {
    log: PathBuf,
    peak: PathBuf,
}

impl JobFiles {
    fn of(job: &Job) -> JobFiles {
        let built = &job.mounted.built;
        let dotted = |suffix: &str| {
            let name = built.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
            built.with_file_name(format!(".{name}.{suffix}"))
        };
        JobFiles { log: dotted("log"), peak: dotted("peak") }
    }
}

/// How a job ended.
enum Ended {
    /// Its child exited with this status.
    Exited(ExitStatus),
    /// Its child could not start.
    NotStarted(String),
    /// Its child started, and waiting for it failed.
    Lost(String),
}

/// Builds the jobs in child processes as `setup` allows, copies their reports to `host`
/// in plan order, and records their peaks. Returns a sentence on what ran; fails with
/// the first failure in plan order (Theorem 5 of GDS 0020), or when interrupted.
pub(crate) fn run(
    jobs: &[Job],
    work: &Path,
    options: &SiteBuildOptions,
    setup: &Setup,
    host: &mut dyn SiteHost,
) -> Result<String, Problem> {
    let files: Vec<JobFiles> = jobs.iter().map(JobFiles::of).collect();
    let mut schedule = Schedule::new(setup.weights.clone(), setup.capacity, setup.slots);
    let mut ended: Vec<Option<Ended>> = jobs.iter().map(|_| None).collect();
    let most = std::thread::scope(|scope| {
        let (sender, receiver) = mpsc::channel::<(usize, std::io::Result<ExitStatus>)>();
        let mut most = 0;
        let mut shown = 0;
        loop {
            // Read once a round: on macOS it runs a program, and the jobs this round
            // starts have not grown yet, so a later reading would not differ.
            let available = gd_host::available_memory();
            while !gd_host::was_interrupted() {
                let Some(j) = schedule.next(|weight| room(available, weight)) else { break };
                schedule.start(j);
                match start(&jobs[j], &files[j], options) {
                    Ok(mut child) => {
                        let sender = sender.clone();
                        scope.spawn(move || sender.send((j, child.wait())));
                    }
                    Err(reason) => {
                        schedule.finish(j, false);
                        ended[j] = Some(Ended::NotStarted(reason));
                    }
                }
                most = most.max(schedule.running_count());
            }
            if schedule.running_count() == 0 {
                break;
            }
            // A job runs, so a waiting thread holds a sender: this receives.
            let Ok((j, status)) = receiver.recv() else { break };
            let result = status.map(Ended::Exited).unwrap_or_else(|e| Ended::Lost(e.to_string()));
            schedule.finish(j, matches!(&result, Ended::Exited(s) if s.success()));
            ended[j] = Some(result);
            shown = show_reports(host, &schedule, &files, shown, false);
        }
        show_reports(host, &schedule, &files, shown, true);
        most
    });
    write_record(work, jobs, &files, &schedule);
    outcome(jobs, &schedule, &ended, setup, most)
}

/// Starts the child that builds `job`, its report going to its log file.
fn start(job: &Job, files: &JobFiles, options: &SiteBuildOptions) -> Result<Child, String> {
    if let Some(folder) = files.log.parent() {
        std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    }
    let _ = std::fs::remove_file(&files.peak);
    let log = File::create(&files.log).map_err(|e| format!("{}: {e}", files.log.display()))?;
    let err = log.try_clone().map_err(|e| e.to_string())?;
    let mut command = gd_plugins::build_command(&options.program, &job.request);
    command.arg("--peak-memory-file").arg(&files.peak).args(&options.child_args);
    command.stdin(Stdio::null()).stdout(Stdio::from(log)).stderr(Stdio::from(err));
    command.spawn().map_err(|e| format!("{} cannot start: {e}", options.program))
}

/// Whether `available`, the memory available now, still has room for `weight`: the
/// environment's consent of GDS 0020. When it could not be read, the capacity alone
/// decides.
fn room(available: Option<usize>, weight: u64) -> bool {
    available.is_none_or(|a| gd_host::fitting_budget(a) as u64 >= weight)
}

/// Copies to `host` the reports of the jobs from `shown` on that have ended, in plan
/// order, stopping at the first that has not; at the end (`all`), every ended job's,
/// including jobs after a failure that were already running.
/// Returns how many jobs are done with.
fn show_reports(
    host: &mut dyn SiteHost,
    schedule: &Schedule,
    files: &[JobFiles],
    mut shown: usize,
    all: bool,
) -> usize {
    while shown < files.len() {
        match schedule.phase(shown) {
            JobPhase::Succeeded | JobPhase::Failed => {
                // A report is shown even when a program wrote bytes that are not UTF-8.
                if let Ok(bytes) = std::fs::read(&files[shown].log) {
                    host.report(&String::from_utf8_lossy(&bytes));
                }
            }
            JobPhase::Waiting | JobPhase::Running if !all => break,
            JobPhase::Waiting | JobPhase::Running => {}
        }
        shown += 1;
    }
    shown
}

/// The result of the run: the first failure in plan order, an interruption, or a
/// sentence on what ran.
fn outcome(
    jobs: &[Job],
    schedule: &Schedule,
    ended: &[Option<Ended>],
    setup: &Setup,
    most: usize,
) -> Result<String, Problem> {
    let all = (0..jobs.len()).all(|j| schedule.phase(j) == JobPhase::Succeeded);
    // Ctrl+C reaches the children too, which then fail: the person stopped the build.
    if !all && gd_host::was_interrupted() {
        return Err(gd_host::interrupted_problem());
    }
    if let Some(f) = schedule.first_failure() {
        return Err(failure(&jobs[f], ended[f].as_ref()));
    }
    if !all {
        return Err(gd_host::interrupted_problem());
    }
    let gib = setup.capacity as f64 / (1u64 << 30) as f64;
    Ok(format!(
        "Built {} editions in child processes, up to {most} at once within {gib:.1} GiB.",
        jobs.len()
    ))
}

/// The problem of a job that failed, as the sequential build names it.
fn failure(job: &Job, ended: Option<&Ended>) -> Problem {
    let (message, exit) = match ended {
        Some(Ended::Exited(status)) => (
            format!(
                "The build of {} stopped with status {}; its report is above.",
                job.mounted.key,
                status.code().map_or_else(|| "a signal".into(), |c| c.to_string())
            ),
            status.code().filter(|&c| c != 0).unwrap_or(1),
        ),
        Some(Ended::NotStarted(reason)) => {
            (format!("The build of {} did not run: {reason}.", job.mounted.key), 4)
        }
        Some(Ended::Lost(reason)) => {
            (format!("The build of {} was lost: {reason}.", job.mounted.key), 4)
        }
        None => (format!("The build of {} did not end.", job.mounted.key), 1),
    };
    Problem::new(
        "ssg.mount_build",
        "MOUNT BUILD FAILED",
        &job.request.project.display().to_string(),
        message,
        "Fix what the build reported, then build the site again.",
        exit,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_weight_is_the_peak_with_a_quarter_more_in_whole_steps() {
        assert_eq!(weight_of(0), WEIGHT_STEP);
        assert_eq!(weight_of(64 * MIB), 128 * MIB);
        assert_eq!(weight_of(100 * MIB), 128 * MIB);
        assert_eq!(weight_of(2000 * MIB), 2560 * MIB);
        assert_eq!(weight_of(u64::MAX), u64::MAX);
    }

    fn job(work: &Path, key: &str) -> Job {
        let built = work.join(key);
        let mounted = crate::mounts::Mounted {
            key: key.into(),
            output: String::new(),
            built,
            exclude: Vec::new(),
            pdf: false,
            manifest: PathBuf::new(),
        };
        Job { request: gd_plugins::BuildRequest::default(), mounted }
    }

    #[test]
    fn a_job_that_did_not_succeed_never_lowers_its_peak() {
        let work = gd_plugins::files::make_temp_dir(&std::env::temp_dir(), "ssg-peaks")
            .unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(work.join(RECORD), "\"a/en\" = 1000\n\"b/en\" = 1000\n\"gone/en\" = 5\n")
            .unwrap();
        let jobs = [job(&work, "a/en"), job(&work, "b/en"), job(&work, "c/en")];
        let files: Vec<JobFiles> = jobs.iter().map(JobFiles::of).collect();
        std::fs::create_dir_all(work.join("a")).unwrap();
        std::fs::create_dir_all(work.join("b")).unwrap();
        std::fs::create_dir_all(work.join("c")).unwrap();
        for file in &files {
            std::fs::write(&file.peak, "10").unwrap();
        }
        let mut schedule = Schedule::new(vec![1, 1, 1], 8, 8);
        for (j, ok) in [(0, true), (1, false)] {
            schedule.start(j);
            schedule.finish(j, ok);
        }
        write_record(&work, &jobs, &files, &schedule);
        let record = read_record(&work);
        assert_eq!(record.get("a/en"), Some(&10), "a success records its new peak");
        assert_eq!(record.get("b/en"), Some(&1000), "a failure keeps the larger peak");
        assert_eq!(record.get("c/en"), Some(&10), "a job that never ran keeps what it wrote");
        assert_eq!(record.get("gone/en"), None, "jobs of other plans are dropped");
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn a_report_is_read_back_and_bad_entries_are_dropped() {
        let work = gd_plugins::files::make_temp_dir(&std::env::temp_dir(), "ssg-record")
            .unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(
            work.join(RECORD),
            "\"manual/en\" = 2048\n\"bad\" = -1\n\"text\" = \"x\"\n",
        )
        .unwrap();
        let record = read_record(&work);
        assert_eq!(record.get("manual/en"), Some(&2048));
        assert_eq!(record.len(), 1);
        let _ = std::fs::remove_dir_all(&work);
    }
}
