//! Discussion catalogs: proposals and decisions kept as reStructuredText records.
//!
//! A catalog is a folder of records under a prefix such as `gds`, `sds`, or `eds`, one
//! file each, with metadata in a leading field list (`:eds-number: 0007`). This crate
//! loads a catalog ([`load_catalog`]), plans changes to it (a new draft, a promotion, a
//! lifecycle decision, the generated index) and applies a plan as one recoverable
//! transaction under the catalog's lock ([`lock_catalog`], [`apply`], [`recover`]).
//! Publication is a [`Builder`]'s work: [`GuidedogProgram`] runs an installed
//! `guidedog`; a program with Guidedog linked in supplies its own.
//!
//! ```no_run
//! use gd_discussion as ds;
//! # fn main() -> ds::Result<()> {
//! let prefix = ds::Prefix::new("eds")?;
//! let lock = ds::lock_catalog("docs/eds".as_ref(), &prefix)?; // released on drop
//! let catalog = ds::load_catalog("docs/eds".as_ref(), &prefix)?;
//! for r in &catalog.records {
//!     println!("{} {} {}", r.number, r.state, r.title);
//! }
//! let decision = ds::Decision {
//!     identifier: "0003".into(),
//!     to: "accepted".into(),
//!     reason: "Review converged".into(),
//!     ..ds::Decision::default()
//! };
//! let plan = ds::state_plan(&catalog, &decision)?; // changes nothing yet
//! ds::apply(&lock, &plan)?; // journaled: ds::recover rolls an interruption back
//! # Ok(()) }
//! ```
//!
//! Failure is a [`Problem`] with a hint and an exit status, never a panic.

mod build;
mod catalog;
// The C library's localtime_r for the local calendar date: FFI.
#[allow(unsafe_code)]
mod clock;
pub mod command;
mod config;
mod definition;
mod edit;
mod external;
mod index;
mod init;
mod metadata;
mod problem;
mod record_plan;
mod rst;
mod scan;
mod transaction;
mod typst;

#[cfg(test)]
mod tests;

pub use build::{Book, BuildOptions, BuildRequest, Builder, Target, artifact_name, build};
pub use catalog::{Catalog, Format, catalog_format, check_sources, load_catalog, resolve};
pub use clock::today;
pub use config::{Definition, Numbering, Process, State, read_config, render_config};
pub use definition::{definition, discover};
pub use edit::{Decision, NewDraft, new_plan, next_number, promote_plan, state_plan};
pub use external::{ENGINE_VARIABLE, GuidedogProgram};
pub use index::{check_index, index_plan};
pub use init::{Init, init_plan, initialize};
pub use metadata::{FieldValue, Record, Sections, read_record, replace_field};
pub use problem::{Problem, ProblemData, Result};
pub use transaction::{Change, Lock, Plan, apply, check_recovery, lock_catalog, recover};

use problem::{io_problem, prefix_problem, shown};
use std::fmt;
use std::path::Path;

/// The prefix of a catalog that names none: Guidedog's own discussions (GDS).
pub const DEFAULT_PREFIX: &str = "gds";

/// A catalog's configuration file: identity, process, and numbering.
pub const CONFIG_FILE: &str = "discussion.toml";

/// The identity file of a catalog made by version 0.2.1, before [`CONFIG_FILE`]; still
/// read, and such a catalog follows the default process.
pub const DEFINITION_FILE: &str = ".discussion.json";

/// The highest number any catalog may give a record.
pub const MAX_NUMBER: u32 = 999_999_999;

/// The number of a record that has none yet.
pub const DRAFT_NUMBER: &str = "XXXXX";

/// The labels of GDS 0001. A `gds` catalog uses them unless its configuration names
/// others; another catalog accepts any well-formed label unless it lists its own.
pub const LABELS: [&str; 14] = [
    "architecture",
    "parser",
    "renderer",
    "cli",
    "build",
    "theme",
    "typst",
    "library",
    "documentation",
    "process",
    "security",
    "compatibility",
    "performance",
    "release",
];

/// The kinds of record of the default process.
pub const TYPES: [&str; 3] = ["Standards Track", "Process", "Informational"];

/// The largest record; `load_catalog` reads only its metadata.
const MAX_RECORD_BYTES: u64 = 16 * 1024 * 1024;
/// The most entries a catalog's records folder may hold.
const MAX_RECORDS: usize = 100_000;
/// The generated index lists every record, about 150 bytes each.
const MAX_INDEX_BYTES: u64 = 64 * 1024 * 1024;
/// A journal holds the files of one update, before and after.
const MAX_JOURNAL_BYTES: u64 = 4 * MAX_INDEX_BYTES;

/// A catalog's prefix: 2 to 16 lowercase ASCII letters. It names the records' fields
/// (`:eds-number:`), the lock (`.eds.lock`), and problem codes (`eds.date`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Prefix(String);

impl Prefix {
    /// Checks `text` and makes it a prefix; anything else is `discussion.prefix`.
    pub fn new(text: &str) -> Result<Prefix> {
        if is_valid_prefix(text) {
            Ok(Prefix(text.to_string()))
        } else {
            Err(prefix_problem("", text))
        }
    }

    /// The prefix as text, borrowed.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// A problem code in this prefix's namespace: `"eds.date"` for `"date"`.
    pub fn code(&self, name: &str) -> String {
        format!("{}.{name}", self.0)
    }

    /// The prefix in capitals, as headings show it: `EDS`.
    pub fn upper(&self) -> String {
        self.0.to_ascii_uppercase()
    }
}

impl Default for Prefix {
    /// [`DEFAULT_PREFIX`].
    fn default() -> Prefix {
        Prefix(DEFAULT_PREFIX.to_string())
    }
}

impl fmt::Display for Prefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Tells whether `text` can name a catalog: 2 to 16 lowercase ASCII letters.
pub fn is_valid_prefix(text: &str) -> bool {
    (2..=16).contains(&text.len()) && text.bytes().all(|c| c.is_ascii_lowercase())
}

/// Reads the regular file at `path`, at most `limit` bytes. A link, a special file, or a
/// larger file is a problem in the namespace `ns`.
fn read_text(ns: &str, path: &Path, limit: u64) -> Result<String> {
    let info = std::fs::symlink_metadata(path).map_err(|e| io_problem(ns, path, &e))?;
    if !info.is_file() {
        return Err(Problem::new(
            format!("{ns}.file-type"),
            "Expected a regular file",
            shown(path),
            "Symbolic links and special files are not accepted.",
            "Use a regular local file.",
        ));
    }
    if info.len() > limit {
        return Err(too_large(ns, path, "The file exceeds the input budget."));
    }
    let text = std::fs::read_to_string(path).map_err(|e| io_problem(ns, path, &e))?;
    if text.len() as u64 > limit {
        return Err(too_large(ns, path, "The file changed while it was read."));
    }
    Ok(text)
}

fn too_large(ns: &str, path: &Path, message: &str) -> Problem {
    Problem::new(
        format!("{ns}.size"),
        "File is too large",
        shown(path),
        message,
        "Split the discussion into smaller records.",
    )
    .exit(3)
}

/// Tells whether anything, even a broken link, is at `path`.
fn exists(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

/// Tells whether `values` holds `value`.
fn contains<S: AsRef<str>>(values: &[S], value: &str) -> bool {
    values.iter().any(|v| v.as_ref() == value)
}
