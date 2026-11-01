use crate::parser::syntax_error;
use crate::{Error, Expr, ExprKind, Span, Syntax, SyntaxId, TokenKind};

#[derive(Debug)]
pub(crate) struct Cursor<'a> {
    pub(crate) syntax: &'a Syntax,
    pub(crate) ids: &'a [SyntaxId],
    pub(crate) at: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(syntax: &'a Syntax, ids: &'a [SyntaxId]) -> Self {
        Self { syntax, ids, at: 0 }
    }

    pub(crate) fn peek(&self) -> Option<&'a Expr> {
        self.ids.get(self.at).and_then(|id| self.syntax.node(*id))
    }

    pub(crate) fn next(&mut self) -> Result<&'a Expr, Error> {
        let node = self
            .peek()
            .ok_or_else(|| syntax_error(self.span(), "Incomplete music expression."))?;
        self.at += 1;
        Ok(node)
    }

    pub(crate) fn span(&self) -> Span {
        self.peek()
            .map(|n| n.span)
            .or_else(|| self.ids.last().and_then(|id| self.syntax.node(*id)).map(|n| n.span))
            .unwrap_or_default()
    }

    pub(crate) fn symbol(&mut self, expected: char) -> bool {
        if matches!(self.peek().map(|n| &n.kind), Some(ExprKind::Atom(TokenKind::Symbol(c)))
            if *c == expected)
        {
            self.at += 1;
            true
        } else {
            false
        }
    }

    pub(crate) fn command(&mut self, expected: &str) -> bool {
        if matches!(self.peek().map(|n| &n.kind), Some(ExprKind::Atom(TokenKind::Command(c)))
            if c == expected)
        {
            self.at += 1;
            true
        } else {
            false
        }
    }

    pub(crate) fn number(&mut self) -> Result<u16, Error> {
        let node = self.next()?;
        let ExprKind::Atom(TokenKind::Number(value)) = &node.kind else {
            return Err(syntax_error(node.span, "Expected a positive integer."));
        };
        value.parse().map_err(|_| syntax_error(node.span, "The integer is too large."))
    }

    pub(crate) fn text(&mut self) -> Result<&'a str, Error> {
        let node = self.next()?;
        match &node.kind {
            ExprKind::Atom(TokenKind::Text(value) | TokenKind::Word(value)) => Ok(value),
            _ => Err(syntax_error(node.span, "Expected a name or a quoted string.")),
        }
    }

    pub(crate) fn sequence(&mut self) -> Result<&'a [SyntaxId], Error> {
        let node = self.next()?;
        match &node.kind {
            ExprKind::Sequence(ids) => Ok(ids),
            _ => Err(syntax_error(node.span, "Expected music or lyrics inside braces.")),
        }
    }

    pub(crate) fn ratio(&mut self) -> Result<gd_score::Ratio, Error> {
        let span = self.span();
        let num = self.integer()?;
        if !self.symbol('/') {
            return Err(syntax_error(span, "Expected a fraction such as 3/2."));
        }
        let den = self.integer()?;
        gd_score::Ratio::new(num, den).map_err(|e| e.at(span))
    }

    pub(crate) fn integer(&mut self) -> Result<i64, Error> {
        let node = self.next()?;
        let ExprKind::Atom(TokenKind::Number(value)) = &node.kind else {
            return Err(syntax_error(node.span, "Expected a positive integer."));
        };
        value.parse().map_err(|_| syntax_error(node.span, "The integer exceeds i64."))
    }
}
