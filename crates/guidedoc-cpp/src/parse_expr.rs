//! Expressions, as `DefinitionParser` parses them (`sphinx/domains/cpp/_parser.py`).
//! Each production returns the node, or `Fail` where Sphinx raises `DefinitionError`;
//! those that may find nothing return `Ok(None)`, as Sphinx returns `None`.

use crate::ast::{Node, NodeId};
use crate::lex;
use crate::parse_decl::{Named, Outer, ParamMode};
use crate::parser::{Fail, ListBuilder, PResult, Parser};

const CASTS: [&str; 4] = ["dynamic_cast", "static_cast", "const_cast", "reinterpret_cast"];

const UNARY_OPS: [&str; 10] = ["++", "--", "*", "&", "+", "-", "!", "not", "~", "compl"];

const ASSIGNMENT_OPS: [&str; 14] = [
    "=", "*=", "/=", "%=", "+=", "-=", ">>=", "<<=", "&=", "and_eq", "^=", "|=", "xor_eq", "or_eq",
];

impl<'a> Parser<'a> {
    /// A literal: integer, character, floating, string, boolean, pointer, or
    /// user-defined.
    pub(crate) fn literal(&mut self) -> PResult<Option<NodeId>> {
        self.skip_ws();
        if self.skip_word("nullptr") {
            return self.add(Node::LitPointer).map(Some);
        }
        if self.skip_word("true") || self.skip_word("false") {
            return self.add(Node::LitBool(self.last == "true")).map(Some);
        }
        let s = self.bytes();
        let start = self.pos;
        if let Some(e) = lex::float_literal(s, start) {
            return self.number(e, lex::float_suffix(s, e)).map(Some);
        }
        if let Some(e) = lex::integer_literal(s, start) {
            return self.number(e, lex::integer_suffix(s, e)).map(Some);
        }
        if let Some(e) = lex::string_literal(s, start) {
            let text = self.take_to(e);
            let id = self.add(Node::LitString(text))?;
            return self.udl(id).map(Some);
        }
        if s.get(start) == Some(&b'"') {
            return Err(Fail); // an unterminated string.
        }
        if let Some(c) = lex::char_literal(self.def, start) {
            let value = lex::char_value(c.data).ok_or(Fail)?;
            self.pos = c.end;
            let id = self.add(Node::LitChar { prefix: c.prefix, data: c.data, value })?;
            return self.udl(id).map(Some);
        }
        Ok(None)
    }

    /// A number ending at `end`, with its suffix, or else a user-defined suffix.
    fn number(&mut self, end: usize, suffix: Option<usize>) -> PResult<NodeId> {
        let start = self.pos;
        self.pos = suffix.unwrap_or(end);
        let id = self.add(Node::LitNumber(self.since(start)))?;
        if suffix.is_some() { Ok(id) } else { self.udl(id) }
    }

    /// Makes a user-defined literal when an identifier follows.
    fn udl(&mut self, literal: NodeId) -> PResult<NodeId> {
        let Some(e) = lex::udl_identifier(self.bytes(), self.pos) else { return Ok(literal) };
        let text = self.take_to(e);
        let suffix = self.add(Node::Identifier(text))?;
        self.add(Node::LitUser { literal, suffix })
    }

    /// `(` expression `)` or a fold expression; `None` without `(`.
    fn fold_or_paren(&mut self) -> PResult<Option<NodeId>> {
        if self.current() != b'(' {
            return Ok(None);
        }
        self.pos += 1;
        self.skip_ws();
        if self.skip_string_ws("...") {
            // ( ... op expr )
            let e = lex::fold_operator(self.bytes(), self.pos).ok_or(Fail)?;
            let op = self.take_to(e);
            let right = self.cast_expression()?;
            if !self.skip_string(")") {
                return Err(Fail);
            }
            return self.add(Node::Fold { left: None, op, right: Some(right) }).map(Some);
        }
        let m = self.mark();
        self.skip_ws();
        let head = if fold_possible(self.bytes(), self.pos) { self.fold_head() } else { None };
        let Some((left, op)) = head else {
            self.reset(m);
            let inner = self.expression()?;
            self.skip_ws();
            if !self.skip_string(")") {
                return Err(Fail);
            }
            return self.add(Node::Paren(inner)).map(Some);
        };
        if self.skip_string(")") {
            return self.add(Node::Fold { left: Some(left), op, right: None }).map(Some);
        }
        let e = lex::fold_operator(self.bytes(), self.pos).ok_or(Fail)?;
        if self.def.get(self.pos..e) != Some(op) {
            return Err(Fail);
        }
        self.pos = e;
        let right = self.cast_expression()?;
        self.skip_ws();
        if !self.skip_string(")") {
            return Err(Fail);
        }
        self.add(Node::Fold { left: Some(left), op, right: Some(right) }).map(Some)
    }

    /// Reads `expr op ...` of a right or binary fold.
    fn fold_head(&mut self) -> Option<(NodeId, &'a str)> {
        let left = self.cast_expression().ok()?;
        self.skip_ws();
        let e = lex::fold_operator(self.bytes(), self.pos)?;
        let op = self.take_to(e);
        self.skip_ws();
        self.skip_string_ws("...").then_some((left, op))
    }

    /// A literal, `this`, a parenthesized or fold expression, or an id-expression.
    pub(crate) fn primary_expression(&mut self) -> PResult<NodeId> {
        self.guarded(|p| {
            p.skip_ws();
            if let Some(lit) = p.literal()? {
                return Ok(lit);
            }
            p.skip_ws();
            if p.skip_word("this") {
                return p.add(Node::This);
            }
            if let Some(res) = p.fold_or_paren()? {
                return Ok(res);
            }
            let name = p.nested_name(false)?;
            p.add(Node::IdExpr(name))
        })
    }

    /// Reads `open`, initializer clauses (each maybe with `...`), and `close`: the head
    /// of the list and whether a comma trails it; `None` when `open` is not there.
    fn initializer_list(
        &mut self,
        open: &str,
        close: &str,
    ) -> PResult<Option<(ListBuilder, bool)>> {
        let mut items = ListBuilder::default();
        self.skip_ws();
        if !self.skip_string_ws(open) {
            return Ok(None);
        }
        if self.skip_string(close) {
            return Ok(Some((items, false)));
        }
        loop {
            self.skip_ws();
            let mut expr = self.initializer_clause()?;
            self.skip_ws();
            if self.skip_string("...") {
                expr = self.add(Node::PackExpansion(expr))?;
            }
            items.push(self, expr);
            self.skip_ws();
            if self.skip_string(close) {
                return Ok(Some((items, false)));
            }
            if !self.skip_string_ws(",") {
                return Err(Fail);
            }
            if close == "}" && self.current() == b'}' {
                self.pos += 1;
                return Ok(Some((items, true)));
            }
        }
    }

    /// `(` expression-list `)`, empty allowed; `None` without `(`.
    pub(crate) fn paren_list(&mut self) -> PResult<Option<NodeId>> {
        let Some((items, _)) = self.initializer_list("(", ")")? else { return Ok(None) };
        self.add(Node::ParenList(items.head())).map(Some)
    }

    fn initializer_clause(&mut self) -> PResult<NodeId> {
        if let Some(braced) = self.braced_init()? {
            return Ok(braced);
        }
        self.assignment_expression(false)
    }

    /// `{` initializer-list `,`? `}`; `None` without `{`.
    pub(crate) fn braced_init(&mut self) -> PResult<Option<NodeId>> {
        self.guarded(|p| {
            let Some((items, trailing)) = p.initializer_list("{", "}")? else {
                return Ok(None);
            };
            let node = Node::BracedInit { items: items.head(), has_trailing_comma: trailing };
            p.add(node).map(Some)
        })
    }

    fn list_or_braced(&mut self) -> PResult<Option<NodeId>> {
        if let Some(paren) = self.paren_list()? {
            return Ok(Some(paren));
        }
        self.braced_init()
    }

    /// A cast, `typeid`, a primary expression, or a type (a functional cast), then
    /// postfix operations.
    fn postfix_expression(&mut self) -> PResult<NodeId> {
        self.skip_ws();
        let mut expr_like = true;
        let prefix = if let Some(cast) = CASTS.into_iter().find(|c| self.skip_word_ws(c)) {
            self.explicit_cast(cast)?
        } else if self.skip_word_ws("typeid") {
            self.typeid_expression()?
        } else {
            let m = self.mark();
            match self.primary_expression() {
                Ok(e) => e,
                Err(Fail) => {
                    self.reset(m);
                    let ty = self.parse_type(Named::False, Outer::OperatorCast)?;
                    expr_like = false;
                    self.skip_ws();
                    if !matches!(self.current(), b'(' | b'{') {
                        return Err(Fail);
                    }
                    ty
                }
            }
        };
        let mut ops = ListBuilder::default();
        loop {
            self.skip_ws();
            if expr_like && let Some(op) = self.postfix_operation()? {
                ops.push(self, op);
                continue;
            }
            let Some(list) = self.list_or_braced()? else { break };
            let call = self.add(Node::PfCall(list))?;
            ops.push(self, call);
        }
        self.add(Node::Postfix { prefix, ops: ops.head() })
    }

    fn explicit_cast(&mut self, cast: &'static str) -> PResult<NodeId> {
        if !self.skip_string("<") {
            return Err(Fail);
        }
        let ty = self.parse_type(Named::False, Outer::None)?;
        self.skip_ws();
        if !self.skip_string_ws(">") || !self.skip_string("(") {
            return Err(Fail);
        }
        let expr = self.expression_fallback(")", true, Parser::expression)?;
        self.skip_ws();
        if !self.skip_string(")") {
            return Err(Fail);
        }
        self.add(Node::ExplicitCast { cast, ty, expr })
    }

    fn typeid_expression(&mut self) -> PResult<NodeId> {
        if !self.skip_string_ws("(") {
            return Err(Fail);
        }
        let m = self.mark();
        if let Ok(ty) = self.parse_type(Named::False, Outer::None)
            && self.skip_string(")")
        {
            return self.add(Node::TypeId { operand: ty, is_type: true });
        }
        self.reset(m);
        let expr = self.expression_fallback(")", true, Parser::expression)?;
        if !self.skip_string(")") {
            return Err(Fail);
        }
        self.add(Node::TypeId { operand: expr, is_type: false })
    }

    /// Reads `[e]`, `.name`, `->name`, `++`, or `--`; `None` for none.
    fn postfix_operation(&mut self) -> PResult<Option<NodeId>> {
        if self.skip_string_ws("[") {
            let expr = self.expression()?;
            self.skip_ws();
            if !self.skip_string("]") {
                return Err(Fail);
            }
            return self.add(Node::PfArray(expr)).map(Some);
        }
        if self.skip_string(".") {
            if self.skip_string("*") {
                self.pos -= 2; // the dot is not ours.
            } else if self.skip_string("..") {
                self.pos -= 3;
            } else {
                let name = self.nested_name(false)?;
                return self.add(Node::PfMember(name)).map(Some);
            }
        }
        if self.skip_string("->") {
            if self.skip_string("*") {
                self.pos -= 3;
            } else {
                let name = self.nested_name(false)?;
                return self.add(Node::PfMemberPtr(name)).map(Some);
            }
        }
        if self.skip_string("++") {
            return self.add(Node::PfInc).map(Some);
        }
        if self.skip_string("--") {
            return self.add(Node::PfDec).map(Some);
        }
        Ok(None)
    }

    /// A unary operator, `sizeof`, `alignof`, `noexcept`, `new`, `delete`, or a postfix
    /// expression.
    fn unary_expression(&mut self) -> PResult<NodeId> {
        self.guarded(|p| {
            p.skip_ws();
            if let Some(op) = UNARY_OPS.into_iter().find(|op| p.skip_operator(op)) {
                let operand = p.cast_expression()?;
                return p.add(Node::Unary { op, operand });
            }
            if p.skip_word_ws("sizeof") {
                return p.sizeof_expression();
            }
            if p.skip_word_ws("alignof") {
                if !p.skip_string_ws("(") {
                    return Err(Fail);
                }
                let ty = p.parse_type(Named::False, Outer::None)?;
                p.skip_ws();
                if !p.skip_string(")") {
                    return Err(Fail);
                }
                return p.add(Node::Alignof(ty));
            }
            if p.skip_word_ws("noexcept") {
                if !p.skip_string_ws("(") {
                    return Err(Fail);
                }
                let expr = p.expression()?;
                p.skip_ws();
                if !p.skip_string(")") {
                    return Err(Fail);
                }
                return p.add(Node::NoexceptExpr(expr));
            }
            if let Some(res) = p.new_or_delete()? {
                return Ok(res);
            }
            p.postfix_expression()
        })
    }

    fn sizeof_expression(&mut self) -> PResult<NodeId> {
        if self.skip_string_ws("...") {
            if !self.skip_string_ws("(") || !self.match_identifier() {
                return Err(Fail);
            }
            let ident = self.add_identifier()?;
            self.skip_ws();
            if !self.skip_string(")") {
                return Err(Fail);
            }
            return self.add(Node::SizeofPack(ident));
        }
        if self.skip_string_ws("(") {
            let ty = self.parse_type(Named::False, Outer::None)?;
            self.skip_ws();
            if !self.skip_string(")") {
                return Err(Fail);
            }
            return self.add(Node::SizeofType(ty));
        }
        let expr = self.unary_expression()?;
        self.add(Node::SizeofExpr(expr))
    }

    /// A new- or delete-expression; `None` when the text is neither.
    fn new_or_delete(&mut self) -> PResult<Option<NodeId>> {
        let m = self.mark();
        let is_rooted = self.skip_string("::");
        self.skip_ws();
        if self.skip_word_ws("new") {
            if self.skip_string_ws("(") {
                return Err(Fail); // not supported by Sphinx.
            }
            let specs = self.decl_specs(Outer::None, true)?;
            let decl = self.declarator(Named::False, ParamMode::New, true)?;
            let init = self.list_or_braced()?;
            let ty = self.add(Node::Type { specs, decl })?;
            return self.add(Node::New { is_rooted, ty, init }).map(Some);
        }
        self.reset(m);
        let is_rooted = self.skip_string("::");
        self.skip_ws();
        if !self.skip_word_ws("delete") {
            self.reset(m);
            return Ok(None);
        }
        let is_array = self.skip_string_ws("[");
        if is_array && !self.skip_string_ws("]") {
            return Err(Fail);
        }
        let operand = self.cast_expression()?;
        self.add(Node::Delete { is_rooted, is_array, operand }).map(Some)
    }

    /// `(` type-id `)` cast-expression, or a unary expression.
    pub(crate) fn cast_expression(&mut self) -> PResult<NodeId> {
        self.guarded(|p| {
            let m = p.mark();
            p.skip_ws();
            if !p.skip_string("(") {
                return p.unary_expression();
            }
            if let Ok(ty) = p.parse_type(Named::False, Outer::None)
                && p.skip_string(")")
                && let Ok(operand) = p.cast_expression()
            {
                return p.add(Node::Cast { ty, operand });
            }
            p.reset(m);
            p.unary_expression()
        })
    }

    /// `?` expression `:` assignment-expression after the condition; `None` without `?`.
    fn conditional_tail(&mut self, cond: NodeId, in_template: bool) -> PResult<Option<NodeId>> {
        self.skip_ws();
        if !self.skip_string("?") {
            return Ok(None);
        }
        let then = self.expression()?;
        self.skip_ws();
        if !self.skip_string(":") {
            return Err(Fail);
        }
        let other = self.assignment_expression(in_template)?;
        self.add(Node::Conditional { cond, then, other }).map(Some)
    }

    pub(crate) fn assignment_expression(&mut self, in_template: bool) -> PResult<NodeId> {
        let left = self.logical_or_expression(in_template)?;
        if let Some(cond) = self.conditional_tail(left, in_template)? {
            return Ok(cond);
        }
        if let Some(op) = ASSIGNMENT_OPS.into_iter().find(|op| self.skip_operator(op)) {
            let right = self.initializer_clause()?;
            return self.add(Node::Assignment { left, op, right });
        }
        Ok(left)
    }

    pub(crate) fn constant_expression(&mut self, in_template: bool) -> PResult<NodeId> {
        let or_expr = self.logical_or_expression(in_template)?;
        if let Some(cond) = self.conditional_tail(or_expr, in_template)? {
            return Ok(cond);
        }
        Ok(or_expr)
    }

    pub(crate) fn expression(&mut self) -> PResult<NodeId> {
        let first = self.assignment_expression(false)?;
        let mut items = ListBuilder::default();
        items.push(self, first);
        let mut count = 1;
        loop {
            self.skip_ws();
            if !self.skip_string(",") {
                break;
            }
            let expr = self.assignment_expression(false)?;
            items.push(self, expr);
            count += 1;
        }
        if count == 1 {
            return Ok(first);
        }
        self.add(Node::Comma(items.head()))
    }

    pub(crate) fn constant_plain(&mut self) -> PResult<NodeId> {
        self.constant_expression(false)
    }

    pub(crate) fn assignment_plain(&mut self) -> PResult<NodeId> {
        self.assignment_expression(false)
    }

    pub(crate) fn assignment_template(&mut self) -> PResult<NodeId> {
        self.assignment_expression(true)
    }

    /// Parses with `parser`, or, when that fails and `allow` holds, scans the text up to
    /// a byte of `end` outside brackets (Sphinx warns then).
    pub(crate) fn expression_fallback(
        &mut self,
        end: &str,
        allow: bool,
        parser: impl FnOnce(&mut Self) -> PResult<NodeId>,
    ) -> PResult<NodeId> {
        let m = self.mark();
        if let Ok(res) = parser(self) {
            return Ok(res);
        }
        if !allow || self.is_full || self.stopped.is_some() {
            return Err(Fail);
        }
        self.ast.used_fallback = true;
        self.reset(m);
        self.skip_ws();
        let start = self.pos;
        let value = if let Some(e) = lex::fallback_string(self.bytes(), start) {
            self.take_to(e)
        } else {
            self.scan_balanced_to(end.as_bytes());
            if !end.is_empty() && self.is_eof() {
                return Err(Fail);
            }
            self.since(start)
        };
        self.add(Node::Fallback(lex::trim_space(value)))
    }

    /// Moves to the first byte of `end` outside brackets, or to the end.
    fn scan_balanced_to(&mut self, end: &[u8]) {
        let mut stack = [0u8; 64];
        let mut depth = 0usize;
        while !self.is_eof() {
            let c = self.current();
            if depth == 0 && end.contains(&c) {
                break;
            }
            if let Some(closer) = bracket_closer(c) {
                if depth < stack.len() {
                    stack[depth] = closer;
                }
                depth += 1;
            } else if depth > 0 && depth <= stack.len() && c == stack[depth - 1] {
                depth -= 1;
            }
            self.pos += 1;
        }
    }
}

fn bracket_closer(c: u8) -> Option<u8> {
    match c {
        b'(' => Some(b')'),
        b'{' => Some(b'}'),
        b'[' => Some(b']'),
        b'<' => Some(b'>'),
        _ => None,
    }
}

/// Tells whether the parenthesized group starting at `start` (after its `(`) has `...`
/// outside nested brackets and literals, as every fold expression has. Without it,
/// trying a fold first would parse the operand twice at every level of nested
/// parentheses, which takes time exponential in their depth.
fn fold_possible(s: &[u8], start: usize) -> bool {
    let mut depth = 0usize;
    let mut i = start;
    while i < s.len() {
        match s[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            quote @ (b'"' | b'\'') => {
                i += 1;
                while i < s.len() && s[i] != quote {
                    if s[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b'.' if depth == 0 && s[i..].starts_with(b"...") => return true,
            _ => {}
        }
        i += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Stop;

    /// A parse that backtracks past its step budget stops with a Limit.
    #[test]
    fn parse_steps_are_bounded() {
        let mut p = Parser::new("f(((((1)))))", &crate::Limits::default()).with_max_steps(5);
        assert!(p.expression().is_err());
        assert_eq!(p.stopped, Some(Stop::Work));
        assert_eq!(p.stop_error().map(|e| e.code), Some("core.limit.work"));
    }

    #[test]
    fn folds_are_tried_only_with_an_ellipsis() {
        assert!(fold_possible(b"a + ...)", 0));
        assert!(!fold_possible(b"(a, \"...\"))", 0));
        assert!(!fold_possible(b"(a + ...))", 0));
    }
}
