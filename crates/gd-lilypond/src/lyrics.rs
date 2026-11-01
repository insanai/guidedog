use gd_score::{
    Anchor, EventId, EventKind, LyricLine, Moment, NoteRef, SpannerKind, Syllable, VoiceId,
};

use crate::cursor::Cursor;
use crate::interpret::{Reader, State};
use crate::limits::push;
use crate::notes::read_duration;
use crate::parser::syntax_error;
use crate::{Error, ErrorKind, ExprKind, Span, SyntaxId, TokenKind};

#[derive(Clone, Debug)]
pub(crate) struct PendingLyrics {
    pub(crate) name: Option<String>,
    pub(crate) voice: VoiceId,
    pub(crate) ids: Vec<SyntaxId>,
    pub(crate) span: Span,
}

impl Reader<'_> {
    pub(crate) fn lyrics(
        &mut self,
        name: &str,
        cursor: &mut Cursor<'_>,
        state: &State,
        span: Span,
        depth: usize,
    ) -> Result<(), Error> {
        let target =
            if name == "lyricsto" { Some(self.copy_text(cursor.text()?, span)?) } else { None };
        let voice = state
            .last
            .and_then(|id| self.score.events.get(id.0 as usize))
            .map(|e| e.voice)
            .unwrap_or(state.voice);
        let ids = self.lyric_expression(cursor, depth)?;
        push(&mut self.pending_lyrics, PendingLyrics { name: target, voice, ids, span }, span)
    }

    fn lyric_expression(
        &self,
        cursor: &mut Cursor<'_>,
        depth: usize,
    ) -> Result<Vec<SyntaxId>, Error> {
        if depth > self.limits.depth {
            return Err(crate::limits::limit("lyric expansion", cursor.span()));
        }
        cursor.command("lyricmode");
        if let Some(ExprKind::Atom(TokenKind::Command(name))) = cursor.peek().map(|n| &n.kind)
            && let Some(ids) = self.variables.get(name)
        {
            cursor.at += 1;
            let mut inner = Cursor::new(self.syntax, ids);
            return self.lyric_expression(&mut inner, depth + 1);
        }
        let span = cursor.span();
        let ids = cursor.sequence()?;
        self.memory.take(ids.len() * 8 + 64, span)?;
        Ok(ids.to_vec())
    }

    pub(crate) fn timed_lyrics(
        &mut self,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
    ) -> Result<(), Error> {
        if cursor.command("lyricsto") {
            return self.lyrics_target(cursor, state, span);
        }
        let ids = self.lyric_expression(cursor, 0)?;
        let mut inner = Cursor::new(self.syntax, &ids);
        let mut duration = state.duration;
        let mut at = state.at;
        let mut syllables: Vec<Syllable> = Vec::new();
        while inner.peek().is_some() {
            let location = inner.span();
            self.work.take(1, location)?;
            self.memory.take(512, location)?;
            if inner.symbol('|') {
                self.bar_checks.push((at.main, location));
                continue;
            }
            if inner.symbol('-') {
                if !inner.symbol('-') {
                    return Err(syntax_error(location, "A lyric hyphen uses --."));
                }
                if let Some(previous) = syllables.last_mut() {
                    previous.hyphen = true;
                }
                continue;
            }
            if inner.symbol('_') {
                if inner.symbol('_') {
                    if let Some(previous) = syllables.last_mut() {
                        previous.extender = Some(Anchor::Moment(at));
                    }
                    continue;
                }
                duration = read_duration(&mut inner, duration)?;
                let step = duration.time().map_err(|e| e.or_at(location))?;
                at.main = at.main.checked_add(step).map_err(|e| e.or_at(location))?;
                continue;
            }
            let (text, word_span) = self.read_syllable_text(&mut inner, false)?;
            duration = read_duration(&mut inner, duration)?;
            push(
                &mut syllables,
                Syllable {
                    text,
                    anchor: Anchor::Moment(at),
                    hyphen: false,
                    extender: None,
                    span: word_span,
                },
                word_span,
            )?;
            let step = duration.time().map_err(|e| e.or_at(word_span))?;
            at.main = at.main.checked_add(step).map_err(|e| e.or_at(word_span))?;
        }
        push(&mut self.score.lyrics, LyricLine { voice: state.voice, syllables }, span)
    }

    fn lyrics_target(
        &mut self,
        cursor: &mut Cursor<'_>,
        state: &State,
        span: Span,
    ) -> Result<(), Error> {
        self.lyrics("lyricsto", cursor, state, span, 0)
    }

    pub(crate) fn finish_lyrics(&mut self) -> Result<(), Error> {
        for pending in std::mem::take(&mut self.pending_lyrics) {
            let voice = match &pending.name {
                Some(name) => self
                    .score
                    .voices
                    .iter()
                    .position(|v| &v.name == name)
                    .map(|i| VoiceId(i as u32))
                    .ok_or_else(|| {
                        syntax_error(pending.span, &format!("No voice is named `{name}`."))
                    })?,
                None => pending.voice,
            };
            self.attach_lyrics(&pending.ids, voice, pending.span)?;
        }
        Ok(())
    }

    fn attach_lyrics(
        &mut self,
        ids: &[SyntaxId],
        voice: VoiceId,
        span: Span,
    ) -> Result<(), Error> {
        self.memory.take(self.score.events.len().saturating_mul(64), span)?;
        let candidates = self.lyric_candidates(voice, span)?;
        let mut at = 0usize;
        let mut cursor = Cursor::new(self.syntax, ids);
        let mut syllables: Vec<Syllable> = Vec::new();
        while let Some(node) = cursor.peek() {
            self.work.take(1, node.span)?;
            self.memory.take(512, node.span)?;
            if cursor.symbol('|') {
                continue;
            }
            if cursor.symbol('-') {
                if !cursor.symbol('-') {
                    return Err(syntax_error(node.span, "A lyric hyphen uses --."));
                }
                if let Some(previous) = syllables.last_mut() {
                    previous.hyphen = true;
                }
                continue;
            }
            if cursor.symbol('_') {
                if cursor.symbol('_') {
                    if let Some(previous) = syllables.last_mut() {
                        let end = candidates.get(at.saturating_sub(1)).and_then(|(_, end)| *end);
                        previous.extender =
                            end.map(|event| Anchor::Note(NoteRef { event, note: 0 }));
                    }
                } else {
                    at += 1;
                }
                continue;
            }
            let (text, word_span) = self.read_syllable_text(&mut cursor, true)?;
            self.place_attached_syllable(&mut syllables, &candidates, at, text, word_span)?;
            at += 1;
        }
        push(&mut self.score.lyrics, LyricLine { voice, syllables }, span)
    }

    fn place_attached_syllable(
        &mut self,
        syllables: &mut Vec<Syllable>,
        candidates: &[(EventId, Option<EventId>)],
        at: usize,
        text: String,
        span: Span,
    ) -> Result<(), Error> {
        if let Some((event, _)) = candidates.get(at) {
            push(
                syllables,
                Syllable {
                    text,
                    anchor: Anchor::Note(NoteRef { event: *event, note: 0 }),
                    hyphen: false,
                    extender: None,
                    span,
                },
                span,
            )?;
        } else {
            self.warnings.push(
                Error::new(ErrorKind::Lyrics, "A lyric syllable has no remaining note.")
                    .at(span)
                    .warning()
                    .hint("Remove extra syllables or add lyric skips and melismata."),
            );
        }
        Ok(())
    }

    fn read_syllable_text(
        &self,
        cursor: &mut Cursor<'_>,
        allow_dot: bool,
    ) -> Result<(String, Span), Error> {
        let node = cursor.next()?;
        let (mut text, is_word) = match &node.kind {
            ExprKind::Atom(TokenKind::Text(s)) => (self.copy_text(s, node.span)?, false),
            ExprKind::Atom(TokenKind::Word(s)) => (self.copy_text(s, node.span)?, true),
            _ => return Err(syntax_error(node.span, "Expected a word or string.")),
        };
        let mut last_span = node.span;
        if is_word {
            while let Some(next) = cursor.peek() {
                if next.span.file != last_span.file || next.span.start != last_span.end {
                    break;
                }
                match &next.kind {
                    ExprKind::Atom(TokenKind::Symbol(c))
                        if matches!(c, ',' | '!' | '?' | ';' | '\'')
                            || (allow_dot && matches!(c, '.' | ':')) =>
                    {
                        text.push(*c);
                        last_span = next.span;
                        cursor.at += 1;
                    }
                    ExprKind::Atom(TokenKind::Word(w)) if text.ends_with('\'') => {
                        self.memory.take(w.len(), next.span)?;
                        text.push_str(w);
                        last_span = next.span;
                        cursor.at += 1;
                    }
                    _ => break,
                }
            }
        }
        Ok((text, Span { file: node.span.file, start: node.span.start, end: last_span.end }))
    }

    fn lyric_candidates(
        &mut self,
        voice: VoiceId,
        span: Span,
    ) -> Result<Vec<(EventId, Option<EventId>)>, Error> {
        let events = self
            .score
            .voices
            .get(voice.0 as usize)
            .ok_or_else(|| syntax_error(span, "Lyrics name a missing voice."))?
            .events
            .clone();
        let melismas = self.voice_melismas(voice);
        let mut result = Vec::new();
        for id in events {
            let event = &self.score.events[id.0 as usize];
            if !matches!(event.kind, EventKind::Chord(_))
                || event.at.grace != gd_score::Ratio::ZERO
            {
                continue;
            }
            let mut covered = false;
            let mut end = None;
            for &(start_id, end_id, start_at, end_at) in &melismas {
                self.work.take(1, event.span)?;
                covered |= event.at > start_at && event.at <= end_at;
                if id == start_id {
                    let reached = self.extend_melisma(end_id, &melismas);
                    if end.is_none_or(|prev: EventId| {
                        self.score.events[reached.0 as usize].at
                            > self.score.events[prev.0 as usize].at
                    }) {
                        end = Some(reached);
                    }
                }
            }
            if !covered {
                result.push((id, end));
            }
        }
        Ok(result)
    }

    fn voice_melismas(&self, voice: VoiceId) -> Vec<(EventId, EventId, Moment, Moment)> {
        let mut out = Vec::new();
        for spanner in &self.score.spanners {
            let a = &self.score.events[spanner.start.event.0 as usize];
            let b = &self.score.events[spanner.end.event.0 as usize];
            let melisma = match &spanner.kind {
                SpannerKind::Tie | SpannerKind::Slur | SpannerKind::PhrasingSlur => true,
                SpannerKind::Beam => matches!(&a.kind, EventKind::Chord(c) if !c.auto_beam),
                _ => false,
            };
            if melisma && a.voice == voice {
                out.push((spanner.start.event, spanner.end.event, a.at, b.at));
            }
        }
        out
    }

    fn extend_melisma(
        &self,
        mut current: EventId,
        melismas: &[(EventId, EventId, Moment, Moment)],
    ) -> EventId {
        for _ in 0..melismas.len() {
            let cur_at = self.score.events[current.0 as usize].at;
            let next = melismas
                .iter()
                .filter(|&&(start_id, _, _, end_at)| start_id == current && end_at > cur_at)
                .max_by_key(|&&(_, _, _, end_at)| end_at)
                .map(|&(_, end_id, _, _)| end_id);
            match next {
                Some(next_id) => current = next_id,
                None => break,
            }
        }
        current
    }
}
