use crate::cursor::Cursor;
use crate::interpret::Reader;
use crate::limits::limit;
use crate::parser::syntax_error;
use crate::{Error, ExprKind, SyntaxId, TokenKind};

impl Reader<'_> {
    pub(crate) fn capture(&self, cursor: &mut Cursor<'_>, depth: usize) -> Result<(), Error> {
        if depth > self.limits.depth {
            return Err(limit("binding syntax", cursor.span()));
        }
        let node = cursor.next()?;
        match &node.kind {
            ExprKind::Atom(TokenKind::Command(name)) => {
                self.capture_command(name, cursor, depth + 1)?
            }
            ExprKind::Atom(TokenKind::Word(_)) | ExprKind::Chord(_) => {
                self.capture_suffix(cursor, depth + 1)?
            }
            ExprKind::Atom(TokenKind::Text(_))
            | ExprKind::Sequence(_)
            | ExprKind::Simultaneous(_) => {}
            _ => {
                return Err(syntax_error(
                    node.span,
                    "An assignment needs music, lyrics or a string.",
                ));
            }
        }
        Ok(())
    }

    fn capture_command(
        &self,
        name: &str,
        cursor: &mut Cursor<'_>,
        depth: usize,
    ) -> Result<(), Error> {
        match name {
            "relative" => {
                if matches!(
                    cursor.peek().map(|n| &n.kind),
                    Some(ExprKind::Atom(TokenKind::Word(_)))
                ) {
                    self.capture_pitch(cursor)?;
                }
                self.capture(cursor, depth)
            }
            "fixed" | "transpose" => {
                self.capture_pitch(cursor)?;
                if name == "transpose" {
                    self.capture_pitch(cursor)?;
                }
                self.capture(cursor, depth)
            }
            "tuplet" | "times" => {
                cursor.ratio()?;
                self.capture(cursor, depth)
            }
            "grace" | "acciaccatura" | "appoggiatura" | "lyricmode" | "chordmode" | "chords" => {
                self.capture(cursor, depth)
            }
            "afterGrace" => {
                self.capture(cursor, depth)?;
                self.capture(cursor, depth)
            }
            "repeat" => {
                cursor.text()?;
                cursor.number()?;
                self.capture(cursor, depth)?;
                if cursor.command("alternative") {
                    self.capture(cursor, depth)?;
                }
                Ok(())
            }
            "new" => {
                cursor.text()?;
                if cursor.symbol('=') {
                    cursor.text()?;
                }
                if cursor.command("with") {
                    cursor.sequence()?;
                }
                self.capture(cursor, depth)
            }
            _ => self.capture_notation(name, cursor, depth),
        }
    }

    fn capture_notation(
        &self,
        name: &str,
        cursor: &mut Cursor<'_>,
        depth: usize,
    ) -> Result<(), Error> {
        match name {
            "clef" | "language" | "version" | "bar" => {
                cursor.text()?;
            }
            "key" => {
                self.capture_pitch(cursor)?;
                cursor.next()?;
            }
            "time" => {
                let _ = cursor.number()?;
                while cursor.symbol(',') {
                    let _ = cursor.number()?;
                }
                if matches!(
                    cursor.peek().map(|n| &n.kind),
                    Some(ExprKind::Atom(TokenKind::Number(_)))
                ) {
                    let _ = cursor.number()?;
                }
                if cursor.symbol('/') {
                    let _ = cursor.number()?;
                }
            }
            "partial" => self.capture_suffix(cursor, depth)?,
            "markup" => self.capture_markup(cursor, depth)?,
            "mark" => {
                if !cursor.command("default") {
                    self.capture(cursor, depth)?;
                }
            }
            "tempo" => {
                if matches!(
                    cursor.peek().map(|n| &n.kind),
                    Some(ExprKind::Atom(TokenKind::Text(_)))
                ) {
                    let _ = cursor.next()?;
                } else if cursor.command("markup") {
                    self.capture_markup(cursor, depth)?;
                }
                if matches!(
                    cursor.peek().map(|n| &n.kind),
                    Some(ExprKind::Atom(TokenKind::Number(_)))
                ) {
                    self.capture_suffix(cursor, depth)?;
                }
            }
            "set" => {
                cursor.text()?;
                if cursor.symbol('.') {
                    cursor.text()?;
                }
                if cursor.symbol('=') {
                    self.capture(cursor, depth)?;
                }
            }
            "header" | "layout" | "midi" | "score" => {
                cursor.sequence()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn capture_pitch(&self, cursor: &mut Cursor<'_>) -> Result<(), Error> {
        let node = cursor.next()?;
        if !matches!(node.kind, ExprKind::Atom(TokenKind::Word(_))) {
            return Err(syntax_error(node.span, "Expected a reference pitch."));
        }
        while cursor.symbol('\'') || cursor.symbol(',') {}
        Ok(())
    }

    fn capture_suffix(&self, cursor: &mut Cursor<'_>, depth: usize) -> Result<(), Error> {
        while let Some(node) = cursor.peek() {
            match &node.kind {
                ExprKind::Atom(TokenKind::Symbol(
                    '\'' | ',' | '!' | '?' | '.' | '=' | '*' | '/' | '~' | '(' | ')' | '[' | ']',
                ))
                | ExprKind::Atom(TokenKind::Number(_)) => cursor.at += 1,
                ExprKind::Atom(TokenKind::Symbol('-' | '^' | '_')) => {
                    cursor.at += 1;
                    if let Some(next) = cursor.peek() {
                        let is_markup = matches!(
                            &next.kind,
                            ExprKind::Atom(TokenKind::Command(c)) if c == "markup"
                        );
                        if is_markup {
                            cursor.at += 1;
                            self.capture_markup(cursor, depth + 1)?;
                        } else {
                            cursor.at += 1;
                        }
                    }
                }
                ExprKind::Atom(TokenKind::Command(name))
                    if name == "breve"
                        || matches!(name.as_str(), "(" | ")" | "<" | ">" | "!")
                        || crate::ornaments::is_post_mark(name) =>
                {
                    cursor.at += 1;
                }
                ExprKind::Atom(TokenKind::Command(name)) if name == "markup" => {
                    cursor.at += 1;
                    self.capture_markup(cursor, depth + 1)?;
                }
                _ => break,
            }
        }
        Ok(())
    }

    pub(crate) fn string_value(
        &self,
        cursor: &mut Cursor<'_>,
        depth: usize,
    ) -> Result<String, Error> {
        if depth > self.limits.depth {
            return Err(limit("string expansion", cursor.span()));
        }
        let node = cursor.next()?;
        match &node.kind {
            ExprKind::Atom(TokenKind::Text(value) | TokenKind::Word(value)) => {
                self.copy_text(value, node.span)
            }
            ExprKind::Atom(TokenKind::Command(name)) => {
                let ids: &[SyntaxId] = self
                    .variables
                    .get(name)
                    .ok_or_else(|| syntax_error(node.span, "Unknown string variable."))?;
                let mut value = Cursor::new(self.syntax, ids);
                let text = self.string_value(&mut value, depth + 1)?;
                if value.peek().is_some() {
                    return Err(syntax_error(node.span, "The variable is not a string."));
                }
                Ok(text)
            }
            _ => Err(syntax_error(node.span, "Expected a literal string or a string variable.")),
        }
    }
}
