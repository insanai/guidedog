//! Guitar tablature: the string and fret that play each note of a chord.

use crate::{Alter, Error, ErrorKind, Note, Pitch, Step};

/// The open strings of a six-string guitar in standard tuning, string 1 (the highest)
/// first: E4, B3, G3, D3, A2 and E2.
pub const GUITAR_TUNING: [Pitch; 6] = [
    Pitch { step: Step::E, alter: Alter::Natural, octave: 1 },
    Pitch { step: Step::B, alter: Alter::Natural, octave: 0 },
    Pitch { step: Step::G, alter: Alter::Natural, octave: 0 },
    Pitch { step: Step::D, alter: Alter::Natural, octave: 0 },
    Pitch { step: Step::A, alter: Alter::Natural, octave: -1 },
    Pitch { step: Step::E, alter: Alter::Natural, octave: -1 },
];

/// The highest fret a tablature number may name.
pub const MAX_FRET: u8 = 24;

/// Where a note is played on a fretted instrument.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TabPosition {
    /// String number, 1 for the highest string.
    pub string: u8,
    /// Fret number, 0 for the open string.
    pub fret: u8,
}

/// Places the notes of one chord on the strings of [`GUITAR_TUNING`], returning one
/// position per note in note order.
///
/// As LilyPond does by default, a note with a string number keeps that string, and each
/// other note, highest first, takes the highest free string on which its fret lies
/// between 0 and [`MAX_FRET`]. Fails when a string number is outside 1 to 6, when two
/// notes share a string, or when a note has no free string that can play it.
pub fn tab_positions(notes: &[Note]) -> Result<Vec<TabPosition>, Error> {
    let mut positions = vec![TabPosition::default(); notes.len()];
    let mut used = [false; GUITAR_TUNING.len()];
    for (position, note) in positions.iter_mut().zip(notes) {
        let Some(string) = note.string else { continue };
        let index = usize::from(string).wrapping_sub(1);
        if index >= GUITAR_TUNING.len() {
            return Err(tab_error("A string number must be 1 through 6.", "Use \\1 to \\6."));
        }
        if used[index] {
            return Err(tab_error(
                "Two notes of a chord name the same string.",
                "Give each note of the chord its own string number.",
            ));
        }
        let fret = fret_on(note.pitch, index).ok_or_else(|| {
            tab_error(
                "The named string cannot play this pitch.",
                "Choose a string whose open pitch is at or below the note.",
            )
        })?;
        used[index] = true;
        *position = TabPosition { string, fret };
    }
    let mut free: Vec<usize> = (0..notes.len()).filter(|&i| notes[i].string.is_none()).collect();
    free.sort_by_key(|&i| std::cmp::Reverse(notes[i].pitch.chromatic()));
    for index in free {
        let pitch = notes[index].pitch;
        let string = (0..GUITAR_TUNING.len())
            .find(|&s| !used[s] && fret_on(pitch, s).is_some())
            .ok_or_else(|| {
            tab_error(
                "No free guitar string can play this pitch.",
                "Standard tuning reaches from E2 (e,) to the 24th fret of E4; \
                     move the note into that range or give the chord fewer notes.",
            )
        })?;
        used[string] = true;
        let fret = fret_on(pitch, string).unwrap_or_default();
        positions[index] = TabPosition { string: string as u8 + 1, fret };
    }
    Ok(positions)
}

/// The fret that plays `pitch` on the string with zero-based `index`, if any.
fn fret_on(pitch: Pitch, index: usize) -> Option<u8> {
    let fret = pitch.chromatic() - GUITAR_TUNING[index].chromatic();
    u8::try_from(fret).ok().filter(|&fret| fret <= MAX_FRET)
}

fn tab_error(message: &str, hint: &str) -> Error {
    Error::new(ErrorKind::Pitch, message).hint(hint)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.
mod tests {
    use super::*;

    fn note(step: Step, octave: i8, string: Option<u8>) -> Note {
        Note { pitch: Pitch { step, alter: Alter::Natural, octave }, string, ..Note::default() }
    }

    fn places(notes: &[Note]) -> Vec<(u8, u8)> {
        tab_positions(notes).unwrap().iter().map(|p| (p.string, p.fret)).collect()
    }

    #[test]
    fn single_notes_take_the_highest_string_that_can_play_them() {
        assert_eq!(places(&[note(Step::E, 1, None)]), [(1, 0)]);
        assert_eq!(places(&[note(Step::C, 1, None)]), [(2, 1)]);
        assert_eq!(places(&[note(Step::C, 0, None)]), [(5, 3)]);
        assert_eq!(places(&[note(Step::E, -1, None)]), [(6, 0)]);
        assert_eq!(places(&[note(Step::E, 2, None)]), [(1, 12)]);
    }

    #[test]
    fn chords_fill_strings_from_the_top_and_keep_named_strings() {
        // An open C major chord: C3 E3 G3 C4 E4.
        let chord = [
            note(Step::C, 0, None),
            note(Step::E, 0, None),
            note(Step::G, 0, None),
            note(Step::C, 1, None),
            note(Step::E, 1, None),
        ];
        assert_eq!(places(&chord), [(5, 3), (4, 2), (3, 0), (2, 1), (1, 0)]);
        // E4 on the second string is the fifth fret.
        assert_eq!(places(&[note(Step::E, 1, Some(2))]), [(2, 5)]);
    }

    #[test]
    fn unplayable_notes_and_strings_fail() {
        assert!(tab_positions(&[note(Step::D, -1, None)]).is_err());
        assert!(tab_positions(&[note(Step::E, 1, Some(7))]).is_err());
        assert!(tab_positions(&[note(Step::C, 1, Some(1))]).is_err());
        assert!(tab_positions(&[note(Step::E, 1, Some(1)), note(Step::G, 1, Some(1))]).is_err());
        let seven: Vec<_> = (0..7).map(|_| note(Step::E, 1, None)).collect();
        assert!(tab_positions(&seven).is_err());
    }
}
