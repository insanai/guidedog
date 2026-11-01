//! Verse metrics, inter-syllable links and continuations share one positioned text run.

use gd_score::{Anchor, Error, Moment, Score, Syllable, TextStyle};

use crate::drawing::{Draw, Request};
use crate::limits::{charge, push};
use crate::{Mark, Point, Role, TextRun};

struct Ink {
    index: usize,
    x: f64,
    run: TextRun,
}

impl Draw<'_, '_, '_, '_, '_> {
    pub(crate) fn lyrics(&mut self, request: &Request<'_, '_>) -> Result<(), Error> {
        let space = self.options.staff_size / 4.0;
        let mut rows = [None::<(f64, f64)>; 24];
        let first = request.columns[0].at;
        let last = request.columns.last().map(|c| c.at).unwrap_or(first);
        for verse in &request.score.lyrics {
            self.staff = request.score.voices[verse.voice.0 as usize].staff;
            let staff = self.staff.0 as usize;
            let middle = (4.0 + staff as f64 * 9.0) * space;
            let mut ink = Vec::new();
            for (index, syllable) in verse.syllables.iter().enumerate() {
                charge(self.work, 1)?;
                let (at, x) = anchor(request.score, syllable.anchor, request, middle, space);
                if at < first || at >= last {
                    continue;
                }
                let run = crate::text::shape(
                    self.fonts,
                    &syllable.text,
                    2.4 * space,
                    TextStyle::Regular,
                    &mut self.warnings,
                    self.work,
                    &mut self.options.reserve,
                )?;
                let x = x - run.advance / 2.0;
                push(&mut ink, Ink { index, x, run }, &mut self.options.reserve)?;
            }
            let ascent = ink.iter().map(|i| i.run.ascent).fold(2.4 * space, f64::max);
            let descent = ink.iter().map(|i| i.run.descent).fold(0.6 * space, f64::max);
            let baseline = rows[staff]
                .map(|(y, previous)| y + (previous + ascent + 0.5 * space).max(3.0 * space))
                .unwrap_or(middle + 7.0 * space);
            rows[staff] = Some((baseline, descent));
            self.lyric_links(request, &verse.syllables, &ink, middle, baseline)?;
            for item in ink {
                let bounds = item.run.bounds.translated(item.x, baseline);
                self.add(
                    Mark::Text { run: item.run, x: item.x, y: baseline, role: Role::Lyric },
                    bounds,
                )?;
            }
        }
        Ok(())
    }

    fn lyric_links(
        &mut self,
        request: &Request<'_, '_>,
        syllables: &[Syllable],
        ink: &[Ink],
        middle: f64,
        baseline: f64,
    ) -> Result<(), Error> {
        let space = self.options.staff_size / 4.0;
        let first = request.columns[0].at;
        let last = request.columns.last().map(|c| c.at).unwrap_or(first);
        for (index, syllable) in syllables.iter().enumerate() {
            charge(self.work, 1)?;
            let (start, x) = anchor(request.score, syllable.anchor, request, middle, space);
            if start >= last {
                continue;
            }
            let current =
                ink.binary_search_by_key(&index, |item| item.index).ok().map(|i| &ink[i]);
            let right = current.map(|item| item.x + item.run.advance + 0.25 * space).unwrap_or(x);
            if let Some(end) = syllable.extender {
                let (at, to) = anchor(request.score, end, request, middle, space);
                let inclusive = matches!(end, Anchor::Note(_));
                if (at > first || (inclusive && at == first)) && to > right {
                    self.line(
                        Point { x: right, y: baseline + 0.2 * space },
                        Point { x: to, y: baseline + 0.2 * space },
                        0.12 * space,
                        Role::Lyric,
                    )?;
                }
            }
            if syllable.hyphen
                && start >= first
                && let Some(next) = syllables.get(index + 1)
            {
                let (at, to) = anchor(request.score, next.anchor, request, middle, space);
                let next_ink = ink
                    .binary_search_by_key(&(index + 1), |item| item.index)
                    .ok()
                    .map(|i| &ink[i]);
                let left = next_ink.map(|item| item.x - 0.25 * space).unwrap_or(to);
                let x = if at >= last { left - 0.5 * space } else { (right + left) / 2.0 };
                if left - right > 0.6 * space {
                    self.line(
                        Point { x: x - 0.3 * space, y: baseline - 0.7 * space },
                        Point { x: x + 0.3 * space, y: baseline - 0.7 * space },
                        0.12 * space,
                        Role::Lyric,
                    )?;
                }
            }
        }
        Ok(())
    }
}

fn anchor(
    score: &Score,
    anchor: Anchor,
    request: &Request<'_, '_>,
    middle: f64,
    space: f64,
) -> (Moment, f64) {
    match anchor {
        Anchor::Note(reference) => (
            score.events[reference.event.0 as usize].at,
            crate::curves::endpoint(score, reference, request, middle, space).x,
        ),
        Anchor::Moment(at) => {
            let first = request.columns[0].at;
            let last = request.columns.last().map(|c| c.at).unwrap_or(first);
            let index = request.columns.partition_point(|c| c.at < at);
            let x = if at < first {
                request.indent + 0.5 * space
            } else if at >= last {
                request.positions.last().copied().unwrap_or(0.0) - 0.5 * space
            } else {
                request.positions.get(index).copied().unwrap_or(0.0)
            };
            (at, x)
        }
    }
}
