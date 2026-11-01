use gd_score::{BarStyle, BreakRule, Clef, Direction, EventKind, Key, Mode, Pitch, Step};

use crate::cursor::Cursor;
use crate::interpret::{PitchMode, Reader, State, unsupported};
use crate::names::Language;
use crate::notes::read_duration;
use crate::parser::syntax_error;
use crate::{Error, ErrorKind, ExprKind, Span, TokenKind};

impl Reader<'_> {
    pub(crate) fn command(
        &mut self,
        name: &str,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
        depth: usize,
    ) -> Result<(), Error> {
        if let Some(ids) = self.variables.get(name).cloned() {
            return self.sequence(&ids, state, depth + 1);
        }
        match name {
            "relative" | "fixed" | "transpose" => self.pitch_group(name, cursor, state, depth),
            "score" => {
                self.scores += 1;
                if self.scores > 1 {
                    return Err(unsupported(span, "more than one score"));
                }
                let ids = cursor.sequence()?;
                self.sequence(ids, state, depth + 1)
            }
            "language" => {
                state.language = Language::read(cursor.text()?, span)?;
                Ok(())
            }
            "version" => {
                cursor.text()?;
                self.ignored(
                    span,
                    "The version declaration does not change the Guidedog profile.",
                );
                Ok(())
            }
            "header" | "midi" | "layout" => self.metadata(name, cursor, span),
            "clef" | "key" | "time" | "partial" | "bar" | "break" | "noBreak" => {
                self.notation(name, cursor, state, span)
            }
            "stemUp" | "stemDown" | "stemNeutral" | "voiceOne" | "voiceTwo" | "oneVoice" => {
                state.direction = match name {
                    "stemUp" | "voiceOne" => Direction::Up,
                    "stemDown" | "voiceTwo" => Direction::Down,
                    _ => Direction::Auto,
                };
                Ok(())
            }
            "autoBeamOff" | "autoBeamOn" => {
                state.auto_beam = name == "autoBeamOn";
                Ok(())
            }
            "tuplet" | "times" => self.tuplet(name, cursor, state, span, depth),
            "new" => self.context(cursor, state, span, depth),
            "set" => self.set_instrument(cursor, state, span),
            "addlyrics" | "lyricsto" => self.lyrics(name, cursor, state, span, depth),
            "grace" | "acciaccatura" | "appoggiatura" | "afterGrace" => {
                self.grace(name, cursor, state, span, depth)
            }
            "repeat" => self.repeat(cursor, state, span, depth),
            "chordmode" => self.chord_names(cursor, state, span),
            _ if self.mark_command(name, cursor, state, span)? => Ok(()),
            _ => Err(self.command_error(name, span)),
        }
    }

    fn pitch_group(
        &mut self,
        name: &str,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        depth: usize,
    ) -> Result<(), Error> {
        let mut child = self.copy_state(state, cursor.span())?;
        child.pitch_mode = match name {
            "relative" => {
                let start = if matches!(
                    cursor.peek().map(|n| &n.kind),
                    Some(ExprKind::Atom(TokenKind::Word(_)))
                ) {
                    self.pitch_argument(cursor, state)?
                } else {
                    Pitch { step: Step::F, ..Pitch::default() }
                };
                PitchMode::Relative(start)
            }
            "fixed" => PitchMode::Fixed(self.pitch_argument(cursor, state)?.octave),
            _ => {
                let from = self.pitch_argument(cursor, state)?;
                let to = self.pitch_argument(cursor, state)?;
                child.transpositions.push((from, to));
                PitchMode::Absolute
            }
        };
        self.one(cursor, &mut child, depth + 1)?;
        let mode = state.pitch_mode;
        let transpositions = std::mem::take(&mut state.transpositions);
        *state = child;
        state.pitch_mode = mode;
        state.transpositions = transpositions;
        Ok(())
    }

    fn notation(
        &mut self,
        name: &str,
        cursor: &mut Cursor<'_>,
        state: &mut State,
        span: Span,
    ) -> Result<(), Error> {
        let kind = match name {
            "clef" => EventKind::Clef(read_clef(cursor.text()?, span)?),
            "key" => {
                let mut tonic = self.pitch_argument(cursor, state)?;
                for (from, to) in state.transpositions.iter().rev() {
                    tonic = tonic.transpose(*from, *to)?;
                }
                let node = cursor.next()?;
                let ExprKind::Atom(TokenKind::Command(mode)) = &node.kind else {
                    return Err(syntax_error(node.span, "A key needs a mode such as \\major."));
                };
                EventKind::Key(Key { tonic, mode: read_mode(mode, node.span)? })
            }
            "time" => {
                let first = cursor.number()?;
                let mut beats = Vec::new();
                let numerator = if cursor.symbol(',') {
                    beats.push(first);
                    loop {
                        beats.push(cursor.number()?);
                        if !cursor.symbol(',') {
                            break;
                        }
                    }
                    cursor.number()?
                } else {
                    first
                };
                if !cursor.symbol('/') {
                    return Err(syntax_error(span, "A meter needs n/d."));
                }
                let denominator = cursor.number()?;
                if numerator == 0
                    || numerator > 128
                    || !denominator.is_power_of_two()
                    || denominator > 128
                    || beats.contains(&0)
                    || (!beats.is_empty()
                        && beats.iter().map(|n| u32::from(*n)).sum::<u32>()
                            != u32::from(numerator))
                {
                    return Err(syntax_error(span, "Invalid meter or beaming-group sum."));
                }
                state.meter = gd_score::Meter { numerator, denominator, beats };
                EventKind::Time(state.meter.clone())
            }
            "partial" => {
                if self.partial.is_some()
                    || self.score.events.iter().any(|e| e.kind.duration().is_some())
                {
                    return Err(syntax_error(
                        span,
                        "A pickup is allowed only once, before the first note.",
                    ));
                }
                let duration = read_duration(cursor, state.duration)?.time()?;
                self.partial = Some(duration);
                state.at.main = duration.checked_neg()?;
                for event in &mut self.score.events {
                    event.at.main = state.at.main;
                }
                return Ok(());
            }
            "bar" => EventKind::Bar(read_bar(cursor.text()?, span)?),
            "break" => EventKind::Break(BreakRule::Force),
            _ => EventKind::Break(BreakRule::Forbid),
        };
        self.emit(state, kind, span)?;
        Ok(())
    }

    fn metadata(&mut self, name: &str, cursor: &mut Cursor<'_>, span: Span) -> Result<(), Error> {
        let ids = cursor.sequence()?;
        if name != "header" && !ids.is_empty() {
            return Err(unsupported(span, &format!("nonempty \\{name}")).hint(
                "Use staff-size, width and justify directive options for the house style.",
            ));
        }
        let mut body = Cursor::new(self.syntax, ids);
        while body.peek().is_some() {
            body.text()?;
            if !body.symbol('=') {
                return Err(syntax_error(body.span(), "A header field needs =."));
            }
            if body.symbol('#') {
                if body.symbol('#') {
                    let value = body.text()?;
                    if !matches!(value, "t" | "f") {
                        return Err(unsupported(body.span(), "Scheme"));
                    }
                } else if body.symbol('\'') {
                    body.text()?;
                } else {
                    let node = body.next()?;
                    if !matches!(
                        node.kind,
                        ExprKind::Atom(TokenKind::Number(_) | TokenKind::Text(_))
                    ) {
                        return Err(unsupported(node.span, "Scheme"));
                    }
                }
            } else {
                let node = body.next()?;
                if !matches!(node.kind, ExprKind::Atom(TokenKind::Text(_))) {
                    return Err(syntax_error(node.span, "Header values must be literal strings."));
                }
            }
        }
        self.ignored(span, &format!("The \\{name} block is accepted without printing metadata."));
        Ok(())
    }

    fn ignored(&mut self, span: Span, message: &str) {
        self.warnings.push(
            Error::new(ErrorKind::Unsupported, message).at(span).warning().hint(
                "Remove the metadata to avoid this warning; captions name embedded examples.",
            ),
        );
    }

    fn command_error(&self, name: &str, span: Span) -> Error {
        if crate::command(name).is_some() {
            return unsupported(span, &format!("\\{name}"));
        }
        let suggestion = gd_report::suggest(name, crate::COMMANDS.iter().map(|c| c.name));
        let mut hint = "Correct the command or use the documented notation profile.".to_string();
        if let Some(suggestion) = suggestion {
            hint.push_str(&format!(" Did you mean `\\{suggestion}`?"));
        }
        Error::new(ErrorKind::UnknownCommand, format!("Unknown LilyPond command `\\{name}`."))
            .at(span)
            .hint(hint)
    }
}

fn read_clef(name: &str, span: Span) -> Result<Clef, Error> {
    match name {
        "treble" => Ok(Clef::Treble),
        "bass" => Ok(Clef::Bass),
        "alto" => Ok(Clef::Alto),
        "tenor" => Ok(Clef::Tenor),
        "soprano" => Ok(Clef::Soprano),
        "treble_8" => Ok(Clef::TrebleEight),
        "bass_8" => Ok(Clef::BassEight),
        _ => Err(unsupported(span, &format!("clef {name}"))),
    }
}

fn read_mode(name: &str, span: Span) -> Result<Mode, Error> {
    match name {
        "major" | "ionian" => Ok(Mode::Major),
        "minor" | "aeolian" => Ok(Mode::Minor),
        "dorian" => Ok(Mode::Dorian),
        "phrygian" => Ok(Mode::Phrygian),
        "lydian" => Ok(Mode::Lydian),
        "mixolydian" => Ok(Mode::Mixolydian),
        "locrian" => Ok(Mode::Locrian),
        _ => Err(unsupported(span, &format!("mode {name}"))),
    }
}

fn read_bar(name: &str, span: Span) -> Result<BarStyle, Error> {
    match name {
        "|" => Ok(BarStyle::Single),
        "||" => Ok(BarStyle::Double),
        "|." => Ok(BarStyle::Final),
        ".|" => Ok(BarStyle::Initial),
        ".|:" => Ok(BarStyle::RepeatStart),
        ":|." => Ok(BarStyle::RepeatEnd),
        ":..:" => Ok(BarStyle::RepeatBoth),
        _ => Err(unsupported(span, &format!("bar style {name}"))),
    }
}
