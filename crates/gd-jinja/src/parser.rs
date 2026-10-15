//! The parser: tokens into a [`Compiled`] template. This file holds the token stream and
//! expressions, in Jinja's precedence order; statements are in `parser_stmt.rs`.

use std::borrow::Cow;
use std::rc::Rc;

use crate::ast::{
    Args, BinOp, BlockDef, CmpOp, Compiled, Expr, ExprKind, ForDef, MacroDef, Name, UnaryOp,
};
use crate::convert::quoted;
use crate::error::{Error, ErrorKind};
use crate::lexer::{self, LexOptions, Token, TokenKind};
use crate::stack::StackGuard;
use crate::value::Value;

pub(crate) struct Parser<'s> {
    pub tokens: Vec<Token<'s>>,
    pub pos: usize,
    pub src: &'s str,
    pub err: Option<Error>,
    pub macros: Vec<MacroDef>,
    pub blocks: Vec<BlockDef>,
    pub loops: Vec<ForDef>,
    pub tag_stack: Vec<Name>,
    pub end_stack: Vec<&'static [&'static str]>,
    pub loop_depth: usize,
    depth: usize,
    max_depth: usize,
    stack: StackGuard,
}

/// What the parser needs from the environment.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ParseOptions {
    pub lex: LexOptions,
    pub max_nesting: usize,
    pub stack: StackGuard,
}

/// Parses template source into a Compiled template named name.
pub(crate) fn parse(
    source: &str,
    name: &str,
    autoescape: bool,
    opts: ParseOptions,
) -> Result<Compiled, Error> {
    let src = lexer::normalize(source, opts.lex.keep_trailing_newline);
    let located = |mut err: Error| {
        err.template = name.to_string();
        err.excerpt = crate::error::source_line(&src, err.line).to_string();
        err
    };
    let tokens = lexer::tokenize(&src, opts.lex).map_err(located)?;
    let mut p = Parser::new(tokens, &src, opts);
    let body = p.subparse(None);
    if let Some(err) = p.err.take() {
        return Err(located(err));
    }
    let Parser { macros, blocks, loops, .. } = p;
    Ok(Compiled {
        name: Rc::from(name),
        source: Box::from(src.as_ref()),
        body,
        macros,
        blocks,
        loops,
        autoescape,
    })
}

/// Parses a single expression (Jinja's compile_expression).
pub(crate) fn parse_expression(source: &str, opts: ParseOptions) -> Result<Expr, Error> {
    let src = format!("{{{{ {source} }}}}");
    let named = |mut err: Error| {
        err.template = "<expression>".to_string();
        err
    };
    let tokens = lexer::tokenize(&src, LexOptions::default()).map_err(named)?;
    let mut p = Parser::new(tokens, &src, opts);
    p.expect(TokenKind::VariableBegin, "");
    let e = p.parse_tuple(TupleOptions::default());
    p.expect(TokenKind::VariableEnd, "");
    if p.peek(0).kind != TokenKind::Eof {
        p.fail_unexpected();
    }
    match p.err.take() {
        Some(err) => Err(named(err)),
        None => Ok(e),
    }
}

/// How parse_tuple reads its items.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TupleOptions {
    pub simplified: bool,
    pub without_condexpr: bool,
    pub extra_end: &'static [&'static str],
    pub explicit_parens: bool,
}

impl<'s> Parser<'s> {
    fn new(tokens: Vec<Token<'s>>, src: &'s str, opts: ParseOptions) -> Parser<'s> {
        Parser {
            tokens,
            pos: 0,
            src,
            err: None,
            macros: Vec::new(),
            blocks: Vec::new(),
            loops: Vec::new(),
            tag_stack: Vec::new(),
            end_stack: Vec::new(),
            loop_depth: 0,
            depth: 0,
            max_depth: opts.max_nesting,
            stack: opts.stack,
        }
    }

    // -----------------------------------------------------------------------------------
    // Token stream

    /// The current token, or one ahead of it.
    pub fn peek(&self, ahead: usize) -> &Token<'s> {
        let last = self.tokens.len() - 1;
        &self.tokens[(self.pos + ahead).min(last)]
    }

    /// Consumes the current token and returns it.
    pub fn next_token(&mut self) -> Token<'s> {
        let t = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    /// Whether the current token (or one ahead) is the operator or name text.
    pub fn at(&self, text: &str, ahead: usize) -> bool {
        let t = self.peek(ahead);
        matches!(t.kind, TokenKind::Operator | TokenKind::Name) && t.text == text
    }

    pub fn at_op(&self, text: &str) -> bool {
        let t = self.peek(0);
        t.kind == TokenKind::Operator && t.text == text
    }

    pub fn at_name(&self, text: &str, ahead: usize) -> bool {
        let t = self.peek(ahead);
        t.kind == TokenKind::Name && t.text == text
    }

    pub fn skip_if(&mut self, text: &str) -> bool {
        let found = self.at(text, 0);
        if found {
            self.next_token();
        }
        found
    }

    pub fn failed(&self) -> bool {
        self.err.is_some()
    }

    /// Records the first error and skips to the end of the tokens.
    pub fn fail(&mut self, line: usize, message: impl Into<String>) {
        if self.err.is_none() {
            self.err = Some(Error::new(ErrorKind::Syntax, line, message));
            self.pos = self.tokens.len() - 1;
        }
    }

    /// Sets the hint of the error just raised.
    pub fn hint(&mut self, hint: &str) {
        if let Some(err) = &mut self.err
            && err.hint.is_empty()
        {
            err.hint = hint.to_string();
        }
    }

    /// Enters one more level of nesting: a nested expression or tag, or one more link
    /// of an operator, filter, attribute or elif chain. Past max_nesting or the stack
    /// budget it reports a Limit error; the caller undoes its levels either way.
    pub fn nest(&mut self) {
        self.depth += 1;
        if self.failed() || (self.depth <= self.max_depth && !self.stack.exhausted()) {
            return;
        }
        let line = self.peek(0).line;
        if self.depth > self.max_depth {
            let max = self.max_depth;
            self.fail(line, format!("the template nests more than {max} levels deep"));
            self.hint("Split the nested expression or tags, or raise Options::max_nesting.");
        } else {
            let limit = self.stack.limit;
            self.fail(
                line,
                format!("the template nests too deeply for the stack budget of {limit} bytes"),
            );
            self.hint("Split the nested expression or tags, or raise Options::stack_bytes.");
        }
        if let Some(err) = &mut self.err {
            err.kind = ErrorKind::Limit;
        }
    }

    pub fn unnest(&mut self, levels: usize) {
        self.depth -= levels;
    }

    pub fn fail_unexpected(&mut self) {
        let t = self.peek(0);
        let line = t.line;
        if t.kind == TokenKind::Eof {
            self.fail_eof("", line);
            return;
        }
        let what = quoted(&describe_token(t));
        self.fail(line, format!("unexpected {what}"));
    }

    /// Consumes a token of the given kind (and text, when text is not empty).
    pub fn expect(&mut self, kind: TokenKind, text: &str) -> Token<'s> {
        let t = self.peek(0);
        if t.kind == kind && (text.is_empty() || t.text == text) {
            return self.next_token();
        }
        let want = quoted(if text.is_empty() { describe_kind(kind) } else { text });
        let (line, eof, got) = (t.line, t.kind == TokenKind::Eof, quoted(&describe_token(t)));
        let t = t.clone();
        if eof {
            self.fail(line, format!("unexpected end of template, expected {want}."));
        } else {
            self.fail(line, format!("expected token {want}, got {got}"));
        }
        t
    }

    /// A name token's text as an identifier.
    pub fn expect_name(&mut self) -> (Name, usize) {
        let t = self.expect(TokenKind::Name, "");
        (Rc::from(t.text.as_ref()), t.line)
    }

    // -----------------------------------------------------------------------------------
    // Expressions

    /// Parses an expression, or a tuple when commas follow.
    pub fn parse_tuple(&mut self, o: TupleOptions) -> Expr {
        let line = self.peek(0).line;
        let mut items = Vec::new();
        if !self.failed() && !self.tuple_end(o.extra_end) {
            let first = self.parse_tuple_item(o);
            if !self.at_op(",") {
                return first;
            }
            items.push(first);
            while self.at_op(",") && !self.failed() {
                self.next_token();
                if self.tuple_end(o.extra_end) {
                    break;
                }
                items.push(self.parse_tuple_item(o));
            }
        } else if !o.explicit_parens {
            let t = self.peek(0);
            let (line, got) = (t.line, quoted(&describe_token(t)));
            self.fail(line, format!("Expected an expression, got {got}"));
        }
        Expr { line, kind: ExprKind::Tuple(items) }
    }

    fn parse_tuple_item(&mut self, o: TupleOptions) -> Expr {
        if o.simplified {
            return self.parse_primary();
        }
        self.parse_expr(!o.without_condexpr)
    }

    fn tuple_end(&self, extra_end: &[&str]) -> bool {
        let t = self.peek(0);
        matches!(t.kind, TokenKind::VariableEnd | TokenKind::BlockEnd | TokenKind::Eof)
            || (t.kind == TokenKind::Operator && t.text == ")")
            || extra_end.iter().any(|name| self.at_name(name, 0))
    }

    pub fn parse_expr(&mut self, with_condexpr: bool) -> Expr {
        if with_condexpr { self.parse_condexpr() } else { self.parse_or() }
    }

    fn parse_condexpr(&mut self) -> Expr {
        let line = self.peek(0).line;
        let mut expr = self.parse_or();
        let mut levels = 0;
        while self.at_name("if", 0) && !self.failed() {
            self.next_token();
            levels += 1;
            self.nest();
            let test = self.parse_or();
            let otherwise = if self.at_name("else", 0) {
                self.next_token();
                Some(Box::new(self.parse_condexpr()))
            } else {
                None
            };
            let kind = ExprKind::Cond { test: Box::new(test), then: Box::new(expr), otherwise };
            expr = Expr { line, kind };
        }
        self.unnest(levels);
        expr
    }

    /// Parses an `or` expression: the operand of a condition, and the level a bracket
    /// starts, which nests one level.
    fn parse_or(&mut self) -> Expr {
        self.nest();
        let e = self.parse_level(Level::Or);
        self.unnest(1);
        e
    }

    /// Parses an expression whose operators bind at least as tightly as min, by
    /// precedence climbing: one loop handles every binary level, so a bracket costs a few
    /// frames rather than one per level of Jinja's grammar. Each link of an `or`, `and`,
    /// arithmetic or power chain nests one level; comparison and `~` chains are n-ary.
    fn parse_level(&mut self, min: Level) -> Expr {
        let mut left = self.parse_operand(min);
        let mut levels = 0;
        while !self.failed() {
            let Some((op, level)) = self.binary_operator() else { break };
            if level < min {
                break;
            }
            left = match op {
                Infix::Compare => self.parse_compare_chain(left),
                Infix::Concat => self.parse_concat_chain(left),
                Infix::Binary(op) => {
                    levels += 1;
                    self.nest();
                    let line = self.next_token().line;
                    let right = self.parse_level(level.tighter());
                    binary(op, line, left, right)
                }
            };
        }
        self.unnest(levels);
        left
    }

    /// The first operand at level min: a `not` where it may stand, else a unary.
    fn parse_operand(&mut self, min: Level) -> Expr {
        if min > Level::Not || !self.at_name("not", 0) {
            return self.parse_unary(true);
        }
        let line = self.next_token().line;
        self.nest();
        let operand = self.parse_level(Level::Not);
        self.unnest(1);
        Expr { line, kind: ExprKind::Unary(UnaryOp::Not, Box::new(operand)) }
    }

    /// The binary operator at the current token and how tightly it binds.
    fn binary_operator(&self) -> Option<(Infix, Level)> {
        if self.at_name("or", 0) {
            return Some((Infix::Binary(BinOp::Or), Level::Or));
        }
        if self.at_name("and", 0) {
            return Some((Infix::Binary(BinOp::And), Level::And));
        }
        if self.compare_operator().is_some() {
            return Some((Infix::Compare, Level::Compare));
        }
        let t = self.peek(0);
        if t.kind != TokenKind::Operator {
            return None;
        }
        Some(match t.text.as_ref() {
            "+" => (Infix::Binary(BinOp::Add), Level::Sum),
            "-" => (Infix::Binary(BinOp::Sub), Level::Sum),
            "~" => (Infix::Concat, Level::Concat),
            "*" => (Infix::Binary(BinOp::Mul), Level::Product),
            "/" => (Infix::Binary(BinOp::Div), Level::Product),
            "//" => (Infix::Binary(BinOp::FloorDiv), Level::Product),
            "%" => (Infix::Binary(BinOp::Mod), Level::Product),
            "**" => (Infix::Binary(BinOp::Pow), Level::Power),
            _ => return None,
        })
    }

    /// Parses a comparison chain after its first operand.
    fn parse_compare_chain(&mut self, first: Expr) -> Expr {
        let line = first.line;
        let mut rest = Vec::new();
        while let Some(op) = self.compare_operator().filter(|_| !self.failed()) {
            self.next_token();
            if op == CmpOp::NotIn {
                self.next_token();
            }
            rest.push((op, self.parse_level(Level::Compare.tighter())));
        }
        Expr { line, kind: ExprKind::Compare(Box::new(first), rest) }
    }

    /// Parses a `~` chain after its first operand.
    fn parse_concat_chain(&mut self, first: Expr) -> Expr {
        let line = first.line;
        let mut items = vec![first];
        while self.at_op("~") && !self.failed() {
            self.next_token();
            items.push(self.parse_level(Level::Concat.tighter()));
        }
        Expr { line, kind: ExprKind::Concat(items) }
    }

    fn compare_operator(&self) -> Option<CmpOp> {
        let t = self.peek(0);
        if t.kind == TokenKind::Operator {
            match t.text.as_ref() {
                "==" => return Some(CmpOp::Eq),
                "!=" => return Some(CmpOp::Ne),
                "<" => return Some(CmpOp::Lt),
                "<=" => return Some(CmpOp::Le),
                ">" => return Some(CmpOp::Gt),
                ">=" => return Some(CmpOp::Ge),
                _ => {}
            }
        }
        if self.at_name("in", 0) {
            return Some(CmpOp::In);
        }
        (self.at_name("not", 0) && self.at_name("in", 1)).then_some(CmpOp::NotIn)
    }

    fn parse_unary(&mut self, with_filter: bool) -> Expr {
        let node = if self.at_op("-") || self.at_op("+") {
            let t = self.next_token();
            let op = if t.text == "-" { UnaryOp::Neg } else { UnaryOp::Pos };
            self.nest();
            let operand = self.parse_unary(false);
            self.unnest(1);
            Expr { line: t.line, kind: ExprKind::Unary(op, Box::new(operand)) }
        } else {
            self.parse_primary()
        };
        let node = self.parse_postfix(node);
        if with_filter { self.parse_filter_expr(node) } else { node }
    }

    /// Parses a bracketed expression or a literal.
    pub fn parse_primary(&mut self) -> Expr {
        if self.at_op("(") {
            self.next_token();
            let e = self.parse_tuple(TupleOptions { explicit_parens: true, ..Default::default() });
            self.expect(TokenKind::Operator, ")");
            return e;
        }
        if self.at_op("[") {
            return self.parse_list();
        }
        if self.at_op("{") {
            return self.parse_dict();
        }
        self.parse_literal()
    }

    fn parse_literal(&mut self) -> Expr {
        let t = self.peek(0).clone();
        let value = match t.kind {
            TokenKind::Name => {
                self.next_token();
                match t.text.as_ref() {
                    "true" | "True" => Value::Bool(true),
                    "false" | "False" => Value::Bool(false),
                    "none" | "None" => Value::None,
                    name => return Expr { line: t.line, kind: ExprKind::Name(Rc::from(name)) },
                }
            }
            TokenKind::String => {
                self.next_token();
                let mut text = t.text;
                while self.peek(0).kind == TokenKind::String {
                    let more = self.next_token().text;
                    text = Cow::Owned(text.into_owned() + &more);
                }
                Value::str(&text)
            }
            TokenKind::Integer => {
                self.next_token();
                Value::Int(t.number)
            }
            TokenKind::Float => {
                self.next_token();
                Value::Float(t.float)
            }
            _ => {
                self.fail_unexpected();
                Value::None
            }
        };
        Expr { line: t.line, kind: ExprKind::Const(value) }
    }

    fn parse_list(&mut self) -> Expr {
        let line = self.expect(TokenKind::Operator, "[").line;
        let mut items = Vec::new();
        while !self.at_op("]") && !self.failed() {
            if !items.is_empty() {
                self.expect(TokenKind::Operator, ",");
                if self.at_op("]") {
                    break;
                }
            }
            items.push(self.parse_expr(true));
        }
        self.expect(TokenKind::Operator, "]");
        Expr { line, kind: ExprKind::List(items) }
    }

    fn parse_dict(&mut self) -> Expr {
        let line = self.expect(TokenKind::Operator, "{").line;
        let mut pairs = Vec::new();
        while !self.at_op("}") && !self.failed() {
            if !pairs.is_empty() {
                self.expect(TokenKind::Operator, ",");
                if self.at_op("}") {
                    break;
                }
            }
            let key = self.parse_expr(true);
            self.expect(TokenKind::Operator, ":");
            pairs.push((key, self.parse_expr(true)));
        }
        self.expect(TokenKind::Operator, "}");
        Expr { line, kind: ExprKind::Dict(pairs) }
    }

    /// Parses attribute, item and call links; each link nests one level.
    pub fn parse_postfix(&mut self, mut node: Expr) -> Expr {
        let mut levels = 0;
        while !self.failed() && (self.at_op(".") || self.at_op("[") || self.at_op("(")) {
            levels += 1;
            self.nest();
            node =
                if self.at_op("(") { self.parse_call(node) } else { self.parse_subscript(node) };
        }
        self.unnest(levels);
        node
    }

    /// Parses filter, test and call links; tests and calls nest one level each.
    fn parse_filter_expr(&mut self, mut node: Expr) -> Expr {
        let mut levels = 0;
        while !self.failed() {
            if self.at_op("|") {
                let line = node.line;
                let filtered = self.parse_filter(Some(node), false);
                node = filtered.unwrap_or(Expr { line, kind: ExprKind::Const(Value::None) });
                continue;
            }
            let is_test = self.at_name("is", 0);
            if !is_test && !self.at_op("(") {
                break;
            }
            levels += 1;
            self.nest();
            node = if is_test { self.parse_test(node) } else { self.parse_call(node) };
        }
        self.unnest(levels);
        node
    }

    fn parse_subscript(&mut self, node: Expr) -> Expr {
        let t = self.next_token();
        if t.text == "." {
            let attr = self.peek(0).clone();
            if attr.kind == TokenKind::Name {
                self.next_token();
                let kind = ExprKind::Getattr(Box::new(node), Rc::from(attr.text.as_ref()));
                return Expr { line: t.line, kind };
            }
            if attr.kind == TokenKind::Integer {
                self.next_token();
                let key = Expr { line: attr.line, kind: ExprKind::Const(Value::Int(attr.number)) };
                return Expr {
                    line: t.line,
                    kind: ExprKind::Getitem(Box::new(node), Box::new(key)),
                };
            }
            self.fail(attr.line, "expected name or number");
            return node;
        }
        let mut items = Vec::new();
        while !self.at_op("]") && !self.failed() {
            if !items.is_empty() {
                self.expect(TokenKind::Operator, ",");
                if self.at_op("]") {
                    break;
                }
            }
            items.push(self.parse_subscribed());
        }
        self.expect(TokenKind::Operator, "]");
        let key = if items.len() == 1 {
            items.pop().unwrap_or(Expr { line: t.line, kind: ExprKind::Const(Value::None) })
        } else {
            Expr { line: t.line, kind: ExprKind::Tuple(items) }
        };
        Expr { line: t.line, kind: ExprKind::Getitem(Box::new(node), Box::new(key)) }
    }

    fn parse_subscribed(&mut self) -> Expr {
        let line = self.peek(0).line;
        let mut start = None;
        if !self.at_op(":") {
            let e = self.parse_expr(true);
            if !self.at_op(":") {
                return e;
            }
            start = Some(Box::new(e));
        }
        self.next_token();
        let mut stop = None;
        let mut step = None;
        if !self.at_op("]") && !self.at_op(",") && !self.at_op(":") {
            stop = Some(Box::new(self.parse_expr(true)));
        }
        if self.skip_if(":") && !self.at_op("]") && !self.at_op(",") {
            step = Some(Box::new(self.parse_expr(true)));
        }
        Expr { line, kind: ExprKind::Slice(start, stop, step) }
    }

    /// Parses `(args, names = values, *b, **c)`.
    pub fn parse_call_args(&mut self) -> Args {
        self.expect(TokenKind::Operator, "(");
        let mut a = Args::default();
        while !self.at_op(")") && !self.failed() {
            let any = !a.args.is_empty() || !a.kwargs.is_empty();
            if any || a.star.is_some() || a.dstar.is_some() {
                self.expect(TokenKind::Operator, ",");
                if self.at_op(")") {
                    break;
                }
            }
            let t = self.peek(0).clone();
            if self.at_op("**") {
                self.next_token();
                a.dstar = Some(self.parse_expr(true));
            } else if self.at_op("*") {
                self.next_token();
                a.star = Some(self.parse_expr(true));
            } else if t.kind == TokenKind::Name
                && self.peek(1).kind == TokenKind::Operator
                && self.peek(1).text == "="
            {
                self.next_token();
                self.next_token();
                a.kwargs.push((Rc::from(t.text.as_ref()), self.parse_expr(true)));
            } else {
                if !a.kwargs.is_empty() || a.star.is_some() || a.dstar.is_some() {
                    self.fail(t.line, "Invalid argument syntax");
                }
                a.args.push(self.parse_expr(true));
            }
        }
        self.expect(TokenKind::Operator, ")");
        a
    }

    fn parse_call(&mut self, node: Expr) -> Expr {
        let line = self.peek(0).line;
        let args = self.parse_call_args();
        Expr { line, kind: ExprKind::Call(Box::new(node), Box::new(args)) }
    }

    fn dotted_name(&mut self) -> Name {
        let mut name = self.expect(TokenKind::Name, "").text.into_owned();
        while self.at_op(".") && self.peek(1).kind == TokenKind::Name && !self.failed() {
            self.next_token();
            name.push('.');
            name.push_str(&self.next_token().text);
        }
        Rc::from(name)
    }

    /// Parses "| name(args)" repeatedly; inline parses a first filter without a leading
    /// "|" (filter blocks).
    pub fn parse_filter(&mut self, mut node: Option<Expr>, mut inline: bool) -> Option<Expr> {
        let mut levels = 0;
        while (inline || self.at_op("|")) && !self.failed() {
            levels += 1;
            self.nest();
            if !inline {
                self.next_token();
            }
            inline = false;
            let line = self.peek(0).line;
            let name = self.dotted_name();
            let args = if self.at_op("(") { self.parse_call_args() } else { Args::default() };
            let kind = ExprKind::Filter(node.map(Box::new), name, Box::new(args));
            node = Some(Expr { line, kind });
        }
        self.unnest(levels);
        node
    }

    fn parse_test(&mut self, node: Expr) -> Expr {
        let line = self.next_token().line;
        let negated = self.at_name("not", 0);
        if negated {
            self.next_token();
        }
        let name = self.dotted_name();
        let mut args = Args::default();
        if self.at_op("(") {
            args = self.parse_call_args();
        } else if test_arg_follows(self.peek(0)) {
            let primary = self.parse_primary();
            args.args.push(self.parse_postfix(primary));
        }
        let test = Expr { line, kind: ExprKind::Test(Box::new(node), name, Box::new(args)) };
        if !negated {
            return test;
        }
        Expr { line, kind: ExprKind::Unary(UnaryOp::Not, Box::new(test)) }
    }
}

/// How tightly binary operators bind, loosest first, as in Jinja's grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    Or,
    And,
    Not,
    Compare,
    Sum,
    Concat,
    Product,
    Power,
    Unary,
}

impl Level {
    /// The next tighter level: where the right operand of an operator at self starts.
    fn tighter(self) -> Level {
        match self {
            Level::Or => Level::And,
            Level::And => Level::Not,
            Level::Not => Level::Compare,
            Level::Compare => Level::Sum,
            Level::Sum => Level::Concat,
            Level::Concat => Level::Product,
            Level::Product => Level::Power,
            Level::Power | Level::Unary => Level::Unary,
        }
    }
}

/// What an infix operator builds.
#[derive(Debug, Clone, Copy)]
enum Infix {
    Binary(BinOp),
    Compare,
    Concat,
}

fn binary(op: BinOp, line: usize, left: Expr, right: Expr) -> Expr {
    Expr { line, kind: ExprKind::Binary(op, Box::new(left), Box::new(right)) }
}

fn test_arg_follows(t: &Token<'_>) -> bool {
    match t.kind {
        TokenKind::Name => !matches!(
            t.text.as_ref(),
            "else" | "or" | "and" | "if" | "in" | "is" | "not" | "recursive"
        ),
        TokenKind::String | TokenKind::Integer | TokenKind::Float => true,
        TokenKind::Operator => t.text == "[" || t.text == "{",
        _ => false,
    }
}

/// How a token is named in messages.
pub(crate) fn describe_token<'a>(t: &'a Token<'_>) -> Cow<'a, str> {
    match t.kind {
        TokenKind::Name | TokenKind::Operator => Cow::Borrowed(t.text.as_ref()),
        kind => Cow::Borrowed(describe_kind(kind)),
    }
}

fn describe_kind(kind: TokenKind) -> &'static str {
    match kind {
        TokenKind::Eof => "end of template",
        TokenKind::Data => "template data / text",
        TokenKind::VariableBegin => "begin of print statement",
        TokenKind::VariableEnd => "end of print statement",
        TokenKind::BlockBegin => "begin of statement block",
        TokenKind::BlockEnd => "end of statement block",
        TokenKind::Name => "name",
        TokenKind::Operator => "operator",
        TokenKind::String => "string",
        TokenKind::Integer => "integer",
        TokenKind::Float => "float",
    }
}
