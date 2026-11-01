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
            let candidates = self.vocabulary();
            let mut hint =
                "Use a note name in the selected language, with whole-tone accidentals."
                    .to_string();
            if let Some(suggestion) = gd_report::suggest(name, candidates.iter().copied()) {
                hint.push_str(&format!(" Did you mean `{suggestion}`?"));
            }
            Error::new(ErrorKind::Pitch, format!("`{name}` is not a supported note name."))
                .at(span)
                .hint(hint)
        };
        let Some(first) = name.chars().next() else { return Err(invalid()) };
        let suffix = &name[first.len_utf8()..];
        // LilyPond's deutsch names: b alone is B flat, and B double flat is heses.
        if matches!(self, Self::German) && first == 'b' {
            if !suffix.is_empty() {
                return Err(invalid());
            }
            return Ok(Pitch { step: Step::B, alter: Alter::Flat, octave: 0 });
        }
        if first == 'h' {
            if !matches!(self, Self::German) {
                return Err(invalid());
            }
            let alter = match suffix {
                "" => Alter::Natural,
                "is" => Alter::Sharp,
                "isis" => Alter::DoubleSharp,
                "eses" => Alter::DoubleFlat,
                _ => return Err(invalid()),
            };
            return Ok(Pitch { step: Step::B, alter, octave: 0 });
        }
        let step = match first {
            'c' => Step::C,
            'd' => Step::D,
            'e' => Step::E,
            'f' => Step::F,
            'g' => Step::G,
            'a' => Step::A,
            'b' => Step::B,
            _ => return Err(invalid()),
        };
        let offset = match (self, suffix) {
            (Self::English, "s" | "sharp") => 1,
            (Self::English, "ss" | "x" | "sharpsharp") => 2,
            (Self::English, "f" | "flat") => -1,
            (Self::English, "ff" | "flatflat") => -2,
            (_, "") => 0,
            // In deutsch, A and E take their flats as as, ases (or asas), es and eses only.
            (Self::German, "s") if matches!(first, 'a' | 'e') => -1,
            (Self::German, "ses") if matches!(first, 'a' | 'e') => -2,
            (Self::German, "sas") if first == 'a' => -2,
            (Self::German, _) if matches!(first, 'a' | 'e') && suffix.starts_with('e') => {
                return Err(invalid());
            }
            (Self::Dutch | Self::German, "is") => 1,
            (Self::Dutch | Self::German, "isis") => 2,
            (Self::Dutch | Self::German, "es") => -1,
            (Self::Dutch | Self::German, "eses") => -2,
            (Self::Dutch | Self::German, "s") if matches!(first, 'a' | 'e') => -1,
            (Self::Dutch | Self::German, "ses") if matches!(first, 'a' | 'e') => -2,
            _ => return Err(invalid()),
        };
        let alter = Alter::from_semitones(offset).map_err(|_| invalid())?;
        Ok(Pitch { step, alter, octave: 0 })
    }

    fn vocabulary(self) -> &'static [&'static str] {
        match self {
            Self::Dutch => &[
                "c", "cis", "ces", "d", "dis", "des", "e", "eis", "es", "f", "fis", "fes", "g",
                "gis", "ges", "a", "ais", "as", "b", "bis", "bes",
            ],
            Self::English => &[
                "c", "cs", "cf", "cx", "d", "ds", "df", "e", "es", "ef", "f", "fs", "ff", "fx",
                "g", "gs", "gf", "gx", "a", "as", "af", "b", "bs", "bf",
            ],
            Self::German => &[
                "c", "cis", "ces", "d", "dis", "des", "e", "eis", "es", "f", "fis", "fes", "g",
                "gis", "ges", "a", "ais", "as", "asas", "b", "h", "his", "heses",
            ],
        }
    }
}
