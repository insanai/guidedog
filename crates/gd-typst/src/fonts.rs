//! Build-owned font handles share collection bytes and load only selected fallback faces.

use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use crate::Options;

const MAX_FONT_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone)]
pub(crate) enum Data {
    #[cfg(feature = "embedded")]
    Embedded(typst::foundations::Bytes),
    File(Arc<FontFile>),
}

pub(crate) struct FontFile {
    pub(crate) path: PathBuf,
    bytes: OnceLock<Result<Arc<[u8]>, FontError>>,
}

impl FontFile {
    pub(crate) fn new(path: PathBuf) -> Arc<Self> {
        Arc::new(Self { path, bytes: OnceLock::new() })
    }

    pub(crate) fn data(&self) -> Result<&[u8], FontError> {
        self.bytes
            .get_or_init(|| read_font(&self.path))
            .as_ref()
            .map(AsRef::as_ref)
            .map_err(Clone::clone)
    }
}

/// A text font could not be read within its regular-file and size bounds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontError(pub String);

impl fmt::Display for FontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FontError {}

/// An owned lazy font handle whose original bytes can be borrowed by a shaper.
#[derive(Clone)]
pub struct TextFace {
    /// Typographic family name.
    pub family: String,
    /// Face index within a TrueType collection, or zero for an ordinary font.
    pub index: u32,
    /// Whether this face is bold.
    pub bold: bool,
    /// Whether this face is italic or oblique.
    pub italic: bool,
    pub(crate) data: Data,
    #[cfg(feature = "embedded")]
    pub(crate) coverage: Option<typst::text::Coverage>,
    #[cfg(not(feature = "embedded"))]
    pub(crate) coverage: Vec<(u32, u32)>,
}

impl fmt::Debug for TextFace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextFace")
            .field("family", &self.family)
            .field("index", &self.index)
            .field("bold", &self.bold)
            .field("italic", &self.italic)
            .field("path", &self.path())
            .finish()
    }
}

impl TextFace {
    /// Borrows the original bytes, loading a selected file at most once per build.
    /// Fails if a file is unreadable, changes size during reading or exceeds 256 MiB.
    pub fn data(&self) -> Result<&[u8], FontError> {
        match &self.data {
            #[cfg(feature = "embedded")]
            Data::Embedded(bytes) => Ok(bytes),
            Data::File(file) => file.data(),
        }
    }

    /// Borrows the file dependency, or None for an embedded face. Does not read it.
    pub fn path(&self) -> Option<&Path> {
        match &self.data {
            #[cfg(feature = "embedded")]
            Data::Embedded(_) => None,
            Data::File(file) => Some(&file.path),
        }
    }

    /// Tests discovered Unicode coverage without loading bytes when metadata is available.
    /// A true result is provisional; shaping still checks the selected OpenType face.
    pub fn covers(&self, character: char) -> bool {
        #[cfg(feature = "embedded")]
        if let Some(coverage) = &self.coverage {
            return coverage.contains(character as u32);
        }
        #[cfg(not(feature = "embedded"))]
        return self
            .coverage
            .iter()
            .any(|&(first, last)| (first..=last).contains(&(character as u32)));
        #[cfg(feature = "embedded")]
        true
    }
}

/// A build-owned set of lazy handles in fallback order, with the PDF text family first.
#[derive(Clone, Debug, Default)]
pub struct TextFonts {
    /// Explicit and system faces allowed by policy, plus embedded faces when available.
    pub faces: Vec<TextFace>,
}

/// Discovers fonts using pdf_font_paths and embedded_fonts_only from options.
/// Borrows options only for this call. File bytes stay unloaded until a face is selected.
/// Unreadable metadata is skipped as in the compiler's font discovery.
pub fn load_text_fonts(options: &Options<'_>) -> TextFonts {
    #[cfg(feature = "embedded")]
    let mut faces = crate::embedded::text_fonts(options);
    #[cfg(not(feature = "embedded"))]
    let mut faces = crate::font_scan::text_fonts(options);
    faces.sort_by_key(|f| !f.family.eq_ignore_ascii_case("Libertinus Serif"));
    TextFonts { faces }
}

pub(crate) fn read_font(path: &Path) -> Result<Arc<[u8]>, FontError> {
    let read = || -> std::io::Result<_> {
        let mut file = std::fs::File::open(path)?;
        let info = file.metadata()?;
        if !info.is_file() || info.len() > MAX_FONT_BYTES {
            return Err(std::io::Error::other("Expected a regular font file below 256 MiB."));
        }
        let size = info.len() as usize;
        let mut data = Vec::new();
        data.try_reserve_exact(size).map_err(std::io::Error::other)?;
        (&mut file).take(info.len()).read_to_end(&mut data)?;
        let mut probe = [0];
        if data.len() != size || file.read(&mut probe)? != 0 {
            return Err(std::io::Error::other("The font changed size during reading."));
        }
        Ok(Arc::from(data))
    };
    read().map_err(|e| FontError(format!("Cannot read font {}: {e}", path.display())))
}
