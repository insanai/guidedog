//! Expressions: binary and unary operators, operands with their suffixes (calls,
//! selectors, indexes, compound literals), and lists of them.

use super::Parser;
use crate::ast::{Expr, NodeId};
use crate::tokenizer::Tk;

impl Parser<'_> {
    pub(super) fn parse_expr(&mut self, lhs: bool) -> NodeId {
        self.parse_binary_expr(lhs, 1)
    }

    /// An expression whose `..` ranges are allowed: a value in a list or an attribute.
    pub(super) fn parse_value(&mut self) -> NodeId {
        let allow_range = self.allow_range;
        self.allow_range = true;
        let value = self.parse_expr(false);
        self.allow_range = allow_range;
        value
    }

    pub(super) fn parse_expr_list(&mut self, lhs: bool) -> Vec<NodeId> {
        let mut list = Vec::new();
        loop {
            list.push(self.parse_expr(lhs));
            if self.curr.kind != Tk::Comma {
                break;
            }
            self.advance();
        }
        list
    }

    pub(super) fn parse_ident(&mut self) -> NodeId {
        let tok = self.curr;
        if tok.kind == Tk::Ident {
            self.advance();
            return self.push(Expr::Ident { blank: false }, tok.start, tok.end);
        }
        self.expect(Tk::Ident);
        self.push(Expr::Ident { blank: true }, tok.start, tok.end)
    }

    fn precedence(&self, kind: Tk) -> u32 {
        use Tk::*;
        match kind {
            Question | If | When | OrElse => 1,
            Ellipsis | RangeHalf | RangeFull if self.allow_range => 2,
            CmpOr => 3,
            CmpAnd => 4,
            CmpEq | NotEq | Lt | Gt | LtEq | GtEq => 5,
            In | NotIn if self.expr_level >= 0 || self.allow_in_expr => 6,
            Add | Sub | Or | Xor => 6,
            Mul | Quo | Mod | ModMod | And | AndNot | Shl | Shr => 7,
            _ => 0,
        }
    }

    /// Operators of precedence `prec_in` and above, left to right; ternaries and
    /// `or_else` recurse for their operands.
    pub(super) fn parse_binary_expr(&mut self, lhs: bool, prec_in: u32) -> NodeId {
        let start = self.curr.start;
        if !self.enter() {
            self.leave();
            return self.push(Expr::Bad, start, start);
        }
        let mut expr = match self.parse_unary_expr(lhs) {
            Some(e) => e,
            None => self.push(Expr::Bad, start, self.prev.end),
        };
        let mut prec = self.precedence(self.curr.kind);
        while prec >= prec_in {
            loop {
                let op = self.curr;
                if self.precedence(op.kind) != prec {
                    break;
                }
                let on_next_line = self.prev.line < op.line;
                if matches!(op.kind, Tk::If | Tk::When) && on_next_line && self.expr_level <= 0 {
                    break;
                }
                self.expect_operator();
                expr = self.parse_binary_rest(expr, op.kind, lhs, prec);
            }
            prec -= 1;
        }
        self.leave();
        expr
    }

    fn expect_operator(&mut self) {
        let kind = self.curr.kind;
        if !matches!(kind, Tk::If | Tk::When | Tk::OrElse) && !kind.is_operator() {
            self.error();
        }
        self.advance();
    }

    fn parse_binary_rest(&mut self, left: NodeId, op: Tk, lhs: bool, prec: u32) -> NodeId {
        let start = self.start_of(left);
        let kind = match op {
            Tk::Question => {
                self.parse_expr(lhs);
                self.expect(Tk::Colon);
                self.parse_expr(lhs);
                Expr::Ternary
            }
            Tk::If | Tk::When => {
                self.parse_expr(lhs);
                if op == Tk::When {
                    self.skip_possible_newline();
                }
                self.expect(Tk::Else);
                self.parse_expr(lhs);
                Expr::Ternary
            }
            Tk::OrElse => {
                self.parse_expr(lhs);
                Expr::OrElse
            }
            _ => {
                let right = self.parse_binary_expr(false, prec + 1);
                Expr::Binary { op, left, right }
            }
        };
        self.push(kind, start, self.prev.end)
    }

    /// A unary operator, cast, or implicit selector, else an operand with its suffixes.
    pub(super) fn parse_unary_expr(&mut self, lhs: bool) -> Option<NodeId> {
        if !self.enter() {
            self.leave();
            return None;
        }
        let expr = self.parse_unary_inner(lhs);
        self.leave();
        expr
    }

    fn parse_unary_inner(&mut self, lhs: bool) -> Option<NodeId> {
        use Tk::*;
        let tok = self.curr;
        match tok.kind {
            Transmute | Cast => {
                self.advance();
                self.expect(OpenParen);
                self.parse_type();
                self.expect(CloseParen);
                let expr = self.parse_unary_expr(lhs);
                Some(self.wrap(Expr::TypeCast, tok.start, expr))
            }
            AutoCast => {
                self.advance();
                let expr = self.parse_unary_expr(lhs);
                Some(self.wrap(Expr::AutoCast, tok.start, expr))
            }
            Add | Sub | Not | Xor | And | MulMul | Increment | Decrement => {
                self.advance();
                if matches!(tok.kind, Increment | Decrement) {
                    self.error();
                }
                let expr = self.parse_unary_expr(lhs);
                Some(self.wrap(Expr::Unary { op: tok.kind, expr }, tok.start, expr))
            }
            Period => {
                self.advance();
                let field = self.parse_ident();
                let end = self.end_of(field);
                Some(self.push(Expr::ImplicitSelector { field }, tok.start, end))
            }
            _ => {
                let operand = self.parse_operand(lhs);
                self.parse_atom_expr(operand, lhs)
            }
        }
    }

    /// A node from `start` to the end of `inner` (or to `start`, when there is none).
    pub(super) fn wrap(&mut self, kind: Expr, start: u32, inner: Option<NodeId>) -> NodeId {
        let end = inner.map_or(start, |e| self.end_of(e));
        self.push(kind, start, end)
    }

    pub(super) fn parse_type_or_ident(&mut self) -> Option<NodeId> {
        if !self.enter() {
            self.leave();
            return None;
        }
        let (allow_type, level) = (self.allow_type, self.expr_level);
        self.allow_type = true;
        self.expr_level = -1;
        let operand = self.parse_operand(true);
        let ty = self.parse_atom_expr(operand, true);
        (self.allow_type, self.expr_level) = (allow_type, level);
        self.leave();
        ty
    }

    pub(super) fn parse_type(&mut self) -> NodeId {
        match self.parse_type_or_ident() {
            Some(ty) => ty,
            None => {
                self.error();
                let tok = self.curr;
                self.push(Expr::Bad, tok.start, tok.end)
            }
        }
    }

    /// An operand's suffixes: calls, indexes and slices, selectors, type assertions,
    /// dereferences, `or_return`, and compound literals.
    pub(super) fn parse_atom_expr(&mut self, value: Option<NodeId>, lhs: bool) -> Option<NodeId> {
        let mut operand = match value {
            Some(v) => v,
            None if self.allow_type => return None,
            None => {
                self.error();
                self.fix_advance_to_next_stmt();
                let tok = self.curr;
                self.push(Expr::Bad, tok.start, tok.end)
            }
        };
        let mut is_lhs = lhs;
        loop {
            let next = match self.curr.kind {
                Tk::OpenParen => self.parse_call_expr(operand),
                Tk::OpenBracket => self.parse_index(operand),
                Tk::Period => self.parse_selector(operand),
                Tk::ArrowRight => self.parse_arrow(operand),
                Tk::Pointer => self.suffix(Expr::Deref, operand),
                Tk::OrReturn => self.suffix(Expr::OrReturn, operand),
                Tk::OrBreak | Tk::OrContinue => {
                    self.advance();
                    if self.curr.kind == Tk::Ident {
                        self.parse_ident();
                    }
                    let start = self.start_of(operand);
                    self.push(Expr::OrBranch, start, self.prev.end)
                }
                Tk::OpenBrace
                    if !is_lhs && self.is_literal_type(operand) && self.expr_level >= 0 =>
                {
                    self.parse_literal_value(Some(operand))
                }
                Tk::Increment | Tk::Decrement if !lhs => {
                    self.advance();
                    self.error();
                    operand
                }
                _ => break,
            };
            operand = next;
            is_lhs = false;
        }
        Some(operand)
    }

    /// A one-token suffix (`^`, `or_return`) after `operand`.
    fn suffix(&mut self, kind: Expr, operand: NodeId) -> NodeId {
        let tok = self.advance();
        let start = self.start_of(operand);
        self.push(kind, start, tok.end)
    }

    fn is_literal_type(&self, id: NodeId) -> bool {
        let mut node = self.file.node(id);
        while let Expr::Paren(inner) = node.kind {
            node = self.file.node(inner);
        }
        matches!(
            node.kind,
            Expr::Bad
                | Expr::Ident { .. }
                | Expr::Selector { .. }
                | Expr::Array
                | Expr::Struct(_)
                | Expr::Union
                | Expr::Enum(_)
                | Expr::DynamicArray
                | Expr::FixedCapacityDynamicArray
                | Expr::Map
                | Expr::BitSet
                | Expr::Matrix
                | Expr::Call { .. }
                | Expr::BitField(_)
        )
    }

    fn parse_index(&mut self, operand: NodeId) -> NodeId {
        let allow_range = self.allow_range;
        self.allow_range = false;
        self.expr_level += 1;
        self.expect(Tk::OpenBracket);
        if !matches!(self.curr.kind, Tk::Colon | Tk::Ellipsis | Tk::RangeHalf | Tk::RangeFull) {
            self.parse_expr(false);
        }
        let mut kind = Expr::Index;
        if matches!(self.curr.kind, Tk::Ellipsis | Tk::RangeHalf | Tk::RangeFull) {
            self.error();
        }
        let interval = matches!(self.curr.kind, Tk::Colon | Tk::Comma);
        if interval || matches!(self.curr.kind, Tk::Ellipsis | Tk::RangeHalf | Tk::RangeFull) {
            self.advance();
            kind = Expr::Slice;
            if !matches!(self.curr.kind, Tk::CloseBracket | Tk::Eof) {
                self.parse_expr(false);
            }
        }
        self.expr_level -= 1;
        let close = self.expect(Tk::CloseBracket);
        self.allow_range = allow_range;
        let start = self.start_of(operand);
        self.push(kind, start, close.end)
    }

    fn parse_selector(&mut self, operand: NodeId) -> NodeId {
        let dot = self.expect(Tk::Period);
        let start = self.start_of(operand);
        match self.curr.kind {
            Tk::Ident => {
                let field = self.parse_ident();
                let end = self.end_of(field);
                self.push(Expr::Selector { expr: operand, field }, start, end)
            }
            Tk::OpenParen => {
                self.expect(Tk::OpenParen);
                self.parse_type();
                let close = self.expect(Tk::CloseParen);
                self.push(Expr::TypeAssertion, start, close.end)
            }
            Tk::Question => {
                let question = self.expect(Tk::Question);
                self.push(Expr::TypeAssertion, start, question.end)
            }
            _ => {
                self.error();
                let field = self.push(Expr::Ident { blank: false }, dot.start, dot.end);
                self.push(Expr::Selector { expr: operand, field }, start, dot.end)
            }
        }
    }

    fn parse_arrow(&mut self, operand: NodeId) -> NodeId {
        let arrow = self.expect(Tk::ArrowRight);
        let start = self.start_of(operand);
        let field = if self.curr.kind == Tk::Ident {
            self.parse_ident()
        } else {
            self.error();
            self.push(Expr::Ident { blank: false }, arrow.start, arrow.end)
        };
        let end = self.end_of(field);
        self.push(Expr::Selector { expr: operand, field }, start, end)
    }

    /// `operand(args)`; arguments may be named (`x = 1`) or spread (`..xs`).
    pub(super) fn parse_call_expr(&mut self, operand: NodeId) -> NodeId {
        let level = self.expr_level;
        self.expr_level = 0;
        self.expect(Tk::OpenParen);
        while !matches!(self.curr.kind, Tk::CloseParen | Tk::Eof) {
            if matches!(self.curr.kind, Tk::Comma | Tk::Eq) {
                self.error();
            }
            self.allow(Tk::Ellipsis);
            self.parse_expr(false);
            if self.curr.kind == Tk::Eq {
                self.expect(Tk::Eq);
                self.parse_value();
            }
            if !self.allow(Tk::Comma) {
                break;
            }
        }
        self.expr_level = level;
        let close = self.expect_closing(Tk::CloseParen);
        let start = self.start_of(operand);
        self.push(Expr::Call { callee: operand }, start, close.end)
    }

    /// `{elems}`, after a type or alone.
    pub(super) fn parse_literal_value(&mut self, ty: Option<NodeId>) -> NodeId {
        let open = self.expect(Tk::OpenBrace);
        let level = self.expr_level;
        self.expr_level = 0;
        if self.curr.kind != Tk::CloseBrace {
            self.parse_elem_list();
        }
        self.expr_level = level;
        self.skip_possible_newline();
        let close = self.expect_closing(Tk::CloseBrace);
        let start = ty.map_or(open.start, |t| self.start_of(t));
        self.push(Expr::CompLit, start, close.end)
    }

    /// Values separated by commas up to `}`, each maybe `name = value`.
    pub(super) fn parse_elem_list(&mut self) -> Vec<NodeId> {
        let mut elems = Vec::new();
        while !matches!(self.curr.kind, Tk::CloseBrace | Tk::Eof) {
            let mut elem = self.parse_value();
            if self.curr.kind == Tk::Eq {
                self.expect(Tk::Eq);
                let value = self.parse_value();
                let (start, end) = (self.start_of(elem), self.end_of(value));
                elem = self.push(Expr::FieldValue { field: elem, value }, start, end);
            }
            elems.push(elem);
            if !self.allow(Tk::Comma) {
                break;
            }
        }
        elems
    }
}
