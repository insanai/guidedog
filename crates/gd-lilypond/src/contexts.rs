use gd_score::{
    Direction, EventKind, GroupKind, Markup, Ratio, Staff, StaffGroup, StaffId, StaffKind, Voice,
    VoiceId,
};

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
        let mut pitch_mode = start.pitch_mode;
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
            let mut child =
                self.prepare_branch(&mut cursor, &start, pitch_mode, explicit, branch)?;
            self.run_branch(&mut cursor, &mut child, explicit, depth + 1)?;
            self.hand_back_pending(&mut child, state);
            self.closed(&child)?;
            end = end.max(child.at);
            pitch_mode = child.pitch_mode;
            if branch == 0 {
                state.last = child.last;
                state.duration = child.duration;
            }
            branch += 1;
        }
        state.pitch_mode = pitch_mode;
        state.at = end;
        clear_open_spans(state);
        Ok(())
    }

    fn prepare_branch(
        &mut self,
        cursor: &mut Cursor<'_>,
        start: &State,
        pitch_mode: crate::interpret::PitchMode,
        explicit: bool,
        branch: usize,
    ) -> Result<State, Error> {
        let span = cursor.span();
        let mut child = self.copy_state(start, span)?;
        child.pitch_mode = pitch_mode;
        // A staff that begins beside a pickup starts with it; music that follows the
        // pickup in the same sequence (its last event is set) is already in time.
        if start.at.main == Ratio::ZERO
            && start.last.is_none()
            && let Some(partial) = self.partial
        {
            child.at.main = partial.checked_neg().map_err(|e| e.or_at(span))?;
        }
        let is_context = matches!(
            cursor.peek().map(|n| &n.kind),
            Some(ExprKind::Atom(TokenKind::Command(name)))
                if matches!(name.as_str(), "new" | "chords")
        );
        if branch > 0 && !is_context {
            let staff = self.score.voices[start.voice.0 as usize].staff;
            child.voice =
                self.reuse_or_new_voice(staff, "", child.at.main, Some(start.voice), span)?;
            child.last = None;
            clear_open_spans(&mut child);
        }
        let voice_context = is_context
            && cursor
                .ids
                .get(cursor.at + 1)
                .and_then(|id| self.syntax.node(*id))
                .is_some_and(is_voice);
        if (!is_context || voice_context) && (explicit || branch > 0) {
            child.set_voice_style(match branch {
                0 => gd_score::VoiceStyle::One,
                1 => gd_score::VoiceStyle::Two,
                2 => gd_score::VoiceStyle::Three,
                _ => gd_score::VoiceStyle::Four,
            });
        }
        Ok(child)
    }

    fn run_branch(
        &mut self,
        cursor: &mut Cursor<'_>,
        child: &mut State,
        explicit: bool,
        depth: usize,
    ) -> Result<(), Error> {
        if explicit {
            while cursor.peek().is_some()
                && !matches!(cursor.peek().map(|n| &n.kind),
                Some(ExprKind::Atom(TokenKind::Command(name))) if name == "\\")
            {
                self.one(cursor, child, depth)?;
            }
            Ok(())
        } else {
            self.one(cursor, child, depth)?;
            while let Some(node) = cursor.peek() {
                let is_postfix = match &node.kind {
                    ExprKind::Atom(TokenKind::Symbol(c)) => "~()[]-^_".contains(*c),
                    ExprKind::Atom(TokenKind::Command(name)) => {
                        matches!(name.as_str(), "(" | ")" | "<" | ">" | "!" | "markup")
                            || crate::ornaments::is_post_mark(name)
                    }
                    _ => false,
                };
                if !is_postfix {
                    break;
                }
                self.one(cursor, child, depth)?;
            }
            Ok(())
        }
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
            "Staff" | "TabStaff" => {
                let staff_kind =
                    if kind == "TabStaff" { StaffKind::Tablature } else { StaffKind::Standard };
                let context = (name.as_str(), staff_kind, properties);
                self.staff_context(context, cursor, &mut child, span, depth)?;
            }
            "Voice" | "TabVoice" => {
                if let Some(properties) = properties {
                    self.reject_with_properties(properties, kind)?;
                }
                self.voice_context(&name, state.voice, cursor, &mut child, span, depth)?;
            }
            "PianoStaff" | "GrandStaff" | "StaffGroup" | "ChoirStaff" => {
                self.staff_group(kind, cursor, &mut child, span, depth)?;
                if let Some(properties) = properties {
                    // The group's own name stands beside its brace; its staves keep theirs.
                    let group = Label::Group(self.score.groups.len() - 1);
                    self.instrument_properties(properties, group)?;
                }
            }
            "Lyrics" => {
                if let Some(properties) = properties {
                    self.reject_with_properties(properties, "Lyrics")?;
                }
                return self.timed_lyrics(cursor, state, span);
            }
            "ChordNames" => {
                if let Some(properties) = properties {
                    self.reject_with_properties(properties, "ChordNames")?;
                }
                cursor.command("chordmode");
                return self.chord_names(cursor, state, span, depth + 1);
            }
            _ => return Err(unsupported(span, &format!("context {kind}"))),
        }
        self.hand_back_pending(&mut child, state);
        self.closed(&child)?;
        state.at = state.at.max(child.at);
        state.last = child.last;
        state.duration = child.duration;
        Ok(())
    }

    /// Opens a staff of `kind` (standard notation or tablature) named `name`, with
    /// optional `\with` properties, and reads its music.
    fn staff_context(
        &mut self,
        (name, kind, properties): (&str, StaffKind, Option<&[SyntaxId]>),
        cursor: &mut Cursor<'_>,
        child: &mut State,
        span: Span,
        depth: usize,
    ) -> Result<(), Error> {
        let reuse = self.can_reuse_staff();
        self.staff_contexts += 1;
        let staff = if reuse { StaffId(0) } else { self.alloc_staff(span)? };
        self.score.staves[staff.0 as usize].kind = kind;
        child.voice =
            if reuse { VoiceId(0) } else { self.new_voice(staff, "", child.at.main, span)? };
        if !name.is_empty() && self.score.voices[child.voice.0 as usize].name.is_empty() {
            self.score.voices[child.voice.0 as usize].name = name.into();
        }
        child.last = None;
        if let Some(properties) = properties {
            self.instrument_properties(properties, Label::Staff(staff))?;
        }
        self.one(cursor, child, depth + 1)
    }

    fn voice_context(
        &mut self,
        name: &str,
        parent_voice: VoiceId,
        cursor: &mut Cursor<'_>,
        child: &mut State,
        span: Span,
        depth: usize,
    ) -> Result<(), Error> {
        let staff = self.score.voices[parent_voice.0 as usize].staff;
        child.voice = if !self.voice_has_rhythmic(parent_voice)
            && !self.explicit_voices.contains(&parent_voice)
        {
            parent_voice
        } else {
            self.reuse_or_new_voice(staff, name, child.at.main, None, span)?
        };
        self.explicit_voices.insert(child.voice);
        if !name.is_empty() {
            self.score.voices[child.voice.0 as usize].name = name.into();
        }
        self.one(cursor, child, depth + 1)
    }

    fn alloc_staff(&mut self, span: Span) -> Result<StaffId, Error> {
        if self.score.staves.len() >= 24 {
            return Err(limit("staves", span));
        }
        let staff = StaffId(self.score.staves.len() as u32);
        push(&mut self.score.staves, Staff::default(), span)?;
        Ok(staff)
    }

    pub(crate) fn voice_has_rhythmic(&self, voice: VoiceId) -> bool {
        self.score.voices[voice.0 as usize].events.iter().any(|id| {
            matches!(
                self.score.events[id.0 as usize].kind,
                EventKind::Chord(_)
                    | EventKind::Rest(_)
                    | EventKind::Skip(_)
                    | EventKind::MeasureRest(_)
            )
        })
    }

    fn can_reuse_staff(&self) -> bool {
        self.staff_contexts == 0 && !self.voice_has_rhythmic(VoiceId(0))
    }

    fn staff_group(
        &mut self,
        kind: &str,
        cursor: &mut Cursor<'_>,
        child: &mut State,
        span: Span,
        depth: usize,
    ) -> Result<StaffId, Error> {
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
            StaffGroup::new(first, StaffId(self.score.staves.len() as u32 - 1), kind),
            span,
        )?;
        Ok(first)
    }

    pub(crate) fn reuse_or_new_voice(
        &mut self,
        staff: StaffId,
        name: &str,
        at: Ratio,
        exclude: Option<VoiceId>,
        span: Span,
    ) -> Result<VoiceId, Error> {
        for (index, voice) in self.score.voices.iter().enumerate() {
            let id = VoiceId(index as u32);
            if voice.staff != staff || exclude == Some(id) {
                continue;
            }
            if !name.is_empty() && !voice.name.is_empty() && voice.name != name {
                continue;
            }
            if self.voice_free(voice, at) {
                return Ok(id);
            }
        }
        self.new_voice(staff, name, at, span)
    }

    /// Whether a voice has finished by `at`. A voice's events are sequential and ordered,
    /// so its last event ends last; only that one needs checking.
    fn voice_free(&self, voice: &Voice, at: Ratio) -> bool {
        voice.events.last().is_none_or(|id| {
            let event = &self.score.events[id.0 as usize];
            let time = event.kind.duration().and_then(|d| d.time().ok()).unwrap_or(Ratio::ZERO);
            event.at.main.checked_add(time).is_ok_and(|end| end <= at)
        })
    }

    /// Adds a voice to `staff` from `at`. Four voices may sound at once, as LilyPond's
    /// voice styles describe; finished voices with other names do not count.
    pub(crate) fn new_voice(
        &mut self,
        staff: StaffId,
        name: &str,
        at: Ratio,
        span: Span,
    ) -> Result<VoiceId, Error> {
        let on_staff = || self.score.voices.iter().filter(move |v| v.staff == staff);
        if on_staff().count() >= gd_score::MAX_STAFF_VOICES {
            return Err(limit("voices per staff", span));
        }
        let count = on_staff().filter(|v| !self.voice_free(v, at)).count();
        if count >= 4 {
            return Err(Error::new(
                crate::ErrorKind::ReadLimit,
                "This voice would be the fifth sounding at once on its staff; at most four may.",
            )
            .at(span)
            .hint("End a voice before the next begins, or put this one on its own staff."));
        }
        let id = VoiceId(self.score.voices.len() as u32);
        push(
            &mut self.score.voices,
            Voice {
                staff,
                name: name.into(),
                direction: match count {
                    0 => Direction::Auto,
                    2 => Direction::Up,
                    _ => Direction::Down,
                },
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
        let ctx = cursor.text()?;
        if !matches!(ctx, "Staff" | "PianoStaff" | "GrandStaff" | "StaffGroup" | "ChoirStaff")
            || !cursor.symbol('.')
        {
            return Err(unsupported(span, "\\set outside Staff instrument names"));
        }
        let property = cursor.text()?;
        if !cursor.symbol('=') {
            return Err(syntax_error(span, "An instrument name needs =."));
        }
        let (value, markup) = self.instrument_label_expr(cursor)?;
        let staff = self.score.voices[state.voice.0 as usize].staff;
        self.instrument_value(Label::Staff(staff), property, value, markup, span)
    }

    fn reject_with_properties(&self, ids: &[SyntaxId], context: &str) -> Result<(), Error> {
        let cursor = Cursor::new(self.syntax, ids);
        if let Some(node) = cursor.peek() {
            if let ExprKind::Atom(TokenKind::Command(name)) = &node.kind {
                return Err(self.command_error(name, node.span));
            }
            if matches!(node.kind, ExprKind::Atom(TokenKind::Symbol('#' | '$'))) {
                return Err(unsupported(node.span, "Scheme"));
            }
            return Err(unsupported(node.span, &format!("{context} \\with properties")));
        }
        Ok(())
    }

    fn instrument_properties(&mut self, ids: &[SyntaxId], target: Label) -> Result<(), Error> {
        let mut cursor = Cursor::new(self.syntax, ids);
        while let Some(node) = cursor.peek() {
            let span = node.span;
            if let ExprKind::Atom(TokenKind::Command(name)) = &node.kind {
                return Err(self.command_error(name, span));
            }
            if matches!(node.kind, ExprKind::Atom(TokenKind::Symbol('#' | '$'))) {
                return Err(unsupported(span, "Scheme"));
            }
            let property = cursor.text()?;
            if !cursor.symbol('=') {
                return Err(syntax_error(span, "An instrument name needs =."));
            }
            let (value, markup) = self.instrument_label_expr(&mut cursor)?;
            self.instrument_value(target, property, value, markup, span)?;
        }
        Ok(())
    }

    fn instrument_label_expr(
        &self,
        cursor: &mut Cursor<'_>,
    ) -> Result<(String, Option<Markup>), Error> {
        let span = cursor.span();
        if cursor.symbol('#') {
            let node = cursor.next()?;
            return match &node.kind {
                ExprKind::Atom(TokenKind::Text(text)) => {
                    Ok((self.copy_text(text, node.span)?, None))
                }
                _ => Err(unsupported(node.span, "Scheme")),
            };
        }
        if cursor.command("markup") {
            let markup = self.read_markup(cursor, 0)?;
            return Ok((markup.plain_text(), Some(markup)));
        }
        if let Some(ExprKind::Atom(TokenKind::Command(name))) = cursor.peek().map(|n| &n.kind)
            && let Some(ids) = self.variables.get(name)
        {
            cursor.at += 1;
            if matches!(
                ids.first().and_then(|id| self.syntax.node(*id)).map(|n| &n.kind),
                Some(ExprKind::Atom(TokenKind::Command(c))) if c == "markup"
            ) {
                let markup = self.expand_markup_var(ids, span, 0)?;
                return Ok((markup.plain_text(), Some(markup)));
            }
            let mut value = Cursor::new(self.syntax, ids);
            let text = self.string_value(&mut value, 1)?;
            if value.peek().is_some() {
                return Err(syntax_error(span, "The variable is not a string."));
            }
            return Ok((text, None));
        }
        Ok((self.string_value(cursor, 0)?, None))
    }

    fn instrument_value(
        &mut self,
        target: Label,
        property: &str,
        value: String,
        markup: Option<Markup>,
        span: Span,
    ) -> Result<(), Error> {
        let (name, short_name, name_markup, short_name_markup) = match target {
            Label::Staff(staff) => {
                let s = &mut self.score.staves[staff.0 as usize];
                (&mut s.name, &mut s.short_name, &mut s.name_markup, &mut s.short_name_markup)
            }
            Label::Group(group) => {
                let g = &mut self.score.groups[group];
                (&mut g.name, &mut g.short_name, &mut g.name_markup, &mut g.short_name_markup)
            }
        };
        match property {
            "instrumentName" => {
                *name = value;
                *name_markup = markup;
            }
            "shortInstrumentName" => {
                *short_name = value;
                *short_name_markup = markup;
            }
            _ => return Err(unsupported(span, &format!("staff property {property}"))),
        }
        Ok(())
    }
}

/// Who an instrument name belongs to: one staff, or a staff group by index.
#[derive(Clone, Copy)]
enum Label {
    Staff(StaffId),
    Group(usize),
}

fn is_voice(node: &crate::Expr) -> bool {
    matches!(&node.kind, ExprKind::Atom(TokenKind::Word(kind) | TokenKind::Text(kind))
        if kind == "Voice")
}

fn clear_open_spans(state: &mut State) {
    state.ties.clear();
    state.slurs.clear();
    state.grace_slurs.clear();
    state.beam = None;
    state.hairpin = None;
}
