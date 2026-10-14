//! gd-typst adapts the Typst toolchain for the host. The development backend runs an
//! installed, pinned `typst` program with an argument vector, never a shell; the embedded
//! backend (cargo feature `embedded`, on by default) links the compiler in.
//!
//! ```no_run
//! let backend = gd_typst::locate("typst").expect("Typst is installed or embedded");
//! if !backend.is_pinned() {
//!     eprintln!("expected Typst {}", gd_typst::PINNED_VERSION);
//! }
//! let options = gd_typst::Options { offline: true, ..Default::default() };
//! match gd_typst::compile_text(&backend, "= Hello\n", ".".as_ref(), "hello.pdf".as_ref(),
//!     &options)
//! {
//!     Ok(compiled) => compiled.diagnostics.iter().for_each(|w| eprintln!("{w}")),
//!     Err(gd_typst::Error::Compile { diagnostics, .. }) => {
//!         diagnostics.iter().for_each(|d| eprintln!("{d}"))
//!     }
//!     Err(error) => eprintln!("{error}"),
//! }
//! ```
//!
//! # Trust
//!
//! Typst is not a sandbox of its own; what it may do is what the caller gives it:
//! - Files: Typst reads only paths below the root it is given, and refuses a path that
//!   leaves it; a link below the root is followed. Fonts come from `font_paths` and,
//!   unless `embedded_fonts_only` is set, from the system.
//! - Packages: an `@preview` import downloads the package, unless `offline` is set (the
//!   embedded backend only; the typst program has no such switch).
//! - Memory and time: Typst's own, in the process (embedded) or in the typst program; no
//!   budget of the host bounds them. Only the output the embedded backend hands back is
//!   counted, through `reserve`.
//!
//! So Typst code reaches everything below its root: a caller that compiles Typst it does
//! not trust gives a root holding only what that code may read.

mod cli;
mod clock;
#[cfg(feature = "embedded")]
mod embedded;
mod error;

use std::fmt;
use std::path::{Path, PathBuf};

pub use error::{Diagnostic, Error, Severity};

/// The Typst release Guidedog is built and tested with.
pub const PINNED_VERSION: &str = "0.15.1";

/// Whether this build links the Typst compiler in (cargo feature `embedded`).
pub const EMBEDDED: bool = cfg!(feature = "embedded");

/// The virtual path Typst reports for generated text compiled by the embedded backend.
pub const GENERATED_MAIN: &str = "/<generated>.typ";

/// The artifact a compile writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Target {
    /// A PDF document.
    #[default]
    Pdf,
    /// An HTML page (Typst's experimental HTML export).
    Html,
}

/// The compiler a compile runs: the embedded one, or an installed typst program.
/// [`locate`] makes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Backend {
    /// The statically linked compiler; no external program runs.
    Embedded,
    /// An installed typst program.
    Program {
        /// The program's path.
        path: PathBuf,
        /// The version it reported; `None` when it could not tell.
        version: Option<String>,
    },
}

impl Backend {
    /// The compiler's version: [`PINNED_VERSION`] when embedded, the program's own
    /// otherwise; `None` when the program could not tell. Borrows from the backend.
    pub fn version(&self) -> Option<&str> {
        match self {
            Backend::Embedded => Some(PINNED_VERSION),
            Backend::Program { version, .. } => version.as_deref(),
        }
    }

    /// Reports whether this is the Typst release Guidedog is tested with.
    pub fn is_pinned(&self) -> bool {
        self.version() == Some(PINNED_VERSION)
    }

    /// Reports whether this is the embedded compiler.
    pub fn is_embedded(&self) -> bool {
        matches!(self, Backend::Embedded)
    }
}

/// Compile settings, mirroring the typst CLI. The default writes a PDF, downloads missing
/// packages into the system cache and uses system fonts before the embedded ones.
#[derive(Default)]
pub struct Options<'a> {
    /// The artifact to write.
    pub target: Target,
    /// Use only packages already on disk (honoured by the embedded backend only).
    pub offline: bool,
    /// The package cache directory; `None` is the system cache.
    pub package_cache: Option<PathBuf>,
    /// Ignore system fonts, for reproducible output.
    pub embedded_fonts_only: bool,
    /// Extra font directories, searched first.
    pub font_paths: Vec<PathBuf>,
    /// Asked with the size of the embedded compiler's output before it is written; `false`
    /// refuses the compile. The typst program writes its output itself and is not asked.
    pub reserve: Option<&'a dyn Fn(usize) -> bool>,
}

impl fmt::Debug for Options<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Options")
            .field("target", &self.target)
            .field("offline", &self.offline)
            .field("package_cache", &self.package_cache)
            .field("embedded_fonts_only", &self.embedded_fonts_only)
            .field("font_paths", &self.font_paths)
            .field("reserve", &self.reserve.is_some())
            .finish()
    }
}

/// What a successful compile reports besides the artifact it wrote.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Compiled {
    /// The warnings Typst reported.
    pub diagnostics: Vec<Diagnostic>,
    /// The compiler's output (or the diagnostics as lines), bounded.
    pub detail: String,
}

/// Looks `name` up on PATH, handling Windows' `;` and `.exe`; a name holding a path
/// separator is checked as given. Returns the program's path, or `None` when none is found.
pub fn find_program(name: &str) -> Option<PathBuf> {
    let suffix = if cfg!(windows) && !name.to_lowercase().ends_with(".exe") { ".exe" } else { "" };
    if name.contains('/') || name.contains('\\') {
        let path = PathBuf::from(name);
        if path.is_file() {
            return Some(path);
        }
        let path = PathBuf::from(format!("{name}{suffix}"));
        return path.is_file().then_some(path);
    }
    let file = format!("{name}{suffix}");
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(&file))
        .find(|path| path.is_file())
}

/// Returns the embedded compiler when this build links one and `program` is `""` or
/// `"typst"`; otherwise finds the program and asks for its version. `None` when no program
/// is found.
pub fn locate(program: &str) -> Option<Backend> {
    if EMBEDDED && (program.is_empty() || program == "typst") {
        return Some(Backend::Embedded);
    }
    let path = find_program(program)?;
    let output = std::process::Command::new(&path).arg("--version").output();
    let version = match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            let version = text.split_whitespace().nth(1).unwrap_or(text.trim());
            (!version.is_empty()).then(|| version.to_string())
        }
        _ => None,
    };
    Some(Backend::Program { path, version })
}

/// Compiles the Typst file `source` into `output`; paths resolve against `root`, and Typst
/// reads nothing outside it. Borrows everything for the call. Returns the warnings, or
/// an [`Error`]: [`Error::Compile`] with Typst's diagnostics when the source has errors.
/// Write to a staged path and rename it on success when a failed compile must not
/// disturb an earlier artifact.
pub fn compile_file(
    backend: &Backend,
    source: &Path,
    root: &Path,
    output: &Path,
    options: &Options<'_>,
) -> Result<Compiled, Error> {
    match backend {
        Backend::Embedded => {
            let Some(main) = rooted_path(root, source) else {
                return Err(Error::OutsideRoot {
                    source: source.to_path_buf(),
                    root: root.to_path_buf(),
                });
            };
            embedded_compile(root, Some(&main), "", output, options)
        }
        Backend::Program { path, .. } => {
            cli::run(path, cli::Input::File(source), root, output, options)
        }
    }
}

/// Compiles generated Typst `text`, so no file is written beside the user's sources;
/// relative paths in the text resolve against `root`. Fails like [`compile_file`].
pub fn compile_text(
    backend: &Backend,
    text: &str,
    root: &Path,
    output: &Path,
    options: &Options<'_>,
) -> Result<Compiled, Error> {
    match backend {
        Backend::Embedded => embedded_compile(root, None, text, output, options),
        Backend::Program { path, .. } => {
            cli::run(path, cli::Input::Text(text), root, output, options)
        }
    }
}

/// Converts a 1-based `line` and `column` (in characters) into a byte offset of `text`,
/// clamped to the line's (or text's) end. Allocates nothing.
pub fn offset_of(text: &str, line: usize, column: usize) -> usize {
    let Some(start) = gd_report::line_offset(text, line.max(1)) else { return text.len() };
    let (_, end) = gd_report::line_bounds(text, start);
    let mut chars = text[start..end].char_indices().map(|(at, _)| start + at);
    chars.nth(column.saturating_sub(1)).unwrap_or(end)
}

/// The root-relative Typst path (`/dir/file.typ`) of `source`, comparing absolute forms
/// when the given ones are not alike; `None` when it is outside `root`.
fn rooted_path(root: &Path, source: &Path) -> Option<String> {
    let relative = match source.strip_prefix(root) {
        Ok(relative) => relative.to_path_buf(),
        Err(_) => {
            let root = std::path::absolute(root).ok()?;
            let source = std::path::absolute(source).ok()?;
            source.strip_prefix(&root).ok()?.to_path_buf()
        }
    };
    let mut path = String::new();
    for part in relative.components() {
        match part {
            std::path::Component::Normal(name) => {
                path.push('/');
                path.push_str(name.to_str()?);
            }
            std::path::Component::CurDir => {}
            _ => return None,
        }
    }
    (!path.is_empty()).then_some(path)
}

#[cfg(feature = "embedded")]
fn embedded_compile(
    root: &Path,
    main: Option<&str>,
    text: &str,
    output: &Path,
    options: &Options<'_>,
) -> Result<Compiled, Error> {
    embedded::compile(root, main, text, output, options)
}

#[cfg(not(feature = "embedded"))]
fn embedded_compile(
    _root: &Path,
    _main: Option<&str>,
    _text: &str,
    _output: &Path,
    _options: &Options<'_>,
) -> Result<Compiled, Error> {
    Err(Error::NotEmbedded)
}

#[cfg(test)]
mod tests;
