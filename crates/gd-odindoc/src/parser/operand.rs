//! Operands (Odin's `parse_operand`): names, literals, parentheses, directives,
//! procedures and procedure groups, and the prefix types (`^T`, `[]T`, `map[K]V`, ...).
//! Record types and field lists are in `types.rs`.

use super::Parser;
use crate::ast::{Expr, NodeId};
use crate::tokenizer::{Tk, Token};

impl Parser<'_> {
    /// An operand, or `None` when the current token cannot start one.
    pub(super) fn parse_operand(&mut self, lhs: bool) -> Option<NodeId> {
        use Tk::*;
        let tok = self.curr;
        let node = match tok.kind {
            Ident => self.parse_ident(),
            Undef | Context | Integer | Float | Imag | Rune | String => {
                self.advance();
                let kind = match tok.kind {
                    Undef => Expr::Undef,
                    Context => Expr::Implicit,
                    _ => Expr::BasicLit,
                };
                self.push(kind, tok.start, tok.end)
            }
            OpenBrace if !lhs => self.parse_literal_value(None),
            OpenParen => self.parse_paren(),
            Distinct => {
                self.advance();
                let ty = self.parse_type();
                self.wrap(Expr::Distinct, tok.start, Some(ty))
            }
            Hash => self.parse_hash_operand(lhs),
            Proc => self.parse_proc_operand(),
            Dollar => self.parse_poly_type(),
            Typeid => {
                self.advance();
                self.push(Expr::Typeid, tok.start, tok.end)
            }
            Pointer => {
                self.advance();
                let elem = self.parse_type();
                self.wrap(Expr::Pointer, tok.start, Some(elem))
            }
            OpenBracket => self.parse_array_type(),
            Map => {
                self.advance();
                self.expect(OpenBracket);
                self.parse_type();
                self.expect(CloseBracket);
                let value = self.parse_type();
                self.wrap(Expr::Map, tok.start, Some(value))
            }
            Struct => self.parse_struct_type(),
            Union => self.parse_union_type(),
            Enum => self.parse_enum_type(),
            BitSet => self.parse_bit_set_type(),
            Matrix => self.parse_matrix_type(),
            BitField => self.parse_bit_field_type(),
            Asm => self.parse_asm(),
            _ => return None,
        };
        Some(node)
    }

    fn parse_paren(&mut self) -> NodeId {
        let open = self.expect(Tk::OpenParen);
        let level = self.expr_level;
        self.expr_level = level.max(0) + 1;
        let inner = self.parse_expr(false);
        self.expr_level = level;
        let close = self.expect(Tk::CloseParen);
        self.push(Expr::Paren(inner), open.start, close.end)
    }

    /// `#type T`, `#config(...)`, `#soa[]T`, `#force_inline proc ...`, and other
    /// directives in an expression.
    fn parse_hash_operand(&mut self, lhs: bool) -> NodeId {
        let hash = self.expect(Tk::Hash);
        let name = self.expect(Tk::Ident);
        let start = hash.start;
        match self.text(self.span_of(name)) {
            "type" => {
                let ty = self.parse_type();
                self.wrap(Expr::Helper, start, Some(ty))
            }
            "file" | "directory" | "line" | "procedure" | "caller_location" => {
                self.push(Expr::BasicDirective, start, name.end)
            }
            "caller_expression" => {
                let bd = self.push(Expr::BasicDirective, start, name.end);
                if self.peek_kind_is(Tk::OpenParen) { self.parse_call_expr(bd) } else { bd }
            }
            "location" | "exists" | "load" | "load_directory" | "load_hash" | "hash"
            | "assert" | "panic" | "defined" | "config" => {
                let bd = self.push(Expr::BasicDirective, start, name.end);
                self.parse_call_expr(bd)
            }
            "soa" | "simd" | "sparse" => self.parse_type(),
            "partial" | "bounds_check" | "no_bounds_check" => self.parse_expr(lhs),
            "relative" => {
                let tag = self.push(Expr::BasicDirective, start, name.end);
                self.parse_call_expr(tag);
                let ty = self.parse_type();
                self.wrap(Expr::Relative, start, Some(ty))
            }
            "force_inline" | "force_no_inline" => match self.parse_unary_expr(lhs) {
                Some(expr) => expr,
                None => {
                    self.error();
                    self.push(Expr::Bad, name.start, name.end)
                }
            },
            _ => {
                let expr = self.parse_expr(lhs);
                let end = self.start_of(expr);
                self.push(Expr::Tag, start, end)
            }
        }
    }

    /// Whether the token after the current one is of `kind`.
    fn peek_kind_is(&mut self, kind: Tk) -> bool {
        self.peek(0).kind == kind
    }

    fn parse_proc_operand(&mut self) -> NodeId {
        let tok = self.expect(Tk::Proc);
        if self.curr.kind == Tk::OpenBrace {
            return self.parse_proc_group(tok);
        }
        let ty = self.parse_proc_type(tok);
        self.skip_possible_newline_for_literal();
        let mut has_where = false;
        if self.curr.kind == Tk::Where {
            self.expect(Tk::Where);
            has_where = true;
            let level = self.expr_level;
            self.expr_level = -1;
            self.parse_expr_list(false);
            self.expr_level = level;
        }
        self.parse_proc_tags();
        if self.allow_type && self.expr_level < 0 {
            if has_where {
                self.error();
            }
            return ty;
        }
        self.skip_possible_newline_for_literal();
        let body = if self.allow(Tk::Undef) {
            None
        } else if self.curr.kind == Tk::OpenBrace {
            let start = self.curr.start;
            self.skip_body();
            Some(start)
        } else if self.allow(Tk::Do) {
            let start = self.curr.start;
            self.skip_stmt();
            Some(start)
        } else {
            return ty;
        };
        self.push(Expr::ProcLit { body }, tok.start, self.prev.end)
    }

    fn parse_proc_group(&mut self, tok: Token) -> NodeId {
        self.expect(Tk::OpenBrace);
        let mut count = 0;
        while !matches!(self.curr.kind, Tk::CloseBrace | Tk::Eof) {
            self.parse_expr(false);
            if self.curr.kind == Tk::Where {
                self.expect(Tk::Where);
                self.parse_expr(false);
            }
            count += 1;
            if !self.allow(Tk::Comma) {
                break;
            }
        }
        let close = self.expect_closing(Tk::CloseBrace);
        if count == 0 {
            self.error();
        }
        self.push(Expr::ProcGroup, tok.start, close.end)
    }

    fn parse_proc_tags(&mut self) {
        while self.curr.kind == Tk::Hash {
            self.expect(Tk::Hash);
            self.expect(Tk::Ident);
        }
    }

    fn parse_poly_type(&mut self) -> NodeId {
        let dollar = self.advance();
        let name = self.parse_ident();
        let mut end = self.end_of(name);
        if self.allow(Tk::Quo) {
            let spec = self.parse_type();
            end = self.end_of(spec);
        }
        if self.text(self.file.node(name).span) == "_" {
            self.error();
        }
        self.push(Expr::Poly, dollar.start, end)
    }

    /// `[N]T`, `[?]T`, `[]T`, `[^]T`, `[dynamic]T`, `[dynamic; N]T`.
    fn parse_array_type(&mut self) -> NodeId {
        let open = self.expect(Tk::OpenBracket);
        let kind = match self.curr.kind {
            Tk::Pointer => {
                self.advance();
                Expr::MultiPointer
            }
            Tk::Dynamic => {
                self.advance();
                if self.allow(Tk::Semicolon) {
                    self.parse_expr(false);
                    Expr::FixedCapacityDynamicArray
                } else {
                    Expr::DynamicArray
                }
            }
            Tk::Question => {
                self.advance();
                Expr::Array
            }
            Tk::CloseBracket => Expr::Array,
            _ => {
                self.expr_level += 1;
                self.parse_expr(false);
                self.expr_level -= 1;
                Expr::Array
            }
        };
        self.expect(Tk::CloseBracket);
        let elem = self.parse_type();
        self.wrap(kind, open.start, Some(elem))
    }

    fn parse_bit_set_type(&mut self) -> NodeId {
        let tok = self.expect(Tk::BitSet);
        self.expect(Tk::OpenBracket);
        let allow_range = self.allow_range;
        self.allow_range = true;
        self.parse_expr(false);
        self.allow_range = allow_range;
        if self.allow(Tk::Semicolon) {
            self.parse_type();
        }
        let close = self.expect(Tk::CloseBracket);
        self.push(Expr::BitSet, tok.start, close.end)
    }

    fn parse_matrix_type(&mut self) -> NodeId {
        let tok = self.expect(Tk::Matrix);
        self.expect(Tk::OpenBracket);
        self.parse_expr(false);
        self.expect(Tk::Comma);
        self.parse_expr(false);
        self.expect(Tk::CloseBracket);
        let elem = self.parse_type();
        self.wrap(Expr::Matrix, tok.start, Some(elem))
    }

    /// Inline assembly: `asm { ... }` groups and `asm(...) {...}` templates. Only
    /// procedure bodies hold them, so they are read as balanced brackets.
    fn parse_asm(&mut self) -> NodeId {
        let tok = self.expect(Tk::Asm);
        while matches!(self.curr.kind, Tk::OpenParen | Tk::OpenBrace | Tk::String) {
            if self.curr.kind == Tk::String {
                self.advance();
                continue;
            }
            self.skip_brackets();
        }
        self.push(Expr::Asm, tok.start, self.prev.end)
    }

    /// Skips a bracketed run of tokens from its opening bracket to the matching one.
    fn skip_brackets(&mut self) {
        let mut depth = 0usize;
        loop {
            match self.curr.kind {
                Tk::Eof => return,
                Tk::OpenBrace | Tk::OpenParen | Tk::OpenBracket => depth += 1,
                Tk::CloseBrace | Tk::CloseParen | Tk::CloseBracket => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        self.advance();
                        return;
                    }
                }
                _ => {}
            }
            self.advance();
        }
    }
}
