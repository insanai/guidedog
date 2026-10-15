//! What a book is made from (builders.odin's typst_options and book_inputs): Typst
//! runs only when one of these changed.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use gd_conf::Config;
use gd_env::{Digest, Digester};
use gd_resolve::BuildView;
use gd_typst::Backend;

use crate::compose::Book;

/// The Typst settings conf.toml gives a book.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct TypstSettings {
    /// `pdf_packages = "offline"`: use only packages already on disk.
    pub offline: bool,
    /// `pdf_fonts = "embedded"`: no system fonts, for reproducible PDFs.
    pub embedded_fonts_only: bool,
    /// `pdf_font_paths` (below the source folder), then the bundled fonts' folder.
    pub font_paths: Vec<PathBuf>,
}

impl TypstSettings {
    /// The settings of `config`, with the bundled fonts' folder (`bundled`) searched
    /// after the project's font folders.
    pub fn of(config: &Config, bundled: Option<PathBuf>) -> TypstSettings {
        let pdf = &config.pdf;
        let mut font_paths: Vec<PathBuf> =
            pdf.font_paths.iter().map(|f| config.source_dir.join(f)).collect();
        font_paths.extend(bundled);
        TypstSettings {
            offline: pdf.packages == "offline",
            embedded_fonts_only: pdf.fonts == "embedded",
            font_paths,
        }
    }

    /// The compile options: these settings, the folders the embedded compiler may read,
    /// and the budget's say on the PDF it hands back.
    pub fn options<'a>(
        &self,
        read_roots: Vec<PathBuf>,
        reserve: &'a dyn Fn(usize) -> bool,
    ) -> gd_typst::Options<'a> {
        gd_typst::Options {
            offline: self.offline,
            embedded_fonts_only: self.embedded_fonts_only,
            font_paths: self.font_paths.clone(),
            read_roots,
            reserve: Some(reserve),
            ..gd_typst::Options::default()
        }
    }
}

/// The digest of what a book is made from: the output settings, whether it is the only
/// book, the Typst settings, the compiler's version and kind, the composed source, and
/// for every file the source reads and the typst program its path, size and
/// modification time (their content reaches only Typst, so their stamps stand for it).
pub(crate) fn book_inputs(
    view: &BuildView<'_>,
    book: &Book,
    settings: &TypstSettings,
    backend: &Backend,
) -> Digest {
    let mut d = Digester::new();
    d.digest(&view.settings).flag(view.config.pdf.documents.len() == 1);
    d.text(&format!("{settings:?}"));
    d.text(backend.version().unwrap_or("")).flag(backend.is_embedded());
    d.text(&book.source);
    let program = match backend {
        Backend::Program { path, .. } => Some(path.as_path()),
        Backend::Embedded => None,
    };
    for path in book.files.iter().map(PathBuf::as_path).chain(program) {
        stamp_file(&mut d, path);
    }
    d.finish()
}

fn stamp_file(d: &mut Digester, path: &Path) {
    d.text(&path.display().to_string());
    match std::fs::metadata(path) {
        Ok(info) => {
            let nanos = info
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |t| t.as_nanos());
            d.text(&info.len().to_string()).text(&nanos.to_string());
        }
        Err(_) => {
            d.text("missing");
        }
    }
}
