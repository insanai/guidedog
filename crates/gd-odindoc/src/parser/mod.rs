//! A declaration-level port of Odin's parser (core:odin/parser): it reads file-scope
//! statements and every expression and type in them as Odin's parser does, so nodes
//! start and end where Odin's do and doc comments attach to the same declarations, but
//! it skips procedure bodies, which declare nothing to document.
//!
//! Odin's parser has no nesting limit; this one counts how deeply it recurses and stops
//! at `MAX_NESTING`, marking the file `too_deep`.

mod expr;
mod operand;
mod stmt;
mod types;

use crate::ast::{CommentGroup, Expr, File, GroupId, Node, NodeId, Span, line_starts};
use crate::tokenizer::{Tk, Token, Tokenizer};

/// How deeply the parser recurses (nested brackets, type prefixes, casts, ternaries,
/// `when` blocks) before it gives a file up as too deeply nested.
pub(crate) const MAX_NESTING: u32 = 160;

/// The parser's state. `Tokenizer` is `Copy`, so peeking copies the scanning state.
pub(crate) struct Parser<'a> {
    src: &'a str,
    tok: Tokenizer<'a>,
    prev: Token,
    curr: Token,
    /// Odin's `expr_level`: >= 0 in an expression, < 0 in a control clause or type.
    expr_level: i32,
    allow_range: bool,
    allow_in_expr: bool,
    allow_type: bool,
    lead_comment: Option<GroupId>,
    line_comment: Option<GroupId>,
    peeking: bool,
    depth: u32,
    stopped: bool,
    file: File,
}

/// Parses `src`, an Odin file: `None` when it has no package clause, or its package
/// clause has a syntax error, as Odin's parser gives up on such a file. The file says
/// how many syntax errors were found and whether it nests too deeply to parse.
pub(crate) fn parse_file(src: &str) -> File {
    let mut p = Parser::new(src);
    p.parse_file_scope();
    p.file
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        let file = File { line_starts: line_starts(src), ..File::default() };
        Parser {
            src,
            tok: Tokenizer::new(src),
            prev: Token::default(),
            curr: Token::default(),
            expr_level: 0,
            allow_range: false,
            allow_in_expr: false,
            allow_type: false,
            lead_comment: None,
            line_comment: None,
            peeking: false,
            depth: 0,
            stopped: false,
            file,
        }
    }

    /// Reads the package clause and the file's statements (Odin's `parse_file`).
    fn parse_file_scope(&mut self) {
        if self.src.is_empty() {
            return;
        }
        self.advance();
        self.consume_comment_groups(self.prev);
        let docs = self.lead_comment;
        let mut invalid = false;
        while self.curr.kind != Tk::Package && self.curr.kind != Tk::Eof {
            match self.curr.kind {
                Tk::Comment => self.consume_comment_groups(self.prev),
                Tk::FileTag => {
                    self.file.tags.push(self.span_of(self.curr));
                    self.advance();
                }
                _ => {
                    invalid = true;
                    self.advance();
                }
            }
        }
        if self.curr.kind != Tk::Package || invalid {
            self.error();
            return;
        }
        self.expect(Tk::Package);
        let name = self.expect(Tk::Ident);
        if matches!(self.text(self.span_of(name)), "_" | "builtin" | "intrinsics") {
            // A blank or reserved package name.
            self.error();
        }
        self.expect_semicolon(true);
        if self.file.errors > 0 {
            return;
        }
        self.file.package = Some(self.span_of(name));
        self.file.package_docs = docs;
        while self.curr.kind != Tk::Eof {
            let stmt = self.parse_stmt();
            self.file.decls.push(stmt);
        }
    }

    // --- Tokens and comments -----------------------------------------------------

    fn span_of(&self, tok: Token) -> Span {
        Span { start: tok.start, end: tok.end }
    }

    fn text(&self, span: Span) -> &'a str {
        let end = (span.end as usize).min(self.src.len());
        self.src.get(span.start as usize..end).unwrap_or("")
    }

    fn error(&mut self) {
        self.file.errors += 1;
    }

    /// Scans the next token into `curr`; false at the end of the file. A parser that
    /// stopped (too deeply nested) sees only the end of the file.
    fn next_token0(&mut self) -> bool {
        if self.stopped {
            self.curr = Token { kind: Tk::Eof, ..self.curr };
            return false;
        }
        self.curr = self.tok.scan();
        self.curr.kind != Tk::Eof
    }

    /// Moves to the next token and returns the one left; comments on the way are
    /// grouped, and newlines inside brackets skipped (Odin's `advance_token`).
    fn advance(&mut self) -> Token {
        let left = self.curr;
        loop {
            self.lead_comment = None;
            self.line_comment = None;
            self.prev = self.curr;
            let prev = self.prev;
            if !self.next_token0() {
                break;
            }
            if self.curr.kind == Tk::Comment {
                self.consume_comment_groups(prev);
            }
            let skip = self.curr.is_newline() && self.expr_level > 0;
            if !skip {
                break;
            }
        }
        left
    }

    /// Reads one comment, returning the line it ends on.
    fn consume_comment(&mut self) -> (Span, u32) {
        let tok = self.curr;
        let text = self.text(self.span_of(tok));
        let mut end_line = tok.line;
        if text.as_bytes().get(1) == Some(&b'*') {
            end_line += text.bytes().filter(|&b| b == b'\n').count() as u32;
        }
        self.next_token0();
        (self.span_of(tok), end_line)
    }

    /// Reads comments that each start within `n` lines of the last one's end.
    fn consume_comment_group(&mut self, n: u32) -> (Option<GroupId>, u32) {
        let mut list = Vec::new();
        let mut end_line = self.curr.line;
        let first_line = self.curr.line;
        while self.curr.kind == Tk::Comment && self.curr.line <= end_line + n {
            let (span, end) = self.consume_comment();
            end_line = end;
            list.push(span);
        }
        if list.is_empty() || self.peeking {
            return (None, end_line);
        }
        let start = list[0].start;
        let id = GroupId(self.file.groups.len() as u32);
        self.file.groups.push(CommentGroup { list, line: first_line, end_line, start });
        (Some(id), end_line)
    }

    /// Groups the comments before the current token: one on the previous token's line
    /// is its line comment, and the last group, when it ends on the line before the
    /// current token (or on its line), is the current token's lead comment.
    fn consume_comment_groups(&mut self, prev: Token) {
        if self.curr.kind != Tk::Comment {
            return;
        }
        if self.curr.line == prev.line {
            self.line_comment = self.consume_comment_group(0).0;
        }
        let mut comment = None;
        let mut end_line = 0;
        while self.curr.kind == Tk::Comment {
            (comment, end_line) = self.consume_comment_group(1);
        }
        if end_line + 1 >= self.curr.line {
            self.lead_comment = comment;
        }
    }

    /// The token `lookahead + 1` tokens ahead, without moving.
    fn peek(&mut self, lookahead: usize) -> Token {
        let saved = (self.tok, self.prev, self.curr, self.lead_comment, self.line_comment);
        let was_peeking = self.peeking;
        self.peeking = true;
        for _ in 0..=lookahead {
            self.advance();
        }
        let tok = self.curr;
        (self.tok, self.prev, self.curr, self.lead_comment, self.line_comment) = saved;
        self.peeking = was_peeking;
        tok
    }

    fn expect(&mut self, kind: Tk) -> Token {
        let tok = self.curr;
        if tok.kind != kind {
            self.error();
        }
        self.advance();
        tok
    }

    fn allow(&mut self, kind: Tk) -> bool {
        if self.curr.kind == kind {
            self.advance();
            return true;
        }
        false
    }

    fn skip_possible_newline(&mut self) {
        if self.curr.is_newline() {
            self.advance();
        }
    }

    /// Skips a newline when the next line starts with `{`, `else`, or `where`, which
    /// continue what came before it.
    fn skip_possible_newline_for_literal(&mut self) {
        if !self.curr.is_newline() {
            return;
        }
        let line = self.curr.line;
        let next = self.peek(0);
        let continues = matches!(next.kind, Tk::OpenBrace | Tk::Else | Tk::Where);
        if line + 1 >= next.line && continues {
            self.advance();
        }
    }

    /// Ends a statement (Odin's `expect_semicolon` with optional semicolons): a
    /// statement with a node ends anywhere; one without needs a semicolon or the end.
    fn expect_semicolon(&mut self, has_node: bool) -> bool {
        if self.allow(Tk::Semicolon) || self.prev.kind == Tk::Semicolon {
            return true;
        }
        if self.curr.kind == Tk::Eof || has_node {
            return true;
        }
        self.error();
        self.fix_advance_to_next_stmt();
        false
    }

    fn fix_advance_to_next_stmt(&mut self) {
        while !matches!(self.curr.kind, Tk::Eof | Tk::Semicolon) {
            self.advance();
        }
    }

    fn expect_closing(&mut self, closing: Tk) -> Token {
        let tok = self.curr;
        if self.allow(closing) {
            return tok;
        }
        if self.allow(Tk::Semicolon) && !tok.is_newline() {
            self.error();
        }
        let close = self.expect(closing);
        if close.kind != closing {
            while !matches!(self.curr.kind, Tk::Eof) && self.curr.kind != closing {
                if self.curr.kind == Tk::Semicolon && !self.curr.nl {
                    break;
                }
                self.advance();
            }
            return self.curr;
        }
        close
    }

    fn expect_closing_parens(&mut self) -> Token {
        let tok = self.curr;
        if self.allow(Tk::CloseParen) {
            return tok;
        }
        if self.allow(Tk::Semicolon) && !tok.is_newline() {
            self.error();
        }
        while !matches!(self.curr.kind, Tk::CloseParen | Tk::Eof) {
            if self.curr.kind == Tk::Semicolon && !self.curr.nl {
                break;
            }
            self.advance();
        }
        self.expect(Tk::CloseParen)
    }

    // --- Nodes and nesting -------------------------------------------------------

    fn push(&mut self, kind: Expr, start: u32, end: u32) -> NodeId {
        let id = NodeId(self.file.nodes.len() as u32);
        self.file.nodes.push(Node { kind, span: Span { start, end } });
        id
    }

    fn start_of(&self, id: NodeId) -> u32 {
        self.file.node(id).span.start
    }

    fn end_of(&self, id: NodeId) -> u32 {
        self.file.node(id).span.end
    }

    /// Enters one level of recursion; false (and the parse stops) past `MAX_NESTING`.
    fn enter(&mut self) -> bool {
        self.depth += 1;
        if self.depth > MAX_NESTING && !self.stopped {
            self.stopped = true;
            self.file.too_deep = true;
            self.curr = Token { kind: Tk::Eof, ..self.curr };
        }
        !self.stopped
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    /// Skips a procedure body from its `{` to the matching `}`; nothing in it is
    /// documented. Odin parses it with `expr_level` 0, which the closing brace keeps.
    fn skip_body(&mut self) {
        let level = self.expr_level;
        self.expr_level = 0;
        self.expect(Tk::OpenBrace);
        let mut depth = 0usize;
        while self.curr.kind != Tk::Eof && !(depth == 0 && self.curr.kind == Tk::CloseBrace) {
            match self.curr.kind {
                Tk::OpenBrace => depth += 1,
                Tk::CloseBrace => depth -= 1,
                _ => {}
            }
            self.advance();
        }
        self.expect(Tk::CloseBrace);
        self.expr_level = level;
    }

    /// Skips one statement that declares nothing: to the end of its line outside
    /// brackets, with any blocks and `else` branches it has.
    fn skip_stmt(&mut self) {
        let mut depth = 0usize;
        loop {
            let kind = self.curr.kind;
            // A statement ends at a newline, unless an `else` branch follows it.
            if kind == Tk::Semicolon && depth == 0 && self.peek(0).kind != Tk::Else {
                self.advance();
                return;
            }
            match kind {
                Tk::Eof => return,
                Tk::OpenBrace | Tk::OpenParen | Tk::OpenBracket => depth += 1,
                Tk::CloseBrace | Tk::CloseParen | Tk::CloseBracket if depth == 0 => return,
                Tk::CloseBrace | Tk::CloseParen | Tk::CloseBracket => depth -= 1,
                _ => {}
            }
            self.advance();
        }
    }
}

#[cfg(test)]
mod tests;
