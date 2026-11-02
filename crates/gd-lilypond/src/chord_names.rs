//! Chord names in the style of LilyPond's default chord namer.
//!
//! LilyPond names a chord by its root, a quality written on the line (`m`, `+`) and the
//! extensions and alterations raised as a superscript: `g:7` prints G with a raised 7, `c:13`
//! C with a raised "9 13". A major seventh is a small triangle, a diminished chord a raised
//! `o`, a half-diminished chord a raised slashed o, and accidentals are music glyphs, not the
//! letters `#` and `b`.

use gd_score::{Alter, Markup, Pitch, Step};

/// One element of the raised part of a chord name.
enum Raised {
    /// Text such as `7` or `sus4`.
    Word(&'static str),
    /// A raised degree with a sharp glyph, such as sharp 5.
    Sharp(&'static str),
    /// A lowered degree with a flat glyph, such as flat 5.
    Flat(&'static str),
    /// The major-seventh triangle.
    Triangle,
}

/// Formats a chord name, or returns `None` when `quality` is not a supported chord-mode
/// quality.
pub(crate) fn chord_markup(root: Pitch, quality: &str, bass: Option<Pitch>) -> Option<Markup> {
    let (main, raised) = quality_parts(quality)?;
    let mut items = Vec::with_capacity(6);
    push_pitch(&mut items, root);
    if !main.is_empty() {
        items.push(Markup::Text(main.into()));
    }
    if !raised.is_empty() {
        let words = raised.iter().map(raised_markup).collect();
        items.push(Markup::Scaled {
            scale_percent: 72,
            shift_percent: 35,
            body: Box::new(Markup::Line(words)),
        });
    }
    if let Some(bass) = bass {
        items.push(Markup::Text("/".into()));
        push_pitch(&mut items, bass);
    }
    Some(Markup::Concat(items))
}

/// Splits a chord-mode quality into the text on the line and the raised words, as LilyPond's
/// default chord namer prints them.
fn quality_parts(quality: &str) -> Option<(&'static str, &'static [Raised])> {
    use Raised::{Flat, Sharp, Triangle, Word};
    Some(match quality {
        "" => ("", &[]),
        "m" | "min" => ("m", &[]),
        "aug" => ("+", &[]),
        // In chord mode `maj` and `7+` add the major seventh, so they name the same chord.
        "maj" | "maj7" | "7+" => ("", &[Triangle]),
        "dim" => ("", &[Word("o")]),
        "dim7" => ("", &[Word("o7")]),
        "m7.5-" => ("", &[Word("\u{f8}")]),
        "sus2" => ("", &[Word("sus2")]),
        "sus" | "sus4" => ("", &[Word("sus4")]),
        "5" => ("", &[Word("5")]),
        "6" => ("", &[Word("6")]),
        "7" => ("", &[Word("7")]),
        "9" => ("", &[Word("9")]),
        "11" => ("", &[Word("11")]),
        "13" => ("", &[Word("9"), Word("13")]),
        "m7" | "min7" => ("m", &[Word("7")]),
        "7.5+" => ("", &[Word("7"), Sharp("5")]),
        "7.5-" => ("", &[Word("7"), Flat("5")]),
        "7sus4" => ("", &[Word("7"), Word("sus4")]),
        _ => return None,
    })
}

fn raised_markup(raised: &Raised) -> Markup {
    match raised {
        Raised::Word(text) => Markup::Text((*text).into()),
        Raised::Sharp(degree) => {
            Markup::Concat(vec![accidental("accidentalSharp"), Markup::Text((*degree).into())])
        }
        Raised::Flat(degree) => {
            Markup::Concat(vec![accidental("accidentalFlat"), Markup::Text((*degree).into())])
        }
        Raised::Triangle => Markup::MusicGlyph("csymMajorSeventh".into()),
    }
}

/// Pushes a pitch name: the letter and, when altered, its accidental glyph.
fn push_pitch(items: &mut Vec<Markup>, pitch: Pitch) {
    let letter = match pitch.step {
        Step::C => "C",
        Step::D => "D",
        Step::E => "E",
        Step::F => "F",
        Step::G => "G",
        Step::A => "A",
        Step::B => "B",
    };
    items.push(Markup::Text(letter.into()));
    let glyph = match pitch.alter {
        Alter::DoubleFlat => "accidentalDoubleFlat",
        Alter::Flat => "accidentalFlat",
        Alter::Natural => return,
        Alter::Sharp => "accidentalSharp",
        Alter::DoubleSharp => "accidentalDoubleSharp",
    };
    items.push(accidental(glyph));
}

/// An accidental a little smaller than the letters, as LilyPond sets it in chord names.
fn accidental(glyph: &str) -> Markup {
    Markup::Scaled {
        scale_percent: 89,
        shift_percent: 0,
        body: Box::new(Markup::MusicGlyph(glyph.into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pitch(step: Step, alter: Alter) -> Pitch {
        Pitch { step, alter, octave: 0 }
    }

    #[test]
    fn names_follow_lilypond_default_chord_names() {
        let g = pitch(Step::G, Alter::Natural);
        let cases = [
            ("7", "G7"),
            ("m7", "Gm7"),
            ("min", "Gm"),
            ("13", "G9 13"),
            ("sus", "Gsus4"),
            ("7.5-", "G7 b5"),
            ("aug", "G+"),
            ("dim7", "Go7"),
        ];
        for (quality, expected) in cases {
            let markup = chord_markup(g, quality, None).expect("supported quality");
            assert_eq!(markup.plain_text(), expected, "quality {quality}");
        }
        assert!(chord_markup(g, "maj9", None).is_none());
    }

    #[test]
    fn accidentals_and_bass_notes_use_music_glyphs() {
        let markup =
            chord_markup(pitch(Step::B, Alter::Flat), "7", Some(pitch(Step::F, Alter::Sharp)))
                .expect("supported quality");
        assert_eq!(markup.plain_text(), "Bb7/F#");
        let Markup::Concat(items) = markup else { panic!("a chord name is a concatenation") };
        assert!(matches!(
            &items[1],
            Markup::Scaled { body, .. } if **body == Markup::MusicGlyph("accidentalFlat".into())
        ));
    }
}
