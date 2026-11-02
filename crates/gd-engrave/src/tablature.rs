//! Guitar tablature: six lines, one per string, with fret numbers in place of noteheads.
//!
//! As in LilyPond's default tablature, a tab staff shows the strings, a TAB clef, bar
//! lines and fret numbers. It has no key or time signature, stems, beams, rests, dots,
//! ties or scripts. A tied continuation is not repeated, except at the start of a line,
//! where it stands in parentheses.

use gd_score::{Chord, Error, TextStyle};

use crate::drawing::{Draw, Request};
use crate::limits::{charge, push};
use crate::prepare::EventInfo;
use crate::{Fonts, Mark, Options, Point, Rect, Role};

/// Half the height of a tab staff in staff spaces: six lines a space and a half apart.
pub(crate) const HALF_HEIGHT: f64 = 3.75;

/// Fret numbers are centered over the middle of where a notehead would stand, so they
/// line up with the notes of a standard staff above.
pub(crate) const CENTER: f64 = 0.59;

/// The size of a fret number in staff spaces.
const SIZE: f64 = 1.7;

/// Grace notes print their fret numbers smaller, as their heads are on a staff.
const GRACE: f64 = 0.7;

/// One fret number of a tab chord.
#[derive(Clone, Debug)]
pub(crate) struct Fret {
    /// String, 1 for the highest.
    pub(crate) string: u8,
    /// Fret number.
    pub(crate) fret: u8,
    /// Width of the printed number in staff spaces.
    pub(crate) width: f64,
    /// The note continues a tie, so the number is not repeated.
    pub(crate) continuation: bool,
    /// The chord begins a measure after the first, where a line break prints `(fret)`.
    pub(crate) at_bar: bool,
}

/// The vertical place of `string` in staff spaces below the middle of the staff.
pub(crate) fn string_y(string: u8) -> f64 {
    (f64::from(string) - 3.5) * 1.5
}

/// Fills `info` with the fret numbers of a chord on a tab staff.
pub(crate) fn plan(
    info: &mut EventInfo,
    (chord, grace, at_bar): (&Chord, bool, bool),
    tied: &std::collections::BTreeSet<(u32, u8)>,
    fonts: &Fonts<'_>,
    options: &mut Options<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    charge(work, chord.notes.len())?;
    let lookup = tied.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let space = options.staff_size / 4.0;
    let positions = gd_score::tab_positions(&chord.notes)?;
    info.head_scale = if grace { GRACE } else { 1.0 };
    for (note, position) in positions.iter().enumerate() {
        charge(work, lookup)?;
        let continuation = tied.contains(&(info.id.0, note as u8));
        let text = if continuation && at_bar {
            format!("({})", position.fret)
        } else {
            position.fret.to_string()
        };
        let run = crate::text::shape(
            fonts,
            &text,
            SIZE * info.head_scale * space,
            TextStyle::Regular,
            &mut Vec::new(),
            work,
            &mut options.reserve,
        )?;
        let fret = Fret {
            string: position.string,
            fret: position.fret,
            width: (run.bounds.right - run.bounds.left) / space,
            continuation,
            at_bar,
        };
        push(&mut info.frets, fret, &mut options.reserve)?;
    }
    Ok(())
}

/// The horizontal room the fret numbers of `info` need, relative to the column, for
/// spacing: one box per number with a little air.
pub(crate) fn boxes(info: &EventInfo) -> impl Iterator<Item = Rect> + '_ {
    info.frets.iter().filter(|fret| !fret.continuation || fret.at_bar).map(move |fret| {
        let center = info.prefix + CENTER;
        let y = string_y(fret.string);
        Rect {
            left: center - fret.width / 2.0 - 0.2,
            right: center + fret.width / 2.0 + 0.2,
            top: y - 0.7,
            bottom: y + 0.7,
        }
    })
}

impl Draw<'_, '_, '_, '_, '_> {
    /// Draws the TAB clef at `x`, scaled to span the middle four strings.
    pub(crate) fn tab_clef(&mut self, x: f64, middle: f64) -> Result<(), Error> {
        let space = self.options.staff_size / 4.0;
        let name = "6stringTabClef";
        let glyph = self.fonts.glyph(name)?;
        let bounds = self
            .fonts
            .glyph_data(glyph)
            .ok_or_else(|| crate::limits::failure("Missing TAB clef bounds."))?
            .bounds;
        let height = (bounds.bottom - bounds.top).max(0.1);
        let size = 4.5 / height * space;
        let y = middle - (bounds.top + bounds.bottom) / 2.0 * size;
        self.glyph(name, x, y, size, Role::Clef)
    }

    /// Draws the fret numbers of a tab chord whose column stands at `x`, and remembers
    /// where they break the string lines.
    pub(crate) fn frets(
        &mut self,
        info: &EventInfo,
        x: f64,
        middle: f64,
        line_start: bool,
    ) -> Result<(), Error> {
        let space = self.options.staff_size / 4.0;
        let prefix = if line_start { 0.0 } else { info.prefix };
        let center = x + (prefix + CENTER) * space;
        for fret in &info.frets {
            if fret.continuation && !line_start {
                continue;
            }
            let text =
                if fret.continuation { format!("({})", fret.fret) } else { fret.fret.to_string() };
            let run = crate::text::shape(
                self.fonts,
                &text,
                SIZE * info.head_scale * space,
                TextStyle::Regular,
                &mut self.warnings,
                self.work,
                &mut self.options.reserve,
            )?;
            let line = middle + string_y(fret.string) * space;
            let width = run.bounds.right - run.bounds.left;
            let left = center - width / 2.0 - run.bounds.left;
            let baseline = line - (run.bounds.top + run.bounds.bottom) / 2.0;
            let bounds = run.bounds.translated(left, baseline);
            let gap = (fret.string, bounds.left - 0.2 * space, bounds.right + 0.2 * space);
            push(&mut self.tab_gaps, gap, &mut self.options.reserve)?;
            self.add(Mark::Text { run, x: left, y: baseline, role: Role::Fret }, bounds)?;
        }
        Ok(())
    }

    /// Draws the six string lines from `left` to `right`, broken around fret numbers so
    /// that the numbers read clearly on any background.
    pub(crate) fn string_lines(
        &mut self,
        request: &Request<'_, '_>,
        (left, right): (f64, f64),
        middle: f64,
    ) -> Result<(), Error> {
        let space = self.options.staff_size / 4.0;
        let thickness = request.fonts.default_value("staffLineThickness", 0.13) * space;
        let mut gaps = std::mem::take(&mut self.tab_gaps);
        gaps.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        for string in 1..=6u8 {
            let y = middle + string_y(string) * space;
            let mut from = left;
            for &(_, start, end) in gaps.iter().filter(|gap| gap.0 == string) {
                charge(self.work, 1)?;
                if start > from {
                    self.line(
                        Point { x: from, y },
                        Point { x: start, y },
                        thickness,
                        Role::Staff,
                    )?;
                }
                from = from.max(end);
            }
            if right > from {
                self.line(Point { x: from, y }, Point { x: right, y }, thickness, Role::Staff)?;
            }
        }
        Ok(())
    }
}
