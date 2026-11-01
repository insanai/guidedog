use gd_score::{
    Base, Chord, Duration, Event, EventId, EventKind, Note, NoteRef, Pitch, Ratio, Spanner,
    SpannerKind,
};

use crate::cursor::Cursor;
use crate::interpret::{PitchMode, Reader, State};
use crate::limits::{limit, push};
use crate::parser::syntax_error;
use crate::{Error, ErrorKind, ExprKind, Span, SyntaxId, TokenKind};

type ChordPostfixes = (Option<(gd_score::Direction, Span)>, Vec<gd_score::Annotation>);

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
    ) -> Result<(Note, Pitch), Error> {
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
        let untransposed = note.pitch;
        for (from, to) in state.transpositions.iter().rev() {
            note.pitch = note.pitch.transpose(*from, *to).map_err(|e| e.or_at(span))?;
        }
        note.pitch.check_range().map_err(|e| e.or_at(span))?;
        Ok((note, untransposed))
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
            if let (PitchMode::Relative(_), Some(first)) =
                (state.pitch_mode, state.last_chord.first())
            {
                state.pitch_mode = PitchMode::Relative(*first);
            }
            let mut repeated = Vec::with_capacity(state.last_chord.len());
            for &(mut pitch) in &state.last_chord {
                for (from, to) in state.transpositions.iter().rev() {
                    pitch = pitch.transpose(*from, *to).map_err(|e| e.or_at(span))?;
                }
                pitch.check_range().map_err(|e| e.or_at(span))?;
                repeated.push(Note { pitch, forced: false, cautionary: false });
            }
            repeated
        } else if matches!(word, "r" | "s" | "R") {
            Vec::new()
        } else {
            let raw = self.raw_pitch(word, cursor, state, span)?;
            vec![self.resolve_pitch(raw, cursor, state, span)?.0]
        };
        let duration = read_duration(cursor, state.duration)?;
        state.duration = duration;
        let scale = duration.scale.checked_mul(state.scale).map_err(|e| e.or_at(span))?;
        let duration = Duration { scale, ..duration };
        let kind = match word {
            "r" => EventKind::Rest(duration),
            "s" => EventKind::Skip(duration),
            "R" => EventKind::MeasureRest(duration),
            _ => EventKind::Chord(Chord {
                notes,
                duration,
                direction: state.direction,
                auto_beam: state.auto_beam,
                grace_slash: state.grace_slash,
            }),
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
        let mut raw_pitches = Vec::new();
        let mut first_mode = None;
        let mut tied = Vec::new();
        let mut chord_marks = Vec::new();
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
            let (note, untransposed) = self.resolve_pitch(raw, &mut inner, state, node.span)?;
            first_mode.get_or_insert(state.pitch_mode);
            let (tie_info, extra_marks) = self.chord_postfixes(&mut inner, state)?;
            chord_marks.extend(extra_marks);
            if let Some((tie_dir, t_span)) = tie_info {
                tied.push((notes.len(), tie_dir, t_span));
            }
            if notes.len() >= 16 {
                return Err(limit("notes per chord", span));
            }
            push(&mut raw_pitches, untransposed, span)?;
            push(&mut notes, note, span)?;
        }
        if notes.is_empty() {
            return Err(syntax_error(span, "A chord must contain a pitch."));
        }
        if let Some(mode) = first_mode {
            state.pitch_mode = mode;
        }
        state.last_chord = raw_pitches;
        let written = read_duration(cursor, state.duration)?;
        state.duration = written;
        let scale = written.scale.checked_mul(state.scale).map_err(|e| e.or_at(span))?;
        let duration = Duration { scale, ..written };
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
        self.score.events[id.0 as usize].marks.extend(chord_marks);
        for (note, dir, t_span) in tied {
            state.ties.push((NoteRef { event: id, note: note as u8 }, dir, t_span));
        }
        Ok(())
    }

    fn chord_postfixes(
        &self,
        inner: &mut Cursor<'_>,
        state: &State,
    ) -> Result<ChordPostfixes, Error> {
        let mut tie_span = None;
        let mut tie_dir = state.tie_direction;
        let mut marks = Vec::new();
        while let Some(post) = inner.peek() {
            match &post.kind {
                ExprKind::Atom(TokenKind::Symbol('~')) => {
                    tie_span = Some(post.span);
                    inner.at += 1;
                }
                ExprKind::Atom(TokenKind::Symbol(p @ ('-' | '^' | '_'))) => {
                    let prefix = *p;
                    let p_span = post.span;
                    inner.at += 1;
                    if inner.symbol('~') {
                        tie_dir = match prefix {
                            '^' => gd_score::Direction::Up,
                            '_' => gd_score::Direction::Down,
                            _ => state.tie_direction,
                        };
                        tie_span = Some(p_span);
                    } else if let Some(mark) = self.chord_note_mark(prefix, inner, p_span)? {
                        marks.push(mark);
                    }
                }
                _ => break,
            }
        }
        Ok((tie_span.map(|s| (tie_dir, s)), marks))
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
            if state.at.grace == Ratio::ZERO
                && matches!(kind, EventKind::Chord(_))
                && !state.grace_slurs.is_empty()
            {
                for (start, g_span) in std::mem::take(&mut state.grace_slurs) {
                    self.spanner(
                        start,
                        NoteRef { event: id, note: 0 },
                        SpannerKind::Slur,
                        state.slur_direction,
                        g_span,
                    )?;
                }
            }
        }
        let time = kind
            .duration()
            .map(|d| d.time().map_err(|e| e.or_at(span)))
            .transpose()?
            .unwrap_or(Ratio::ZERO);
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
                state.at.grace = state.at.grace.checked_add(time).map_err(|e| e.or_at(span))?;
            } else {
                state.at.main = state.at.main.checked_add(time).map_err(|e| e.or_at(span))?;
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
        for (reference, direction, tie_span) in ties {
            self.memory.take(256, span)?;
            let EventKind::Chord(previous) = &self.score.events[reference.event.0 as usize].kind
            else {
                return Err(syntax_error(tie_span, "Only pitched notes can start a tie."));
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
                    direction,
                    span: tie_span,
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
            base: Base::from_denominator(cursor.number()?).map_err(|e| e.or_at(span))?,
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
        let m_span = cursor.span();
        let numerator = cursor.integer()?;
        let denominator = if cursor.symbol('/') { cursor.integer()? } else { 1 };
        let factor = Ratio::new(numerator, denominator).map_err(|e| e.or_at(m_span))?;
        duration.scale = duration.scale.checked_mul(factor).map_err(|e| e.or_at(m_span))?;
    }
    duration.time().map_err(|e| e.or_at(span))?;
    Ok(duration)
}
