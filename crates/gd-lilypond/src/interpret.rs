use std::collections::{BTreeMap, BTreeSet};

use gd_score::{
    BarStyle, Direction, Duration, EventId, EventKind, Meter, Moment, NoteRef, Pitch, Ratio,
    Score, Staff, StaffId, Voice, VoiceId,
};

use crate::cursor::Cursor;
use crate::limits::{Work, limit};
use crate::names::Language;
use crate::parser::syntax_error;
use crate::{
    Error, ErrorKind, ExprKind, Failed, Limits, Loaded, Span, Syntax, SyntaxId, TokenKind,
};

/// Interprets a borrowed syntax arena under explicit expansion and event bounds.
/// Returns a separate score and retained source, or the first placed diagnostic.
pub fn interpret_syntax(syntax: &Syntax, limits: &Limits) -> Result<Loaded, Failed> {
    let mut reader = Reader::new(syntax, limits);
    let mut state = State::default();
    let result = reader
        .sequence(&syntax.roots, &mut state, 0)
        .and_then(|()| reader.flush_pending(&mut state))
        .and_then(|()| reader.closed(&state))
        .and_then(|()| reader.finish());
    match result {
        Ok(()) => Ok(Loaded {
            score: reader.score,
            files: syntax.files.clone(),
            warnings: reader.warnings,
        }),
        Err(error) => Err(Failed { error, files: syntax.files.clone() }),
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) enum PitchMode {
    #[default]
    Absolute,
    Relative(Pitch),
    Fixed(i8),
}

#[derive(Clone, Debug)]
pub(crate) struct State {
    pub(crate) voice: VoiceId,
    pub(crate) voice_style: Option<gd_score::VoiceStyle>,
    pub(crate) at: Moment,
    pub(crate) duration: Duration,
    pub(crate) scale: Ratio,
    pub(crate) language: Language,
    pub(crate) pitch_mode: PitchMode,
    pub(crate) transpositions: Vec<(Pitch, Pitch)>,
    pub(crate) direction: Direction,
    pub(crate) slur_direction: Direction,
    pub(crate) tie_direction: Direction,
    pub(crate) dynamic_direction: Direction,
    pub(crate) auto_beam: bool,
    pub(crate) last_chord: Vec<Pitch>,
    pub(crate) last: Option<EventId>,
    pub(crate) ties: Vec<(NoteRef, Direction, Span)>,
    pub(crate) slurs: Vec<(NoteRef, bool, Direction, Span)>,
    pub(crate) grace_slurs: Vec<(NoteRef, Span)>,
    pub(crate) beam: Option<(NoteRef, Span)>,
    pub(crate) hairpin: Option<(NoteRef, bool, Direction, Span)>,
    pub(crate) pending: Vec<gd_score::Annotation>,
    pub(crate) meter: Meter,
    pub(crate) grace_slash: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            voice: VoiceId(0),
            voice_style: None,
            at: Moment::default(),
            duration: Duration::default(),
            scale: Ratio::ONE,
            language: Language::default(),
            pitch_mode: PitchMode::Absolute,
            transpositions: Vec::new(),
            direction: Direction::Auto,
            slur_direction: Direction::Auto,
            tie_direction: Direction::Auto,
            dynamic_direction: Direction::Auto,
            auto_beam: true,
            last_chord: Vec::new(),
            last: None,
            ties: Vec::new(),
            slurs: Vec::new(),
            grace_slurs: Vec::new(),
            beam: None,
            hairpin: None,
            pending: Vec::new(),
            meter: Meter::default(),
            grace_slash: false,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Reader<'a> {
    pub(crate) syntax: &'a Syntax,
    pub(crate) limits: &'a Limits,
    pub(crate) work: Work,
    pub(crate) memory: crate::memory::Memory,
    pub(crate) score: Score,
    pub(crate) warnings: Vec<Error>,
    pub(crate) variables: BTreeMap<String, Vec<SyntaxId>>,
    /// The default duration and language at each variable's definition.
    pub(crate) variable_context: BTreeMap<String, (Duration, Language)>,
    pub(crate) bar_checks: Vec<(Ratio, Span)>,
    pub(crate) partial: Option<Ratio>,
    pub(crate) scores: usize,
    pub(crate) rehearsal: u16,
    pub(crate) staff_contexts: usize,
    pub(crate) explicit_voices: BTreeSet<VoiceId>,
    pub(crate) pending_lyrics: Vec<crate::lyrics::PendingLyrics>,
}

impl<'a> Reader<'a> {
    fn new(syntax: &'a Syntax, limits: &'a Limits) -> Self {
        Self {
            syntax,
            limits,
            work: Work::new(limits),
            memory: crate::memory::Memory::new(limits.memory_bytes),
            warnings: Vec::new(),
            score: Score {
                staves: vec![Staff::default()],
                voices: vec![Voice { staff: StaffId(0), ..Voice::default() }],
                ..Score::default()
            },
            variables: BTreeMap::new(),
            variable_context: BTreeMap::new(),
            bar_checks: Vec::new(),
            partial: None,
            scores: 0,
            rehearsal: 0,
            staff_contexts: 0,
            explicit_voices: BTreeSet::new(),
            pending_lyrics: Vec::new(),
        }
    }

    pub(crate) fn sequence(
        &mut self,
        ids: &[SyntaxId],
        state: &mut State,
        depth: usize,
    ) -> Result<(), Error> {
        let mut cursor = Cursor::new(self.syntax, ids);
        while cursor.peek().is_some() {
            self.one(&mut cursor, state, depth)?;
        }
        Ok(())
    }

    pub(crate) fn one(
        &mut self,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        depth: usize,
    ) -> Result<(), Error> {
        self.work.take(1, cursor.span())?;
        self.memory.take(512, cursor.span())?;
        if depth > self.limits.depth {
            return Err(limit("expansion depth", cursor.span()));
        }
        let node = cursor.next()?;
        match &node.kind {
            ExprKind::Sequence(ids) => self.sequence(ids, state, depth + 1),
            ExprKind::Simultaneous(ids) => self.simultaneous(ids, state, depth + 1),
            ExprKind::Chord(ids) => self.chord(ids, cursor, state, node.span),
            ExprKind::Atom(TokenKind::Word(word)) => {
                if cursor.symbol('=') {
                    self.assignment(word, cursor, state)
                } else {
                    self.note(word, cursor, state, node.span)
                }
            }
            ExprKind::Atom(TokenKind::Command(name)) => {
                self.command(name, cursor, state, node.span, depth)
            }
            ExprKind::Atom(TokenKind::Symbol('|')) => {
                self.bar_checks.push((state.at.main, node.span));
                Ok(())
            }
            ExprKind::Atom(TokenKind::Symbol(c)) if "~()[]-^_".contains(*c) => {
                self.postfix(*c, cursor, state, node.span)
            }
            ExprKind::Atom(TokenKind::Symbol('#' | '$')) => Err(unsupported(node.span, "Scheme")),
            _ => Err(syntax_error(
                node.span,
                "Expected notes, a music group or a supported command.",
            )),
        }
    }

    /// Stores a variable with the default duration and note-name language in effect where
    /// it is written: LilyPond resolves both while reading the definition, not at its use.
    pub(crate) fn assignment(
        &mut self,
        name: &str,
        cursor: &mut Cursor<'_>,
        state: &State,
    ) -> Result<(), Error> {
        let start = cursor.at;
        self.capture(cursor, 0)?;
        self.memory.take(name.len() + (cursor.at - start) * 8 + 128, cursor.span())?;
        self.variables.insert(name.to_string(), cursor.ids[start..cursor.at].to_vec());
        self.variable_context.insert(name.to_string(), (state.duration, state.language));
        Ok(())
    }

    /// Attaches marks written after the last note of the whole piece. Only the top level
    /// may carry them on a bar of their own: there the bar is the closing bar line.
    pub(crate) fn flush_pending(&mut self, state: &mut State) -> Result<(), Error> {
        if self.attach_pending(state) {
            return Ok(());
        }
        let pending = std::mem::take(&mut state.pending);
        let id = self.emit(state, EventKind::Bar(BarStyle::Single), Span::default())?;
        self.score.events[id.0 as usize].marks.extend(pending);
        Ok(())
    }

    /// Moves pending marks to an event of the voice at the state's moment; false when
    /// marks remain because no such event exists.
    pub(crate) fn attach_pending(&mut self, state: &mut State) -> bool {
        if state.pending.is_empty() {
            return true;
        }
        let Some(event) = self
            .score
            .events
            .iter_mut()
            .rev()
            .find(|e| e.voice == state.voice && e.at == state.at)
        else {
            return false;
        };
        event.marks.append(&mut state.pending);
        true
    }

    /// Ends a branch or context: marks it could not attach move to the enclosing music,
    /// whose next event takes them, rather than becoming a bar line inside a measure.
    pub(crate) fn hand_back_pending(&mut self, child: &mut State, parent: &mut State) {
        if !self.attach_pending(child) {
            parent.pending.append(&mut child.pending);
        }
    }

    pub(crate) fn closed(&self, state: &State) -> Result<(), Error> {
        let span = state
            .slurs
            .last()
            .map(|s| s.3)
            .or(state.beam.map(|s| s.1))
            .or(state.hairpin.map(|s| s.3));
        if let Some(span) = span {
            return Err(syntax_error(span, "A slur, beam or hairpin is not closed."));
        }
        if let Some((_, _, span)) = state.ties.last() {
            return Err(syntax_error(*span, "A tie has no following note."));
        }
        Ok(())
    }
}

pub(crate) fn unsupported(span: Span, name: &str) -> Error {
    Error::new(
        ErrorKind::Unsupported,
        format!("`{name}` is outside the Guidedog LilyPond profile."),
    )
    .at(span)
    .hint("Use the documented notation profile. Guidedog engraves a fixed house style.")
}
