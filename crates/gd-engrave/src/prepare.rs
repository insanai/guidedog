use std::collections::BTreeMap;

use gd_score::{
    Alter, Base, Clef, Direction, Error, EventKind, Key, Meter, NoteRef, Ratio, Score, Step,
};

use crate::columns::Column;
use crate::limits::{charge, failure, push};
use crate::{Fonts, Options, Point, Rect};

#[derive(Clone, Debug, Default)]
pub(crate) struct StaffState {
    pub(crate) clef: Clef,
    pub(crate) key: Key,
    pub(crate) meter: Meter,
    remembered: BTreeMap<(Step, i8), Alter>,
    measure: usize,
}

impl StaffState {
    fn snapshot(&self) -> Self {
        Self {
            clef: self.clef,
            key: self.key,
            meter: self.meter.clone(),
            measure: self.measure,
            remembered: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Head {
    pub(crate) note: u8,
    pub(crate) at: Point,
    pub(crate) accidental: Option<&'static str>,
    pub(crate) accidental_x: f64,
    pub(crate) cautionary: bool,
    pub(crate) continuation: bool,
    pub(crate) accidental_forced: bool,
    pub(crate) dots: Vec<Point>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct EventInfo {
    pub(crate) id: gd_score::EventId,
    pub(crate) staff: usize,
    pub(crate) state: StaffState,
    pub(crate) prefix: f64,
    pub(crate) previous_key: Option<Key>,
    pub(crate) notation_x: f64,
    pub(crate) heads: Vec<Head>,
    pub(crate) head_glyph: &'static str,
    pub(crate) head_scale: f64,
    pub(crate) bounds: Rect,
    pub(crate) up: bool,
    pub(crate) stem_x: f64,
    pub(crate) stem_base: f64,
    pub(crate) stem_tip: f64,
    pub(crate) flags: u8,
    pub(crate) beam: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Plan {
    pub(crate) events: Vec<EventInfo>,
    pub(crate) measures: Vec<Vec<StaffState>>,
    pub(crate) beams: Vec<crate::beaming::BeamGroup>,
}

impl Plan {
    pub(crate) fn load(
        &mut self,
        score: &Score,
        columns: &[Column],
        fonts: &Fonts<'_>,
        options: &mut Options<'_>,
        work: &mut usize,
    ) -> Result<(), Error> {
        while self.events.len() < score.events.len() {
            push(&mut self.events, EventInfo::default(), &mut options.reserve)?;
        }
        self.events.truncate(score.events.len());
        self.measures.clear();
        for _ in &score.measures {
            push(&mut self.measures, Vec::new(), &mut options.reserve)?;
        }
        let mut states = vec![StaffState::default(); score.staves.len()];
        for column in columns {
            charge(work, score.staves.len() + column.events.len())?;
            let measure =
                score.measures.partition_point(|m| m.start <= column.at.main).saturating_sub(1);
            for state in &mut states {
                if state.measure != measure {
                    state.remembered.clear();
                    state.measure = measure;
                }
            }
            let prefixes = self.contexts(column, score, &mut states)?;
            for id in &column.events {
                let event = &score.events[id.0 as usize];
                let info = &mut self.events[id.0 as usize];
                let state = &mut states[info.staff];
                info.prefix = prefixes[info.staff];
                info.state = state.snapshot();
                if let EventKind::Chord(chord) = &event.kind {
                    heads(info, chord, state, fonts, event.at.grace < Ratio::ZERO, work)?;
                    for head in &mut info.heads {
                        head.continuation = score.spanners.iter().any(|s| {
                            s.kind == gd_score::SpannerKind::Tie
                                && s.end == NoteRef { event: *id, note: head.note }
                        });
                    }
                }
            }
            if let Some(snapshot) = self.measures.get_mut(measure)
                && (snapshot.is_empty() || score.measures[measure].start == column.at.main)
            {
                *snapshot = states.iter().map(StaffState::snapshot).collect();
            }
        }
        crate::beaming::groups(self, score, fonts, options, work)?;
        Ok(())
    }
    fn contexts(
        &mut self,
        column: &Column,
        score: &Score,
        states: &mut [StaffState],
    ) -> Result<Vec<f64>, Error> {
        let mut prefixes = vec![0.0; states.len()];
        for id in &column.events {
            let event = &score.events[id.0 as usize];
            let staff = score.voices[event.voice.0 as usize].staff.0 as usize;
            let state = &mut states[staff];
            let info = &mut self.events[id.0 as usize];
            info.heads.clear();
            info.id = *id;
            info.beam = None;
            info.previous_key = None;
            info.staff = staff;
            info.notation_x = prefixes[staff];
            match &event.kind {
                EventKind::Clef(clef) => {
                    state.clef = *clef;
                    prefixes[staff] += 2.5;
                }
                EventKind::Key(key) => {
                    info.previous_key = Some(state.key);
                    state.key = *key;
                    state.remembered.clear();
                    prefixes[staff] +=
                        key.alterations()?.iter().filter(|a| **a != Alter::Natural).count() as f64
                            * 0.9
                            + 0.7;
                }
                EventKind::Time(meter) => {
                    state.meter = meter.clone();
                    prefixes[staff] += 2.5;
                }
                _ => {}
            }
            info.state = state.snapshot();
        }
        Ok(prefixes)
    }
}

fn heads(
    info: &mut EventInfo,
    chord: &gd_score::Chord,
    state: &mut StaffState,
    fonts: &Fonts<'_>,
    grace: bool,
    work: &mut usize,
) -> Result<(), Error> {
    charge(work, chord.notes.len())?;
    info.head_glyph = match chord.duration.base {
        Base::Breve => "noteheadDoubleWhole",
        Base::Whole => "noteheadWhole",
        Base::Half => "noteheadHalf",
        _ => "noteheadBlack",
    };
    info.head_scale = if grace { 0.7 } else { 1.0 };
    info.flags = match chord.duration.base {
        Base::Eighth => 1,
        Base::Sixteenth => 2,
        Base::ThirtySecond => 3,
        Base::SixtyFourth => 4,
        Base::HundredTwentyEighth => 5,
        _ => 0,
    };
    let positions: Vec<_> =
        chord.notes.iter().map(|n| n.pitch.diatonic() - state.clef.middle()).collect();
    let low = positions.iter().min().copied().unwrap_or(0);
    let high = positions.iter().max().copied().unwrap_or(0);
    info.up = match chord.direction {
        Direction::Up => true,
        Direction::Down => false,
        Direction::Auto => low + high < 0,
    };
    let signature = state.key.alterations()?;
    for (note, position) in chord.notes.iter().zip(positions) {
        let previous = state
            .remembered
            .get(&(note.pitch.step, note.pitch.octave))
            .copied()
            .unwrap_or(signature[note.pitch.step.index() as usize]);
        let shown = previous != note.pitch.alter || note.forced || note.cautionary;
        state.remembered.insert((note.pitch.step, note.pitch.octave), note.pitch.alter);
        let y = -f64::from(position) / 2.0;
        let dots = (0..chord.duration.dots)
            .map(|dot| Point {
                x: 1.8 + f64::from(dot) * 0.6,
                y: if position % 2 == 0 { y - 0.5 } else { y },
            })
            .collect();
        info.heads.push(Head {
            note: info.heads.len() as u8,
            at: Point { x: 0.0, y },
            accidental: shown.then_some(accidental(note.pitch.alter)),
            accidental_x: -1.2,
            cautionary: note.cautionary,
            continuation: false,
            accidental_forced: note.forced || note.cautionary,
            dots,
        });
    }
    arrange_heads(info, fonts)?;
    stems(info, fonts)?;
    Ok(())
}

pub(crate) fn arrange_heads(info: &mut EventInfo, fonts: &Fonts<'_>) -> Result<(), Error> {
    let glyph = fonts.glyph(info.head_glyph)?;
    let data = fonts.glyph_data(glyph).ok_or_else(|| failure("Missing notehead bounds."))?;
    let width = (data.bounds.right - data.bounds.left) * info.head_scale;
    let mut order: Vec<_> = (0..info.heads.len()).collect();
    order.sort_by(|a, b| info.heads[*a].at.y.total_cmp(&info.heads[*b].at.y));
    if info.up {
        order.reverse();
    }
    let mut previous: Option<(f64, bool)> = None;
    let mut bounds = None;
    for index in order {
        let head = &mut info.heads[index];
        let shifted =
            previous.is_some_and(|(y, shifted)| (y - head.at.y).abs() == 0.5 && !shifted);
        head.at.x = if shifted { if info.up { width } else { -width } } else { 0.0 };
        previous = Some((head.at.y, shifted));
        let ink = Rect {
            left: data.bounds.left * info.head_scale + head.at.x,
            right: data.bounds.right * info.head_scale + head.at.x,
            top: data.bounds.top * info.head_scale + head.at.y,
            bottom: data.bounds.bottom * info.head_scale + head.at.y,
        };
        bounds = Some(bounds.map(|b: Rect| b.union(ink)).unwrap_or(ink));
    }
    info.bounds = bounds.unwrap_or_default();
    Ok(())
}

pub(crate) fn stems(info: &mut EventInfo, fonts: &Fonts<'_>) -> Result<(), Error> {
    let glyph = fonts.glyph(info.head_glyph)?;
    let data = fonts.glyph_data(glyph).ok_or_else(|| failure("Missing stem anchors."))?;
    let anchor = data
        .anchors
        .get(if info.up { "stemUpSE" } else { "stemDownNW" })
        .copied()
        .unwrap_or(Point { x: if info.up { 1.1 } else { 0.05 }, y: 0.0 });
    info.stem_x = anchor.x * info.head_scale;
    let top = info.heads.iter().map(|h| h.at.y).fold(f64::INFINITY, f64::min);
    let bottom = info.heads.iter().map(|h| h.at.y).fold(f64::NEG_INFINITY, f64::max);
    let length = if info.head_scale < 1.0 { 2.5 } else { 3.5 }
        + f64::from(info.flags.saturating_sub(2)) * 0.5;
    info.stem_base = if info.up { bottom } else { top } - anchor.y * info.head_scale;
    info.stem_tip = if info.up { (top - length).min(0.0) } else { (bottom + length).max(0.0) };
    Ok(())
}

pub(crate) fn accidental(alter: Alter) -> &'static str {
    match alter {
        Alter::DoubleFlat => "accidentalDoubleFlat",
        Alter::Flat => "accidentalFlat",
        Alter::Natural => "accidentalNatural",
        Alter::Sharp => "accidentalSharp",
        Alter::DoubleSharp => "accidentalDoubleSharp",
    }
}
