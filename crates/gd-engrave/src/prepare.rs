use std::collections::{BTreeMap, BTreeSet};

use gd_score::{Alter, Base, Clef, Direction, Error, EventKind, Key, Meter, Ratio, Score, Step};

use crate::columns::Column;
use crate::limits::{charge, failure, push};
use crate::{Fonts, Options, Point, Rect};

#[derive(Clone, Debug, Default)]
pub(crate) struct StaffState {
    pub(crate) clef: Clef,
    pub(crate) key: Key,
    pub(crate) meter: Meter,
    remembered: BTreeMap<(Step, i8), Option<Alter>>,
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

type AccidentalChanges = BTreeMap<(usize, Step, i8), Option<Alter>>;

fn accidental_changes(
    column: &Column,
    score: &Score,
    work: &mut usize,
) -> Result<AccidentalChanges, Error> {
    let mut changes = AccidentalChanges::new();
    for id in &column.events {
        let event = &score.events[id.0 as usize];
        let staff = score.voices[event.voice.0 as usize].staff.0 as usize;
        if let EventKind::Chord(chord) = &event.kind {
            for note in &chord.notes {
                charge(work, 1)?;
                let pitch = note.pitch;
                changes
                    .entry((staff, pitch.step, pitch.octave))
                    .and_modify(|previous| {
                        if *previous != Some(pitch.alter) {
                            *previous = None;
                        }
                    })
                    .or_insert(Some(pitch.alter));
            }
        }
    }
    Ok(changes)
}

#[derive(Clone, Debug)]
pub(crate) struct Head {
    pub(crate) note: u8,
    pub(crate) shared: bool,
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
    pub(crate) voice_shift: f64,
    pub(crate) dot_down: bool,
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
    pub(crate) beat: (usize, i64, i64),
    pub(crate) tuplet: Option<usize>,
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
        charge(work, score.spanners.len())?;
        let tied: BTreeSet<_> = score
            .spanners
            .iter()
            .filter(|s| s.kind == gd_score::SpannerKind::Tie)
            .map(|s| (s.end.event.0, s.end.note))
            .collect();
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
            let changes = accidental_changes(column, score, work)?;
            for (&(staff, step, octave), &alter) in &changes {
                if alter.is_none() {
                    states[staff].remembered.insert((step, octave), None);
                }
            }
            for id in &column.events {
                let event = &score.events[id.0 as usize];
                let info = &mut self.events[id.0 as usize];
                let state = &mut states[info.staff];
                info.prefix = prefixes[info.staff];
                info.state = state.snapshot();
                info.dot_down = score.voices[event.voice.0 as usize].direction == Direction::Down;
                if let EventKind::Chord(chord) = &event.kind {
                    heads(
                        info,
                        chord,
                        state,
                        fonts,
                        event.at.grace < Ratio::ZERO,
                        crate::voices::direction(score, event),
                        work,
                    )?;
                    for head in &mut info.heads {
                        charge(work, tied.len().checked_ilog2().unwrap_or(0) as usize + 1)?;
                        head.continuation = tied.contains(&(id.0, head.note));
                    }
                }
            }
            crate::voices::arrange(self, column, score, fonts, work)?;
            for ((staff, step, octave), alter) in changes {
                states[staff].remembered.insert((step, octave), alter);
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
            let mut heads = std::mem::take(&mut info.heads);
            heads.clear();
            *info = EventInfo { heads, ..Default::default() };
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
                    let old = state.key.alterations()?;
                    let new = key.alterations()?;
                    let canceled = old
                        .iter()
                        .zip(new)
                        .filter(|(old, new)| **old != Alter::Natural && **old != *new)
                        .count();
                    prefixes[staff] += canceled as f64 * 0.8;
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
    direction: Direction,
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
    info.up = match direction {
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
            .unwrap_or(Some(signature[note.pitch.step.index() as usize]));
        let shown = previous != Some(note.pitch.alter) || note.forced || note.cautionary;
        let y = -f64::from(position) / 2.0;
        let dots = (0..chord.duration.dots)
            .map(|dot| Point {
                x: 1.8 + f64::from(dot) * 0.6,
                y: if position % 2 == 0 { y - 0.5 } else { y },
            })
            .collect();
        info.heads.push(Head {
            note: info.heads.len() as u8,
            shared: false,
            at: Point { x: 0.0, y },
            accidental: shown.then_some(accidental(note.pitch.alter)),
            accidental_x: -1.2,
            cautionary: note.cautionary,
            continuation: false,
            accidental_forced: note.forced || note.cautionary,
            dots,
        });
    }
    arrange_heads(info, fonts, work)?;
    stems(info, fonts)?;
    crate::accidentals::arrange(info, fonts, work)?;
    Ok(())
}

pub(crate) fn arrange_heads(
    info: &mut EventInfo,
    fonts: &Fonts<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    charge(work, info.heads.len().saturating_mul(info.heads.len()).saturating_mul(40))?;
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
        head.at.x += info.voice_shift;
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
    crate::dots::arrange(info);
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
    info.stem_x = anchor.x * info.head_scale + info.voice_shift;
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

pub(crate) fn line_clefs(
    plan: &Plan,
    score: &Score,
    columns: &[Column],
    work: &mut usize,
) -> Result<[(Clef, Clef); 24], Error> {
    let mut result = [(Clef::Treble, Clef::Treble); 24];
    let measure =
        score.measures.partition_point(|m| m.start <= columns[0].at.main).saturating_sub(1);
    if let Some(states) = plan.measures.get(measure) {
        for (index, state) in states.iter().enumerate() {
            result[index] = (state.clef, state.clef);
        }
    }
    for column in &columns[..columns.len().saturating_sub(1)] {
        for id in &column.events {
            charge(work, 1)?;
            let info = &plan.events[id.0 as usize];
            result[info.staff].1 = info.state.clef;
        }
    }
    Ok(result)
}
