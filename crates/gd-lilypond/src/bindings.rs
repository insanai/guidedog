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
            ExprKind::Atom(TokenKind::Command(name)) => match name.as_str() {
                "relative" => {
                    if matches!(
                        cursor.peek().map(|n| &n.kind),
                        Some(ExprKind::Atom(TokenKind::Word(_)))
                    ) {
                        self.capture_pitch(cursor)?;
                    }
                    self.capture(cursor, depth + 1)?;
                }
                "fixed" | "transpose" => {
                    self.capture_pitch(cursor)?;
                    if name == "transpose" {
                        self.capture_pitch(cursor)?;
                    }
                    self.capture(cursor, depth + 1)?;
                }
                "tuplet" | "times" => {
                    cursor.ratio()?;
                    self.capture(cursor, depth + 1)?;
                }
                "grace" | "acciaccatura" | "appoggiatura" | "lyricmode" | "chordmode" => {
                    self.capture(cursor, depth + 1)?;
                }
                "afterGrace" => {
                    self.capture(cursor, depth + 1)?;
                    self.capture(cursor, depth + 1)?;
                }
                "repeat" => {
                    cursor.text()?;
                    cursor.number()?;
                    self.capture(cursor, depth + 1)?;
                    if cursor.command("alternative") {
                        self.capture(cursor, depth + 1)?;
                    }
                }
                "new" => {
                    cursor.text()?;
                    if cursor.symbol('=') {
                        cursor.text()?;
                    }
                    if cursor.command("with") {
                        cursor.sequence()?;
                    }
                    self.capture(cursor, depth + 1)?;
                }
                _ => {}
            },
            ExprKind::Atom(TokenKind::Word(_)) | ExprKind::Chord(_) => {
                self.capture_suffix(cursor)?
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

    fn capture_pitch(&self, cursor: &mut Cursor<'_>) -> Result<(), Error> {
        let node = cursor.next()?;
        if !matches!(node.kind, ExprKind::Atom(TokenKind::Word(_))) {
            return Err(syntax_error(node.span, "Expected a reference pitch."));
        }
        while cursor.symbol('\'') || cursor.symbol(',') {}
        Ok(())
    }

    fn capture_suffix(&self, cursor: &mut Cursor<'_>) -> Result<(), Error> {
        while let Some(node) = cursor.peek() {
            match &node.kind {
                ExprKind::Atom(TokenKind::Symbol(
                    '\'' | ',' | '!' | '?' | '.' | '=' | '*' | '/',
                ))
                | ExprKind::Atom(TokenKind::Number(_)) => cursor.at += 1,
                ExprKind::Atom(TokenKind::Command(name)) if name == "breve" => cursor.at += 1,
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
            ExprKind::Atom(TokenKind::Text(value) | TokenKind::Word(value)) => Ok(value.clone()),
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
