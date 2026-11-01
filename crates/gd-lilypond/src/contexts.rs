use gd_score::{Direction, GroupKind, Staff, StaffGroup, StaffId, Voice, VoiceId};

use crate::cursor::Cursor;
use crate::interpret::{Reader, State, unsupported};
use crate::limits::{limit, push};
use crate::parser::syntax_error;
use crate::{Error, ExprKind, Span, SyntaxId, TokenKind};

impl Reader<'_> {
    pub(crate) fn simultaneous(
        &mut self,
        ids: &[SyntaxId],
        state: &mut State,
        depth: usize,
    ) -> Result<(), Error> {
        let mut cursor = Cursor::new(self.syntax, ids);
        let start = self.copy_state(state, cursor.span())?;
        let mut end = state.at;
        let mut branch = 0;
        let explicit = ids.iter().any(|id| {
            matches!(self.syntax.node(*id).map(|n| &n.kind),
            Some(ExprKind::Atom(TokenKind::Command(name))) if name == "\\")
        });
        while cursor.peek().is_some() {
            if cursor.command("\\") {
                continue;
            }
            let mut child = self.copy_state(&start, cursor.span())?;
            let is_context = matches!(cursor.peek().map(|n| &n.kind),
                Some(ExprKind::Atom(TokenKind::Command(name))) if name == "new");
            if branch > 0 && !is_context {
                let staff = self.score.voices[start.voice.0 as usize].staff;
                child.voice = self.new_voice(staff, "", cursor.span())?;
                child.last = None;
                child.ties.clear();
                child.slurs.clear();
                child.beam = None;
                child.hairpin = None;
            }
            let voice_context = is_context && cursor.ids.get(cursor.at + 1)
                .and_then(|id| self.syntax.node(*id))
                .is_some_and(|node| matches!(&node.kind,
                    ExprKind::Atom(TokenKind::Word(kind) | TokenKind::Text(kind)) if kind == "Voice"));
            if (!is_context || voice_context) && (explicit || branch > 0) {
                child.direction = if branch == 0 { Direction::Up } else { Direction::Down };
                if let Some(voice) = self.score.voices.get_mut(start.voice.0 as usize) {
                    voice.direction = Direction::Up;
                }
            }
            if explicit {
                while cursor.peek().is_some()
                    && !matches!(cursor.peek().map(|n| &n.kind),
                    Some(ExprKind::Atom(TokenKind::Command(name))) if name == "\\")
                {
                    self.one(&mut cursor, &mut child, depth + 1)?;
                }
            } else {
                self.one(&mut cursor, &mut child, depth + 1)?;
            }
            self.closed(&child)?;
            end = end.max(child.at);
            if branch == 0 {
                state.last = child.last;
                state.duration = child.duration;
                state.pitch_mode = child.pitch_mode;
            }
            branch += 1;
        }
        state.at = end;
        state.ties.clear();
        state.slurs.clear();
        state.beam = None;
        state.hairpin = None;
        Ok(())
    }

    pub(crate) fn context(
        &mut self,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
        depth: usize,
    ) -> Result<(), Error> {
        let kind = cursor.text()?;
        let name =
            if cursor.symbol('=') { self.copy_text(cursor.text()?, span)? } else { String::new() };
        let properties = if cursor.command("with") { Some(cursor.sequence()?) } else { None };
        let mut child = self.copy_state(state, cursor.span())?;
        match kind {
            "Staff" => {
                let reuse = self.can_reuse_staff();
                self.staff_contexts += 1;
                let staff = if reuse {
                    StaffId(0)
                } else {
                    if self.score.staves.len() >= 24 {
                        return Err(limit("staves", span));
                    }
                    let staff = StaffId(self.score.staves.len() as u32);
                    push(&mut self.score.staves, Staff::default(), span)?;
                    staff
                };
                child.voice = if reuse { VoiceId(0) } else { self.new_voice(staff, "", span)? };
                self.score.voices[child.voice.0 as usize].name = name;
                child.last = None;
                if let Some(properties) = properties {
                    self.instrument_properties(properties, staff)?;
                }
                self.one(cursor, &mut child, depth + 1)?;
            }
            "Voice" => {
                let staff = self.score.voices[state.voice.0 as usize].staff;
                let current = &self.score.voices[state.voice.0 as usize];
                child.voice = if current.events.is_empty() && current.name.is_empty() {
                    state.voice
                } else {
                    self.new_voice(staff, &name, span)?
                };
                self.score.voices[child.voice.0 as usize].name = name;
                self.one(cursor, &mut child, depth + 1)?;
            }
            "PianoStaff" | "GrandStaff" | "StaffGroup" | "ChoirStaff" => {
                self.staff_group(kind, cursor, &mut child, span, depth)?;
            }
            "Lyrics" => return self.timed_lyrics(cursor, state, span),
            "ChordNames" => {
                if !cursor.command("chordmode") {
                    return Err(syntax_error(span, "ChordNames needs \\chordmode."));
                }
                return self.chord_names(cursor, state, span);
            }
            _ => return Err(unsupported(span, &format!("context {kind}"))),
        }
        self.closed(&child)?;
        state.at = state.at.max(child.at);
        state.last = child.last;
        state.duration = child.duration;
        Ok(())
    }

    fn can_reuse_staff(&self) -> bool {
        self.staff_contexts == 0
            && !self.score.voices[0].events.iter().any(|id| {
                matches!(
                    self.score.events[id.0 as usize].kind,
                    gd_score::EventKind::Chord(_)
                        | gd_score::EventKind::Rest(_)
                        | gd_score::EventKind::Skip(_)
                        | gd_score::EventKind::MeasureRest(_)
                )
            })
    }

    fn staff_group(
        &mut self,
        kind: &str,
        cursor: &mut Cursor<'_>,
        child: &mut State,
        span: Span,
        depth: usize,
    ) -> Result<(), Error> {
        let first = if self.can_reuse_staff() {
            StaffId(0)
        } else {
            StaffId(self.score.staves.len() as u32)
        };
        self.one(cursor, child, depth + 1)?;
        if first.0 as usize >= self.score.staves.len() {
            return Err(syntax_error(span, "A staff group needs at least one Staff."));
        }
        let kind = match kind {
            "PianoStaff" | "GrandStaff" => GroupKind::Brace,
            "ChoirStaff" => GroupKind::Choir,
            _ => GroupKind::Bracket,
        };
        push(
            &mut self.score.groups,
            StaffGroup { first, last: StaffId(self.score.staves.len() as u32 - 1), kind },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn new_voice(
        &mut self,
        staff: StaffId,
        name: &str,
        span: Span,
    ) -> Result<VoiceId, Error> {
        let count = self.score.voices.iter().filter(|v| v.staff == staff).count();
        if count >= 2 {
            return Err(limit("two voices per staff", span));
        }
        let id = VoiceId(self.score.voices.len() as u32);
        push(
            &mut self.score.voices,
            Voice {
                staff,
                name: name.into(),
                direction: if count == 0 { Direction::Auto } else { Direction::Down },
                events: Vec::new(),
            },
            span,
        )?;
        Ok(id)
    }

    pub(crate) fn set_instrument(
        &mut self,
        cursor: &mut Cursor<'_>,
        state: &State,
        span: Span,
    ) -> Result<(), Error> {
        if cursor.text()? != "Staff" || !cursor.symbol('.') {
            return Err(unsupported(span, "\\set outside Staff instrument names"));
        }
        let property = cursor.text()?;
        if !cursor.symbol('=') {
            return Err(syntax_error(span, "An instrument name needs =."));
        }
        let value = self.string_value(cursor, 0)?;
        let staff = self.score.voices[state.voice.0 as usize].staff;
        self.instrument_value(staff, property, &value, span)
    }

    fn instrument_properties(&mut self, ids: &[SyntaxId], staff: StaffId) -> Result<(), Error> {
        let mut cursor = Cursor::new(self.syntax, ids);
        while cursor.peek().is_some() {
            let span = cursor.span();
            let property = cursor.text()?;
            if !cursor.symbol('=') {
                return Err(syntax_error(span, "An instrument name needs =."));
            }
            cursor.symbol('#');
            let value = self.string_value(&mut cursor, 0)?;
            self.instrument_value(staff, property, &value, span)?;
        }
        Ok(())
    }

    fn instrument_value(
        &mut self,
        staff: StaffId,
        property: &str,
        value: &str,
        span: Span,
    ) -> Result<(), Error> {
        let staff = &mut self.score.staves[staff.0 as usize];
        match property {
            "instrumentName" => staff.name = value.into(),
            "shortInstrumentName" => staff.short_name = value.into(),
            _ => return Err(unsupported(span, &format!("staff property {property}"))),
        }
        Ok(())
    }
}
