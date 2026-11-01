use crate::{Error, ErrorKind, Ratio};

/// A written duration before augmentation dots and proportional scaling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Base {
    /// Two whole notes.
    Breve,
    /// One whole note.
    Whole,
    /// Half a whole note.
    Half,
    /// A quarter note, the initial inherited duration.
    #[default]
    Quarter,
    /// An eighth note.
    Eighth,
    /// A sixteenth note.
    Sixteenth,
    /// A thirty-second note.
    ThirtySecond,
    /// A sixty-fourth note.
    SixtyFourth,
    /// A hundred-and-twenty-eighth note.
    HundredTwentyEighth,
}

impl Base {
    /// Returns the denominator of a whole-note fraction, or zero for a breve.
    pub fn denominator(self) -> u16 {
        match self {
            Self::Breve => 0,
            Self::Whole => 1,
            Self::Half => 2,
            Self::Quarter => 4,
            Self::Eighth => 8,
            Self::Sixteenth => 16,
            Self::ThirtySecond => 32,
            Self::SixtyFourth => 64,
            Self::HundredTwentyEighth => 128,
        }
    }

    /// Reads a supported duration denominator. Zero denotes a breve; others may fail.
    pub fn from_denominator(den: u16) -> Result<Self, Error> {
        match den {
            0 => Ok(Self::Breve),
            1 => Ok(Self::Whole),
            2 => Ok(Self::Half),
            4 => Ok(Self::Quarter),
            8 => Ok(Self::Eighth),
            16 => Ok(Self::Sixteenth),
            32 => Ok(Self::ThirtySecond),
            64 => Ok(Self::SixtyFourth),
            128 => Ok(Self::HundredTwentyEighth),
            _ => Err(Error::new(ErrorKind::Duration, "Unsupported note duration.")
                .hint("Use a power of two from 1 to 128, or \\breve.")),
        }
    }
}

/// Written duration and exact scaling. The default is an undotted quarter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Duration {
    /// Written notehead and flag duration.
    pub base: Base,
    /// Augmentation dots, at most eight.
    pub dots: u8,
    /// Proportional duration, including nested tuplet factors.
    pub scale: Ratio,
}

impl Default for Duration {
    fn default() -> Self {
        Self { base: Base::Quarter, dots: 0, scale: Ratio::ONE }
    }
}

impl Duration {
    /// Returns exact time in whole notes. Fails for invalid dots, scale or overflow.
    pub fn time(self) -> Result<Ratio, Error> {
        if self.dots > 8 || self.scale <= Ratio::ZERO {
            return Err(Error::new(ErrorKind::Duration, "Invalid dots or duration multiplier.")
                .hint("Use at most eight dots and a positive duration multiplier."));
        }
        let divisor = 1i64 << self.dots;
        let dots = Ratio::new(2 * divisor - 1, divisor)?;
        let base = if self.base == Base::Breve {
            Ratio::new(2, 1)?
        } else {
            Ratio::new(1, i64::from(self.base.denominator()))?
        };
        base.checked_mul(dots)?.checked_mul(self.scale)
    }
}
