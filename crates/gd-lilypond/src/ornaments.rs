use std::collections::{BTreeMap, BTreeSet};

use gd_score::{
    Annotation, AnnotationKind, Base, Direction, EventId, EventKind, Markup, MarkupRole, NoteRef,
    Ratio, Spanner, SpannerKind, TextStyle,
};

use crate::cursor::Cursor;
use crate::interpret::{Reader, State, unsupported};
use crate::limits::{limit, push};
use crate::notes::read_duration;
use crate::parser::syntax_error;
use crate::{Error, ErrorKind, ExprKind, Span, TokenKind};

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

pub(crate) fn is_post_mark(name: &str) -> bool {
    ARTICULATIONS.contains(&name) || DYNAMICS.contains(&name)
}

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
            '~' => self.start_chord_ties(state, last, state.tie_direction, span)?,
            '(' => self.open_slur(state, reference, false, state.slur_direction, span)?,
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

    fn start_chord_ties(
        &self,
        state: &mut State,
        last: EventId,
        direction: Direction,
        span: Span,
    ) -> Result<(), Error> {
        let EventKind::Chord(chord) = &self.score.events[last.0 as usize].kind else {
            return Err(syntax_error(span, "A rest cannot start a tie."));
        };
        for note in 0..chord.notes.len() {
            push(
                &mut state.ties,
                (NoteRef { event: last, note: note as u8 }, direction, span),
                span,
            )?;
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
        let direction = prefix_direction(prefix);
        if self.directed_spanner(direction, cursor, state, span)? {
            return Ok(());
        }
        let mut mark_dir = direction;
        let node = cursor.next()?;
        let annotation = match &node.kind {
            ExprKind::Atom(TokenKind::Command(name)) if DYNAMICS.contains(&name.as_str()) => {
                self.close_hairpin(state, span)?;
                if mark_dir == Direction::Auto {
                    mark_dir = state.dynamic_direction;
                }
                AnnotationKind::Dynamic(name.clone())
            }
            _ => self.parse_script_node(node, cursor, span)?,
        };
        self.annotate(state, Annotation { kind: annotation, direction: mark_dir }, span)
    }

    fn directed_spanner(
        &mut self,
        direction: Direction,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
    ) -> Result<bool, Error> {
        let last = self.last_ref(state, span)?;
        let Some(node) = cursor.peek() else {
            return Ok(false);
        };
        match &node.kind {
            ExprKind::Atom(TokenKind::Symbol('~')) => {
                cursor.at += 1;
                let dir =
                    if direction == Direction::Auto { state.tie_direction } else { direction };
                self.start_chord_ties(state, last.event, dir, span)?;
            }
            ExprKind::Atom(TokenKind::Symbol('(')) => {
                cursor.at += 1;
                let dir =
                    if direction == Direction::Auto { state.slur_direction } else { direction };
                self.open_slur(state, last, false, dir, span)?;
            }
            ExprKind::Atom(TokenKind::Symbol(')')) => {
                cursor.at += 1;
                self.close_slur(state, false, span)?;
            }
            ExprKind::Atom(TokenKind::Command(name)) if name == "(" => {
                cursor.at += 1;
                let dir =
                    if direction == Direction::Auto { state.slur_direction } else { direction };
                self.open_slur(state, last, true, dir, span)?;
            }
            ExprKind::Atom(TokenKind::Command(name)) if name == ")" => {
                cursor.at += 1;
                self.close_slur(state, true, span)?;
            }
            ExprKind::Atom(TokenKind::Command(name)) if name == "<" || name == ">" => {
                let crescendo = name == "<";
                cursor.at += 1;
                let dir = if direction != Direction::Auto {
                    direction
                } else if state.dynamic_direction != Direction::Auto {
                    state.dynamic_direction
                } else {
                    Direction::Down
                };
                self.open_hairpin(state, crescendo, dir, span)?;
            }
            ExprKind::Atom(TokenKind::Command(name)) if name == "!" => {
                cursor.at += 1;
                self.close_hairpin(state, span)?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(crate) fn chord_note_mark(
        &self,
        prefix: char,
        cursor: &mut Cursor<'_>,
        span: Span,
    ) -> Result<Option<Annotation>, Error> {
        let direction = prefix_direction(prefix);
        let node = cursor.next()?;
        let kind = match &node.kind {
            ExprKind::Atom(TokenKind::Command(name)) if DYNAMICS.contains(&name.as_str()) => {
                AnnotationKind::Dynamic(name.clone())
            }
            _ => self.parse_script_node(node, cursor, span)?,
        };
        let mark = Annotation { kind, direction };
        self.memory.take(crate::memory::annotation_bytes(&mark), span)?;
        Ok(Some(mark))
    }

    fn parse_script_node(
        &self,
        node: &crate::Expr,
        cursor: &mut Cursor<'_>,
        span: Span,
    ) -> Result<AnnotationKind, Error> {
        match &node.kind {
            ExprKind::Atom(TokenKind::Text(text)) => {
                Ok(AnnotationKind::Text(self.copy_text(text, node.span)?, TextStyle::Regular))
            }
            ExprKind::Atom(TokenKind::Number(number)) => {
                let finger =
                    number.parse::<u8>().ok().filter(|n| (1..=5).contains(n)).ok_or_else(
                        || syntax_error(node.span, "A fingering must be from 1 to 5."),
                    )?;
                Ok(AnnotationKind::Fingering(finger))
            }
            ExprKind::Atom(TokenKind::Symbol(symbol)) => Ok(AnnotationKind::Articulation(
                shorthand_articulation(*symbol, node.span)?.into(),
            )),
            ExprKind::Atom(TokenKind::Command(name)) if name == "markup" => {
                Ok(markup_annotation(self.read_markup(cursor, 0)?, MarkupRole::Script))
            }
            ExprKind::Atom(TokenKind::Command(name)) if ARTICULATIONS.contains(&name.as_str()) => {
                Ok(AnnotationKind::Articulation(name.clone()))
            }
            ExprKind::Atom(TokenKind::Command(name)) if self.variables.contains_key(name) => {
                self.variable_annotation(name, MarkupRole::Script, span)
            }
            _ => Err(unsupported(node.span, "text or articulation attachment")),
        }
    }

    fn variable_annotation(
        &self,
        name: &str,
        role: MarkupRole,
        span: Span,
    ) -> Result<AnnotationKind, Error> {
        let ids =
            self.variables.get(name).ok_or_else(|| syntax_error(span, "Unknown variable."))?;
        if matches!(
            ids.first().and_then(|id| self.syntax.node(*id)).map(|n| &n.kind),
            Some(ExprKind::Atom(TokenKind::Command(c))) if c == "markup"
        ) {
            let markup = self.expand_markup_var(ids, span, 0)?;
            return Ok(markup_annotation(markup, role));
        }
        let mut inner = Cursor::new(self.syntax, ids);
        let text = self.string_value(&mut inner, 1)?;
        if inner.peek().is_some() {
            return Err(syntax_error(span, "The variable is not a string or markup value."));
        }
        Ok(markup_annotation(Markup::Text(text), role))
    }

    pub(crate) fn mark_command(
        &mut self,
        name: &str,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
    ) -> Result<bool, Error> {
        if ARTICULATIONS.contains(&name) {
            let kind = AnnotationKind::Articulation(name.into());
            self.annotate(state, Annotation { kind, direction: Direction::Auto }, span)?;
            return Ok(true);
        }
        if DYNAMICS.contains(&name) {
            self.close_hairpin(state, span)?;
            let kind = AnnotationKind::Dynamic(name.into());
            self.annotate(state, Annotation { kind, direction: state.dynamic_direction }, span)?;
            return Ok(true);
        }
        self.other_mark(name, cursor, state, span)
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
                self.open_slur(state, start, true, state.slur_direction, span)?;
            }
            ")" => self.close_slur(state, true, span)?,
            "<" | ">" => {
                let dir = if state.dynamic_direction == Direction::Auto {
                    Direction::Down
                } else {
                    state.dynamic_direction
                };
                self.open_hairpin(state, name == "<", dir, span)?;
            }
            "!" => self.close_hairpin(state, span)?,
            "markup" => {
                // Before any note, LilyPond prints \markup as a text block of its own; in a
                // document that text belongs in the prose, not on the first note.
                if state.last.is_none() {
                    return Err(Error::new(
                        ErrorKind::Unsupported,
                        "A standalone `\\markup` before the music is a text block, not a mark.",
                    )
                    .at(span)
                    .hint(
                        "Write the text in the document, or attach it to a note with ^\\markup.",
                    ));
                }
                let kind = markup_annotation(self.read_markup(cursor, 0)?, MarkupRole::Script);
                self.annotate(state, Annotation { kind, direction: Direction::Auto }, span)?;
            }
            "tempo" => self.tempo_mark(cursor, state, span)?,
            "mark" => self.rehearsal_mark(cursor, state, span)?,
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn tempo_mark(
        &mut self,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
    ) -> Result<(), Error> {
        let label = if cursor.command("markup") {
            Some(self.read_markup(cursor, 0)?)
        } else if matches!(
            cursor.peek().map(|n| &n.kind),
            Some(ExprKind::Atom(TokenKind::Text(_)))
        ) {
            Some(Markup::Text(self.copy_text(cursor.text()?, span)?))
        } else {
            None
        };
        let rate = if matches!(
            cursor.peek().map(|n| &n.kind),
            Some(ExprKind::Atom(TokenKind::Number(_)))
        ) {
            let duration = read_duration(cursor, state.duration)?;
            if !cursor.symbol('=') {
                return Err(syntax_error(span, "A tempo needs = and a rate."));
            }
            let bpm = cursor.number()?;
            if bpm == 0 {
                return Err(syntax_error(span, "A tempo rate must be positive."));
            }
            let dots = ".".repeat(usize::from(duration.dots));
            let unit = if duration.base == Base::Breve {
                format!("breve{dots}")
            } else {
                format!("1/{}{dots}", duration.base.denominator())
            };
            Some(format!("{unit} = {bpm}"))
        } else {
            None
        };
        let markup = match (label, rate) {
            (Some(Markup::Text(mut text)), Some(rate)) => {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(&rate);
                Markup::Text(text)
            }
            (Some(label), Some(rate)) => Markup::Line(vec![label, Markup::Text(rate)]),
            (Some(label), None) => label,
            (None, Some(rate)) => Markup::Text(rate),
            (None, None) => {
                return Err(syntax_error(span, "A tempo needs text or a metronome rate."));
            }
        };
        let kind = markup_annotation(markup, MarkupRole::Tempo);
        self.memory.take(
            crate::memory::annotation_bytes(&Annotation {
                kind: kind.clone(),
                direction: Direction::Up,
            }),
            span,
        )?;
        push(&mut state.pending, Annotation { kind, direction: Direction::Up }, span)
    }

    fn rehearsal_mark(
        &mut self,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
    ) -> Result<(), Error> {
        let kind = if cursor.command("default") {
            self.rehearsal =
                self.rehearsal.checked_add(1).ok_or_else(|| limit("rehearsal labels", span))?;
            AnnotationKind::Rehearsal(rehearsal_name(self.rehearsal))
        } else if cursor.command("markup") {
            markup_annotation(self.read_markup(cursor, 0)?, MarkupRole::Rehearsal)
        } else if let Some(ExprKind::Atom(TokenKind::Command(name))) =
            cursor.peek().map(|n| &n.kind)
            && self.variables.contains_key(name)
        {
            let var = name.clone();
            cursor.at += 1;
            self.variable_annotation(&var, MarkupRole::Rehearsal, span)?
        } else {
            AnnotationKind::Rehearsal(self.copy_text(cursor.text()?, span)?)
        };
        let mark = Annotation { kind, direction: Direction::Up };
        self.memory.take(crate::memory::annotation_bytes(&mark), span)?;
        push(&mut state.pending, mark, span)
    }

    fn annotate(&mut self, state: &mut State, mark: Annotation, span: Span) -> Result<(), Error> {
        self.memory.take(crate::memory::annotation_bytes(&mark), span)?;
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

    fn open_slur(
        &self,
        state: &mut State,
        start: NoteRef,
        phrasing: bool,
        direction: Direction,
        span: Span,
    ) -> Result<(), Error> {
        if state.slurs.len() >= self.limits.depth {
            return Err(limit("nested slurs", span));
        }
        self.memory.take(128, span)?;
        push(&mut state.slurs, (start, phrasing, direction, span), span)
    }

    fn close_slur(&mut self, state: &mut State, phrasing: bool, span: Span) -> Result<(), Error> {
        let index = state
            .slurs
            .iter()
            .rposition(|s| s.1 == phrasing)
            .ok_or_else(|| syntax_error(span, "There is no matching open slur."))?;
        let (start, _, slur_dir, opening) = state.slurs.remove(index);
        let direction = if slur_dir != Direction::Auto { slur_dir } else { state.direction };
        self.spanner(
            start,
            self.last_ref(state, span)?,
            if phrasing { SpannerKind::PhrasingSlur } else { SpannerKind::Slur },
            direction,
            opening,
        )
    }

    fn open_hairpin(
        &mut self,
        state: &mut State,
        crescendo: bool,
        direction: Direction,
        span: Span,
    ) -> Result<(), Error> {
        self.close_hairpin(state, span)?;
        state.hairpin = Some((self.last_ref(state, span)?, crescendo, direction, span));
        Ok(())
    }

    fn close_hairpin(&mut self, state: &mut State, span: Span) -> Result<(), Error> {
        if let Some((start, crescendo, direction, opening)) = state.hairpin.take() {
            self.spanner(
                start,
                self.last_ref(state, span)?,
                if crescendo { SpannerKind::Crescendo } else { SpannerKind::Decrescendo },
                direction,
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
        self.memory.take(256, span)?;
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
        let ratio = Ratio::new(numerator, denominator).map_err(|e| e.or_at(span))?;
        if ratio <= Ratio::ZERO {
            return Err(syntax_error(span, "Tuplet ratios must be positive."));
        }
        let scale = if name == "tuplet" {
            Ratio::ONE.checked_div(ratio).map_err(|e| e.or_at(span))?
        } else {
            ratio
        };
        let previous = state.scale;
        state.scale = previous.checked_mul(scale).map_err(|e| e.or_at(span))?;
        let first = self.score.events.len();
        self.one(cursor, state, depth + 1)?;
        state.scale = previous;
        let mut by_voice: BTreeMap<u32, (EventId, EventId)> = BTreeMap::new();
        for (i, event) in self.score.events[first..].iter().enumerate() {
            if event.kind.duration().is_some() {
                let id = EventId((first + i) as u32);
                by_voice
                    .entry(event.voice.0)
                    .and_modify(|(_, last)| *last = id)
                    .or_insert((id, id));
            }
        }
        let (shown_num, shown_den) =
            if name == "tuplet" { (numerator, denominator) } else { (denominator, numerator) };
        for (start_id, end_id) in by_voice.into_values() {
            self.spanner(
                NoteRef { event: start_id, note: 0 },
                NoteRef { event: end_id, note: 0 },
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
        let saved_at = state.at;
        if name == "afterGrace" {
            self.one(cursor, state, depth + 1)?;
        }
        let target = state.at;
        let parent_last = state.last;
        let first = self.score.events.len();
        let spanners_before = self.score.spanners.len();
        let mut child = self.copy_state(state, span)?;
        child.ties.clear();
        child.beam = None;
        child.grace_slurs.clear();
        child.grace_slash = false;
        self.one(cursor, &mut child, depth + 1)?;
        let total = child.at.main.checked_sub(target.main).map_err(|e| e.or_at(span))?;
        if total <= Ratio::ZERO {
            return Err(syntax_error(span, "A grace group needs rhythmic notes."));
        }
        self.finish_grace_events(
            name,
            state,
            &child,
            first,
            spanners_before,
            target,
            total,
            span,
        )?;
        state.slurs = child.slurs;
        state.ties.extend(child.ties);
        state.beam = child.beam;
        state.hairpin = child.hairpin;
        state.pitch_mode = child.pitch_mode;
        if name != "afterGrace" {
            state.at = saved_at;
        }
        state.last = parent_last;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_grace_events(
        &mut self,
        name: &str,
        state: &mut State,
        child: &State,
        first: usize,
        spanners_before: usize,
        target: gd_score::Moment,
        total: Ratio,
        span: Span,
    ) -> Result<(), Error> {
        let mut slashed_voices = BTreeSet::new();
        let mut first_voice_chord = None;
        for (i, event) in self.score.events[first..].iter_mut().enumerate() {
            let id = EventId((first + i) as u32);
            if let EventKind::Chord(chord) = &mut event.kind {
                if name == "acciaccatura" && slashed_voices.insert(event.voice) {
                    chord.grace_slash = true;
                }
                if event.voice == state.voice && first_voice_chord.is_none() {
                    first_voice_chord = Some(id);
                }
            }
            let offset = event.at.main.checked_sub(target.main).map_err(|e| e.or_at(span))?;
            event.at.grace = offset.checked_sub(total).map_err(|e| e.or_at(span))?;
            event.at.main = target.main;
        }
        if matches!(name, "acciaccatura" | "appoggiatura")
            && let Some(start_id) = first_voice_chord
        {
            let manual_slur = child.slurs.len() > state.slurs.len()
                || self.score.spanners[spanners_before..].iter().any(|s| {
                    matches!(s.kind, SpannerKind::Slur | SpannerKind::PhrasingSlur)
                        && s.start.event == start_id
                });
            if !manual_slur {
                push(&mut state.grace_slurs, (NoteRef { event: start_id, note: 0 }, span), span)?;
            }
        }
        let staff = self.score.voices[state.voice.0 as usize].staff;
        let neg_total = total.checked_neg().map_err(|e| e.or_at(span))?;
        for event in &mut self.score.events[..first] {
            if self.score.voices[event.voice.0 as usize].staff != staff
                || event.at.main != target.main
            {
                continue;
            }
            if event.at.grace < Ratio::ZERO {
                event.at.grace = event.at.grace.checked_sub(total).map_err(|e| e.or_at(span))?;
            } else if event.kind.duration().is_none() && event.at.grace == Ratio::ZERO {
                event.at.grace = neg_total;
            }
        }
        Ok(())
    }
}

fn prefix_direction(prefix: char) -> Direction {
    match prefix {
        '^' => Direction::Up,
        '_' => Direction::Down,
        _ => Direction::Auto,
    }
}

fn shorthand_articulation(symbol: char, span: Span) -> Result<&'static str, Error> {
    Ok(match symbol {
        '.' => "staccato",
        '-' => "tenuto",
        '>' => "accent",
        '^' => "marcato",
        '_' => "portato",
        '!' => "staccatissimo",
        '+' => "stopped",
        _ => return Err(syntax_error(span, "Unsupported articulation shorthand.")),
    })
}

fn markup_annotation(markup: Markup, role: MarkupRole) -> AnnotationKind {
    match role {
        MarkupRole::Script => match markup {
            Markup::Text(text) => AnnotationKind::Text(text, TextStyle::Regular),
            Markup::Style(change, body) => match *body {
                Markup::Text(text) => AnnotationKind::Text(text, change.apply(TextStyle::Regular)),
                body => AnnotationKind::Markup(
                    Markup::Style(change, Box::new(body)),
                    MarkupRole::Script,
                ),
            },
            other => AnnotationKind::Markup(other, MarkupRole::Script),
        },
        MarkupRole::Tempo => match markup {
            Markup::Text(text) => AnnotationKind::Tempo(text),
            other => AnnotationKind::Markup(other, MarkupRole::Tempo),
        },
        MarkupRole::Rehearsal => match markup {
            Markup::Text(text) => AnnotationKind::Rehearsal(text),
            other => AnnotationKind::Markup(other, MarkupRole::Rehearsal),
        },
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
