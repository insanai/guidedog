//! The steps of a decided commit (publish.odin): switch the output folder and the next
//! one, then rename each record over its destination.
//!
//! Each step can be repeated, and each can be undone. Where the new output is is told
//! by its folder's identity (`gd_host::identity`, kept in the journal), not by its
//! name, so a switch interrupted anywhere, even between an exchange and the rename
//! after it, is recognised; where the system gives no identity there is no exchange,
//! and the names tell. When a step fails, the steps done are undone in reverse
//! (`roll_back`), leaving everything as it was before the first step, the journal is
//! removed, and the problem says what failed and that nothing was published. When the
//! process stops instead, the next build finishes the commit before anything else; if
//! a step cannot be done then, it undoes the commit, says so, and goes on. Only a
//! commit that can be neither finished nor undone is left, with its journal, and
//! reported as such. A journal that cannot be read is discarded (`discard_journal`).
//!
//! No step guesses: a path whose state cannot be read is a failed step, never "not
//! there".

use std::io;
use std::path::{Path, PathBuf};

use gd_env::Digest;
use gd_host::{Budget, PathKind, Problem, io_problem, remove_tree};
use gd_report::Report;

use crate::commit::{CommitState, JOURNAL_SUFFIX, keep_spare};
use crate::hook::{StepAction, renames_only, step_action};
use crate::journal::{Journal, PendingFile};
use crate::manifest::{Manifest, load_text};
use crate::paths::MANIFEST_SUFFIX;

/// The code of the problem a commit stopped by a test hook gives.
const STOPPED: &str = "build.commit-stopped";

const PUBLICATION_HINT: &str = "Close any program that holds a file or folder of the output \
    open (an editor, a terminal, a file server), check that you may write to the build \
    folder and that the disk has free space, then build again. If it keeps failing, \
    `guidedog clean` empties the build folder.";

/// Carries out a decided commit, or what an interrupted one left, and removes its
/// journal (at `path`). A step that fails is undone with the steps before it; a commit
/// stopped by a test hook is left to the next build. Problems that do not change what
/// is published are warnings, added to `reports`.
pub(crate) fn finish_commit(
    j: &Journal,
    path: &Path,
    reports: &mut Vec<Report>,
) -> (Option<Problem>, CommitState) {
    let mut step = 0;
    let done = switch_output(j, &mut step).and_then(|()| rename_records(j, &mut step));
    match done {
        Err(p) if p.code == STOPPED => return (Some(p), CommitState::Unsettled),
        Err(p) => {
            if let Err(undo) = roll_back(j).and_then(|()| drop_journal(j, path)) {
                return (Some(not_undone(j, path, &p, &undo)), CommitState::Unsettled);
            }
            return (Some(not_published(j, &p)), CommitState::Kept);
        }
        Ok(()) => {}
    }
    // The switch and the renames are durable before the journal goes, so a crash after
    // its removal cannot bring back the previous output.
    let mut folders = Vec::with_capacity(1 + j.records.len());
    if j.next.is_some() {
        folders.extend(j.out_dir.parent().map(Path::to_path_buf));
    }
    folders.extend(j.records.iter().filter_map(|r| r.to.parent().map(Path::to_path_buf)));
    if let Err(p) = gd_host::sync_each(&folders) {
        // Published, but not yet durable: the journal stays, and the next build, which
        // finds nothing left to do but flush and remove it, finishes the commit.
        reports.push(warning_of(&p));
        return (None, CommitState::Published);
    }
    if let Err(e) = gd_host::remove_path(path) {
        reports.push(warning_of(&io_problem(path, &e)));
    }
    // The previous output is the spare output the next build completes. After an
    // exchange it is already there.
    if let Some(next) = &j.next {
        keep_spare(next, &j.old);
    }
    for previous in j.records.iter().filter_map(|r| r.previous.as_ref()) {
        let _ = gd_host::remove_path(previous);
    }
    (None, CommitState::Published)
}

/// Removes the output a published commit replaced, when it is not the spare output,
/// and warns when it cannot. Not a step of the commit, which is settled.
pub(crate) fn remove_previous_output(j: &Journal, reports: &mut Vec<Report>) {
    let Err(e) = remove_tree(&j.old) else { return };
    let old = j.old.display();
    reports.push(warning_of(&Problem::new(
        "build.output",
        "PREVIOUS OUTPUT NOT REMOVED",
        &old.to_string(),
        format!(
            "The new output is published in {}, but the previous one could not be removed: \
             {e}.",
            j.out_dir.display()
        ),
        format!("Remove {old} yourself; the next build tries again."),
        0,
    )));
}

/// Whether something is at `path`; a path whose state cannot be read is a problem.
fn present(path: &Path) -> Result<bool, Problem> {
    Ok(gd_host::path_type(path).map_err(|e| io_problem(path, &e))?.is_some())
}

/// Asks the test hook about the next step; outside tests every step goes on.
fn commit_step(step: &mut usize, path: &Path) -> Result<(), Problem> {
    *step += 1;
    match step_action(*step) {
        StepAction::Go => Ok(()),
        StepAction::Crash => Err(Problem::new(
            STOPPED,
            "COMMIT STOPPED",
            &path.display().to_string(),
            "A test stopped the commit.",
            "Build again: the next build finishes the commit.",
            1,
        )),
        StepAction::Fail => {
            Err(io_problem(path, &io::Error::new(io::ErrorKind::InvalidData, "invalid file")))
        }
    }
}

fn commit_move(step: &mut usize, from: &Path, to: &Path) -> Result<(), Problem> {
    commit_step(step, to)?;
    std::fs::rename(from, to).map_err(|e| io_problem(to, &e))
}

/// Where an output switch stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SwitchState {
    /// Not done: the new output is the next folder.
    Before,
    /// Done: the new output is the output folder.
    Switched,
    /// The new output is neither.
    Lost,
}

fn switch_state(j: &Journal, next: &Path) -> Result<SwitchState, Problem> {
    if let Some(id) = j.next_id {
        let out = gd_host::identity(&j.out_dir).map_err(|e| io_problem(&j.out_dir, &e))?;
        if out == Some(id) {
            return Ok(SwitchState::Switched);
        }
        let spare = gd_host::identity(next).map_err(|e| io_problem(next, &e))?;
        return Ok(if spare == Some(id) { SwitchState::Before } else { SwitchState::Lost });
    }
    // Without identities there is no exchange: the next folder is there until it is
    // the output folder.
    if present(next)? {
        return Ok(SwitchState::Before);
    }
    Ok(if present(&j.out_dir)? { SwitchState::Switched } else { SwitchState::Lost })
}

/// Makes the next folder the output folder, keeping the previous output at `old`,
/// from wherever an earlier attempt stopped.
fn switch_output(j: &Journal, step: &mut usize) -> Result<(), Problem> {
    let Some(next) = &j.next else { return Ok(()) };
    match switch_state(j, next)? {
        SwitchState::Lost => Err(lost_problem(next)),
        SwitchState::Switched => Ok(()),
        SwitchState::Before => {
            if !present(&j.out_dir)? {
                return commit_move(step, next, &j.out_dir);
            }
            if j.next_id.is_some() && !renames_only() {
                // An exchange needs identities, to tell after it which folder is which.
                commit_step(step, &j.out_dir)?;
                let swapped =
                    gd_host::exchange(&j.out_dir, next).map_err(|e| io_problem(&j.out_dir, &e))?;
                if swapped {
                    return Ok(()); // the previous output is at next, as the spare output.
                }
            }
            commit_move(step, &j.out_dir, &j.old)?;
            commit_move(step, next, &j.out_dir)
        }
    }
}

/// Renames each record written beside its destination over it; a record no longer
/// beside its destination was renamed before.
fn rename_records(j: &Journal, step: &mut usize) -> Result<(), Problem> {
    for r in &j.records {
        if present(&r.from)? {
            commit_move(step, &r.from, &r.to)?;
        }
    }
    Ok(())
}

/// Undoes what a commit did, in reverse, so that the previous output and records are
/// in place again, and everything the journal names is as it was before the first
/// step: each record back beside its destination, the new output back in the next
/// folder. A crash while undoing leaves a state the journal can still finish.
fn roll_back(j: &Journal) -> Result<(), Problem> {
    for r in j.records.iter().rev() {
        if !present(&r.from)? && present(&r.to)? {
            // Renamed: the new record goes back beside its destination.
            std::fs::rename(&r.to, &r.from).map_err(|e| io_problem(&r.to, &e))?;
        }
        restore_record(r)?;
    }
    let Some(next) = &j.next else { return Ok(()) };
    if switch_state(j, next)? == SwitchState::Switched {
        if present(next)? {
            // Exchanged, the previous output not yet moved to old.
            let swapped =
                gd_host::exchange(&j.out_dir, next).map_err(|e| io_problem(&j.out_dir, &e))?;
            return if swapped { Ok(()) } else { Err(no_swap_problem(&j.out_dir)) };
        }
        std::fs::rename(&j.out_dir, next).map_err(|e| io_problem(&j.out_dir, &e))?;
    }
    if !present(&j.out_dir)? && present(&j.old)? {
        std::fs::rename(&j.old, &j.out_dir).map_err(|e| io_problem(&j.out_dir, &e))?;
    }
    Ok(())
}

/// Puts the record a commit replaced back at its destination, from its `.previous`
/// link, through another link (`.restore`) renamed over the destination: the
/// `.previous` link stays, so undoing can be repeated. A record that had none has
/// nothing to put back.
fn restore_record(r: &PendingFile) -> Result<(), Problem> {
    let Some(previous) = &r.previous else { return Ok(()) };
    let mut name = r.to.clone().into_os_string();
    name.push(".restore");
    let restore = PathBuf::from(name);
    let _ = gd_host::remove_path(&restore);
    gd_host::link_file(previous, &restore, PathKind::File)
        .map_err(|e| io_problem(&restore, &e))?;
    std::fs::rename(&restore, &r.to).map_err(|e| io_problem(&r.to, &e))?;
    // When the destination already was that record, the rename changed nothing and
    // left the link.
    let _ = gd_host::remove_path(&restore);
    Ok(())
}

/// Removes an undone commit's journal, then the records it prepared. The next folder,
/// which holds the new output again, stays as the spare output.
fn drop_journal(j: &Journal, path: &Path) -> Result<(), Problem> {
    gd_host::remove_path(path).map_err(|e| io_problem(path, &e))?;
    if let Some(folder) = path.parent() {
        let _ = gd_host::sync_each(&[folder.to_path_buf()]);
    }
    for r in &j.records {
        let _ = gd_host::remove_path(&r.from);
        if let Some(previous) = &r.previous {
            let _ = gd_host::remove_path(previous);
        }
    }
    Ok(())
}

/// Settles every commit whose journal is in the `doctrees` folder: a build of any
/// builder that stopped after deciding to publish. Each is finished, or, when a step
/// cannot be done, undone, and the build goes on with a warning; only a commit that
/// can be neither finished nor undone stops the build. A journal that cannot be read
/// is discarded. Journals are read within `budget`.
pub(crate) fn finish_interrupted_commits(
    doctrees: &Path,
    budget: &Budget,
    reports: &mut Vec<Report>,
) -> Result<(), Problem> {
    let entries = match gd_host::list_folder(doctrees) {
        Ok(entries) => entries,
        // No build yet; other failures must not pass for absence.
        Err(_) if !present(doctrees)? => return Ok(()),
        Err(e) => return Err(io_problem(doctrees, &e)),
    };
    for e in entries {
        let name = e.name.to_string_lossy();
        if e.kind == PathKind::Folder || !name.ends_with(JOURNAL_SUFFIX) {
            continue;
        }
        let path = doctrees.join(&e.name);
        let text = load_text(&path, budget)?;
        let Some(j) = text.as_deref().and_then(Journal::from_json) else {
            discard_journal(doctrees, &path, budget, reports)?;
            continue;
        };
        match finish_commit(&j, &path, reports) {
            (_, CommitState::Published) => remove_previous_output(&j, reports),
            (problem, CommitState::Kept) => reports.push(undone_warning(&j, problem.as_ref())),
            (problem, CommitState::Unsettled) => {
                return Err(problem.unwrap_or_else(|| lost_problem(&j.out_dir)));
            }
        }
    }
    Ok(())
}

/// Removes a journal this build cannot read: it is damaged, or another Guidedog wrote
/// it. What it published is unknown, so none of the output it describes is trusted:
/// the builder's manifest is kept, for the files it lists, but its digests are
/// cleared, so the build writes every output again and compares it with the file in
/// place.
fn discard_journal(
    doctrees: &Path,
    journal: &Path,
    budget: &Budget,
    reports: &mut Vec<Report>,
) -> Result<(), Problem> {
    let name = journal.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    let builder = name.strip_suffix(JOURNAL_SUFFIX).unwrap_or(&name);
    let manifest_path = doctrees.join(format!("{builder}{MANIFEST_SUFFIX}"));
    let text = load_text(&manifest_path, budget)?;
    match text.as_deref().and_then(Manifest::from_json) {
        Some(mut manifest) => {
            for unit in manifest.units.values_mut() {
                unit.inputs = Digest::default();
            }
            gd_host::publish(&manifest_path, manifest.to_json().as_bytes(), true)?;
        }
        None => {
            let removed = gd_host::remove_path(&manifest_path);
            if let (Err(e), Some(_)) = (removed, &text) {
                return Err(io_problem(&manifest_path, &e));
            }
        }
    }
    gd_host::remove_path(journal).map_err(|e| io_problem(journal, &e))?;
    reports.push(warning_of(&Problem::new(
        "build.publish",
        "UNREADABLE PUBLICATION JOURNAL DISCARDED",
        &journal.display().to_string(),
        "An earlier build stopped while publishing, and its journal cannot be read, so what \
         it published is unknown. This build writes every output again.",
        "If the output still shows files it should not, run `guidedog clean` and build again.",
        0,
    )));
    Ok(())
}

/// A commit step's problem once the commit is undone.
fn not_published(j: &Journal, p: &Problem) -> Problem {
    Problem::new(
        "build.publish",
        "THE OUTPUT COULD NOT BE PUBLISHED",
        &p.path,
        format!(
            "Publishing this build's output in {} failed: {}. Nothing was published: the \
             previous output and the build records are as they were.",
            j.out_dir.display(),
            p.message
        ),
        PUBLICATION_HINT,
        4,
    )
}

/// A commit step's problem when undoing the commit failed too: the journal stays, and
/// the next build finishes or undoes the commit before anything else.
fn not_undone(j: &Journal, journal: &Path, p: &Problem, undo: &Problem) -> Problem {
    Problem::new(
        "build.publish",
        "PUBLICATION NOT FINISHED",
        &p.path,
        format!(
            "Publishing this build's output in {} failed ({}), and so did undoing it ({}: {}). \
             The output may be part old and part new until the next build, which finishes or \
             undoes the publication first; its journal is {}.",
            j.out_dir.display(),
            p.message,
            undo.path,
            undo.message,
            journal.display()
        ),
        PUBLICATION_HINT,
        4,
    )
}

/// The warning of an interrupted commit this build undid.
fn undone_warning(j: &Journal, p: Option<&Problem>) -> Report {
    let why = p.map_or_else(String::new, |p| format!(" ({})", p.message));
    warning_of(&Problem::new(
        "build.publish",
        "INTERRUPTED PUBLICATION UNDONE",
        &p.map_or_else(String::new, |p| p.path.clone()),
        format!(
            "An earlier build stopped while publishing its output in {}, and this build could \
             not finish it{why}, so it was undone: the output and the build records are those \
             the build before it published.",
            j.out_dir.display()
        ),
        "Nothing to do: this build writes the output again.",
        0,
    ))
}

fn lost_problem(next: &Path) -> Problem {
    Problem::new(
        "build.publish",
        "NEW OUTPUT MISSING",
        &next.display().to_string(),
        "The new output an earlier build prepared is no longer there, so its publication \
         cannot be finished.",
        "The previous output is put back; build again to publish.",
        4,
    )
}

fn no_swap_problem(out_dir: &Path) -> Problem {
    Problem::new(
        "build.publish",
        "CANNOT SWAP THE OUTPUT FOLDERS",
        &out_dir.display().to_string(),
        "The folders were swapped in one step, but the system no longer does so.",
        PUBLICATION_HINT,
        4,
    )
}

/// A problem that does not stop the build, as a warning report.
fn warning_of(p: &Problem) -> Report {
    let mut report = p.report();
    report.severity = gd_report::Severity::Warning;
    report
}
