//! The embedded backend: the Typst compiler linked in. Project file reads are confined to
//! the given root. Packages resolve as in the typst CLI (data directory, cache, then a
//! Typst Universe download into the cache); each package reads only inside its own
//! directory, which is Typst's sandbox. An offline compile uses only packages on disk.
//! A panic inside the compiler is caught and reported as [`Error::Internal`].

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use typst::diag::{FileError, FileResult, PackageError, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World, WorldExt};
use typst_kit::downloader::SystemDownloader;
use typst_kit::fonts::FontStore;
use typst_kit::packages::{FsPackages, SystemPackages, UniversePackages};

use crate::clock::{Civil, compiler_clock};
use crate::{Compiled, Diagnostic, Error, GENERATED_MAIN, Options, Severity, Target};

const USER_AGENT: &str = concat!("guidedog/", env!("CARGO_PKG_VERSION"));

/// Compiles with the linked compiler and writes the artifact to `output`. `main` is the
/// root-relative path of the main file (`/book.typ`), or `None` to compile `text`.
pub(crate) fn compile(
    root: &Path,
    main: Option<&str>,
    text: &str,
    output: &Path,
    options: &Options<'_>,
) -> Result<Compiled, Error> {
    let world = make_world(root, main, text, options).ok_or(Error::InvalidRequest)?;
    let compiled = catch_unwind(AssertUnwindSafe(|| run(&world)));
    let Exported { artifact, pages, diagnostics } = compiled.map_err(|_| Error::Internal)?;
    let detail = detail_of(&diagnostics);
    let Some(artifact) = artifact else {
        return Err(Error::Compile { exit_code: None, diagnostics, detail });
    };
    if options.reserve.is_some_and(|reserve| !reserve(artifact.len())) {
        return Err(Error::OutputTooLarge { bytes: artifact.len() });
    }
    std::fs::write(output, &artifact)
        .map_err(|err| Error::Io { path: output.to_path_buf(), message: err.to_string() })?;
    Ok(Compiled { diagnostics, detail, pages })
}

fn detail_of(diagnostics: &[Diagnostic]) -> String {
    let size: usize = diagnostics.iter().map(|d| d.file.len() + d.message.len() + 12).sum();
    let mut detail = String::with_capacity(size);
    for d in diagnostics {
        if !detail.is_empty() {
            detail.push('\n');
        }
        detail.push_str(&d.file);
        detail.push_str(": ");
        detail.push_str(d.severity.as_str());
        detail.push_str(": ");
        detail.push_str(&d.message);
    }
    detail
}

/// What one compile and export produced.
struct Exported {
    /// The artifact, when the compile and export succeeded.
    artifact: Option<Vec<u8>>,
    /// Each page's text, for a paged document that compiled.
    pages: Option<Vec<String>>,
    /// Every warning and error, in the order Typst reported them.
    diagnostics: Vec<Diagnostic>,
}

/// Compiles and exports the world's main file.
fn run(world: &EmbeddedWorld) -> Exported {
    let mut found = Vec::new();
    let mut pages = None;
    let output = if world.html {
        let warned = typst::compile::<typst_html::HtmlDocument>(world);
        record(world, &warned.warnings, &mut found);
        warned.output.and_then(|doc| {
            typst_html::html(&doc, &typst_html::HtmlOptions::default()).map(String::into_bytes)
        })
    } else {
        let warned = typst::compile::<typst_layout::PagedDocument>(world);
        record(world, &warned.warnings, &mut found);
        warned.output.and_then(|doc| {
            pages = Some(doc.pages().iter().map(|page| page_text(&page.frame)).collect());
            typst_pdf::pdf(&doc, &typst_pdf::PdfOptions::default())
        })
    };
    match output {
        Ok(bytes) => Exported { artifact: Some(bytes), pages, diagnostics: found },
        Err(errors) => {
            record(world, &errors, &mut found);
            Exported { artifact: None, pages: None, diagnostics: found }
        }
    }
}

/// The text of a laid-out page: its text runs in layout order, a new line wherever a
/// run starts below or above the one before it, or in another group.
fn page_text(frame: &typst::layout::Frame) -> String {
    let mut text = String::new();
    collect_text(frame, &mut text);
    text
}

fn collect_text(frame: &typst::layout::Frame, text: &mut String) {
    let mut line = None;
    for (point, item) in frame.items() {
        match item {
            typst::layout::FrameItem::Group(group) => {
                end_line(text);
                collect_text(&group.frame, text);
                line = None;
            }
            typst::layout::FrameItem::Text(run) => {
                if line.is_some_and(|y| y != point.y) {
                    end_line(text);
                }
                line = Some(point.y);
                text.push_str(&run.text);
            }
            _ => {}
        }
    }
}

fn end_line(text: &mut String) {
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
}

/// Maps Typst's diagnostics, resolving each span to a file, line, column and byte range.
fn record(world: &EmbeddedWorld, diags: &[SourceDiagnostic], found: &mut Vec<Diagnostic>) {
    for d in diags {
        let severity = match d.severity {
            typst::diag::Severity::Error => Severity::Error,
            typst::diag::Severity::Warning => Severity::Warning,
        };
        let mut out = Diagnostic {
            severity,
            message: d.message.to_string(),
            hints: d.hints.iter().map(|hint| hint.v.to_string()).collect(),
            ..Diagnostic::default()
        };
        if let Some(id) = d.span.id() {
            out.file = match id.root() {
                VirtualRoot::Package(spec) => format!("{spec}{}", id.vpath().get_with_slash()),
                VirtualRoot::Project => id.vpath().get_with_slash().to_string(),
            };
            if let (Ok(source), Some(range)) = (world.source(id), world.range(d.span)) {
                let lines = source.lines();
                out.line = lines.byte_to_line(range.start).map_or(0, |l| l + 1);
                out.column = lines.byte_to_column(range.start).map_or(0, |c| c + 1);
                out.span = Some(range);
            }
        }
        out.stdin = out.file == GENERATED_MAIN;
        found.push(out);
    }
}

fn make_world(
    root: &Path,
    main: Option<&str>,
    text: &str,
    options: &Options<'_>,
) -> Option<EmbeddedWorld> {
    let vpath = VirtualPath::new(main.unwrap_or(GENERATED_MAIN)).ok()?;
    let main_id = FileId::new(RootedPath::new(VirtualRoot::Project, vpath));
    let clock = compiler_clock();
    let Civil { year, month, day, .. } = clock.local;
    let today = Datetime::from_ymd(year, month, day);
    let utc = clock.utc;
    let moment =
        Datetime::from_ymd_hms(utc.year, utc.month, utc.day, utc.hour, utc.minute, utc.second);
    Some(EmbeddedWorld {
        fonts: fonts(!options.embedded_fonts_only, &options.font_paths),
        html: options.target == Target::Html,
        download: !options.offline,
        package_cache: options.package_cache.clone(),
        package_roots: Mutex::new(HashMap::new()),
        root: root.to_path_buf(),
        // A root that cannot be resolved admits nothing (it is left out), never more.
        read_roots: (!options.read_roots.is_empty())
            .then(|| options.read_roots.iter().filter_map(|r| r.canonicalize().ok()).collect()),
        main: main_id,
        main_text: main.is_none().then(|| text.to_string()),
        today,
        moment,
        sources: Mutex::new(HashMap::new()),
    })
}

/// A panic while a cache lock is held must not disable every later compile in a
/// long-running process, so a poisoned lock is recovered; the caches stay consistent
/// because every critical section only inserts complete entries.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

type FontKey = (bool, Vec<PathBuf>);

/// Font stores are built once per configuration and kept for the process: scanning
/// system fonts is expensive.
fn fonts(system: bool, paths: &[PathBuf]) -> &'static FontStore {
    static STORES: OnceLock<Mutex<HashMap<FontKey, &'static FontStore>>> = OnceLock::new();
    let mut map = lock(STORES.get_or_init(|| Mutex::new(HashMap::new())));
    map.entry((system, paths.to_vec())).or_insert_with(|| {
        let mut store = FontStore::new();
        for dir in paths.iter().filter(|p| !p.as_os_str().is_empty()) {
            store.extend(typst_kit::fonts::scan(dir));
        }
        if system {
            store.extend(typst_kit::fonts::system());
        }
        store.extend(typst_kit::fonts::embedded());
        Box::leak(Box::new(store))
    })
}

fn library(html: bool) -> &'static LazyHash<Library> {
    static PAGED: OnceLock<LazyHash<Library>> = OnceLock::new();
    static HTML: OnceLock<LazyHash<Library>> = OnceLock::new();
    if html {
        HTML.get_or_init(|| {
            let features = [typst::Feature::Html].into_iter().collect();
            LazyHash::new(Library::builder().with_features(features).build())
        })
    } else {
        PAGED.get_or_init(|| LazyHash::new(Library::default()))
    }
}

/// Package storage, built once per cache directory and kept for the process.
fn packages(cache: Option<&Path>) -> &'static SystemPackages {
    static SYSTEM: OnceLock<SystemPackages> = OnceLock::new();
    static CUSTOM: OnceLock<Mutex<HashMap<PathBuf, &'static SystemPackages>>> = OnceLock::new();
    let universe = || UniversePackages::new(SystemDownloader::new(USER_AGENT));
    let Some(cache) = cache else {
        return SYSTEM.get_or_init(|| {
            SystemPackages::from_parts(
                FsPackages::system_data(),
                FsPackages::system_cache(),
                universe(),
            )
        });
    };
    let mut map = lock(CUSTOM.get_or_init(|| Mutex::new(HashMap::new())));
    map.entry(cache.to_path_buf()).or_insert_with(|| {
        let cache = Some(FsPackages::new(cache.to_path_buf()));
        let storage = SystemPackages::from_parts(FsPackages::system_data(), cache, universe());
        Box::leak(Box::new(storage))
    })
}

struct EmbeddedWorld {
    fonts: &'static FontStore,
    html: bool,
    download: bool,
    package_cache: Option<PathBuf>,
    package_roots: Mutex<HashMap<PackageSpec, PathBuf>>,
    root: PathBuf,
    /// Canonical folders project files must lie in; `None`: anywhere below the root.
    read_roots: Option<Vec<PathBuf>>,
    main: FileId,
    main_text: Option<String>,
    today: Option<Datetime>,
    moment: Option<Datetime>,
    sources: Mutex<HashMap<FileId, Source>>,
}

impl EmbeddedWorld {
    fn package_root(&self, spec: &PackageSpec) -> FileResult<PathBuf> {
        if let Some(dir) = lock(&self.package_roots).get(spec) {
            return Ok(dir.clone());
        }
        let storage = packages(self.package_cache.as_deref());
        let local = storage
            .data()
            .and_then(|d| d.obtain(spec))
            .or_else(|| storage.cache().and_then(|c| c.obtain(spec)));
        let root = match local {
            Some(root) => root,
            None if self.download => storage.obtain(spec).map_err(FileError::Package)?,
            None => return Err(FileError::Package(PackageError::NotFound(spec.clone()))),
        };
        let dir = root.path().to_path_buf();
        lock(&self.package_roots).insert(spec.clone(), dir.clone());
        Ok(dir)
    }

    /// Whether a project file lies in a read root once its links are followed; a file
    /// that does not exist is let through, so Typst reports it as not found.
    fn may_read(&self, path: &Path) -> bool {
        let Some(roots) = &self.read_roots else { return true };
        match path.canonicalize() {
            Ok(real) => roots.iter().any(|root| real.starts_with(root)),
            Err(_) => !path.exists(),
        }
    }

    fn read(&self, id: FileId) -> FileResult<Vec<u8>> {
        if id == self.main
            && let Some(text) = &self.main_text
        {
            return Ok(text.clone().into_bytes());
        }
        let base = match id.root() {
            VirtualRoot::Package(spec) => self.package_root(spec)?,
            VirtualRoot::Project => self.root.clone(),
        };
        let path = id.vpath().realize(&base).map_err(FileError::from)?;
        if matches!(id.root(), VirtualRoot::Project) && !self.may_read(&path) {
            return Err(FileError::AccessDenied);
        }
        std::fs::read(&path).map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => FileError::NotFound(path.clone()),
            std::io::ErrorKind::PermissionDenied => FileError::AccessDenied,
            _ => FileError::Other(None),
        })
    }
}

impl World for EmbeddedWorld {
    fn library(&self) -> &LazyHash<Library> {
        library(self.html)
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if let Some(s) = lock(&self.sources).get(&id) {
            return Ok(s.clone());
        }
        let text = String::from_utf8(self.read(id)?).map_err(|_| FileError::InvalidUtf8)?;
        let source = Source::new(id, text);
        lock(&self.sources).insert(id, source.clone());
        Ok(source)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        Ok(Bytes::new(self.read(id)?))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        match offset {
            None => self.today,
            Some(offset) => {
                let Datetime::Datetime(moment) = self.moment? else { return None };
                let shifted = moment.checked_add(offset.into())?;
                Some(Datetime::Date(shifted.date()))
            }
        }
    }
}
