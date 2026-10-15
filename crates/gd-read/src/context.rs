//! What reading borrows from the build, and what it gives back.

use gd_conf::{Config, Digest};
use gd_env::{Environment, ObjectStore, PluginRun};
use gd_host::Budget;
use gd_intl::Intl;
use guidedoc::{Limits, Policy, Registry};

/// Everything reading a project's documents needs, borrowed from the build. Plain data:
/// fill every field; nothing is changed while documents are read, so threads share it.
#[derive(Clone, Copy)]
pub struct ReadContext<'a> {
    /// The build's own copy of the configuration.
    pub config: &'a Config,
    /// The readers: `sphinx` reads reStructuredText and `myst` reads Markdown.
    pub registry: &'a Registry,
    /// Where documents' objects and search words are written.
    pub store: &'a ObjectStore,
    /// The translations documents are read with (`Intl::default()` translates none).
    pub intl: &'a Intl,
    /// The build plugins whose source hooks run before a document is parsed
    /// (`PluginRun::default()` runs none).
    pub plugins: &'a PluginRun,
    /// The budget sources and included files are read within.
    pub budget: &'a Budget,
    /// What content documents may hold.
    pub policy: Policy,
    /// The bounds of each document's read.
    pub limits: Limits,
    /// The build settings (`gd_env::build_settings`) the environment is saved with.
    pub settings: Digest,
    /// `|today|`, formatted (gd-intl's `Dates::today`).
    pub today: &'a str,
    /// The build's date as `YYYY-MM-DD`, for Docutils' `date` directive.
    pub date: &'a str,
    /// How many documents are read at once; 0 is one per processor.
    pub jobs: usize,
}

impl std::fmt::Debug for ReadContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadContext")
            .field("source_dir", &self.config.source_dir)
            .field("settings", &self.settings)
            .field("jobs", &self.jobs)
            .finish_non_exhaustive()
    }
}

impl ReadContext<'_> {
    /// The number of reading threads for `documents` documents: `jobs`, or one per
    /// processor, never more than there are documents, at least one.
    pub fn thread_count(&self, documents: usize) -> usize {
        let wanted = match self.jobs {
            0 => std::thread::available_parallelism().map_or(1, usize::from),
            n => n,
        };
        wanted.min(documents).max(1)
    }
}

/// What `read_documents` gives back: the new environment and what was done.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReadOutcome {
    /// Every document's `DocInfo`, sorted by docname, saved with the context's settings.
    pub env: Environment,
    /// Documents parsed this build.
    pub read: usize,
    /// Documents kept from the previous environment without parsing.
    pub reused: usize,
    /// Documents that could not be read (their `DocInfo::failed` is set).
    pub unread: usize,
    /// How long each document parsed this build took to read, in the order of the
    /// sources; documents kept from the previous environment took no time.
    pub reading_times: Vec<ReadingTime>,
}

/// How long one document took to read: its source, the parse, and what reading
/// collects, measured with a monotonic clock (sphinx.ext.duration).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReadingTime {
    /// The document.
    pub docname: String,
    /// The time it took.
    pub time: std::time::Duration,
}
