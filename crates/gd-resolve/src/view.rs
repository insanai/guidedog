//! `BuildView`: everything a builder reads once the documents are read and resolved,
//! borrowed for the writing stage; and `OutputBuilder`, the trait each builder crate
//! implements.

use std::path::PathBuf;

use gd_conf::Config;
use gd_env::{
    BuildOptions, BuilderKind, Digest, DocId, Environment, ObjectStore, PluginRun, UiText,
};
use gd_host::{Budget, Problem};
use gd_outputs::Generation;
use gd_report::Report;
use guidedoc::{Limits, Policy, Registry};

use crate::intersphinx::Intersphinx;
use crate::tables::Tables;

/// The build's dates, made once from `BuildOptions::now`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Dates {
    /// `|today|`: conf.toml's `today`, or the date in `today_fmt`.
    pub today: String,
    /// The date in `html_last_updated_fmt`; empty when it is unset.
    pub last_updated: String,
    /// The build's date as `YYYY-MM-DD`.
    pub date: String,
}

/// What every builder reads, borrowed from the build for the writing stage. It is
/// `Sync`: pages render on worker threads that share one view.
#[derive(Clone, Copy)]
pub struct BuildView<'b> {
    /// The build's own copy of the configuration (confined when untrusted).
    pub config: &'b Config,
    /// The build's options.
    pub options: &'b BuildOptions,
    /// The builder writing.
    pub kind: BuilderKind,
    /// What reading learned, every document sorted by docname.
    pub env: &'b Environment,
    /// The project tables.
    pub tables: &'b Tables,
    /// Other projects' inventories.
    pub intersphinx: &'b Intersphinx,
    /// The cached objects documents are loaded from.
    pub objects: &'b ObjectStore,
    /// The readers and renderers.
    pub registry: &'b Registry,
    /// What content the build lets through (raw, unsafe URLs, math).
    pub policy: &'b Policy,
    /// The limits every session reads and renders with.
    pub limits: &'b Limits,
    /// Guidedog's interface words in the build's language.
    pub ui: &'b dyn UiText,
    /// The build's dates.
    pub dates: &'b Dates,
    /// The build plugins (HTML hooks and assets).
    pub plugins: &'b PluginRun,
    /// The budget files and sessions are held within.
    pub budget: &'b Budget,
    /// The canonical folders the build may read (`gd_conf::readable_roots`).
    pub roots: &'b [PathBuf],
    /// `gd_env::output_settings`: what changes every output of this builder.
    pub settings: Digest,
}

impl std::fmt::Debug for BuildView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuildView")
            .field("kind", &self.kind)
            .field("documents", &self.env.docs.len())
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

/// A builder: writes one kind of output into a generation. gd-site implements it for
/// html, dirhtml and singlehtml, gd-book for pdf and text, gd-intl for gettext;
/// gd-project maps each `BuilderKind` to one in a single `match`.
pub trait OutputBuilder: Sync {
    /// The output it writes.
    fn kind(&self) -> BuilderKind;

    /// Writes every unit of the output into `generation`, reusing unchanged ones
    /// (`Generation::reuse`), adding what writing reports to `reports`. Fails with the
    /// problem that stops the build; pages that fail are counted in the generation.
    fn write(
        &self,
        view: &BuildView<'_>,
        generation: &mut Generation,
        reports: &mut Vec<Report>,
    ) -> Result<(), Problem>;
}

impl BuildView<'_> {
    /// The id of the document named `docname`.
    pub fn doc_named(&self, docname: &str) -> Option<DocId> {
        self.tables.doc_named(docname)
    }

    /// A document's output path below the HTML output folder: `<docname>.html`,
    /// `<docname>/index.html` in dirhtml, the root document's page in singlehtml.
    pub fn page_name(&self, docname: &str) -> String {
        let _ = docname;
        todo!("wave 1: port page_name")
    }

    /// The URL from one document's page to another's, with an optional anchor. From
    /// `None` it is the URL from a special page at the output root (genindex.html).
    pub fn page_url(&self, from: Option<DocId>, to: DocId, anchor: &str) -> String {
        let _ = (from, to, anchor);
        todo!("wave 1: port page_url")
    }

    /// The URL from a document's page (or the output root) to a generated page.
    pub fn generated_url(&self, from: Option<DocId>, file: &str) -> String {
        let _ = (from, file);
        todo!("wave 1: port generated_url")
    }

    /// The fragment that reaches an anchor of a document in this output: the anchor
    /// itself, or in singlehtml the document's key before it.
    pub fn page_anchor(&self, doc: DocId, anchor: &str) -> String {
        let _ = (doc, anchor);
        todo!("wave 1: port page_anchor")
    }

    /// A numbered item's number by its anchor (`"3"`, `"1.2"`); empty when it has none.
    pub fn item_number(&self, doc: DocId, anchor: &str) -> String {
        let _ = (doc, anchor);
        todo!("wave 1: port item_number")
    }
}

/// The shortest relative URL from one page to another (both below the output root,
/// `/` separators).
pub fn relative_url(from: &str, to: &str) -> String {
    let _ = (from, to);
    todo!("wave 1: port relative_url")
}

/// Joins a relative docname onto a folder and resolves `.` and `..`.
pub fn normalize_doc(dir: &str, path: &str) -> String {
    let _ = (dir, path);
    todo!("wave 1: port normalize_doc")
}

/// A docname written in a document in folder `base`, its `.rst`/`.md` suffix dropped;
/// `/name` is relative to the source folder.
pub fn doc_target(base: &str, target: &str) -> String {
    let _ = (base, target);
    todo!("wave 1: port doc_target")
}

/// A file path written in a document in folder `base`, its suffix kept.
pub fn file_target(base: &str, target: &str) -> String {
    let _ = (base, target);
    todo!("wave 1: port file_target")
}

/// The page a toctree names with `genindex`, `modindex` or `search` (Sphinx's virtual
/// documents), or `None` for any other name.
pub fn generated_page(docname: &str) -> Option<&'static str> {
    let _ = docname;
    todo!("wave 1: port generated_page")
}
