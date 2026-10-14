//! gd-typst adapts the Typst toolchain for the host. The development backend runs an
//! installed, pinned `typst` program with an argument vector, never a shell; the embedded
//! backend (cargo feature `embedded`, on by default) links the compiler in.
//!
//! ```no_run
//! let backend = gd_typst::locate("typst").expect("Typst is installed or embedded");
//! if !gd_typst::is_pinned(&backend) {
//!     eprintln!("expected Typst {}", gd_typst::PINNED_VERSION);
//! }
//! let options = gd_typst::Options { offline: true, ..Default::default() };
//! let outcome = gd_typst::compile_text(&backend, "= Hello\n", ".".as_ref(),
//!     "hello.pdf".as_ref(), gd_typst::Target::Pdf, &options);
//! for d in &outcome.diagnostics {
//!     eprintln!("{d}");
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

/// The compiler a compile runs: an installed typst program, or the embedded one.
/// [`locate`] makes it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Backend {
    /// The program's path, or `"embedded"`.
    pub program: String,
    /// The compiler's version, `""` when the program could not tell it.
    pub version: String,
    /// The statically linked compiler; no external program runs.
    pub embedded: bool,
}

/// Compile settings, mirroring the typst CLI. The default downloads missing packages into
/// the system cache and uses system fonts before the embedded ones.
#[derive(Default)]
pub struct Options<'a> {
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
            .field("offline", &self.offline)
            .field("package_cache", &self.package_cache)
            .field("embedded_fonts_only", &self.embedded_fonts_only)
            .field("font_paths", &self.font_paths)
            .field("reserve", &self.reserve.is_some())
            .finish()
    }
}

/// What a compile did. `error` is set when Typst could not run or its output could not be
/// written; `diagnostics` hold what Typst itself reported.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Outcome {
    /// The artifact was written.
    pub ok: bool,
    /// The program's exit code; for the embedded backend 0 ok, 1 compile error, 2 rejected
    /// request, 3 internal failure, 4 no embedded compiler, 5 refused by `reserve`.
    pub exit_code: i32,
    /// The problems Typst reported, errors and warnings.
    pub diagnostics: Vec<Diagnostic>,
    /// The compiler's output (or the diagnostics as lines), bounded.
    pub detail: String,
    /// Why Typst did not run or its artifact was not written.
    pub error: Option<Error>,
}

impl Outcome {
    fn failed(error: Error) -> Outcome {
        Outcome { error: Some(error), ..Outcome::default() }
    }
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
/// is found; a program that cannot tell its version has version `""`.
pub fn locate(program: &str) -> Option<Backend> {
    if EMBEDDED && (program.is_empty() || program == "typst") {
        let version = PINNED_VERSION.to_string();
        return Some(Backend { program: "embedded".into(), version, embedded: true });
    }
    let path = find_program(program)?;
    let program = path.to_string_lossy().into_owned();
    let output = std::process::Command::new(&path).arg("--version").output();
    let version = match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            let version = text.split_whitespace().nth(1).unwrap_or(text.trim());
            version.to_string()
        }
        _ => String::new(),
    };
    Some(Backend { program, version, embedded: false })
}

/// Reports whether `backend` is the Typst release Guidedog is tested with.
pub fn is_pinned(backend: &Backend) -> bool {
    backend.version == PINNED_VERSION
}

/// Compiles the Typst file `source` into `output`; paths resolve against `root`, and Typst
/// reads nothing outside it. Borrows everything for the call. Failures are in the
/// returned [`Outcome`]. Write to a staged path and rename it on success when a failed
/// compile must not disturb an earlier artifact.
pub fn compile_file(
    backend: &Backend,
    source: &Path,
    root: &Path,
    output: &Path,
    target: Target,
    options: &Options<'_>,
) -> Outcome {
    if backend.embedded {
        let Some(main) = rooted_path(root, source) else {
            return Outcome::failed(Error::OutsideRoot {
                source: source.to_path_buf(),
                root: root.to_path_buf(),
            });
        };
        return embedded_compile(root, Some(&main), "", output, target, options);
    }
    let input = cli::Input::File(source);
    cli::run(backend, input, root, output, target, options)
}

/// Compiles generated Typst `text`, so no file is written beside the user's sources;
/// relative paths in the text resolve against `root`. Failures are in the [`Outcome`].
pub fn compile_text(
    backend: &Backend,
    text: &str,
    root: &Path,
    output: &Path,
    target: Target,
    options: &Options<'_>,
) -> Outcome {
    if backend.embedded {
        return embedded_compile(root, None, text, output, target, options);
    }
    cli::run(backend, cli::Input::Text(text), root, output, target, options)
}

/// Converts a 1-based `line` and `column` (in characters) into a byte offset of `text`,
/// clamped to the line's (or text's) end. Allocates nothing.
pub fn offset_of(text: &str, line: usize, column: usize) -> usize {
    let bytes = text.as_bytes();
    let (mut current, mut i) = (1, 0);
    while i < bytes.len() && current < line {
        if bytes[i] == b'\n' {
            current += 1;
        }
        i += 1;
    }
    for (col, (at, ch)) in (1..).zip(text[i..].char_indices()) {
        if col >= column || ch == '\n' {
            return i + at;
        }
    }
    text.len()
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
    target: Target,
    options: &Options<'_>,
) -> Outcome {
    embedded::compile(root, main, text, output, target, options)
}

#[cfg(not(feature = "embedded"))]
fn embedded_compile(
    _root: &Path,
    _main: Option<&str>,
    _text: &str,
    _output: &Path,
    _target: Target,
    _options: &Options<'_>,
) -> Outcome {
    Outcome { exit_code: 4, ..Outcome::failed(Error::NotEmbedded) }
}

#[cfg(test)]
mod tests;
