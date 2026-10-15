//! gd-host: where the Guidedoc engine meets the operating system. Sessions that convert
//! with Ctrl+C cancellation; sources read within limits (UTF-8 checked, byte order mark
//! dropped); included files confined to approved roots; atomic publication; engine
//! diagnostics as `gd_report::Report`s; a coarse memory budget; worker threads with
//! known stacks; a crash reporter; and `convert_artifact`, the route `guidedog convert`
//! takes from a file to a published HTML or PDF artifact.
//!
//! ```no_run
//! use gd_host::{Session, describe_all};
//! let registry = guidedoc::Registry::default(); // guidedoc_defaults::registry() in a program
//! let mut session = Session::new();
//! let request = guidedoc::default_request("hello.rst", "Hello *world*.\n", "rst", "html");
//! let result = session.run(&registry, &request).map(str::to_owned);
//! for report in session.reports() {
//!     eprint!("{}", gd_report::render_text(&report, &gd_report::Style::default()));
//! }
//! if let Ok(html) = result {
//!     println!("{html}");
//! }
//! # let _ = describe_all;
//! ```
//!
//! Failures are `Problem`s (code, title, message, hint, exit status) or engine `Fail`s
//! described as reports; nothing here prints except `catch_crash`, the last resort.

#[cfg(feature = "artifact")]
mod artifact;
mod budget;
mod confine;
mod crash;
mod diagnostics;
mod files;
mod folders;
mod interrupt;
mod problem;
mod publish;
mod session;
// The Ctrl+C handler and the atomic path swap: the crate's only FFI.
#[allow(unsafe_code)]
mod sys;
mod workers;

#[cfg(feature = "artifact")]
pub use artifact::{
    ArtifactRequest, ArtifactResult, Delivered, Format, Target, backend_problem, convert_artifact,
};
pub use budget::{
    AVAILABLE_OVERRIDE, Budget, DEFAULT_BUDGET, Ledger, OnRefusal, Refusal, RefusalChoice, Unit,
    available_memory, budget_advice, budget_hint, budget_problem, fitting_budget, limits_within,
    suggest_budget,
};
pub use confine::{Confinement, Files, canonical_roots, confined, confinement, containing_root};
pub use crash::{CRASH_EXIT, ISSUE_TRACKER, catch_crash, crash_report, install_crash_reporter};
pub use diagnostics::{describe, describe_all, display_path, exit_code};
pub use files::{
    MAX_SOURCE_BYTES, SourceFile, each_chunk, read_file, read_source, read_stream, same_contents,
};
pub use folders::{
    FolderEntry, Identity, PathKind, exchange, identity, link_file, list_folder, path_type,
    remove_path, remove_tree,
};
pub use interrupt::{
    cancel_probe, catch_interrupts, clear_interrupt, request_interrupt, was_interrupted,
};
pub use problem::{Problem, interrupted_problem, io_problem};
pub use publish::{
    STAGE_PREFIX, Stage, folders_of, is_stage_name, publish, remove_stages, sync_all, sync_each,
    write_unsynced,
};
pub use session::Session;
pub use workers::{
    WORKER_STACK_BYTES, default_stack_budget, run_with_stack, run_workers, stack_budget_limit,
};

#[cfg(test)]
mod tests;
