use gd_conf::Config;
use gd_engrave::{FaceData, FontOptions};

/// Owned font handles that outlive one build's borrowed music font tables.
#[derive(Debug)]
pub struct MusicSource {
    fonts: Vec<MusicFace>,
}

/// Shared immutable music tables, or the load failure to report at each affected directive.
#[derive(Debug)]
pub struct MusicFonts<'a> {
    pub(super) loaded: Result<gd_engrave::Fonts<'a>, gd_score::Error>,
}

impl MusicSource {
    /// Borrows this build's text font bytes and loads immutable engraving tables.
    /// A malformed face is retained as a structured failure for affected directives.
    pub fn load_fonts(&self) -> MusicFonts<'_> {
        let mut options = FontOptions::default();
        for source in &self.fonts {
            let face = &source.face;
            let data = FaceData { source: Some(source), index: face.index, ..Default::default() };
            match (face.bold, face.italic) {
                (false, false) => options.regular.push(data),
                (true, false) => options.bold.push(data),
                (false, true) => options.italic.push(data),
                (true, true) => options.bold_italic.push(data),
            }
        }
        MusicFonts { loaded: gd_engrave::load_fonts(&options) }
    }
}

/// Loads build-owned font handles through the same policy used for PDF compilation.
/// Call once before rendering worker threads, then borrow its tables through PageRequest.
pub fn load_music_source(config: &Config) -> MusicSource {
    let options = gd_typst::Options {
        embedded_fonts_only: config.pdf.fonts == "embedded",
        font_paths: config.pdf.font_paths.iter().map(|p| config.source_dir.join(p)).collect(),
        ..Default::default()
    };
    let fonts = gd_typst::load_text_fonts(&options)
        .faces
        .into_iter()
        .map(|face| {
            let path = face.path().map(|p| p.to_string_lossy().into_owned());
            MusicFace { face, path }
        })
        .collect();
    MusicSource { fonts }
}

#[derive(Debug)]
struct MusicFace {
    face: gd_typst::TextFace,
    path: Option<String>,
}

impl gd_engrave::FaceSource for MusicFace {
    fn data(&self) -> Result<&[u8], gd_score::Error> {
        self.face
            .data()
            .map_err(|error| gd_score::Error::new(gd_score::ErrorKind::Font, error.to_string()))
    }

    fn covers(&self, character: char) -> bool {
        self.face.covers(character)
    }

    fn id(&self) -> Option<&str> {
        self.path.as_deref()
    }
}
