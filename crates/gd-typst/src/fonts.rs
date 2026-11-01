//! Font handles selected by the compiler's existing policy, without copying font bytes.

use std::fmt;

use crate::Options;

#[derive(Clone)]
pub(crate) enum Data {
    #[cfg(feature = "embedded")]
    Embedded(typst::foundations::Bytes),
    #[cfg(not(feature = "embedded"))]
    File(std::sync::Arc<[u8]>),
}

impl AsRef<[u8]> for Data {
    fn as_ref(&self) -> &[u8] {
        match self {
            #[cfg(feature = "embedded")]
            Self::Embedded(bytes) => bytes,
            #[cfg(not(feature = "embedded"))]
            Self::File(bytes) => bytes,
        }
    }
}

/// An owned font handle whose bytes can be borrowed by an independent shaper.
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
}

impl fmt::Debug for TextFace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextFace")
            .field("family", &self.family)
            .field("index", &self.index)
            .field("bold", &self.bold)
            .field("italic", &self.italic)
            .field("bytes", &self.data.as_ref().len())
            .finish()
    }
}

impl TextFace {
    /// Borrows the original font bytes. No copy or parsing occurs.
    pub fn data(&self) -> &[u8] {
        self.data.as_ref()
    }
}

/// A build-owned set of font handles in fallback order, with the PDF text family first.
#[derive(Clone, Debug, Default)]
pub struct TextFonts {
    /// Explicit and system faces allowed by policy, plus embedded faces when available.
    pub faces: Vec<TextFace>,
}

/// Loads font handles using pdf_font_paths and embedded_fonts_only from options.
/// Borrows options for this call and shares original font data with the embedded backend.
/// Unreadable font files are skipped as in the compiler's font discovery.
pub fn load_text_fonts(options: &Options<'_>) -> TextFonts {
    #[cfg(feature = "embedded")]
    let mut faces = crate::embedded::text_fonts(options);
    #[cfg(not(feature = "embedded"))]
    let mut faces = crate::font_scan::text_fonts(options);
    faces.sort_by_key(|f| !f.family.eq_ignore_ascii_case("Libertinus Serif"));
    TextFonts { faces }
}
