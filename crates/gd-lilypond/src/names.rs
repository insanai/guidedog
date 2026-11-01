use gd_score::{Alter, Pitch, Step};

use crate::{Error, ErrorKind, Span};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) enum Language {
    #[default]
    Dutch,
    English,
    German,
}

impl Language {
    pub(crate) fn read(name: &str, span: Span) -> Result<Self, Error> {
        match name {
            "nederlands" => Ok(Self::Dutch),
            "english" => Ok(Self::English),
            "deutsch" => Ok(Self::German),
            _ => Err(Error::new(
                ErrorKind::Unsupported,
                format!("Unknown pitch language `{name}`."),
            )
            .at(span)
            .hint("Choose nederlands, english or deutsch.")),
        }
    }

    pub(crate) fn pitch(self, name: &str, span: Span) -> Result<Pitch, Error> {
        let invalid = || {
            Error::new(ErrorKind::Pitch, format!("`{name}` is not a supported note name."))
                .at(span)
                .hint("Use a note name in the selected language, with whole-tone accidentals.")
        };
        let Some(first) = name.chars().next() else { return Err(invalid()) };
        let step = match first {
            'c' => Step::C,
            'd' => Step::D,
            'e' => Step::E,
            'f' => Step::F,
            'g' => Step::G,
            'a' => Step::A,
            'b' | 'h' => Step::B,
            _ => return Err(invalid()),
        };
        let suffix = &name[first.len_utf8()..];
        let offset = match (self, suffix) {
            (Self::English, "s" | "sharp") => 1,
            (Self::English, "ss" | "sharpsharp") => 2,
            (Self::English, "f" | "flat") => -1,
            (Self::English, "ff" | "flatflat") => -2,
            (_, "") => 0,
            (Self::Dutch | Self::German, "is") => 1,
            (Self::Dutch | Self::German, "isis") => 2,
            (Self::Dutch | Self::German, "es") => -1,
            (Self::Dutch | Self::German, "eses") => -2,
            (Self::Dutch | Self::German, "s") if matches!(first, 'a' | 'e') => -1,
            (Self::Dutch | Self::German, "ses") if matches!(first, 'a' | 'e') => -2,
            _ => return Err(invalid()),
        };
        if first == 'h' && !matches!(self, Self::German) {
            return Err(invalid());
        }
        let alter = if matches!(self, Self::German) && first == 'b' && suffix.is_empty() {
            Alter::Flat
        } else {
            Alter::from_semitones(offset).map_err(|_| invalid())?
        };
        Ok(Pitch { step, alter, octave: 0 })
    }
}
