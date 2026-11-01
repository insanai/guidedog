use gd_score::{
    Base, Chord, Duration, Event, EventId, EventKind, Note, NoteRef, Pitch, Ratio, Spanner,
    SpannerKind,
};

use crate::cursor::Cursor;
use crate::interpret::{PitchMode, Reader, State};
use crate::limits::{limit, push};
use crate::parser::syntax_error;
use crate::{Error, ErrorKind, ExprKind, Span, SyntaxId, TokenKind};

impl Reader<'_> {
    pub(crate) fn raw_pitch(
        &self,
        word: &str,
        cursor: &mut Cursor<'_>,
        state: &State,
        span: Span,
    ) -> Result<Note, Error> {
        let mut pitch = state.language.pitch(word, span)?;
        while let Some(ExprKind::Atom(TokenKind::Symbol(c @ ('\'' | ',')))) =
            cursor.peek().map(|n| &n.kind)
        {
            pitch.octave = pitch
                .octave
                .checked_add(if *c == '\'' { 1 } else { -1 })
                .ok_or_else(|| limit("octave marks", span))?;
            cursor.at += 1;
        }
        let forced = cursor.symbol('!');
        let cautionary = cursor.symbol('?');
        Ok(Note { pitch, forced, cautionary })
    }

    pub(crate) fn pitch_argument(
        &self,
        cursor: &mut Cursor<'_>,
        state: &State,
    ) -> Result<Pitch, Error> {
        let node = cursor.next()?;
        let ExprKind::Atom(TokenKind::Word(word)) = &node.kind else {
            return Err(syntax_error(node.span, "Expected a spelled reference pitch."));
        };
        Ok(self.raw_pitch(word, cursor, state, node.span)?.pitch)
    }

    fn resolve_pitch(
        &self,
        mut note: Note,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
    ) -> Result<Note, Error> {
        match state.pitch_mode {
            PitchMode::Absolute => {}
            PitchMode::Fixed(octave) => {
                note.pitch.octave = note
                    .pitch
                    .octave
                    .checked_add(octave)
                    .ok_or_else(|| limit("pitch range", span))?;
            }
            PitchMode::Relative(previous) => {
                let delta =
                    (note.pitch.step.index() - previous.step.index() + 3).rem_euclid(7) - 3;
                let index = previous.diatonic() + delta + 7 * i16::from(note.pitch.octave);
                note.pitch.octave =
                    i8::try_from(index.div_euclid(7)).map_err(|_| limit("pitch range", span))?;
                state.pitch_mode = PitchMode::Relative(note.pitch);
            }
        }
        if cursor.symbol('=') {
            let mut octave = 0i8;
            while cursor.symbol('\'') {
                octave = octave.saturating_add(1);
            }
            while cursor.symbol(',') {
                octave = octave.saturating_sub(1);
            }
            if octave != note.pitch.octave {
                return Err(Error::new(
                    ErrorKind::Pitch,
                    "The octave check disagrees with the resolved pitch.",
                )
                .at(span)
                .hint("Correct the octave check or the relative pitch marks."));
            }
        }
        for (from, to) in state.transpositions.iter().rev() {
            note.pitch = note.pitch.transpose(*from, *to).map_err(|e| e.at(span))?;
        }
        note.pitch.check_range().map_err(|e| e.at(span))?;
        Ok(note)
    }

    pub(crate) fn note(
        &mut self,
        word: &str,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
    ) -> Result<(), Error> {
        let notes = if word == "q" {
            if state.last_chord.is_empty() {
                return Err(syntax_error(span, "Chord repetition needs a preceding chord."));
            }
            state.last_chord.clone()
        } else if matches!(word, "r" | "s" | "R") {
            Vec::new()
        } else {
            let raw = self.raw_pitch(word, cursor, state, span)?;
            vec![self.resolve_pitch(raw, cursor, state, span)?]
        };
        let duration = read_duration(cursor, state.duration)?;
        state.duration = duration;
        let duration = Duration { scale: duration.scale.checked_mul(state.scale)?, ..duration };
        let kind = match word {
            "r" => EventKind::Rest(duration),
            "s" => EventKind::Skip(duration),
            "R" => EventKind::MeasureRest(duration),
            _ => {
                state.last_chord.clone_from(&notes);
                EventKind::Chord(Chord {
                    notes,
                    duration,
                    direction: state.direction,
                    auto_beam: state.auto_beam,
                    grace_slash: state.grace_slash,
                })
            }
        };
        self.emit(state, kind, span)?;
        Ok(())
    }

    pub(crate) fn chord(
        &mut self,
        ids: &[SyntaxId],
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
    ) -> Result<(), Error> {
        let mut inner = Cursor::new(self.syntax, ids);
        let mut notes = Vec::new();
        let mut first_mode = None;
        let mut tied = Vec::new();
        while let Some(node) = inner.peek() {
            self.work.take(1, node.span)?;
            let node = inner.next()?;
            let ExprKind::Atom(TokenKind::Word(word)) = &node.kind else {
                return Err(syntax_error(
                    node.span,
                    "A chord contains pitches and optional ties.",
                ));
            };
            let raw = self.raw_pitch(word, &mut inner, state, node.span)?;
            let note = self.resolve_pitch(raw, &mut inner, state, node.span)?;
            first_mode.get_or_insert(state.pitch_mode);
            if inner.symbol('~') {
                tied.push(notes.len());
            }
            if notes.len() >= 16 {
                return Err(limit("notes per chord", span));
            }
            push(&mut notes, note, span)?;
        }
        if notes.is_empty() {
            return Err(syntax_error(span, "A chord must contain a pitch."));
        }
        if let Some(mode) = first_mode {
            state.pitch_mode = mode;
        }
        state.last_chord.clone_from(&notes);
        let written = read_duration(cursor, state.duration)?;
        state.duration = written;
        let duration = Duration { scale: written.scale.checked_mul(state.scale)?, ..written };
        let id = self.emit(
            state,
            EventKind::Chord(Chord {
                notes,
                duration,
                direction: state.direction,
                auto_beam: state.auto_beam,
                grace_slash: state.grace_slash,
            }),
            span,
        )?;
        for note in tied {
            state.ties.push(NoteRef { event: id, note: note as u8 });
        }
        Ok(())
    }

    pub(crate) fn emit(
        &mut self,
        state: &mut State,
        kind: EventKind,
        span: Span,
    ) -> Result<EventId, Error> {
        if self.score.events.len() >= self.limits.events {
            return Err(limit("events after expansion", span));
        }
        let id = EventId(
            u32::try_from(self.score.events.len()).map_err(|_| limit("event indices", span))?,
        );
        if kind.duration().is_some() {
            self.attach_ties(state, &kind, id, span)?;
        }
        let time = kind.duration().map(|d| d.time()).transpose()?.unwrap_or(Ratio::ZERO);
        let marks = if kind.duration().is_some() {
            std::mem::take(&mut state.pending)
        } else {
            Vec::new()
        };
        push(
            &mut self.score.events,
            Event { voice: state.voice, at: state.at, kind, span, marks },
            span,
        )?;
        push(&mut self.score.voices[state.voice.0 as usize].events, id, span)?;
        if time > Ratio::ZERO {
            state.last = Some(id);
            if state.at.grace < Ratio::ZERO {
                state.at.grace = state.at.grace.checked_add(time)?;
            } else {
                state.at.main = state.at.main.checked_add(time)?;
            }
        }
        Ok(id)
    }

    fn attach_ties(
        &mut self,
        state: &mut State,
        kind: &EventKind,
        id: EventId,
        span: Span,
    ) -> Result<(), Error> {
        let ties = std::mem::take(&mut state.ties);
        for reference in ties {
            self.memory.take(256, span)?;
            let EventKind::Chord(previous) = &self.score.events[reference.event.0 as usize].kind
            else {
                return Err(syntax_error(span, "Only pitched notes can start a tie."));
            };
            let EventKind::Chord(next) = kind else {
                return Err(syntax_error(span, "A tie must continue into a pitched chord."));
            };
            let pitch = previous.notes[reference.note as usize].pitch;
            let note = next.notes.iter().position(|n| n.pitch == pitch).ok_or_else(|| {
                syntax_error(span, "The tied pitch is missing from the following chord.")
            })?;
            push(
                &mut self.score.spanners,
                Spanner {
                    start: reference,
                    end: NoteRef { event: id, note: note as u8 },
                    kind: SpannerKind::Tie,
                    direction: gd_score::Direction::Auto,
                    span,
                },
                span,
            )?;
        }
        Ok(())
    }
}

pub(crate) fn read_duration(
    cursor: &mut Cursor<'_>,
    previous: Duration,
) -> Result<Duration, Error> {
    let span = cursor.span();
    let explicit =
        matches!(cursor.peek().map(|n| &n.kind), Some(ExprKind::Atom(TokenKind::Number(_))));
    let mut duration = if explicit {
        Duration {
            base: Base::from_denominator(cursor.number()?).map_err(|e| e.at(span))?,
            ..Duration::default()
        }
    } else if cursor.command("breve") {
        Duration { base: Base::Breve, ..Duration::default() }
    } else {
        previous
    };
    if cursor.symbol('.') {
        duration.dots = 1;
        while cursor.symbol('.') {
            duration.dots = duration
                .dots
                .checked_add(1)
                .filter(|n| *n <= 8)
                .ok_or_else(|| limit("augmentation dots", span))?;
        }
    }
    while cursor.symbol('*') {
        let numerator = cursor.integer()?;
        let denominator = if cursor.symbol('/') { cursor.integer()? } else { 1 };
        duration.scale = duration.scale.checked_mul(Ratio::new(numerator, denominator)?)?;
    }
    duration.time().map_err(|e| e.at(span))?;
    Ok(duration)
}
