use std::collections::BTreeMap;

use gd_score::{Error, Key, Meter, StaffId};

use crate::{Bezier, Glyph, GlyphData, Point, Rect, TextRun};

/// Last-line and single-line justification policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Justify {
    /// Leave a short final line at its natural width.
    #[default]
    Auto,
    /// Stretch every line to the available measure.
    All,
    /// Compress when needed but never stretch beyond natural width.
    None,
}

/// A mark's semantic role, used by SVG classes and collision audits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Staff line.
    Staff,
    /// Clef.
    Clef,
    /// Key or time signature.
    Signature,
    /// Pitched notehead.
    Notehead,
    /// Fret number on a tablature string.
    Fret,
    /// Ledger line through an outside note.
    Ledger,
    /// Stem.
    Stem,
    /// Flag.
    Flag,
    /// Beam.
    Beam,
    /// Accidental or cautionary parentheses.
    Accidental,
    /// Augmentation dot.
    Dot,
    /// Rest or multi-measure rest bar.
    Rest,
    /// Bar line or repeat dots.
    Bar,
    /// Tie.
    Tie,
    /// Legato or phrasing slur.
    Slur,
    /// Tuplet number or bracket.
    Tuplet,
    /// Articulation or fingering.
    Articulation,
    /// Fingering digit.
    Fingering,
    /// Tempo instruction.
    Tempo,
    /// Rehearsal label.
    Rehearsal,
    /// Dynamic or hairpin.
    Dynamic,
    /// Lyric syllable, hyphen or extender.
    Lyric,
    /// Instrument name or group delimiter.
    Instrument,
    /// Text script, tempo or rehearsal mark.
    Script,
    /// Chord symbol.
    ChordName,
    /// Repeat ending bracket and label.
    Volta,
    /// Measure number.
    MeasureNumber,
}

impl Role {
    /// Returns a stable ASCII class name without allocating.
    pub fn name(self) -> &'static str {
        match self {
            Self::Staff => "staff-line",
            Self::Clef => "clef",
            Self::Signature => "signature",
            Self::Notehead => "notehead",
            Self::Fret => "fret",
            Self::Ledger => "ledger",
            Self::Stem => "stem",
            Self::Flag => "flag",
            Self::Beam => "beam",
            Self::Accidental => "accidental",
            Self::Dot => "dot",
            Self::Rest => "rest",
            Self::Bar => "bar",
            Self::Tie => "tie",
            Self::Slur => "slur",
            Self::Tuplet => "tuplet",
            Self::Articulation => "articulation",
            Self::Fingering => "fingering",
            Self::Tempo => "tempo",
            Self::Rehearsal => "rehearsal",
            Self::Dynamic => "dynamic",
            Self::Lyric => "lyric",
            Self::Instrument => "instrument",
            Self::Script => "script",
            Self::ChordName => "chord-name",
            Self::Volta => "volta",
            Self::MeasureNumber => "measure-number",
        }
    }
}

/// One self-contained geometry instruction in points, using downward y coordinates.
#[derive(Clone, Debug)]
pub enum Mark {
    /// A music-font glyph, scaled from staff spaces to points.
    Glyph {
        /// Glyph definition in Engraving::glyphs.
        glyph: Glyph,
        /// Horizontal origin.
        x: f64,
        /// Vertical origin.
        y: f64,
        /// Points per font staff space.
        size: f64,
        /// Semantic role.
        role: Role,
    },
    /// A straight, solid segment.
    Line {
        /// First endpoint.
        from: Point,
        /// Last endpoint.
        to: Point,
        /// Stroke width.
        thickness: f64,
        /// Semantic role.
        role: Role,
    },
    /// A filled quadrilateral, usually a beam.
    Polygon {
        /// Vertices in perimeter order.
        points: [Point; 4],
        /// Semantic role.
        role: Role,
    },
    /// A filled bow bounded by two cubic curves.
    Curve {
        /// Outer boundary, from left to right.
        outer: Bezier,
        /// Inner boundary, from left to right.
        inner: Bezier,
        /// Semantic role.
        role: Role,
    },
    /// Outlined text with its shaped advance and ink bounds.
    Text {
        /// Positioned text outlines, relative to the baseline origin.
        run: TextRun,
        /// Horizontal baseline origin.
        x: f64,
        /// Vertical baseline origin.
        y: f64,
        /// Semantic role.
        role: Role,
    },
}

impl Mark {
    /// Returns the role shared by every geometry variant.
    pub fn role(&self) -> Role {
        match self {
            Self::Glyph { role, .. }
            | Self::Line { role, .. }
            | Self::Polygon { role, .. }
            | Self::Curve { role, .. }
            | Self::Text { role, .. } => *role,
        }
    }
}

/// Source-independent grouping and ink bounds for one positioned instruction.
#[derive(Clone, Debug)]
pub struct PositionedMark {
    /// Instruction in points relative to its system's top.
    pub mark: Mark,
    /// Owning staff.
    pub staff: StaffId,
    /// Printed measure number; zero for shared system furniture.
    pub measure: u32,
    /// Conservative ink bounds used by audits and vertical placement.
    pub bounds: Rect,
}

/// One system's positioned geometry and vertical extent.
#[derive(Clone, Debug, Default)]
pub struct System {
    /// Actual system ink width in points; zero uses the complete engraving width.
    pub width: f64,
    /// Top in points from the complete drawing's origin.
    pub top: f64,
    /// Height in points, including lyrics and outside marks.
    pub height: f64,
    /// Printed first measure number.
    pub first_measure: u32,
    /// Printed last measure number.
    pub last_measure: u32,
    /// Geometry with staff and measure identities for SVG grouping.
    pub marks: Vec<PositionedMark>,
}

/// Facts used by an adapter to word an accessible description in the page's language.
#[derive(Clone, Debug, Default)]
pub struct Summary {
    /// Number of staves.
    pub staves: u32,
    /// Named instruments, in staff order.
    pub instruments: Vec<String>,
    /// Initial key when supplied.
    pub key: Option<Key>,
    /// Initial meter when supplied.
    pub meter: Option<Meter>,
    /// Inclusive first and last printed measure numbers.
    pub measures: (u32, u32),
    /// Whether any verse has syllables.
    pub lyrics: bool,
}

/// A complete immutable drawing, independent of input syntax and borrowed font bytes.
#[derive(Clone, Debug, Default)]
pub struct Engraving {
    /// Natural output width in points.
    pub width: f64,
    /// Complete output height in points.
    pub height: f64,
    /// Systems in reading order.
    pub systems: Vec<System>,
    /// Only music glyphs used by this drawing, with outlines for standalone export.
    pub glyphs: BTreeMap<Glyph, GlyphData>,
    /// Accessible musical facts.
    pub summary: Summary,
    /// Recoverable layout and missing-text-glyph diagnostics.
    pub warnings: Vec<Error>,
}
