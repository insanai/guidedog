use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Write;

use gd_score::Error;
use ttf_parser::{Face, GlyphId, OutlineBuilder};

use crate::limits::font_error;
use crate::svg::Number;
use crate::{Point, Rect};

const MUSIC: &[u8] = include_bytes!("../../../third_party/bravura/Bravura.otf");
const METADATA: &str = include_str!("../../../third_party/bravura/bravura_metadata.json");

/// A Bravura glyph identifier, independent of its name in SMuFL metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Glyph(pub u16);

/// One immutable music glyph, in staff spaces with upward font coordinates.
#[derive(Clone, Debug)]
pub struct GlyphData {
    /// Stable SMuFL glyph name.
    pub name: String,
    /// SVG path in staff-space units, with upward y.
    pub outline: String,
    /// Ink bounds in the display list's downward-y coordinates.
    pub bounds: Rect,
    /// SMuFL anchors and cut-outs in upward-y font coordinates.
    pub anchors: BTreeMap<String, Point>,
    /// Horizontal advance in staff spaces.
    pub advance: f64,
}

/// Borrowed text faces, in fallback order; fonts outlive the returned tables.
#[derive(Clone, Debug, Default)]
pub struct FontOptions<'f> {
    /// Regular text faces.
    pub regular: Vec<FaceData<'f>>,
    /// Bold text faces, falling back to regular when empty.
    pub bold: Vec<FaceData<'f>>,
    /// Italic text faces, falling back to regular when empty.
    pub italic: Vec<FaceData<'f>>,
}

/// One borrowed OpenType face; a collection needs its explicit face index.
#[derive(Clone, Copy, Default)]
pub struct FaceData<'f> {
    /// Original font or collection bytes.
    pub data: &'f [u8],
    /// Face index; zero is the common single-font case.
    pub index: u32,
    /// Optional lazy source; when present it supplies bytes instead of data.
    pub source: Option<&'f dyn FaceSource>,
}

/// A build-owned immutable font source, loaded only when its coverage is needed.
pub trait FaceSource: Sync {
    /// Borrows original bytes for the lifetime of this source, or reports a load failure.
    fn data(&self) -> Result<&[u8], Error>;
    /// Tests discovered coverage before loading bytes. True may be provisional.
    fn covers(&self, _character: char) -> bool {
        true
    }
    /// Borrows an optional opaque dependency identity for the adapter to observe.
    fn id(&self) -> Option<&str> {
        None
    }
}

impl fmt::Debug for FaceData<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FaceData")
            .field("bytes", &self.data.len())
            .field("index", &self.index)
            .field("lazy", &self.source.is_some())
            .finish()
    }
}

pub(crate) struct TextFace<'f> {
    pub(crate) data: FaceData<'f>,
    parsed: Option<rustybuzz::Face<'f>>,
}

impl<'f> TextFace<'f> {
    pub(crate) fn face(&self) -> Result<rustybuzz::Face<'f>, Error> {
        if let Some(face) = &self.parsed {
            return Ok(face.clone());
        }
        let bytes = match self.data.source {
            Some(source) => source.data()?,
            None => self.data.data,
        };
        rustybuzz::Face::from_slice(bytes, self.data.index)
            .ok_or_else(|| font_error("A selected text face or collection index is malformed."))
    }

    pub(crate) fn covers(&self, text: &str) -> bool {
        self.data.source.is_none_or(|source| {
            text.chars().all(|character| character.is_control() || source.covers(character))
        })
    }
}

/// Explicit immutable font tables, safe to share between engraving threads.
pub struct Fonts<'f> {
    pub(crate) music: Face<'static>,
    pub(crate) glyphs: BTreeMap<Glyph, GlyphData>,
    names: BTreeMap<String, Glyph>,
    pub(crate) regular: Vec<TextFace<'f>>,
    pub(crate) bold: Vec<TextFace<'f>>,
    pub(crate) italic: Vec<TextFace<'f>>,
    pub(crate) defaults: BTreeMap<String, f64>,
}

impl fmt::Debug for Fonts<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fonts")
            .field("glyphs", &self.glyphs.len())
            .field("regular_faces", &self.regular.len())
            .field("bold_faces", &self.bold.len())
            .field("italic_faces", &self.italic.len())
            .finish()
    }
}

impl Fonts<'_> {
    /// Finds a named SMuFL glyph. Fails if the vendored font lacks its outline or metadata.
    pub fn glyph(&self, name: &str) -> Result<Glyph, Error> {
        self.names
            .get(name)
            .copied()
            .ok_or_else(|| font_error(format!("Missing music glyph `{name}`.")))
    }

    /// Borrows one loaded glyph's path, bounds and anchors, or None for a foreign id.
    pub fn glyph_data(&self, glyph: Glyph) -> Option<&GlyphData> {
        self.glyphs.get(&glyph)
    }

    pub(crate) fn default_value(&self, name: &str, fallback: f64) -> f64 {
        self.defaults.get(name).copied().unwrap_or(fallback)
    }
}

/// Loads unmodified embedded music data and borrows the supplied text faces.
/// No global state or filesystem is consulted. Fails for malformed fonts or metadata.
pub fn load_fonts<'f>(options: &FontOptions<'f>) -> Result<Fonts<'f>, Error> {
    let music =
        Face::parse(MUSIC, 0).map_err(|e| font_error(format!("Invalid Bravura font: {e:?}.")))?;
    let metadata = gd_json::parse(METADATA).map_err(|e| font_error(e.to_string()))?;
    let mut defaults = BTreeMap::new();
    if let Some(values) = metadata.get("engravingDefaults").and_then(gd_json::Value::as_object) {
        for (name, value) in values {
            if let Some(value) = value.as_f64().filter(|v| v.is_finite() && *v >= 0.0) {
                defaults.insert(name.clone(), value);
            }
        }
    }
    let mut fonts = Fonts {
        music,
        glyphs: BTreeMap::new(),
        names: BTreeMap::new(),
        defaults,
        regular: faces(&options.regular)?,
        bold: faces(&options.bold)?,
        italic: faces(&options.italic)?,
    };
    for (name, codepoint) in GLYPHS {
        load_glyph(&mut fonts, &metadata, name, *codepoint)?;
    }
    Ok(fonts)
}

fn faces<'f>(bytes: &[FaceData<'f>]) -> Result<Vec<TextFace<'f>>, Error> {
    let mut faces = Vec::new();
    faces
        .try_reserve_exact(bytes.len())
        .map_err(|_| font_error("Cannot allocate text face tables."))?;
    for data in bytes {
        let mut face = TextFace { data: *data, parsed: None };
        if data.source.is_none() {
            face.parsed = Some(face.face()?);
        }
        faces.push(face);
    }
    Ok(faces)
}

fn load_glyph(
    fonts: &mut Fonts<'_>,
    metadata: &gd_json::Value,
    name: &str,
    codepoint: u32,
) -> Result<(), Error> {
    let id = char::from_u32(codepoint)
        .and_then(|c| fonts.music.glyph_index(c))
        .ok_or_else(|| font_error(format!("Bravura has no glyph named `{name}`.")))?;
    let scale = 4.0 / f64::from(fonts.music.units_per_em());
    let (outline, ink) = outline(&fonts.music, id, scale)?;
    let bounds = metadata_bounds(metadata, name)?.union(ink);
    let mut anchors = BTreeMap::new();
    if let Some(values) = metadata
        .get("glyphsWithAnchors")
        .and_then(|v| v.get(name))
        .and_then(gd_json::Value::as_object)
    {
        for (name, point) in values {
            if let Some(point) = point.as_array().filter(|a| a.len() == 2)
                && let (Some(x), Some(y)) = (point[0].as_f64(), point[1].as_f64())
            {
                anchors.insert(name.clone(), Point { x, y });
            }
        }
    }
    let advance = f64::from(fonts.music.glyph_hor_advance(id).unwrap_or(0)) * scale;
    let glyph = Glyph(id.0);
    fonts.names.insert(name.into(), glyph);
    fonts.glyphs.insert(glyph, GlyphData { name: name.into(), outline, bounds, anchors, advance });
    Ok(())
}

fn metadata_bounds(metadata: &gd_json::Value, name: &str) -> Result<Rect, Error> {
    let read = |corner: &str, index: usize| {
        metadata
            .get("glyphBBoxes")
            .and_then(|v| v.get(name))
            .and_then(|v| v.get(corner))
            .and_then(|v| v.get(index))
            .and_then(gd_json::Value::as_f64)
            .filter(|n| n.is_finite())
            .ok_or_else(|| font_error(format!("Invalid glyph bounds for {name}.")))
    };
    Ok(Rect {
        left: read("bBoxSW", 0)?,
        bottom: -read("bBoxSW", 1)?,
        right: read("bBoxNE", 0)?,
        top: -read("bBoxNE", 1)?,
    })
}

pub(crate) fn outline(
    face: &Face<'_>,
    glyph: GlyphId,
    scale: f64,
) -> Result<(String, Rect), Error> {
    let mut builder = PathBuilder { text: String::new(), scale };
    let bounds = face
        .outline_glyph(glyph, &mut builder)
        .ok_or_else(|| font_error("A required glyph has no vector outline."))?;
    Ok((
        builder.text,
        Rect {
            left: f64::from(bounds.x_min) * scale,
            right: f64::from(bounds.x_max) * scale,
            top: -f64::from(bounds.y_max) * scale,
            bottom: -f64::from(bounds.y_min) * scale,
        },
    ))
}

struct PathBuilder {
    text: String,
    scale: f64,
}

impl OutlineBuilder for PathBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        let _ = write!(
            self.text,
            "M{} {}",
            Number(f64::from(x) * self.scale),
            Number(f64::from(y) * self.scale)
        );
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let _ = write!(
            self.text,
            "L{} {}",
            Number(f64::from(x) * self.scale),
            Number(f64::from(y) * self.scale)
        );
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let _ = write!(
            self.text,
            "Q{} {} {} {}",
            Number(f64::from(x1) * self.scale),
            Number(f64::from(y1) * self.scale),
            Number(f64::from(x) * self.scale),
            Number(f64::from(y) * self.scale)
        );
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let _ = write!(
            self.text,
            "C{} {} {} {} {} {}",
            Number(f64::from(x1) * self.scale),
            Number(f64::from(y1) * self.scale),
            Number(f64::from(x2) * self.scale),
            Number(f64::from(y2) * self.scale),
            Number(f64::from(x) * self.scale),
            Number(f64::from(y) * self.scale)
        );
    }
    fn close(&mut self) {
        self.text.push('Z');
    }
}

// SMuFL 1.4 code points and unmodified Bravura optional-glyph metadata.
pub(crate) const GLYPHS: &[(&str, u32)] = &[
    ("gClef", 0xe050),
    ("fClef", 0xe062),
    ("cClef", 0xe05c),
    ("gClefSmall", 0xf472),
    ("fClefSmall", 0xf474),
    ("cClefSmall", 0xf473),
    ("noteheadDoubleWhole", 0xe0a0),
    ("noteheadWhole", 0xe0a2),
    ("noteheadHalf", 0xe0a3),
    ("noteheadBlack", 0xe0a4),
    ("augmentationDot", 0xe1e7),
    ("restDoubleWhole", 0xe4e2),
    ("restWhole", 0xe4e3),
    ("restHalf", 0xe4e4),
    ("restQuarter", 0xe4e5),
    ("rest8th", 0xe4e6),
    ("rest16th", 0xe4e7),
    ("rest32nd", 0xe4e8),
    ("rest64th", 0xe4e9),
    ("rest128th", 0xe4ea),
    ("accidentalFlat", 0xe260),
    ("accidentalNatural", 0xe261),
    ("accidentalSharp", 0xe262),
    ("accidentalDoubleFlat", 0xe264),
    ("accidentalDoubleSharp", 0xe263),
    ("accidentalParensLeft", 0xe26a),
    ("accidentalParensRight", 0xe26b),
    ("flag8thUp", 0xe240),
    ("flag8thDown", 0xe241),
    ("flag16thUp", 0xe242),
    ("flag16thDown", 0xe243),
    ("flag32ndUp", 0xe244),
    ("flag32ndDown", 0xe245),
    ("flag64thUp", 0xe246),
    ("flag64thDown", 0xe247),
    ("flag128thUp", 0xe248),
    ("flag128thDown", 0xe249),
    ("brace", 0xe000),
    ("bracketTop", 0xe003),
    ("bracketBottom", 0xe004),
    ("repeatDot", 0xe044),
    ("articAccentAbove", 0xe4a0),
    ("articAccentBelow", 0xe4a1),
    ("articStaccatoAbove", 0xe4a2),
    ("articStaccatoBelow", 0xe4a3),
    ("articTenutoAbove", 0xe4a4),
    ("articTenutoBelow", 0xe4a5),
    ("articStaccatissimoAbove", 0xe4a6),
    ("articStaccatissimoBelow", 0xe4a7),
    ("articMarcatoAbove", 0xe4ac),
    ("articMarcatoBelow", 0xe4ad),
    ("fermataAbove", 0xe4c0),
    ("fermataBelow", 0xe4c1),
    ("ornamentTrill", 0xe566),
    ("ornamentTurn", 0xe567),
    ("ornamentMordent", 0xe56d),
    ("ornamentShortTrill", 0xe56c),
    ("stringsHarmonic", 0xe614),
    ("brassMuteClosed", 0xe5e5),
    ("brassMuteOpen", 0xe5e7),
    ("pluckedSnapPizzicatoAbove", 0xe631),
    ("timeSig0", 0xe080),
    ("timeSig1", 0xe081),
    ("timeSig2", 0xe082),
    ("timeSig3", 0xe083),
    ("timeSig4", 0xe084),
    ("timeSig5", 0xe085),
    ("timeSig6", 0xe086),
    ("timeSig7", 0xe087),
    ("timeSig8", 0xe088),
    ("timeSig9", 0xe089),
    ("dynamicPiano", 0xe520),
    ("dynamicMezzo", 0xe521),
    ("dynamicForte", 0xe522),
    ("dynamicRinforzando", 0xe523),
    ("dynamicSforzando", 0xe524),
    ("dynamicZ", 0xe525),
    ("tuplet0", 0xe880),
    ("tuplet1", 0xe881),
    ("tuplet2", 0xe882),
    ("tuplet3", 0xe883),
    ("tuplet4", 0xe884),
    ("tuplet5", 0xe885),
    ("tuplet6", 0xe886),
    ("tuplet7", 0xe887),
    ("tuplet8", 0xe888),
    ("tuplet9", 0xe889),
];
