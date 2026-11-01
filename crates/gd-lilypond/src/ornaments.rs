use gd_score::{
    Annotation, AnnotationKind, Direction, EventKind, NoteRef, Ratio, Spanner, SpannerKind,
    TextStyle,
};

use crate::cursor::Cursor;
use crate::interpret::{Reader, State, unsupported};
use crate::limits::{limit, push};
use crate::notes::read_duration;
use crate::parser::syntax_error;
use crate::{Error, ExprKind, Span, TokenKind};

const ARTICULATIONS: &[&str] = &[
    "staccato",
    "accent",
    "tenuto",
    "marcato",
    "fermata",
    "trill",
    "turn",
    "mordent",
    "prall",
    "staccatissimo",
    "portato",
    "stopped",
    "open",
    "flageolet",
    "thumb",
];
const DYNAMICS: &[&str] =
    &["pppp", "ppp", "pp", "p", "mp", "mf", "f", "ff", "fff", "ffff", "fp", "sf", "sfz", "rfz"];

impl Reader<'_> {
    pub(crate) fn postfix(
        &mut self,
        symbol: char,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
    ) -> Result<(), Error> {
        let last =
            state.last.ok_or_else(|| syntax_error(span, "A marking needs a preceding note."))?;
        let reference = NoteRef { event: last, note: 0 };
        match symbol {
            '~' => {
                let EventKind::Chord(chord) = &self.score.events[last.0 as usize].kind else {
                    return Err(syntax_error(span, "A rest cannot start a tie."));
                };
                for note in 0..chord.notes.len() {
                    push(&mut state.ties, NoteRef { event: last, note: note as u8 }, span)?;
                }
            }
            '(' => push(&mut state.slurs, (reference, false, span), span)?,
            ')' => self.close_slur(state, false, span)?,
            '[' => {
                if state.beam.is_some() {
                    return Err(syntax_error(span, "Manual beams cannot nest."));
                }
                state.beam = Some((reference, span));
            }
            ']' => {
                let (start, opening) = state
                    .beam
                    .take()
                    .ok_or_else(|| syntax_error(span, "There is no open manual beam."))?;
                self.spanner(start, reference, SpannerKind::Beam, state.direction, opening)?;
            }
            '-' | '^' | '_' => self.script(symbol, cursor, state, span)?,
            _ => return Err(syntax_error(span, "Unknown note attachment.")),
        }
        Ok(())
    }

    fn script(
        &mut self,
        prefix: char,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
    ) -> Result<(), Error> {
        let direction = match prefix {
            '^' => Direction::Up,
            '_' => Direction::Down,
            _ => Direction::Auto,
        };
        let node = cursor.next()?;
        let annotation = match &node.kind {
            ExprKind::Atom(TokenKind::Text(text)) => {
                AnnotationKind::Text(text.clone(), TextStyle::Regular)
            }
            ExprKind::Atom(TokenKind::Number(number)) => {
                let finger =
                    number.parse::<u8>().ok().filter(|n| (1..=5).contains(n)).ok_or_else(
                        || syntax_error(node.span, "A fingering must be from 1 to 5."),
                    )?;
                AnnotationKind::Fingering(finger)
            }
            ExprKind::Atom(TokenKind::Symbol(symbol)) => AnnotationKind::Articulation(
                match symbol {
                    '.' => "staccato",
                    '-' => "tenuto",
                    '>' => "accent",
                    '^' => "marcato",
                    '_' => "portato",
                    '!' => "staccatissimo",
                    '+' => "stopped",
                    _ => {
                        return Err(syntax_error(
                            node.span,
                            "Unsupported articulation shorthand.",
                        ));
                    }
                }
                .into(),
            ),
            ExprKind::Atom(TokenKind::Command(name)) if name == "markup" => {
                let (text, style) = self.markup(cursor)?;
                AnnotationKind::Text(text, style)
            }
            ExprKind::Atom(TokenKind::Command(name)) if ARTICULATIONS.contains(&name.as_str()) => {
                AnnotationKind::Articulation(name.clone())
            }
            ExprKind::Atom(TokenKind::Command(name)) if DYNAMICS.contains(&name.as_str()) => {
                AnnotationKind::Dynamic(name.clone())
            }
            ExprKind::Atom(TokenKind::Command(name)) if self.variables.contains_key(name) => {
                cursor.at -= 1;
                AnnotationKind::Text(self.string_value(cursor, 0)?, TextStyle::Regular)
            }
            _ => return Err(unsupported(node.span, "text or articulation attachment")),
        };
        self.annotate(state, Annotation { kind: annotation, direction }, span)
    }

    pub(crate) fn mark_command(
        &mut self,
        name: &str,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
    ) -> Result<bool, Error> {
        let kind = if ARTICULATIONS.contains(&name) {
            AnnotationKind::Articulation(name.into())
        } else if DYNAMICS.contains(&name) {
            self.close_hairpin(state, span)?;
            AnnotationKind::Dynamic(name.into())
        } else {
            return self.other_mark(name, cursor, state, span);
        };
        self.annotate(state, Annotation { kind, direction: Direction::Auto }, span)?;
        Ok(true)
    }

    fn other_mark(
        &mut self,
        name: &str,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
    ) -> Result<bool, Error> {
        match name {
            "(" => {
                let start = self.last_ref(state, span)?;
                push(&mut state.slurs, (start, true, span), span)?;
            }
            ")" => self.close_slur(state, true, span)?,
            "<" | ">" => {
                self.close_hairpin(state, span)?;
                state.hairpin = Some((self.last_ref(state, span)?, name == "<", span));
            }
            "!" => self.close_hairpin(state, span)?,
            "tempo" => {
                let mut text = String::new();
                if matches!(
                    cursor.peek().map(|n| &n.kind),
                    Some(ExprKind::Atom(TokenKind::Text(_)))
                ) {
                    text.push_str(cursor.text()?);
                }
                if matches!(
                    cursor.peek().map(|n| &n.kind),
                    Some(ExprKind::Atom(TokenKind::Number(_)))
                ) {
                    let duration = read_duration(cursor, state.duration)?;
                    if !cursor.symbol('=') {
                        return Err(syntax_error(span, "A tempo needs = and a rate."));
                    }
                    let rate = cursor.number()?;
                    if rate == 0 {
                        return Err(syntax_error(span, "A tempo rate must be positive."));
                    }
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.push_str(&format!("1/{} = {rate}", duration.base.denominator()));
                }
                if text.is_empty() {
                    return Err(syntax_error(span, "A tempo needs text or a metronome rate."));
                }
                state.pending.push(Annotation {
                    kind: AnnotationKind::Tempo(text),
                    direction: Direction::Up,
                });
            }
            "mark" => {
                let text = if cursor.command("default") {
                    self.rehearsal = self
                        .rehearsal
                        .checked_add(1)
                        .ok_or_else(|| limit("rehearsal labels", span))?;
                    rehearsal_name(self.rehearsal)
                } else {
                    cursor.text()?.to_string()
                };
                state.pending.push(Annotation {
                    kind: AnnotationKind::Rehearsal(text),
                    direction: Direction::Up,
                });
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn markup(&self, cursor: &mut Cursor<'_>) -> Result<(String, TextStyle), Error> {
        if matches!(cursor.peek().map(|n| &n.kind), Some(ExprKind::Sequence(_))) {
            let mut inner = Cursor::new(self.syntax, cursor.sequence()?);
            let result = self.markup(&mut inner)?;
            if inner.peek().is_some() {
                return Err(unsupported(inner.span(), "compound markup"));
            }
            return Ok(result);
        }
        let style = if cursor.command("bold") {
            TextStyle::Bold
        } else if cursor.command("italic") {
            TextStyle::Italic
        } else {
            TextStyle::Regular
        };
        Ok((self.string_value(cursor, 0)?, style))
    }

    fn annotate(&mut self, state: &mut State, mark: Annotation, span: Span) -> Result<(), Error> {
        if let Some(id) = state.last {
            push(&mut self.score.events[id.0 as usize].marks, mark, span)
        } else {
            push(&mut state.pending, mark, span)
        }
    }

    fn last_ref(&self, state: &State, span: Span) -> Result<NoteRef, Error> {
        Ok(NoteRef {
            event: state
                .last
                .ok_or_else(|| syntax_error(span, "This mark needs a preceding note."))?,
            note: 0,
        })
    }

    fn close_slur(&mut self, state: &mut State, phrasing: bool, span: Span) -> Result<(), Error> {
        let index = state
            .slurs
            .iter()
            .rposition(|s| s.1 == phrasing)
            .ok_or_else(|| syntax_error(span, "There is no matching open slur."))?;
        let (start, _, opening) = state.slurs.remove(index);
        self.spanner(
            start,
            self.last_ref(state, span)?,
            if phrasing { SpannerKind::PhrasingSlur } else { SpannerKind::Slur },
            state.direction,
            opening,
        )
    }

    fn close_hairpin(&mut self, state: &mut State, span: Span) -> Result<(), Error> {
        if let Some((start, crescendo, opening)) = state.hairpin.take() {
            self.spanner(
                start,
                self.last_ref(state, span)?,
                if crescendo { SpannerKind::Crescendo } else { SpannerKind::Decrescendo },
                Direction::Down,
                opening,
            )?;
        }
        Ok(())
    }

    pub(crate) fn spanner(
        &mut self,
        start: NoteRef,
        end: NoteRef,
        kind: SpannerKind,
        direction: Direction,
        span: Span,
    ) -> Result<(), Error> {
        push(&mut self.score.spanners, Spanner { start, end, kind, direction, span }, span)
    }

    pub(crate) fn tuplet(
        &mut self,
        name: &str,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
        depth: usize,
    ) -> Result<(), Error> {
        let numerator = cursor.integer()?;
        if !cursor.symbol('/') {
            return Err(syntax_error(span, "A tuplet needs a fraction such as 3/2."));
        }
        let denominator = cursor.integer()?;
        let ratio = Ratio::new(numerator, denominator)?;
        if ratio <= Ratio::ZERO {
            return Err(syntax_error(span, "Tuplet ratios must be positive."));
        }
        let scale = if name == "tuplet" { Ratio::ONE.checked_div(ratio)? } else { ratio };
        let previous = state.scale;
        state.scale = previous.checked_mul(scale)?;
        let first = self.score.events.len();
        self.one(cursor, state, depth + 1)?;
        state.scale = previous;
        let ids: Vec<_> = self.score.events[first..]
            .iter()
            .enumerate()
            .filter(|(_, e)| e.kind.duration().is_some())
            .map(|(i, _)| gd_score::EventId((first + i) as u32))
            .collect();
        if let (Some(first), Some(last)) = (ids.first(), ids.last()) {
            let (shown_num, shown_den) =
                if name == "tuplet" { (numerator, denominator) } else { (denominator, numerator) };
            self.spanner(
                NoteRef { event: *first, note: 0 },
                NoteRef { event: *last, note: 0 },
                SpannerKind::Tuplet(shown_num as u64, shown_den as u64),
                Direction::Up,
                span,
            )?;
        }
        Ok(())
    }

    pub(crate) fn grace(
        &mut self,
        name: &str,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
        depth: usize,
    ) -> Result<(), Error> {
        let saved = state.clone();
        if name == "afterGrace" {
            self.one(cursor, state, depth + 1)?;
        }
        let target = state.at;
        let parent_last = state.last;
        let first = self.score.events.len();
        let mut child = state.clone();
        child.grace_slash = name == "acciaccatura";
        self.one(cursor, &mut child, depth + 1)?;
        let total = child.at.main.checked_sub(target.main)?;
        if total <= Ratio::ZERO {
            return Err(syntax_error(span, "A grace group needs rhythmic notes."));
        }
        for event in &mut self.score.events[first..] {
            event.at.grace = event.at.main.checked_sub(target.main)?.checked_sub(total)?;
            event.at.main = target.main;
        }
        for event in &mut self.score.events[..first] {
            if event.at == target && event.kind.duration().is_none() {
                event.at.grace = total.checked_neg()?;
            }
        }
        state.slurs = child.slurs;
        state.ties = child.ties;
        state.beam = child.beam;
        state.hairpin = child.hairpin;
        state.pitch_mode = child.pitch_mode;
        if name != "afterGrace" {
            state.at = saved.at;
        }
        state.last = parent_last;
        Ok(())
    }
}

fn rehearsal_name(mut n: u16) -> String {
    let mut chars = Vec::new();
    while n > 0 {
        n -= 1;
        chars.push(char::from(b'A' + (n % 26) as u8));
        n /= 26;
    }
    chars.into_iter().rev().collect()
}
