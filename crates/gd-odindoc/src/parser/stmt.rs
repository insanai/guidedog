//! File-scope statements: declarations, imports, foreign blocks and imports, `when`
//! blocks, and attributes (Odin's `parse_stmt` and the procedures it calls).

use super::Parser;
use crate::ast::{
    Else, Expr, ForeignBlock, ForeignImport, GroupId, ImportDecl, NodeId, Span, Stmt, ValueDecl,
    When,
};
use crate::tokenizer::Tk;

impl Parser<'_> {
    /// Reads one statement at file scope (or in a `when` or foreign block).
    pub(super) fn parse_stmt(&mut self) -> Stmt {
        if !self.enter() {
            self.leave();
            return Stmt::Other;
        }
        let stmt = self.parse_stmt_inner();
        self.leave();
        stmt
    }

    fn parse_stmt_inner(&mut self) -> Stmt {
        use Tk::*;
        match self.curr.kind {
            Context | Proc | Ident | Integer | Float | Imag | Rune | String | OpenParen
            | Pointer | Asm | Add | Sub | Xor | Not | And | Increment | Decrement => {
                let stmt = self.parse_simple_stmt(true);
                self.expect_semicolon(true);
                stmt
            }
            Foreign => self.parse_foreign_decl(),
            Import => Stmt::Import(self.parse_import_decl()),
            When => match self.parse_when_stmt() {
                Some(w) => Stmt::When(w),
                None => Stmt::Other,
            },
            Using => self.parse_using(),
            At => {
                let docs = self.lead_comment;
                let tok = self.advance();
                self.parse_attribute(tok.start, docs)
            }
            Hash => self.parse_hash_stmt(),
            Semicolon => {
                self.advance();
                Stmt::Other
            }
            OpenBrace => {
                // A block at file scope is an error; nothing in it is documented.
                self.error();
                self.skip_possible_newline_for_literal();
                self.skip_body();
                Stmt::Other
            }
            If | For | Switch | Defer | Return | Break | Continue | Fallthrough | Else => {
                // Statements that only procedures hold: errors at file scope.
                self.error();
                self.advance();
                self.skip_stmt();
                Stmt::Other
            }
            _ => {
                self.advance();
                self.error();
                self.fix_advance_to_next_stmt();
                Stmt::Other
            }
        }
    }

    fn parse_using(&mut self) -> Stmt {
        let docs = self.lead_comment;
        self.expect(Tk::Using);
        if self.curr.kind == Tk::Import {
            return Stmt::Import(self.parse_import_decl());
        }
        let list = self.parse_expr_list(true);
        if self.curr.kind != Tk::Colon {
            self.expect_semicolon(!list.is_empty());
            return Stmt::Other;
        }
        self.expect(Tk::Colon);
        self.parse_value_decl(list, docs)
    }

    /// `#assert(...)`, `#force_inline f()`, and other directives at statement start.
    fn parse_hash_stmt(&mut self) -> Stmt {
        let start = self.expect(Tk::Hash).start;
        let tag = self.expect(Tk::Ident);
        match self.text(self.span_of(tag)) {
            "bounds_check" | "no_bounds_check" | "type_assert" | "no_type_assert" | "partial"
            | "reverse" => self.parse_stmt(),
            "assert" | "panic" => {
                let bd = self.push(Expr::BasicDirective, start, tag.end);
                self.parse_call_expr(bd);
                Stmt::Other
            }
            "force_inline" | "force_no_inline" | "must_tail" => {
                self.parse_unary_expr(true);
                Stmt::Other
            }
            "unroll" => {
                self.skip_stmt();
                Stmt::Other
            }
            "include" => {
                self.error();
                Stmt::Other
            }
            _ => {
                self.parse_stmt();
                self.fix_advance_to_next_stmt();
                Stmt::Other
            }
        }
    }

    /// An expression, assignment, or declaration (Odin's `parse_simple_stmt`).
    fn parse_simple_stmt(&mut self, allow_label: bool) -> Stmt {
        let docs = self.lead_comment;
        let lhs = self.parse_expr_list(true);
        let op = self.curr;
        if op.kind.is_assignment() {
            self.advance();
            if self.parse_expr_list(false).is_empty() {
                self.error();
            }
            return Stmt::Other;
        }
        if op.kind == Tk::Colon {
            self.expect(Tk::Colon);
            let labelled = matches!(self.curr.kind, Tk::OpenBrace | Tk::If | Tk::For | Tk::Switch);
            if allow_label && lhs.len() == 1 && labelled {
                return self.parse_stmt();
            }
            return self.parse_value_decl(lhs, docs);
        }
        if lhs.len() > 1 {
            self.error();
            return Stmt::Other;
        }
        if matches!(op.kind, Tk::Increment | Tk::Decrement) {
            self.advance();
            self.error();
        }
        Stmt::Other
    }

    /// The rest of `names :` ... (Odin's `parse_value_decl`).
    fn parse_value_decl(&mut self, names: Vec<NodeId>, docs: Option<GroupId>) -> Stmt {
        let mut mutable = true;
        let mut values = Vec::new();
        let ty = self.parse_type_or_ident();
        if matches!(self.curr.kind, Tk::Eq | Tk::Colon) {
            let sep = self.advance();
            mutable = sep.kind != Tk::Colon;
            values = self.parse_expr_list(false);
            let missing = values.len() < names.len() && !mutable;
            if values.len() > names.len() || missing || values.is_empty() {
                self.error();
            }
        }
        if ty.is_none() && values.is_empty() {
            self.error();
            return Stmt::Other;
        }
        if !values.is_empty() && values.len() != names.len() {
            self.error();
        }
        let comment = self.line_comment;
        if self.expr_level >= 0 {
            let has_end = !mutable && !values.is_empty();
            let closes_line = self.curr.kind == Tk::CloseBrace && self.curr.line == self.prev.line;
            if !closes_line {
                self.expect_semicolon(has_end);
            }
        }
        Stmt::Value(ValueDecl {
            names,
            ty,
            values,
            mutable,
            attributes: Vec::new(),
            docs,
            comment,
        })
    }

    fn parse_import_decl(&mut self) -> ImportDecl {
        self.expect(Tk::Import);
        let mut name = None;
        if self.curr.kind == Tk::Ident {
            let tok = self.advance();
            name = Some(self.span_of(tok));
        }
        let path = self.expect(Tk::String);
        self.expect_semicolon(true);
        ImportDecl { name, path: self.span_of(path), attributes: Vec::new() }
    }

    fn parse_foreign_decl(&mut self) -> Stmt {
        let docs = self.lead_comment;
        let tok = self.expect(Tk::Foreign);
        match self.curr.kind {
            Tk::Ident | Tk::OpenBrace => Stmt::ForeignBlock(self.parse_foreign_block(tok.start)),
            Tk::Import => {
                self.expect(Tk::Import);
                let mut name = None;
                if self.curr.kind == Tk::Ident {
                    name = Some(self.parse_ident());
                }
                let paths = self.parse_foreign_paths();
                self.expect_semicolon(true);
                let comment = self.line_comment;
                Stmt::ForeignImport(ForeignImport {
                    name,
                    paths,
                    attributes: Vec::new(),
                    docs,
                    comment,
                    line: tok.line,
                })
            }
            _ => {
                self.error();
                Stmt::Other
            }
        }
    }

    fn parse_foreign_paths(&mut self) -> Vec<NodeId> {
        let mut paths = Vec::new();
        if self.allow(Tk::OpenBrace) {
            while !matches!(self.curr.kind, Tk::CloseBrace | Tk::Eof) {
                paths.push(self.parse_expr(false));
                if !self.allow(Tk::Comma) {
                    break;
                }
            }
            self.expect(Tk::CloseBrace);
        } else {
            let path = self.expect(Tk::String);
            paths.push(self.push(Expr::BasicLit, path.start, path.end));
        }
        if paths.is_empty() {
            self.error();
        }
        paths
    }

    fn parse_foreign_block(&mut self, foreign_start: u32) -> ForeignBlock {
        let lib = if self.curr.kind == Tk::OpenBrace {
            // Odin names a block without a library by the `foreign` keyword's span.
            Span { start: foreign_start, end: foreign_start + "foreign".len() as u32 }
        } else {
            let id = self.parse_ident();
            self.file.node(id).span
        };
        self.skip_possible_newline_for_literal();
        self.expect(Tk::OpenBrace);
        let mut body = Vec::new();
        while !matches!(self.curr.kind, Tk::CloseBrace | Tk::Eof) {
            match self.parse_stmt() {
                stmt @ (Stmt::Value(_) | Stmt::When(_)) => body.push(stmt),
                Stmt::Other => {}
                _ => self.error(),
            }
        }
        self.expect(Tk::CloseBrace);
        ForeignBlock { lib, attributes: Vec::new(), body }
    }

    /// A `when` statement; `None` when parsing stopped inside it.
    fn parse_when_stmt(&mut self) -> Option<When> {
        if !self.enter() {
            self.leave();
            return None;
        }
        let when = self.parse_when_inner();
        self.leave();
        Some(when)
    }

    fn parse_when_inner(&mut self) -> When {
        self.expect(Tk::When);
        let (level, allow_in) = (self.expr_level, self.allow_in_expr);
        self.expr_level = -1;
        self.allow_in_expr = true;
        let cond = self.parse_expr(false);
        (self.expr_level, self.allow_in_expr) = (level, allow_in);
        let body = if self.allow(Tk::Do) {
            self.parse_stmt();
            None
        } else {
            Some(self.parse_block())
        };
        self.skip_possible_newline_for_literal();
        let mut else_stmt = None;
        if self.allow(Tk::Else) {
            let branch = match self.curr.kind {
                Tk::When => self.parse_when_stmt().map_or(Else::Other, Else::When),
                Tk::OpenBrace => Else::Block(self.parse_block()),
                Tk::Do => {
                    self.advance();
                    self.parse_stmt();
                    Else::Other
                }
                _ => {
                    self.error();
                    Else::Other
                }
            };
            else_stmt = Some(Box::new(branch));
        }
        When { cond, body, else_stmt }
    }

    /// A `when` block's statements, read as file-scope statements.
    fn parse_block(&mut self) -> Vec<Stmt> {
        self.skip_possible_newline_for_literal();
        let level = self.expr_level;
        self.expr_level = 0;
        self.expect(Tk::OpenBrace);
        let mut stmts = Vec::new();
        while !matches!(self.curr.kind, Tk::Case | Tk::CloseBrace | Tk::Eof) {
            stmts.push(self.parse_stmt());
        }
        self.expect(Tk::CloseBrace);
        self.expr_level = level;
        stmts
    }

    /// `@(...)` or `@name` before a declaration: the attribute's span joins the
    /// declaration's, and the attribute's doc comment is the declaration's when it has
    /// none of its own.
    fn parse_attribute(&mut self, start: u32, docs: Option<GroupId>) -> Stmt {
        let close = if self.curr.kind == Tk::Ident {
            let close = self.curr;
            self.parse_ident();
            close
        } else {
            self.expect(Tk::OpenParen);
            self.expr_level += 1;
            while !matches!(self.curr.kind, Tk::CloseParen | Tk::Eof) {
                self.parse_ident();
                if self.curr.kind == Tk::Eq {
                    self.expect(Tk::Eq);
                    self.parse_value();
                }
                if !self.allow(Tk::Comma) {
                    break;
                }
            }
            self.expr_level -= 1;
            self.expect(Tk::CloseParen)
        };
        let span = Span { start, end: close.end };
        self.skip_possible_newline();
        let mut stmt = self.parse_stmt();
        let (attributes, own_docs) = match &mut stmt {
            Stmt::Value(d) => (&mut d.attributes, Some(&mut d.docs)),
            Stmt::ForeignImport(d) => (&mut d.attributes, Some(&mut d.docs)),
            Stmt::ForeignBlock(d) => (&mut d.attributes, None),
            Stmt::Import(d) => (&mut d.attributes, None),
            Stmt::When(_) | Stmt::Other => {
                self.error();
                return stmt;
            }
        };
        attributes.push(span);
        if let Some(own) = own_docs
            && own.is_none()
        {
            *own = docs;
        }
        stmt
    }
}
