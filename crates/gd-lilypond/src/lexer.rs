use crate::limits::{Work, limit, push};
use crate::{Error, ErrorKind, FileId, Limits, Span, Token, TokenKind};

pub(crate) fn lex(
    source: &str,
    file: FileId,
    limits: &Limits,
    work: &mut Work,
) -> Result<Vec<Token>, Error> {
    let mut lexer = Lexer { source, file, at: 0, in_chord: false, limits, work };
    let mut tokens: Vec<Token> = Vec::new();
    while lexer.at < source.len() {
        lexer.space()?;
        if lexer.at == source.len() {
            break;
        }
        let start = lexer.at;
        let after_script = tokens.last().is_some_and(|prev| {
            matches!(prev.kind, TokenKind::Symbol('-' | '^' | '_')) && prev.span.end == start
        });
        let kind = lexer.token(after_script)?;
        let span = lexer.span(start);
        lexer.work.take(1, span)?;
        push(&mut tokens, Token { kind, span }, span)?;
    }
    Ok(tokens)
}

struct Lexer<'a, 'w> {
    source: &'a str,
    file: FileId,
    at: usize,
    in_chord: bool,
    limits: &'a Limits,
    work: &'w mut Work,
}

impl Lexer<'_, '_> {
    fn span(&self, start: usize) -> Span {
        Span { file: self.file, start, end: self.at }
    }

    fn peek(&self) -> Option<char> {
        self.source[self.at..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at += c.len_utf8();
        Some(c)
    }

    fn space(&mut self) -> Result<(), Error> {
        loop {
            while self.peek().is_some_and(|c| c.is_whitespace() || c == '\u{feff}') {
                self.advance();
            }
            if self.source[self.at..].starts_with("%{") {
                self.comment()?;
            } else if self.peek() == Some('%') {
                while self.peek().is_some_and(|c| c != '\n') {
                    self.advance();
                }
            } else {
                return Ok(());
            }
        }
    }

    fn comment(&mut self) -> Result<(), Error> {
        let start = self.at;
        self.at += 2;
        let mut depth = 1usize;
        while self.at < self.source.len() {
            self.work.take(1, self.span(start))?;
            if self.source[self.at..].starts_with("%{") {
                depth += 1;
                if depth > self.limits.depth {
                    return Err(limit("comment depth", self.span(start)));
                }
                self.at += 2;
            } else if self.source[self.at..].starts_with("%}") {
                depth -= 1;
                self.at += 2;
                if depth == 0 {
                    return Ok(());
                }
            } else {
                self.advance();
            }
        }
        Err(self.syntax(start, "The block comment has no closing %}."))
    }

    fn token(&mut self, after_script: bool) -> Result<TokenKind, Error> {
        let start = self.at;
        let Some(c) = self.advance() else {
            return Err(self.syntax(start, "Expected a music token."));
        };
        match c {
            '"' => self.string(start),
            '\\' => {
                let word = self.at;
                if self.peek().is_some_and(char::is_alphabetic) {
                    while self.peek().is_some_and(char::is_alphabetic)
                        || (self.source[self.at..].starts_with('-')
                            && self.source[self.at + 1..]
                                .chars()
                                .next()
                                .is_some_and(char::is_alphabetic))
                    {
                        self.advance();
                    }
                } else if self.advance().is_none() {
                    return Err(self.syntax(start, "A backslash must introduce a command."));
                }
                Ok(TokenKind::Command(self.source[word..self.at].to_string()))
            }
            '<' if self.peek() == Some('<') => {
                self.advance();
                Ok(TokenKind::SimultaneousStart)
            }
            '<' => {
                self.in_chord = true;
                Ok(TokenKind::Symbol('<'))
            }
            '>' if !self.in_chord && !after_script && self.peek() == Some('>') => {
                self.advance();
                Ok(TokenKind::SimultaneousEnd)
            }
            '>' => {
                if !after_script {
                    self.in_chord = false;
                }
                Ok(TokenKind::Symbol('>'))
            }
            '{' | '}' => {
                self.in_chord = false;
                Ok(TokenKind::Symbol(c))
            }
            c if c.is_ascii_digit() => {
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.advance();
                }
                Ok(TokenKind::Number(self.source[start..self.at].to_string()))
            }
            c if c.is_alphabetic() => {
                while self.peek().is_some_and(|c| {
                    c.is_alphabetic()
                        || c == '_'
                        || (c.is_ascii_digit() && self.source[start..self.at].contains('_'))
                }) {
                    self.advance();
                }
                Ok(TokenKind::Word(self.source[start..self.at].to_string()))
            }
            c if "[](),'!?~|.*:/=+-^_#$;".contains(c) => Ok(TokenKind::Symbol(c)),
            _ => Err(self.syntax(start, "This character is not music notation in the profile.")),
        }
    }

    fn string(&mut self, start: usize) -> Result<TokenKind, Error> {
        let mut text = String::new();
        while let Some(c) = self.advance() {
            match c {
                '"' => return Ok(TokenKind::Text(text)),
                '\\' => match self.advance() {
                    Some('n') => text.push('\n'),
                    Some('t') => text.push('\t'),
                    Some(c) => text.push(c),
                    None => break,
                },
                c => text.push(c),
            }
        }
        Err(self.syntax(start, "The quoted string has no closing quote."))
    }

    fn syntax(&self, start: usize, message: &str) -> Error {
        Error::new(ErrorKind::Syntax, message)
            .at(self.span(start))
            .hint("Check the quoted text, comment delimiters and LilyPond punctuation.")
    }
}
