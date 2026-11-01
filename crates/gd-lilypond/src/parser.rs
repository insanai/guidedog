use crate::limits::{Work, limit, push};
use crate::{
    Error, ErrorKind, Expr, ExprKind, Failed, FileId, IncludeRequest, Limits, Options, SourceFile,
    SourceFiles, Span, Syntax, SyntaxId, Token, TokenKind,
};

/// Parses borrowed notation into a bounded syntax arena without interpreting music.
/// Includes use the explicit callback; failures retain every source read so far.
pub fn parse_lilypond(
    source: &str,
    name: &str,
    options: &mut Options<'_>,
) -> Result<Syntax, Failed> {
    let mut files = SourceFiles::default();
    let mut work = Work::new(&options.limits);
    let result = parse(source, name, options, &mut files, &mut work);
    match result {
        Ok((nodes, roots)) => Ok(Syntax { nodes, roots, files }),
        Err(error) => Err(Failed { error, files }),
    }
}

fn parse(
    source: &str,
    name: &str,
    options: &mut Options<'_>,
    files: &mut SourceFiles,
    work: &mut Work,
) -> Result<(Vec<Expr>, Vec<SyntaxId>), Error> {
    if source.len() > options.limits.source_bytes {
        return Err(limit("source bytes", Span::default()));
    }
    reserve_source(source.len(), options, Span::default())?;
    push(
        &mut files.files,
        SourceFile { name: name.into(), text: source.into() },
        Span::default(),
    )?;
    let mut bytes = source.len();
    let mut tokens = Vec::new();
    expand_includes(
        FileId(0),
        options,
        files,
        work,
        &mut bytes,
        &mut vec![name.into()],
        &mut tokens,
    )?;
    let mut parser =
        Parser { tokens: &tokens, at: 0, nodes: Vec::new(), limits: &options.limits, work };
    let roots = parser.group(None, 0)?;
    Ok((parser.nodes, roots))
}

fn expand_includes(
    file: FileId,
    options: &mut Options<'_>,
    files: &mut SourceFiles,
    work: &mut Work,
    bytes: &mut usize,
    stack: &mut Vec<String>,
    out: &mut Vec<Token>,
) -> Result<(), Error> {
    let source = &files.files[file.0 as usize].text;
    let tokens = crate::lexer::lex(source, file, &options.limits, work)?;
    let mut iter = tokens.into_iter();
    while let Some(token) = iter.next() {
        if !matches!(&token.kind, TokenKind::Command(c) if c == "include") {
            push(out, token.clone(), token.span)?;
            continue;
        }
        let next =
            iter.next().ok_or_else(|| include_error(token.span, "An include needs a name."))?;
        let TokenKind::Text(name) = next.kind else {
            return Err(include_error(next.span, "The include name must be a quoted string."));
        };
        if let Some(language) =
            name.strip_suffix(".ly").filter(|n| matches!(*n, "english" | "deutsch" | "nederlands"))
        {
            push(out, Token { kind: TokenKind::Command("language".into()), ..token }, next.span)?;
            push(
                out,
                Token { kind: TokenKind::Text(language.into()), span: next.span },
                next.span,
            )?;
            continue;
        }
        if stack.len() > options.limits.depth || files.files.len() > options.limits.files {
            return Err(limit("included files or nesting", token.span));
        }
        let callback = options.include.as_mut().ok_or_else(|| {
            include_error(token.span, "Filesystem includes are disabled for this score.")
        })?;
        let included = callback(IncludeRequest { from: file, name: &name })
            .map_err(|e| include_error(token.span, &e.message).hint(e.hint))?;
        *bytes = bytes
            .checked_add(included.text.len())
            .filter(|b| *b <= options.limits.source_bytes)
            .ok_or_else(|| limit("source bytes after includes", token.span))?;
        if stack.contains(&included.name) {
            return Err(include_error(token.span, "The include chain contains a cycle."));
        }
        reserve_source(included.text.len(), options, token.span)?;
        let id = FileId(files.files.len() as u32);
        push(stack, included.name.clone(), token.span)?;
        push(
            &mut files.files,
            SourceFile { name: included.name, text: included.text },
            token.span,
        )?;
        expand_includes(id, options, files, work, bytes, stack, out)?;
        stack.pop();
    }
    Ok(())
}

fn include_error(span: Span, message: &str) -> Error {
    Error::new(ErrorKind::Include, message)
        .at(span)
        .hint("Use a quoted local file below the project's allowed include roots.")
}

struct Parser<'a, 'w> {
    tokens: &'a [Token],
    at: usize,
    nodes: Vec<Expr>,
    limits: &'a Limits,
    work: &'w mut Work,
}

impl Parser<'_, '_> {
    fn group(
        &mut self,
        open: Option<(TokenKind, Span)>,
        depth: usize,
    ) -> Result<Vec<SyntaxId>, Error> {
        let mut children = Vec::new();
        while let Some(token) = self.tokens.get(self.at) {
            self.work.take(1, token.span)?;
            if depth > self.limits.depth {
                return Err(limit("syntax depth", token.span));
            }
            let after_script = token.kind == TokenKind::Symbol('>')
                && self.at > 0
                && matches!(self.tokens[self.at - 1].kind, TokenKind::Symbol('-' | '^' | '_'))
                && self.tokens[self.at - 1].span.end == token.span.start;
            if !after_script && open.as_ref().map(|(k, _)| k) == Some(&token.kind) {
                self.at += 1;
                return Ok(children);
            }
            self.at += 1;
            let (end, group) = match &token.kind {
                TokenKind::Symbol('{') => (Some(TokenKind::Symbol('}')), 0),
                TokenKind::Symbol('<') => (Some(TokenKind::Symbol('>')), 1),
                TokenKind::SimultaneousStart => (Some(TokenKind::SimultaneousEnd), 2),
                TokenKind::Symbol('}') | TokenKind::SimultaneousEnd => {
                    return Err(syntax_error(token.span, "Unexpected closing delimiter."));
                }
                _ => (None, 0),
            };
            let kind = if let Some(end) = end {
                let nested = self.group(Some((end, token.span)), depth + 1)?;
                match group {
                    0 => ExprKind::Sequence(nested),
                    1 => ExprKind::Chord(nested),
                    _ => ExprKind::Simultaneous(nested),
                }
            } else {
                ExprKind::Atom(token.kind.clone())
            };
            let id = SyntaxId(
                u32::try_from(self.nodes.len()).map_err(|_| limit("syntax nodes", token.span))?,
            );
            push(&mut self.nodes, Expr { kind, span: token.span }, token.span)?;
            push(&mut children, id, token.span)?;
        }
        if let Some((_, open_span)) = open {
            return Err(syntax_error(open_span, "The music group has no closing delimiter."));
        }
        Ok(children)
    }
}

pub(crate) fn syntax_error(span: Span, message: &str) -> Error {
    Error::new(ErrorKind::Syntax, message)
        .at(span)
        .hint("Check the LilyPond profile and balance braces, chords and simultaneous groups.")
}

fn reserve_source(bytes: usize, options: &Options<'_>, span: Span) -> Result<(), Error> {
    let bytes = bytes.checked_mul(256).ok_or_else(|| limit("syntax memory", span))?;
    if options.reserve.is_some_and(|reserve| !reserve(bytes)) {
        return Err(limit("memory ledger", span));
    }
    Ok(())
}
