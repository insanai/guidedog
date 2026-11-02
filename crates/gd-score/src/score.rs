use crate::{Duration, Key, Markup, MarkupRole, Pitch, Ratio, Span};

/// A staff index in a score.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StaffId(pub u32);
/// A voice index in a score.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VoiceId(pub u32);
/// An event index in a score.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId(pub u32);

/// Exact whole-note time, with grace events ordered before their main note.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Moment {
    /// Main musical time.
    pub main: Ratio,
    /// Negative grace time before the main moment, or zero.
    pub grace: Ratio,
}

/// A particular head of a chord event, not merely the event as a whole.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoteRef {
    /// Chord event.
    pub event: EventId,
    /// Index of the head within its chord.
    pub note: u8,
}

/// A complete notation model, independent of input syntax and drawing geometry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Score {
    /// Staves in top-to-bottom order.
    pub staves: Vec<Staff>,
    /// Braced or bracketed contiguous groups.
    pub groups: Vec<StaffGroup>,
    /// Voices, each owned by one staff.
    pub voices: Vec<Voice>,
    /// Events addressed by stable indices.
    pub events: Vec<Event>,
    /// Ties, slurs, hairpins, beams, tuplets and repeat brackets.
    pub spanners: Vec<Spanner>,
    /// Lyric verses attached to notes or exact time.
    pub lyrics: Vec<LyricLine>,
    /// Measure boundaries and effective meter.
    pub measures: Vec<Measure>,
}

/// What a staff's lines stand for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StaffKind {
    /// Five lines of standard notation.
    #[default]
    Standard,
    /// Six lines of guitar tablature in [`crate::GUITAR_TUNING`], with fret numbers.
    Tablature,
}

/// One staff of standard notation or tablature.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Staff {
    /// Standard notation or tablature.
    pub kind: StaffKind,
    /// Full name on the first system.
    pub name: String,
    /// Abbreviation on subsequent systems.
    pub short_name: String,
    /// Optional structured markup for the full instrument name.
    pub name_markup: Option<Markup>,
    /// Optional structured markup for the abbreviated instrument name.
    pub short_name_markup: Option<Markup>,
}

/// One of at most four independent rhythmic sequences on a staff.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Voice {
    /// Owning staff.
    pub staff: StaffId,
    /// Optional input name, used to attach lyrics.
    pub name: String,
    /// Chronologically ordered event identifiers.
    pub events: Vec<EventId>,
    /// Default stem policy for this voice.
    pub direction: Direction,
}

/// A direction, with an automatic house-style default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    /// Choose by staff position.
    #[default]
    Auto,
    /// Above the staff, or upward stems.
    Up,
    /// Below the staff, or downward stems.
    Down,
}

/// A local polyphonic policy, independent of the owning voice's identity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VoiceStyle {
    /// Choose stems by staff position, without an inner-voice shift.
    #[default]
    Single,
    /// First voice: upward stems and the outer note column.
    One,
    /// Second voice: downward stems and the outer note column.
    Two,
    /// Third voice: upward stems, shifting right when columns conflict.
    Three,
    /// Fourth voice: downward stems, shifting left when columns conflict.
    Four,
}

impl VoiceStyle {
    /// Returns the stem and annotation side selected by this local policy.
    pub fn direction(self) -> Direction {
        match self {
            Self::Single => Direction::Auto,
            Self::One | Self::Three => Direction::Up,
            Self::Two | Self::Four => Direction::Down,
        }
    }

    /// Returns the note-column shift level of this policy: one for the inner voices
    /// three and four, zero otherwise. Among columns with one stem direction, a higher
    /// level stands further from the stem side's outer column.
    pub fn column_shift(self) -> u8 {
        match self {
            Self::Three | Self::Four => 1,
            Self::Single | Self::One | Self::Two => 0,
        }
    }
}

/// The most voice records one staff may hold. At most four of them may sound at once;
/// finished voices stay as records so lyrics can still find them by name.
pub const MAX_STAFF_VOICES: usize = 64;

/// A connected group of contiguous staves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaffGroup {
    /// First staff, inclusive.
    pub first: StaffId,
    /// Last staff, inclusive.
    pub last: StaffId,
    /// Brace or bracket.
    pub kind: GroupKind,
    /// Full name of the whole group on the first system, beside its brace or bracket.
    pub name: String,
    /// Abbreviation of the group's name on subsequent systems.
    pub short_name: String,
    /// Optional structured markup for the group's full name.
    pub name_markup: Option<Markup>,
    /// Optional structured markup for the group's abbreviated name.
    pub short_name_markup: Option<Markup>,
}

impl StaffGroup {
    /// A group of `kind` over `first..=last` without a name.
    pub fn new(first: StaffId, last: StaffId, kind: GroupKind) -> Self {
        Self {
            first,
            last,
            kind,
            name: String::new(),
            short_name: String::new(),
            name_markup: None,
            short_name_markup: None,
        }
    }
}

/// Visual grouping policy for staves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupKind {
    /// PianoStaff and GrandStaff.
    Brace,
    /// StaffGroup, with connected bar lines.
    Bracket,
    /// ChoirStaff, with separate staff bar lines.
    Choir,
}

/// One timed event with its original location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    /// Owning voice.
    pub voice: VoiceId,
    /// Local voice policy; None inherits the owning voice's default direction.
    pub voice_style: Option<VoiceStyle>,
    /// Explicit note-column shift level, as LilyPond's `horizontal-shift`; None uses the
    /// voice policy's level from [`VoiceStyle::column_shift`].
    pub column_shift: Option<u8>,
    /// Exact start time.
    pub at: Moment,
    /// Musical meaning.
    pub kind: EventKind,
    /// Source location retained for diagnostics.
    pub span: Span,
    /// Marks attached to this event.
    pub marks: Vec<Annotation>,
}

/// A rhythmic event or a notation change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    /// One or more pitched noteheads.
    Chord(Chord),
    /// An ordinary rest.
    Rest(Duration),
    /// An invisible rhythmic spacer.
    Skip(Duration),
    /// One or more whole-measure rests.
    MeasureRest(Duration),
    /// A clef change.
    Clef(Clef),
    /// A key change.
    Key(Key),
    /// A meter change on a measure boundary.
    Time(Meter),
    /// A visible bar line.
    Bar(BarStyle),
    /// A forced or forbidden system break.
    Break(BreakRule),
    /// A chord-name symbol at this moment.
    ChordName(String, Duration),
}

impl EventKind {
    /// Borrows a rhythmic duration, or returns None for instantaneous notation changes.
    pub fn duration(&self) -> Option<&Duration> {
        match self {
            Self::Chord(chord) => Some(&chord.duration),
            Self::Rest(d) | Self::Skip(d) | Self::MeasureRest(d) | Self::ChordName(_, d) => {
                Some(d)
            }
            Self::Clef(_) | Self::Key(_) | Self::Time(_) | Self::Bar(_) | Self::Break(_) => None,
        }
    }
}

/// Simultaneous noteheads with one written duration.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chord {
    /// Heads in input order; relative pitch interpretation uses this order.
    pub notes: Vec<Note>,
    /// Written and sounding duration.
    pub duration: Duration,
    /// Explicit stem direction, including neutral; None uses the local voice policy.
    pub direction: Option<Direction>,
    /// Whether automatic beaming applies at this event.
    pub auto_beam: bool,
    /// Whether grace stems are slashed.
    pub grace_slash: bool,
}

/// A spelled pitch and accidental-display requests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Note {
    /// Sounding and written pitch.
    pub pitch: Pitch,
    /// Print an accidental even when implied by state.
    pub forced: bool,
    /// Put the accidental in parentheses.
    pub cautionary: bool,
    /// Tablature string that plays the note, 1 for the highest; None lets tablature
    /// choose, as [`crate::tab_positions`] describes.
    pub string: Option<u8>,
}

/// A clef and optional octave displacement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Clef {
    /// G clef on the second line.
    #[default]
    Treble,
    /// F clef on the fourth line.
    Bass,
    /// C clef on the middle line.
    Alto,
    /// C clef on the fourth line.
    Tenor,
    /// C clef on the first line.
    Soprano,
    /// G clef sounding an octave lower.
    TrebleEight,
    /// F clef sounding an octave lower.
    BassEight,
}

impl Clef {
    /// Returns the diatonic pitch on the middle staff line.
    pub fn middle(self) -> i16 {
        match self {
            Self::Treble => 13,
            Self::Bass => 1,
            Self::Alto => 7,
            Self::Tenor => 5,
            Self::Soprano => 11,
            Self::TrebleEight => 6,
            Self::BassEight => -6,
        }
    }
}

/// A common meter shared by every staff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meter {
    /// Number of denominator units per bar.
    pub numerator: u16,
    /// Power-of-two unit denominator.
    pub denominator: u16,
    /// Optional explicit beaming groups, whose sum is the numerator.
    pub beats: Vec<u16>,
}

impl Default for Meter {
    fn default() -> Self {
        Self { numerator: 4, denominator: 4, beats: Vec::new() }
    }
}

/// A visible bar-line shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BarStyle {
    /// Ordinary thin bar.
    #[default]
    Single,
    /// Two thin bars.
    Double,
    /// Thin then thick.
    Final,
    /// Thick then thin.
    Initial,
    /// Opening repeat.
    RepeatStart,
    /// Closing repeat.
    RepeatEnd,
    /// Closing and opening repeat.
    RepeatBoth,
}

/// A line-break instruction at a bar boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakRule {
    /// End the current system.
    Force,
    /// Keep the adjacent measures in one system.
    Forbid,
}

/// A bounded interval such as a slur or tuplet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spanner {
    /// Opening event or note.
    pub start: NoteRef,
    /// Closing event or note.
    pub end: NoteRef,
    /// Meaning and drawing policy.
    pub kind: SpannerKind,
    /// Explicit direction when present.
    pub direction: Direction,
    /// Original notation for diagnostics.
    pub span: Span,
}

/// An interval's musical meaning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpannerKind {
    /// Connect two notes of exactly equal spelled pitch.
    Tie,
    /// Legato slur.
    Slur,
    /// Phrasing slur.
    PhrasingSlur,
    /// Explicit beam group.
    Beam,
    /// Ratio displayed above a tuplet.
    Tuplet(u64, u64),
    /// Increasing volume.
    Crescendo,
    /// Decreasing volume.
    Decrescendo,
    /// Numbered repeat alternative.
    Volta(String),
}

/// A text or symbol attached to a rhythmic event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Annotation {
    /// Meaning of the mark.
    pub kind: AnnotationKind,
    /// Explicit side, when supplied.
    pub direction: Direction,
}

/// An articulation, dynamic, script or global mark.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnnotationKind {
    /// A named SMuFL articulation.
    Articulation(String),
    /// A finger number in 1..=5.
    Fingering(u8),
    /// A dynamic such as mf or sfz.
    Dynamic(String),
    /// Outlined text, with its simple style.
    Text(String, TextStyle),
    /// A tempo instruction.
    Tempo(String),
    /// A rehearsal label.
    Rehearsal(String),
    /// Rich formatted markup with its semantic role.
    Markup(Markup, MarkupRole),
}

/// A text face chosen from the project's font policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextStyle {
    /// Regular text.
    #[default]
    Regular,
    /// Bold text.
    Bold,
    /// Italic text.
    Italic,
    /// Bold italic text, falling back to bold, then italic, when no such face exists.
    BoldItalic,
}

/// A lyric attachment to a specific note or an independently timed lyric.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    /// A note within a chord.
    Note(NoteRef),
    /// An exact main or grace moment.
    Moment(Moment),
}

/// One verse below a voice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LyricLine {
    /// Voice whose staff owns the verse.
    pub voice: VoiceId,
    /// Ordered syllables.
    pub syllables: Vec<Syllable>,
}

/// One lyric syllable with a possible hyphen or extender.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Syllable {
    /// Text to shape.
    pub text: String,
    /// First note or exact moment.
    pub anchor: Anchor,
    /// Join to the next syllable with a hyphen.
    pub hyphen: bool,
    /// Last note or moment of a melisma extender.
    pub extender: Option<Anchor>,
    /// Source location for diagnostics.
    pub span: Span,
}

/// One measure, including pickups and a final incomplete measure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Measure {
    /// Printed measure number; a pickup is zero.
    pub number: u32,
    /// Exact start, inclusive.
    pub start: Ratio,
    /// Exact end, exclusive.
    pub end: Ratio,
    /// Effective meter.
    pub meter: Meter,
}
