use gd_score::{Annotation, AnnotationKind, BarStyle, EventKind, NoteRef, SpannerKind, TextStyle};

use crate::cursor::Cursor;
use crate::interpret::{Reader, State, unsupported};
use crate::notes::read_duration;
use crate::parser::syntax_error;
use crate::{Error, ExprKind, Span, TokenKind};

impl Reader<'_> {
    pub(crate) fn repeat(
        &mut self,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
        depth: usize,
    ) -> Result<(), Error> {
        let kind = cursor.text()?;
        let count = cursor.number()?;
        if count == 0 {
            return Err(syntax_error(span, "A repeat needs a positive count."));
        }
        let body = cursor.sequence()?;
        match kind {
            "unfold" => {
                let pitch_mode = state.pitch_mode;
                for _ in 0..count {
                    state.pitch_mode = pitch_mode;
                    self.sequence(body, state, depth + 1)?;
                }
            }
            "volta" => {
                self.emit(state, EventKind::Bar(BarStyle::RepeatStart), span)?;
                self.sequence(body, state, depth + 1)?;
                if cursor.command("alternative") {
                    self.alternatives(cursor, state, span, depth, count)?;
                } else {
                    self.emit(state, EventKind::Bar(BarStyle::RepeatEnd), span)?;
                }
                if count != 2
                    && let Some(last) = state.last
                {
                    self.score.events[last.0 as usize].marks.push(Annotation {
                        kind: AnnotationKind::Text(format!("{count}x"), TextStyle::Regular),
                        direction: gd_score::Direction::Up,
                    });
                }
            }
            _ => return Err(unsupported(span, &format!("repeat {kind}"))),
        }
        Ok(())
    }

    fn alternatives(
        &mut self,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
        depth: usize,
        count: u16,
    ) -> Result<(), Error> {
        let alternatives = cursor.sequence()?;
        if alternatives.is_empty() || alternatives.len() > usize::from(count) {
            return Err(syntax_error(
                span,
                "Repeat alternatives must number from one to the repeat count.",
            ));
        }
        for (index, id) in alternatives.iter().enumerate() {
            let node = self
                .syntax
                .node(*id)
                .ok_or_else(|| syntax_error(span, "Invalid alternative syntax."))?;
            let ExprKind::Sequence(ids) = &node.kind else {
                return Err(syntax_error(node.span, "Each repeat alternative needs braces."));
            };
            let first = self.score.events.len();
            self.sequence(ids, state, depth + 1)?;
            let events: Vec<_> = self.score.events[first..]
                .iter()
                .enumerate()
                .filter(|(_, e)| e.kind.duration().is_some())
                .map(|(i, _)| gd_score::EventId((first + i) as u32))
                .collect();
            if let (Some(first), Some(last)) = (events.first(), events.last()) {
                self.spanner(
                    NoteRef { event: *first, note: 0 },
                    NoteRef { event: *last, note: 0 },
                    SpannerKind::Volta((index + 1).to_string()),
                    gd_score::Direction::Up,
                    span,
                )?;
            }
            self.emit(
                state,
                EventKind::Bar(if index + 1 < alternatives.len() {
                    BarStyle::RepeatEnd
                } else {
                    BarStyle::Final
                }),
                span,
            )?;
        }
        Ok(())
    }

    pub(crate) fn chord_names(
        &mut self,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
    ) -> Result<(), Error> {
        let ids = cursor.sequence()?;
        let mut inner = Cursor::new(self.syntax, ids);
        while inner.peek().is_some() {
            self.work.take(1, inner.span())?;
            self.memory.take(512, inner.span())?;
            if inner.symbol('|') {
                self.bar_checks.push((state.at.main, span));
                continue;
            }
            let node = inner.next()?;
            let ExprKind::Atom(TokenKind::Word(word)) = &node.kind else {
                return Err(syntax_error(
                    node.span,
                    "Chord mode needs a root and optional chord quality.",
                ));
            };
            let root = self.raw_pitch(word, &mut inner, state, node.span)?.pitch;
            let written = read_duration(&mut inner, state.duration)?;
            state.duration = written;
            let duration =
                gd_score::Duration { scale: written.scale.checked_mul(state.scale)?, ..written };
            let mut text = pitch_name(root);
            if inner.symbol(':') {
                let mut quality = String::new();
                while let Some(part) = inner.peek() {
                    match &part.kind {
                        ExprKind::Atom(TokenKind::Word(word)) if quality.is_empty() => {
                            self.memory.take(word.len() + 32, part.span)?;
                            quality.push_str(word)
                        }
                        ExprKind::Atom(TokenKind::Number(number)) => {
                            self.memory.take(number.len() + 32, part.span)?;
                            quality.push_str(number);
                        }
                        ExprKind::Atom(TokenKind::Symbol('.' | '+' | '-')) => {
                            if let ExprKind::Atom(TokenKind::Symbol(c)) = part.kind {
                                quality.push(c);
                            }
                        }
                        _ => break,
                    }
                    inner.at += 1;
                }
                if !CHORD_QUALITIES.contains(&quality.as_str()) {
                    return Err(unsupported(node.span, &format!("chord quality {quality}")));
                }
                text.push_str(&quality);
            }
            if inner.symbol('/') {
                let bass = inner.text()?;
                let bass = state.language.pitch(bass, node.span)?;
                text.push('/');
                text.push_str(&pitch_name(bass));
            }
            self.emit(state, EventKind::ChordName(text, duration), node.span)?;
        }
        Ok(())
    }
}

fn pitch_name(pitch: gd_score::Pitch) -> String {
    let letter = match pitch.step {
        gd_score::Step::C => 'C',
        gd_score::Step::D => 'D',
        gd_score::Step::E => 'E',
        gd_score::Step::F => 'F',
        gd_score::Step::G => 'G',
        gd_score::Step::A => 'A',
        gd_score::Step::B => 'B',
    };
    let suffix = match pitch.alter {
        gd_score::Alter::DoubleFlat => "bb",
        gd_score::Alter::Flat => "b",
        gd_score::Alter::Natural => "",
        gd_score::Alter::Sharp => "#",
        gd_score::Alter::DoubleSharp => "##",
    };
    format!("{letter}{suffix}")
}

const CHORD_QUALITIES: &[&str] = &[
    "m", "min", "maj", "dim", "aug", "sus2", "sus4", "5", "6", "7", "9", "11", "13", "m7", "min7",
    "maj7", "dim7", "m7.5-", "7+", "7.5+", "7.5-", "sus", "7sus4",
];
