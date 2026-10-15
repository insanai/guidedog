//! Changing a catalog: the lock, plans, and their recoverable application.
//!
//! [`apply`] checks a whole plan, writes a journal of the originals, and then replaces
//! each file atomically; an interruption leaves the journal for [`recover`], which puts
//! every file back. Each file replacement is atomic; the journal makes the set of them
//! recoverable, not atomic.

use crate::catalog::{Catalog, check_sources, is_safe_change_path};
use crate::definition::definition;
use crate::index::is_balanced;
use crate::metadata::read_record;
use crate::problem::{io_problem, shown};
use crate::{MAX_INDEX_BYTES, MAX_JOURNAL_BYTES, Prefix, Problem, Result, exists, read_text};
use gd_json::{Object, Value};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// One file of a plan: its path in the catalog (`records/0007-cache.rst`), and its
/// contents before and after; `None` is a file that is absent.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Change {
    /// The path relative to the catalog folder, `/`-separated.
    pub path: String,
    /// The contents the plan was made from, or `None` for a file to create.
    pub before: Option<String>,
    /// The contents to write, or `None` for a file to remove.
    pub after: Option<String>,
}

/// A change to a catalog, made by a `*_plan` function and written by [`apply`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    /// The files to change, in order.
    pub changes: Vec<Change>,
    /// One line saying what the plan does.
    pub summary: String,
}

/// The catalog's lock, held while a command loads, plans, and applies; dropping it
/// releases it. A kernel lock is released even when the process crashes.
#[derive(Debug)]
pub struct Lock {
    _file: File,
    dir: PathBuf,
    prefix: Prefix,
}

impl Lock {
    /// The locked catalog's folder, borrowed.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The locked catalog's prefix, borrowed.
    pub fn prefix(&self) -> &Prefix {
        &self.prefix
    }
}

/// Takes the lock of the catalog in `dir` (`.PREFIX.lock`). Another command holding it
/// is `PREFIX.busy`. The lock file is kept: removing it would let two processes lock
/// different files at one path.
pub fn lock_catalog(dir: &Path, prefix: &Prefix) -> Result<Lock> {
    let path = dir.join(format!(".{prefix}.lock"));
    if std::fs::symlink_metadata(&path).is_ok_and(|info| !info.is_file()) {
        return Err(Problem::new(
            prefix.code("lock"),
            "Unsafe lock file",
            shown(&path),
            "The lock path is not a regular file.",
            "Remove the unexpected link or special file.",
        )
        .exit(4));
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|e| io_problem(prefix.as_str(), &path, &e))?;
    // A process starting another program holds a copy of every descriptor for a moment
    // (between fork and exec), so a lock just released may look taken: retry briefly.
    for _ in 0..LOCK_ATTEMPTS {
        match file.try_lock() {
            Ok(()) => {
                return Ok(Lock { _file: file, dir: dir.to_path_buf(), prefix: prefix.clone() });
            }
            Err(std::fs::TryLockError::WouldBlock) => std::thread::sleep(LOCK_PAUSE),
            Err(std::fs::TryLockError::Error(e)) => {
                return Err(io_problem(prefix.as_str(), &path, &e));
            }
        }
    }
    Err(Problem::new(
        prefix.code("busy"),
        "Another discussion command is working",
        shown(dir),
        "The discussion directory is locked.",
        "Wait for the other command, then retry.",
    )
    .exit(4))
}

const LOCK_ATTEMPTS: u32 = 20;
const LOCK_PAUSE: std::time::Duration = std::time::Duration::from_millis(10);

/// The journal of an update in progress: `.PREFIX-journal.json`.
pub(crate) fn journal_path(dir: &Path, prefix: &Prefix) -> PathBuf {
    dir.join(format!(".{prefix}-journal.json"))
}

/// The problem of a catalog whose last update did not finish.
pub(crate) fn recovery_needed(prefix: &Prefix, dir: &Path) -> Problem {
    Problem::new(
        prefix.code("recovery-needed"),
        "An update needs recovery",
        shown(dir),
        "A previous multi-file update did not finish.",
        "Run the recover command with --rollback and the same --dir.",
    )
    .exit(4)
}

fn pending_problem(prefix: &Prefix, path: &str) -> Problem {
    Problem::new(
        prefix.code("recovery-needed"),
        "The update stopped before completion",
        path,
        "Recovery data is retained; do not edit the affected files yet.",
        "Run the recover command with --rollback and the same --dir.",
    )
    .exit(4)
}

/// Writes `plan` to the locked catalog as one transaction. Each file must still have
/// the contents the plan was made from, and each change must be whole (a record that
/// parses and has its sections, an index with one generated region), or nothing is
/// written. A journal records the originals first, and every file is replaced
/// atomically, so an interrupted apply is rolled back by [`recover`].
pub fn apply(lock: &Lock, plan: &Plan) -> Result<()> {
    if plan.changes.is_empty() {
        return Ok(());
    }
    check_plan(lock, plan)?;
    apply_changes(lock, plan, plan.changes.len())
}

/// Writes the journal and then the first `count` changes, as an apply interrupted after
/// `count` files would.
pub(crate) fn apply_changes(lock: &Lock, plan: &Plan, count: usize) -> Result<()> {
    let (dir, prefix) = (&lock.dir, &lock.prefix);
    let journal = journal_path(dir, prefix);
    if exists(&journal) {
        return Err(recovery_needed(prefix, &journal));
    }
    replace_file(prefix.as_str(), &journal, &journal_text(&plan.changes))?;
    for c in &plan.changes[..count] {
        let pending = || pending_problem(prefix, &c.path);
        if !same_snapshot(dir, &c.path, c.before.as_deref(), prefix) {
            return Err(pending());
        }
        let path = dir.join(&c.path);
        match &c.after {
            Some(text) => replace_file(prefix.as_str(), &path, text).map_err(|_| pending())?,
            None => std::fs::remove_file(&path).map_err(|_| pending())?,
        }
    }
    let pending = || pending_problem(prefix, &shown(&journal));
    if count < plan.changes.len() {
        return Err(pending());
    }
    std::fs::remove_file(&journal).map_err(|_| pending())
}

/// Refuses a plan whose files are not safe catalog paths, have changed since it was
/// made, or are not whole.
fn check_plan(lock: &Lock, plan: &Plan) -> Result<()> {
    let (dir, prefix) = (&lock.dir, &lock.prefix);
    let mut whole = Catalog::new(prefix.clone());
    if let Some(d) = definition(dir)?.filter(|d| d.prefix == *prefix) {
        (whole.labels, whole.process) = (d.labels, d.process);
    }
    for (i, c) in plan.changes.iter().enumerate() {
        let incomplete = |message: &str| {
            Problem::new(
                prefix.code("incomplete-plan"),
                "The update is not whole",
                &c.path,
                message,
                "Make the plan again with the new, promote, state, or index \
                command; no file was changed.",
            )
            .exit(4)
        };
        if plan.changes[..i].iter().any(|p| p.path == c.path) {
            return Err(incomplete("The plan changes the same file twice."));
        }
        if !is_safe_change_path(&c.path)
            || !same_snapshot(dir, &c.path, c.before.as_deref(), prefix)
        {
            return Err(Problem::new(
                prefix.code("changed"),
                "A file changed during preparation",
                &c.path,
                "The update no longer matches the original files.",
                "Review the changes to the file named, then run the command again.",
            )
            .exit(4));
        }
        let Some(after) = &c.after else { continue };
        if after.trim().is_empty() {
            return Err(incomplete("The plan would leave the file empty."));
        }
        if c.path.starts_with("records/") {
            // A record is checked as load would check it: by the catalog's own process.
            let r = read_record(&whole, &dir.join(&c.path), after)
                .map_err(|p| incomplete(&p.message))?;
            let single = Catalog { records: vec![r], ..whole.clone() };
            check_sources(&single).map_err(|p| incomplete(&p.message))?;
        } else if !is_balanced(after, prefix, c.path.ends_with(".rst")) {
            return Err(incomplete("The new index does not have one generated region."));
        }
    }
    Ok(())
}

/// Tells whether the file at `path` in `dir` has the contents `expected` (`None`: absent).
fn same_snapshot(dir: &Path, path: &str, expected: Option<&str>, prefix: &Prefix) -> bool {
    let full = dir.join(path);
    if !exists(&full) {
        return expected.is_none();
    }
    let Some(expected) = expected else { return false };
    read_text(prefix.as_str(), &full, MAX_INDEX_BYTES).is_ok_and(|text| text == expected)
}

/// Replaces the file at `path` with `text` atomically: the text is written and synced
/// to a stage beside it, which is then renamed over it. A failed stage is removed.
pub(crate) fn replace_file(ns: &str, path: &Path, text: &str) -> Result<()> {
    let folder = path.parent().filter(|f| !f.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let stage = Stage(folder.join(unique_name(".discussion-stage-")));
    let fail = |e: &std::io::Error| io_problem(ns, path, e);
    let mut file =
        OpenOptions::new().write(true).create_new(true).open(&stage.0).map_err(|e| fail(&e))?;
    file.write_all(text.as_bytes()).map_err(|e| fail(&e))?;
    file.sync_all().map_err(|e| fail(&e))?;
    drop(file);
    std::fs::rename(&stage.0, path).map_err(|e| fail(&e))
}

/// A staging file, removed when dropped unless it was renamed into place.
struct Stage(PathBuf);

impl Drop for Stage {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// A name no other stage of this or another process has: `lead`, the process id, and a
/// counter.
pub(crate) fn unique_name(lead: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    format!("{lead}{}-{n}-{nanos}", std::process::id())
}

/// The journal of `changes`, in the JSON form the Odin version wrote.
fn journal_text(changes: &[Change]) -> String {
    gd_json::to_string(&changes_json(changes))
}

/// `changes` as JSON: each path, its texts, and whether it existed and is present.
pub(crate) fn changes_json(changes: &[Change]) -> Value {
    let items = changes.iter().map(|c| {
        let mut o = Object::new();
        o.insert("path".into(), c.path.clone().into());
        o.insert("before".into(), c.before.clone().unwrap_or_default().into());
        o.insert("after".into(), c.after.clone().unwrap_or_default().into());
        o.insert("existed".into(), c.before.is_some().into());
        o.insert("present".into(), c.after.is_some().into());
        Value::Object(o)
    });
    Value::Array(items.collect())
}

/// Reads a journal written by [`journal_text`]; `None` when it is malformed.
fn read_journal(text: &str) -> Option<Vec<Change>> {
    let value = gd_json::parse(text).ok()?;
    let read = |item: &Value| -> Option<Change> {
        let text = |key: &str| item.get(key).and_then(Value::as_str).map(str::to_string);
        let flag = |key: &str| item.get(key).and_then(Value::as_bool);
        Some(Change {
            path: text("path")?,
            before: if flag("existed")? { Some(text("before")?) } else { None },
            after: if flag("present")? { Some(text("after")?) } else { None },
        })
    };
    value.as_array()?.iter().map(read).collect()
}

/// Checks that an interrupted update of the catalog in `dir` can be rolled back, without
/// changing anything (`recover --rollback --dry-run`). No journal is no work.
pub fn check_recovery(dir: &Path, prefix: &Prefix) -> Result<()> {
    load_journal(dir, prefix).map(|_| ())
}

/// Finishes an interrupted [`apply`] of the locked catalog: restores every file the
/// journal names to its original contents and removes the journal. No journal is no
/// work. A file edited since the interruption is a problem, and nothing is restored.
pub fn recover(lock: &Lock) -> Result<()> {
    let (dir, prefix) = (&lock.dir, &lock.prefix);
    let Some(changes) = load_journal(dir, prefix)? else { return Ok(()) };
    for c in changes.iter().rev() {
        if same_snapshot(dir, &c.path, c.before.as_deref(), prefix) {
            continue;
        }
        let path = dir.join(&c.path);
        match &c.before {
            Some(text) => replace_file(prefix.as_str(), &path, text)?,
            None => {
                std::fs::remove_file(&path).map_err(|e| io_problem(prefix.as_str(), &path, &e))?
            }
        }
    }
    let journal = journal_path(dir, prefix);
    std::fs::remove_file(&journal).map_err(|e| io_problem(prefix.as_str(), &journal, &e))
}

/// Reads and checks the journal: safe paths, each once, each file either as it was or as
/// the update would leave it.
fn load_journal(dir: &Path, prefix: &Prefix) -> Result<Option<Vec<Change>>> {
    let path = journal_path(dir, prefix);
    if !exists(&path) {
        return Ok(None);
    }
    let source = read_text(prefix.as_str(), &path, MAX_JOURNAL_BYTES)?;
    let Some(changes) = read_journal(&source) else {
        return Err(Problem::new(
            prefix.code("journal"),
            "Recovery data is malformed",
            shown(&path),
            "No files have been changed by recovery.",
            "Restore the journal from a backup.",
        )
        .exit(4));
    };
    for (i, c) in changes.iter().enumerate() {
        let repeated = changes[..i].iter().any(|p| p.path == c.path);
        if !is_safe_change_path(&c.path) || repeated {
            return Err(pending_problem(prefix, &shown(&path)));
        }
        if !same_snapshot(dir, &c.path, c.before.as_deref(), prefix)
            && !same_snapshot(dir, &c.path, c.after.as_deref(), prefix)
        {
            return Err(Problem::new(
                prefix.code("recovery-conflict"),
                "Recovery would overwrite a later edit",
                &c.path,
                "The file matches neither the original nor the staged content.",
                "Save your edit separately, restore one recorded version, then retry recovery.",
            )
            .exit(4));
        }
    }
    Ok(Some(changes))
}
