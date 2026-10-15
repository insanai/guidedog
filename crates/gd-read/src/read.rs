//! Bringing the environment up to date (read.odin): documents that are new, changed, or
//! whose dependencies changed are parsed into objects and their symbols collected; the
//! rest keep what the previous build recorded. Documents that no longer exist drop out.
//!
//! Each source is read once, by the thread that handles it: its digest decides whether
//! the previous reading holds, and only then is the text parsed. Threads take documents
//! in turn, each with its own workspace; results are placed by the source's position,
//! so the environment is the same however the threads were scheduled.

use gd_env::{DocInfo, Environment, SourceFile};
use gd_host::Problem;
use gd_report::{Report, Severity};

use crate::context::{ReadContext, ReadOutcome};
use crate::parse::{Prepared, Slot, Worker};

/// Reads `sources` (sorted by docname, as `gd_env::discover` and
/// `gd_env::with_generated` give them) into a new environment, reusing what `previous`
/// recorded for documents that did not change. `previous` must have been saved with
/// `ctx.settings` (`gd_env::load_environment` checks it); one of other settings counts
/// as empty. Documents that fail are in the environment with `failed` set and their
/// reports. Fails only with `build.reader`, when the registry lacks a reader the
/// documents need.
pub fn read_documents(
    ctx: &ReadContext<'_>,
    previous: Environment,
    sources: &[SourceFile],
) -> Result<ReadOutcome, Problem> {
    let prepared = Prepared::new(ctx, sources)?;
    let mut previous =
        if previous.settings == ctx.settings { previous } else { Environment::new(ctx.settings) };
    if !sources.is_empty() {
        // Made before any thread writes an object, so threads never race to make it; a
        // failure shows when the first object is written.
        let _ = ctx.store.prepare();
    }
    let slots = read_all(ctx, &prepared, &previous, sources);
    let mut outcome = ReadOutcome { env: Environment::new(ctx.settings), ..Default::default() };
    for slot in slots {
        let doc = match slot {
            Slot::Reused(i) => {
                outcome.reused += 1;
                // This build wrote nothing of it: the commit has nothing to flush.
                DocInfo { written: false, ..std::mem::take(&mut previous.docs[i]) }
            }
            Slot::Read(doc) if doc.failed => {
                outcome.unread += 1;
                *doc
            }
            Slot::Read(doc) => {
                outcome.read += 1;
                *doc
            }
        };
        outcome.env.docs.push(doc);
    }
    // Sorted already when the sources are; a stable sort keeps the order otherwise.
    outcome.env.docs.sort_by(|a, b| a.docname.cmp(&b.docname));
    Ok(outcome)
}

/// Reads every source on `ctx.thread_count` threads and returns each one's slot, in the
/// order of `sources`.
fn read_all(
    ctx: &ReadContext<'_>,
    prepared: &Prepared,
    previous: &Environment,
    sources: &[SourceFile],
) -> Vec<Slot> {
    let threads = ctx.thread_count(sources.len());
    let init = || {
        // The thread waits for memory the other reading threads hold rather than exceed
        // the budget (gd_host::Budget). The worker is dropped before the membership.
        let member = ctx.budget.join();
        (Worker::new(ctx, prepared), member)
    };
    gd_host::map_ordered(threads, sources, init, |(worker, _), source| {
        worker.read(source, previous)
    })
}

/// Reads a document again from `source` because its cached object is stale (missing,
/// damaged, or another document's), and returns its new reading, whose object is
/// written. What the document declares was resolved from the first reading, so the
/// source must still be the one read then: a source that changed since (another
/// digest), or a reading that fails, is `build.object.stale`, with what the new reading
/// reported in its message. Report [`stale_warning`] before calling it.
pub fn reread_document(
    ctx: &ReadContext<'_>,
    source: &SourceFile,
    doc: &DocInfo,
) -> Result<DocInfo, Problem> {
    let prepared = Prepared::new(ctx, std::slice::from_ref(source))?;
    let mut worker = Worker::new(ctx, &prepared);
    // An empty previous environment names no document, so nothing is reused.
    let Slot::Read(fresh) = worker.read(source, &Environment::default()) else {
        unreachable!("a document was reused from an empty environment")
    };
    if !fresh.failed && fresh.digest == doc.digest {
        return Ok(*fresh);
    }
    let errors: Vec<&str> = fresh
        .reports
        .iter()
        .filter(|r| r.severity == Severity::Error)
        .map(|r| r.message.as_str())
        .collect();
    let why = if fresh.digest != doc.digest && !fresh.failed {
        "The document changed since this build read it.".to_owned()
    } else {
        format!("Reading it again failed: {}", errors.join(" "))
    };
    Err(Problem::new(
        "build.object.stale",
        "CACHED DOCUMENT CANNOT BE REPLACED",
        &doc.relative,
        format!("The cached object of {} is unusable. {why}", doc.relative),
        "Build again; a document that keeps failing is reported with its own errors.",
        5,
    ))
}

/// The warning that a document's cached object was unusable (`stale`, from
/// `gd_env::ObjectStore::load_object`) and the document is read again.
pub fn stale_warning(stale: &Problem, doc: &DocInfo) -> Report {
    let message = format!(
        "{}: {} The document was read again from {}, and its cache rewritten.",
        stale.title, stale.message, doc.relative
    );
    Report {
        path: stale.path.clone(),
        ..Report::warning("build.object.stale", "CACHED DOCUMENT READ AGAIN", &message)
            .hint("Nothing to do. If it happens again, check the disk the build folder is on.")
    }
}
