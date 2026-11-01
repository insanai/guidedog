use crate::{Error, ErrorKind};

/// A diatonic step, in ascending order from C.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Step {
    /// Do.
    #[default]
    C,
    /// Re.
    D,
    /// Mi.
    E,
    /// Fa.
    F,
    /// Sol.
    G,
    /// La.
    A,
    /// Ti.
    B,
}

impl Step {
    /// Every step in diatonic order.
    pub const ALL: [Self; 7] = [Self::C, Self::D, Self::E, Self::F, Self::G, Self::A, Self::B];

    /// Returns this step's index from C, in 0..7.
    pub fn index(self) -> i16 {
        self as i16
    }

    /// Returns the natural semitone distance above C.
    pub fn semitones(self) -> i16 {
        match self {
            Self::C => 0,
            Self::D => 2,
            Self::E => 4,
            Self::F => 5,
            Self::G => 7,
            Self::A => 9,
            Self::B => 11,
        }
    }

    /// Wraps an arbitrary diatonic index to its step, including negative indices.
    pub fn from_index(index: i16) -> Self {
        Self::ALL[index.rem_euclid(7) as usize]
    }
}

/// A chromatic alteration of a written step; quarter tones are outside the profile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Alter {
    /// Two semitones below the natural step.
    DoubleFlat,
    /// One semitone below the natural step.
    Flat,
    /// The natural step.
    #[default]
    Natural,
    /// One semitone above the natural step.
    Sharp,
    /// Two semitones above the natural step.
    DoubleSharp,
}

impl Alter {
    /// Returns the signed semitone offset.
    pub fn semitones(self) -> i16 {
        match self {
            Self::DoubleFlat => -2,
            Self::Flat => -1,
            Self::Natural => 0,
            Self::Sharp => 1,
            Self::DoubleSharp => 2,
        }
    }

    /// Reads an offset, failing for alterations outside the profile.
    pub fn from_semitones(value: i16) -> Result<Self, Error> {
        match value {
            -2 => Ok(Self::DoubleFlat),
            -1 => Ok(Self::Flat),
            0 => Ok(Self::Natural),
            1 => Ok(Self::Sharp),
            2 => Ok(Self::DoubleSharp),
            _ => {
                Err(Error::new(ErrorKind::Pitch, "The pitch needs more than a double accidental.")
                    .hint("Choose an enharmonic transposition target."))
            }
        }
    }
}

/// A spelled pitch; c is octave 0, c' (middle C) is octave 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Pitch {
    /// Diatonic spelling.
    pub step: Step,
    /// Chromatic alteration.
    pub alter: Alter,
    /// Count of written octave marks.
    pub octave: i8,
}

impl Pitch {
    /// Returns the diatonic index without discarding spelling.
    pub fn diatonic(self) -> i16 {
        7 * i16::from(self.octave) + self.step.index()
    }

    /// Returns semitones relative to c in octave 0.
    pub fn chromatic(self) -> i16 {
        12 * i16::from(self.octave) + self.step.semitones() + self.alter.semitones()
    }

    /// Transposes by a spelled interval; fails for an unsupported accidental or octave.
    pub fn transpose(self, from: Self, to: Self) -> Result<Self, Error> {
        let index = self.diatonic() + to.diatonic() - from.diatonic();
        let step = Step::from_index(index);
        let octave = i8::try_from(index.div_euclid(7)).map_err(|_| pitch_range())?;
        let desired = self.chromatic() + to.chromatic() - from.chromatic();
        let alter = Alter::from_semitones(desired - 12 * i16::from(octave) - step.semitones())?;
        let pitch = Self { step, alter, octave };
        pitch.check_range()?;
        Ok(pitch)
    }

    /// Checks the profile's finite ledger-line range, returning a pitch diagnostic.
    pub fn check_range(self) -> Result<(), Error> {
        if (-3..=6).contains(&self.octave) { Ok(()) } else { Err(pitch_range()) }
    }
}

fn pitch_range() -> Error {
    Error::new(ErrorKind::Pitch, "The pitch is outside octaves -3 through 6.")
        .hint("Use pitches in octaves -3 through 6; transpose the source phrase if needed.")
}

/// A diatonic key mode, represented as a rotation of the major scale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// Ionian.
    #[default]
    Major,
    /// Dorian.
    Dorian,
    /// Phrygian.
    Phrygian,
    /// Lydian.
    Lydian,
    /// Mixolydian.
    Mixolydian,
    /// Aeolian.
    Minor,
    /// Locrian.
    Locrian,
}

impl Mode {
    /// Returns the rotation index of the major scale.
    pub fn index(self) -> usize {
        match self {
            Self::Major => 0,
            Self::Dorian => 1,
            Self::Phrygian => 2,
            Self::Lydian => 3,
            Self::Mixolydian => 4,
            Self::Minor => 5,
            Self::Locrian => 6,
        }
    }
}

/// Tonic and mode, independent of the current octave.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Key {
    /// Spelled tonic; octave has no effect on the signature.
    pub tonic: Pitch,
    /// Diatonic mode.
    pub mode: Mode,
}

impl Key {
    /// Returns alterations by step from C. Fails when the key needs triple accidentals.
    pub fn alterations(self) -> Result<[Alter; 7], Error> {
        let major = [0i16, 2, 4, 5, 7, 9, 11];
        let rotation = self.mode.index();
        let mut result = [Alter::Natural; 7];
        let tonic = self.tonic.step.semitones() + self.tonic.alter.semitones();
        for offset in 0..7 {
            let index = self.tonic.step.index() + offset as i16;
            let step = Step::from_index(index);
            let mode_offset = major[(rotation + offset) % 7]
                + if rotation + offset >= 7 { 12 } else { 0 }
                - major[rotation];
            let natural = step.semitones() + 12 * index.div_euclid(7);
            result[step.index() as usize] = Alter::from_semitones(tonic + mode_offset - natural)?;
        }
        Ok(result)
    }
}
