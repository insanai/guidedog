use gd_score::{
    Annotation, AnnotationKind, BarStyle, Direction, Duration, EventId, EventKind, Markup,
    NoteRef, SpannerKind, TextStyle,
};

use crate::cursor::Cursor;
use crate::interpret::{Reader, State, unsupported};
use crate::limits::limit;
use crate::notes::read_duration;
use crate::parser::syntax_error;
use crate::{Error, ExprKind, Span, SyntaxId, TokenKind};

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
            "volta" => self.volta_repeat(body, cursor, state, span, depth, count)?,
            _ => return Err(unsupported(span, &format!("repeat {kind}"))),
        }
        Ok(())
    }

    fn volta_repeat(
        &mut self,
        body: &[SyntaxId],
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
        depth: usize,
        count: u16,
    ) -> Result<(), Error> {
        self.emit(state, EventKind::Bar(BarStyle::RepeatStart), span)?;
        let inline_alt = body.iter().position(|id| {
            matches!(
                self.syntax.node(*id).map(|n| &n.kind),
                Some(ExprKind::Atom(TokenKind::Command(cmd))) if cmd == "alternative"
            )
        });
        let has_alternatives = if let Some(pos) = inline_alt {
            self.sequence(&body[..pos], state, depth + 1)?;
            let mut inner = Cursor::new(self.syntax, &body[pos + 1..]);
            self.alternatives(&mut inner, state, span, depth + 1, count)?;
            if inner.peek().is_some() {
                self.sequence(&body[pos + 1 + inner.at..], state, depth + 1)?;
            }
            true
        } else {
            self.sequence(body, state, depth + 1)?;
            if cursor.command("alternative") {
                self.alternatives(cursor, state, span, depth, count)?;
                true
            } else {
                self.emit(state, EventKind::Bar(BarStyle::RepeatEnd), span)?;
                false
            }
        };
        if !has_alternatives
            && count != 2
            && let Some(last) = state.last
        {
            self.score.events[last.0 as usize].marks.push(Annotation {
                kind: AnnotationKind::Text(format!("{count}x"), TextStyle::Regular),
                direction: Direction::Up,
            });
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
        let raw = cursor.sequence()?;
        let mut alt_cursor = Cursor::new(self.syntax, raw);
        let mut branches = Vec::new();
        while alt_cursor.peek().is_some() {
            let label = if alt_cursor.command("volta") {
                Some(read_volta_numbers(&mut alt_cursor, count, span)?)
            } else {
                None
            };
            let ids = alt_cursor.sequence().map_err(|_| {
                syntax_error(alt_cursor.span(), "Each repeat alternative needs braces.")
            })?;
            branches.push((label, ids));
        }
        if branches.is_empty() || branches.len() > usize::from(count) {
            return Err(syntax_error(
                span,
                "Repeat alternatives must number from one to the repeat count.",
            ));
        }
        let extra = usize::from(count).saturating_sub(branches.len());
        for (index, (explicit_label, ids)) in branches.iter().enumerate() {
            let first = self.score.events.len();
            self.sequence(ids, state, depth + 1)?;
            if let Some((start_id, end_id)) = self.volta_voice_span(first) {
                let label = explicit_label.clone().unwrap_or_else(|| {
                    if index == 0 && extra > 0 {
                        format!("1.-{}.", 1 + extra)
                    } else {
                        format!("{}.", index + 1 + extra)
                    }
                });
                self.spanner(
                    NoteRef { event: start_id, note: 0 },
                    NoteRef { event: end_id, note: 0 },
                    SpannerKind::Volta(label),
                    Direction::Up,
                    span,
                )?;
            }
            if index + 1 < branches.len() {
                self.emit(state, EventKind::Bar(BarStyle::RepeatEnd), span)?;
            }
        }
        Ok(())
    }

    fn volta_voice_span(&self, first: usize) -> Option<(EventId, EventId)> {
        let target_voice = self.score.events[first..]
            .iter()
            .filter(|e| e.kind.duration().is_some())
            .min_by_key(|e| (self.score.voices[e.voice.0 as usize].staff.0, e.voice.0))
            .map(|e| e.voice)?;
        let mut range = None;
        for (i, event) in self.score.events[first..].iter().enumerate() {
            if event.voice == target_voice && event.kind.duration().is_some() {
                let id = EventId((first + i) as u32);
                range = Some(match range {
                    None => (id, id),
                    Some((start, _)) => (start, id),
                });
            }
        }
        range
    }

    pub(crate) fn chord_names(
        &mut self,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        _span: Span,
        depth: usize,
    ) -> Result<(), Error> {
        let ids = self.chord_expression(cursor, depth)?;
        let mut inner = Cursor::new(self.syntax, &ids);
        while let Some(peek_node) = inner.peek() {
            let item_span = peek_node.span;
            self.work.take(1, item_span)?;
            self.memory.take(512, item_span)?;
            if inner.symbol('|') {
                self.bar_checks.push((state.at.main, item_span));
                continue;
            }
            let node = inner.next()?;
            let ExprKind::Atom(TokenKind::Word(word)) = &node.kind else {
                return Err(syntax_error(
                    node.span,
                    "Chord mode needs a root and optional chord quality.",
                ));
            };
            if matches!(word.as_str(), "r" | "s" | "R") {
                let written = read_duration(&mut inner, state.duration)?;
                state.duration = written;
                let scale =
                    written.scale.checked_mul(state.scale).map_err(|e| e.or_at(node.span))?;
                self.emit(state, EventKind::Skip(Duration { scale, ..written }), node.span)?;
                continue;
            }
            let (text, duration) = self.read_chord_symbol(word, &mut inner, state, node.span)?;
            self.emit(state, EventKind::ChordName(text, duration), node.span)?;
        }
        Ok(())
    }

    fn chord_expression(
        &self,
        cursor: &mut Cursor<'_>,
        depth: usize,
    ) -> Result<Vec<SyntaxId>, Error> {
        if depth > self.limits.depth {
            return Err(limit("chord expansion", cursor.span()));
        }
        cursor.command("chordmode");
        if let Some(ExprKind::Atom(TokenKind::Command(name))) = cursor.peek().map(|n| &n.kind)
            && let Some(ids) = self.variables.get(name)
        {
            cursor.at += 1;
            let mut inner = Cursor::new(self.syntax, ids);
            return self.chord_expression(&mut inner, depth + 1);
        }
        let span = cursor.span();
        let ids = cursor.sequence()?;
        self.memory.take(ids.len() * 8 + 64, span)?;
        Ok(ids.to_vec())
    }

    fn read_chord_symbol(
        &self,
        word: &str,
        inner: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
    ) -> Result<(Markup, Duration), Error> {
        let mut root = self.raw_pitch(word, inner, state, span)?.pitch;
        for (from, to) in state.transpositions.iter().rev() {
            root = root.transpose_tonic(*from, *to).map_err(|e| e.or_at(span))?;
        }
        let written = read_duration(inner, state.duration)?;
        state.duration = written;
        let scale = written.scale.checked_mul(state.scale).map_err(|e| e.or_at(span))?;
        let duration = Duration { scale, ..written };
        let quality =
            if inner.symbol(':') { self.read_chord_quality(inner, span)? } else { String::new() };
        let mut bass = None;
        if inner.symbol('/') {
            let bass_node = inner.next()?;
            let ExprKind::Atom(TokenKind::Word(bass_word)) = &bass_node.kind else {
                return Err(syntax_error(bass_node.span, "Expected a chord bass pitch."));
            };
            let mut pitch = self.raw_pitch(bass_word, inner, state, bass_node.span)?.pitch;
            for (from, to) in state.transpositions.iter().rev() {
                pitch = pitch.transpose_tonic(*from, *to).map_err(|e| e.or_at(bass_node.span))?;
            }
            bass = Some(pitch);
        }
        let markup = crate::chord_names::chord_markup(root, &quality, bass)
            .ok_or_else(|| unsupported(span, &format!("chord quality {quality}")))?;
        self.memory.take(markup.approx_bytes(), span)?;
        Ok((markup, duration))
    }

    fn read_chord_quality(&self, inner: &mut Cursor<'_>, span: Span) -> Result<String, Error> {
        let mut quality = String::new();
        let mut end = None;
        while let Some(part) = inner.peek() {
            // A quality is one unspaced run of tokens, such as `7sus4` or `m7.5-`; a token
            // after a space belongs to the next chord.
            if end.is_some_and(|end| part.span.start != end) {
                break;
            }
            match &part.kind {
                ExprKind::Atom(TokenKind::Word(word)) => {
                    self.memory.take(word.len() + 32, part.span)?;
                    quality.push_str(word);
                }
                ExprKind::Atom(TokenKind::Number(number)) => {
                    self.memory.take(number.len() + 32, part.span)?;
                    quality.push_str(number);
                }
                ExprKind::Atom(TokenKind::Symbol(c @ ('.' | '+' | '-'))) => {
                    quality.push(*c);
                }
                _ => break,
            }
            end = Some(part.span.end);
            inner.at += 1;
        }
        if quality.is_empty() {
            return Err(unsupported(span, "chord quality"));
        }
        Ok(quality)
    }
}

fn read_volta_numbers(cursor: &mut Cursor<'_>, count: u16, span: Span) -> Result<String, Error> {
    let mut nums = Vec::new();
    loop {
        let num = cursor.number()?;
        if num == 0 || num > count || nums.len() >= usize::from(count) {
            return Err(syntax_error(span, "Volta ending number is outside the repeat count."));
        }
        nums.push(num);
        if !cursor.symbol(',') {
            break;
        }
    }
    if nums.len() == 1 {
        return Ok(format!("{}.", nums[0]));
    }
    if nums.windows(2).all(|pair| pair[1] == pair[0] + 1) {
        return Ok(format!("{}.-{}.", nums[0], nums[nums.len() - 1]));
    }
    Ok(nums.iter().map(|n| format!("{n}.")).collect::<Vec<_>>().join(", "))
}
