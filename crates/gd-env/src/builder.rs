//! The vocabulary of a build: which builder, its options, how far it got, and what it
//! did.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use gd_host::{OnRefusal, Problem};
use gd_report::Report;

/// The output a build writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BuilderKind {
    /// One HTML page per document.
    #[default]
    Html,
    /// One `index.html` per document folder, for pretty URLs.
    Dirhtml,
    /// Every document on one HTML page.
    Singlehtml,
    /// PDF books, typeset with Typst.
    Pdf,
    /// Plain text, one file per document.
    Text,
    /// Message catalog templates (.pot).
    Gettext,
    /// Reads and checks, writes nothing.
    Dummy,
}

impl BuilderKind {
    /// Every builder, in the order `guidedog formats` lists them.
    pub const ALL: [BuilderKind; 7] = [
        BuilderKind::Html,
        BuilderKind::Dirhtml,
        BuilderKind::Singlehtml,
        BuilderKind::Pdf,
        BuilderKind::Text,
        BuilderKind::Gettext,
        BuilderKind::Dummy,
    ];

    /// The name the command line gives it, and its output folder's: `"html"`, ...
    pub fn name(self) -> &'static str {
        match self {
            BuilderKind::Html => "html",
            BuilderKind::Dirhtml => "dirhtml",
            BuilderKind::Singlehtml => "singlehtml",
            BuilderKind::Pdf => "pdf",
            BuilderKind::Text => "text",
            BuilderKind::Gettext => "gettext",
            BuilderKind::Dummy => "dummy",
        }
    }

    /// Reads a builder name: Guidedog's own, and the Sphinx LaTeX builders (`latex`,
    /// `latexpdf`, `latexpdfja`, `xelatexpdf`), which become `Pdf` because Guidedog
    /// typesets PDF with Typst. `None` for any other name.
    pub fn from_name(name: &str) -> Option<BuilderKind> {
        if let Some(kind) = BuilderKind::ALL.into_iter().find(|k| k.name() == name) {
            return Some(kind);
        }
        match name {
            "latex" | "latexpdf" | "latexpdfja" | "xelatexpdf" => Some(BuilderKind::Pdf),
            _ => None,
        }
    }

    /// Whether it writes HTML pages (html, dirhtml, singlehtml): the builders HTML
    /// plugin hooks and assets run for.
    pub fn is_html(self) -> bool {
        matches!(self, BuilderKind::Html | BuilderKind::Dirhtml | BuilderKind::Singlehtml)
    }

    /// The format its documents are rendered in.
    pub fn format(self) -> OutputFormat {
        match self {
            BuilderKind::Html | BuilderKind::Dirhtml | BuilderKind::Singlehtml => {
                OutputFormat::Html
            }
            BuilderKind::Pdf => OutputFormat::Pdf,
            BuilderKind::Text | BuilderKind::Gettext | BuilderKind::Dummy => OutputFormat::Text,
        }
    }
}

impl fmt::Display for BuilderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The format one document is linked and rendered for: links are URLs in HTML and
/// text, and project anchors in the book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum OutputFormat {
    /// An HTML page.
    #[default]
    Html,
    /// A chapter of a PDF book.
    Pdf,
    /// A plain-text file.
    Text,
}

/// How many threads read documents (`-j`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Jobs {
    /// This many threads; 0 and 1 both read on the calling thread.
    Count(usize),
    /// One thread per processor core (`-j auto`).
    PerCore,
}

impl Default for Jobs {
    fn default() -> Jobs {
        Jobs::Count(1)
    }
}

impl Jobs {
    /// The number of threads, at least 1, at most `work` (no idle threads).
    pub fn threads(self, work: usize) -> usize {
        let n = match self {
            Jobs::Count(n) => n,
            Jobs::PerCore => std::thread::available_parallelism().map_or(1, |n| n.get()),
        };
        n.clamp(1, work.max(1))
    }
}

/// One build's command-line options. Fill it with struct-update syntax:
/// `BuildOptions { fresh: true, ..Default::default() }`.
#[derive(Clone, Default)]
pub struct BuildOptions {
    /// `-a`: write every output, not only outdated ones.
    pub all: bool,
    /// `-E`: ignore the saved environment and read every document.
    pub fresh: bool,
    /// `-n`: warn about every unresolved reference.
    pub nitpicky: bool,
    /// `-W`: a build with warnings fails and publishes nothing.
    pub warnings_as_errors: bool,
    /// `--keep-going`: accepted, as sphinx-build's; Guidedog always reports everything.
    pub keep_going: bool,
    /// `--untrusted`: build a project from elsewhere (see `gd_conf::confine_untrusted`).
    pub untrusted: bool,
    /// `-j`: reading threads.
    pub jobs: Jobs,
    /// `--budget`: the bytes the build holds at once; `None`: [`BUILD_BUDGET`].
    pub budget: Option<usize>,
    /// What to do when the budget is reached (a terminal build asks); `None`: stop.
    pub on_budget: Option<Arc<OnRefusal>>,
    /// `--max-depth`: how deep content may nest; `None`: the engine's default.
    pub max_depth: Option<usize>,
    /// `--max-nodes`: nodes per document; `None`: the engine's default.
    pub max_nodes: Option<usize>,
    /// `--stack-kib`, in bytes: the stack budget; `None`: the default for the threads.
    pub stack_bytes: Option<usize>,
    /// `-t`: tags the `only` directive tests.
    pub tags: Vec<String>,
    /// The sphinx-build form's output folder; `None`: `<build_dir>/<builder>`.
    pub output_dir: Option<PathBuf>,
    /// The build's moment (`|today|`, copyright years); `None`: now.
    pub now: Option<SystemTime>,
}

impl fmt::Debug for BuildOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuildOptions")
            .field("all", &self.all)
            .field("fresh", &self.fresh)
            .field("nitpicky", &self.nitpicky)
            .field("warnings_as_errors", &self.warnings_as_errors)
            .field("untrusted", &self.untrusted)
            .field("jobs", &self.jobs)
            .field("budget", &self.budget)
            .field("on_budget", &self.on_budget.is_some())
            .field("tags", &self.tags)
            .field("output_dir", &self.output_dir)
            .finish_non_exhaustive()
    }
}

/// The bytes a build holds at once by default (1 GiB): its sessions and the files it
/// reads. Reading threads wait rather than exceed it (`gd_host::Budget`).
pub const BUILD_BUDGET: usize = 1 << 30;

/// How far a build got, so its summary says what it did and no more.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum BuildStage {
    /// Settings, the documents to read, the readers: no document read yet.
    #[default]
    Preparing,
    /// Reading the documents and resolving their references.
    Reading,
    /// Writing the outputs beside the output folder.
    Writing,
    /// Every output written; publishing them is what is left.
    Written,
}

/// What a build did: its problem when it failed, the reports of the whole build either
/// way, and its counters.
#[derive(Debug, Clone, Default)]
pub struct BuildResult {
    /// The exit status: 0, or the problem's.
    pub exit: i32,
    /// Why the build failed; `None` when it succeeded.
    pub problem: Option<Problem>,
    /// Every report of the build, `suppress_warnings` applied.
    pub reports: Vec<Report>,
    /// Documents parsed.
    pub read: usize,
    /// Documents that could not be read.
    pub unread: usize,
    /// Documents taken from the environment.
    pub reused: usize,
    /// Outputs written: published, or only prepared when not published.
    pub written: usize,
    /// Outputs already up to date.
    pub unchanged: usize,
    /// The output folder.
    pub output: PathBuf,
    /// How far the build got.
    pub reached: BuildStage,
    /// The outputs are in place; otherwise the previous ones stay, unless `unsettled`.
    pub published: bool,
    /// A publication neither finished nor undone, which the next build settles.
    pub unsettled: bool,
    /// The most memory the budget held at once, in bytes.
    pub peak: usize,
    /// The budget at the end, in bytes: its policy may have raised it.
    pub budget: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builders_are_named_as_sphinx_names_them() {
        for kind in BuilderKind::ALL {
            assert_eq!(BuilderKind::from_name(kind.name()), Some(kind));
        }
        assert_eq!(BuilderKind::from_name("latexpdf"), Some(BuilderKind::Pdf));
        assert_eq!(BuilderKind::from_name("epub"), None);
        assert!(BuilderKind::Singlehtml.is_html() && !BuilderKind::Pdf.is_html());
        assert_eq!(Jobs::Count(0).threads(5), 1);
        assert_eq!(Jobs::Count(8).threads(3), 3);
    }
}
