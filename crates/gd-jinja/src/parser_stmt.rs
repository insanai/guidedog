//! Statement parsing: tags and their bodies.

use std::rc::Rc;

use crate::ast::{
    BlockDef, BlockId, Expr, ExprKind, ForDef, LoopId, MacroDef, MacroId, Name, Stmt, StmtKind,
    walk_names,
};
use crate::convert::quoted;
use crate::error::closest;
use crate::lexer::TokenKind;
use crate::parser::{Parser, TupleOptions};

const KNOWN_TAGS: &[&str] = &[
    "if",
    "elif",
    "else",
    "endif",
    "for",
    "endfor",
    "set",
    "endset",
    "macro",
    "endmacro",
    "call",
    "endcall",
    "filter",
    "endfilter",
    "with",
    "endwith",
    "block",
    "endblock",
    "extends",
    "include",
    "import",
    "from",
    "autoescape",
    "endautoescape",
    "do",
    "break",
    "continue",
    "trans",
    "pluralize",
    "endtrans",
    "raw",
    "endraw",
];

/// How an assignment target may look.
#[derive(Debug, Clone, Copy, Default)]
struct TargetOptions {
    extra_end: &'static [&'static str],
    name_only: bool,
    with_namespace: bool,
}

impl Parser<'_> {
    /// Parses template content until one of end_tags opens a block tag. It returns with
    /// the tag name as the current token; the caller consumes it.
    pub(crate) fn subparse(&mut self, end_tags: Option<&'static [&'static str]>) -> Vec<Stmt> {
        let mut body = Vec::new();
        let ends = self.end_stack.len();
        if let Some(tags) = end_tags {
            self.end_stack.push(tags);
        }
        while !self.failed() {
            let t = self.peek(0);
            let line = t.line;
            match t.kind {
                TokenKind::Eof => {
                    if end_tags.is_some() {
                        self.fail_eof("", line);
                    }
                    break;
                }
                TokenKind::Data => {
                    let start = t.text.as_ptr() as usize - self.src.as_ptr() as usize;
                    let end = start + t.text.len();
                    self.next_token();
                    body.push(Stmt { line, kind: StmtKind::Data(start, end) });
                }
                TokenKind::VariableBegin => {
                    self.next_token();
                    let e = self.parse_tuple(TupleOptions::default());
                    self.expect(TokenKind::VariableEnd, "");
                    body.push(Stmt { line, kind: StmtKind::Output(e) });
                }
                TokenKind::BlockBegin => {
                    self.next_token();
                    let name = self.peek(0);
                    let ends_here = name.kind == TokenKind::Name
                        && end_tags.is_some_and(|tags| tags.contains(&name.text.as_ref()));
                    if ends_here {
                        break;
                    }
                    if let Some(s) = self.parse_statement() {
                        body.push(s);
                    }
                }
                _ => self.fail_unexpected(),
            }
        }
        self.end_stack.truncate(ends);
        body
    }

    /// Reports an unknown tag (name) or an unexpected end (name empty), naming the tags
    /// Jinja expected.
    pub(crate) fn fail_eof(&mut self, name: &str, line: usize) {
        let mut b = if name.is_empty() {
            "Unexpected end of template.".to_string()
        } else {
            format!("Encountered unknown tag {}.", quoted(name))
        };
        if let Some(looking) = self.end_stack.last() {
            let expected = self.end_stack.iter().any(|tags| tags.contains(&name));
            let looking: Vec<String> = looking.iter().map(|t| quoted(t)).collect();
            if !name.is_empty() && expected {
                b.push_str(" You probably made a nesting mistake. Jinja is expecting this tag, ");
                b.push_str("but currently looking for ");
            } else {
                b.push_str(" Jinja was looking for the following tags: ");
            }
            b.push_str(&looking.join(" or "));
            b.push('.');
        }
        if let Some(tag) = self.tag_stack.last() {
            b.push_str(" The innermost block that needs to be closed is ");
            b.push_str(&quoted(tag));
            b.push('.');
        }
        self.fail(line, b);
    }

    fn parse_statement(&mut self) -> Option<Stmt> {
        let t = self.peek(0).clone();
        if t.kind != TokenKind::Name {
            self.fail(t.line, "tag name expected");
            return None;
        }
        let tags = self.tag_stack.len();
        self.tag_stack.push(Rc::from(t.text.as_ref()));
        self.nest();
        let kind = self.parse_tag(&t.text, t.line);
        self.unnest(1);
        self.tag_stack.truncate(tags);
        if kind.is_none() && !self.failed() {
            self.fail_eof(&t.text, t.line);
            let near = closest(&t.text, KNOWN_TAGS.iter().copied());
            if !near.is_empty() {
                self.hint(&format!("Did you mean {{% {near} %}}?"));
            }
        }
        Some(Stmt { line: t.line, kind: kind? })
    }

    /// Parses the tag called name, or returns None for an unknown tag.
    fn parse_tag(&mut self, name: &str, line: usize) -> Option<StmtKind> {
        Some(match name {
            "if" => self.parse_if(),
            "for" => self.parse_for(),
            "set" => self.parse_set(),
            "macro" => self.parse_macro(),
            "call" => self.parse_call_block(),
            "filter" => self.parse_filter_block(),
            "with" => self.parse_with(),
            "block" => self.parse_block(),
            "extends" | "do" | "autoescape" => self.parse_simple(),
            "include" => self.parse_include(),
            "import" => self.parse_import(),
            "from" => self.parse_from(),
            "break" | "continue" => {
                self.next_token();
                if self.loop_depth == 0 {
                    self.fail(line, format!("{} outside of a loop", quoted(name)));
                    return Some(StmtKind::Break);
                }
                self.expect(TokenKind::BlockEnd, "");
                if name == "break" { StmtKind::Break } else { StmtKind::Continue }
            }
            "trans" => self.parse_trans(),
            _ => return None,
        })
    }

    /// Consumes the closing tag name (already current) and the block end.
    pub(crate) fn end_tag(&mut self) {
        self.next_token();
        self.expect(TokenKind::BlockEnd, "");
    }

    fn parse_if(&mut self) -> StmtKind {
        self.next_token();
        let no_cond = TupleOptions { without_condexpr: true, ..Default::default() };
        // The branches nest: each elif is one level deeper. They are parsed into a list
        // first and folded into nested ifs after.
        let mut branches: Vec<(Expr, Vec<Stmt>, usize)> = Vec::new();
        let mut else_body = Vec::new();
        let mut levels = 0;
        let mut line = self.peek(0).line;
        while !self.failed() {
            let test = self.parse_tuple(no_cond);
            self.expect(TokenKind::BlockEnd, "");
            let body = self.subparse(Some(&["elif", "else", "endif"]));
            branches.push((test, body, line));
            let t = self.next_token();
            if t.text == "elif" {
                levels += 1;
                self.nest();
                line = t.line;
                continue;
            }
            if t.text == "else" {
                self.expect(TokenKind::BlockEnd, "");
                else_body = self.subparse(Some(&["endif"]));
                self.next_token();
            }
            self.expect(TokenKind::BlockEnd, "");
            break;
        }
        self.unnest(levels);
        let mut otherwise = else_body;
        while let Some((test, body, at)) = branches.pop() {
            if branches.is_empty() {
                return StmtKind::If(test, body, otherwise);
            }
            otherwise = vec![Stmt { line: at, kind: StmtKind::If(test, body, otherwise) }];
        }
        StmtKind::Break
    }

    fn parse_for(&mut self) -> StmtKind {
        self.next_token();
        let target =
            self.parse_assign_target(TargetOptions { extra_end: &["in"], ..Default::default() });
        self.expect(TokenKind::Name, "in");
        let iter = self.parse_tuple(TupleOptions {
            without_condexpr: true,
            extra_end: &["recursive"],
            ..Default::default()
        });
        let mut filter = None;
        if self.at_name("if", 0) {
            self.next_token();
            filter = Some(self.parse_expr(true));
        }
        let recursive = self.at_name("recursive", 0);
        if recursive {
            self.next_token();
        }
        self.expect(TokenKind::BlockEnd, "");
        self.loop_depth += 1;
        let body = self.subparse(Some(&["endfor", "else"]));
        self.loop_depth -= 1;
        let mut else_body = Vec::new();
        if self.next_token().text == "else" {
            self.expect(TokenKind::BlockEnd, "");
            else_body = self.subparse(Some(&["endfor"]));
            self.next_token();
        }
        self.expect(TokenKind::BlockEnd, "");
        let id = LoopId(self.loops.len() as u32);
        self.loops.push(ForDef { target, iter, filter, recursive, body, else_body });
        StmtKind::For(id)
    }

    /// Parses a name, a tuple of names, or (with_namespace) ns.attr.
    fn parse_assign_target(&mut self, o: TargetOptions) -> Expr {
        let t = self.peek(0).clone();
        if o.with_namespace
            && t.kind == TokenKind::Name
            && self.at(".", 1)
            && self.peek(2).kind == TokenKind::Name
        {
            self.next_token();
            self.next_token();
            let attr: Name = Rc::from(self.next_token().text.as_ref());
            return Expr { line: t.line, kind: ExprKind::NsRef(Rc::from(t.text.as_ref()), attr) };
        }
        let target = if o.name_only {
            let (name, line) = self.expect_name();
            Expr { line, kind: ExprKind::Name(name) }
        } else {
            self.parse_tuple(TupleOptions {
                simplified: true,
                extra_end: o.extra_end,
                ..Default::default()
            })
        };
        if !self.failed() && !can_assign(&target) {
            self.fail(target.line, format!("can't assign to {}", quoted(kind_name(&target))));
        }
        target
    }

    fn parse_set(&mut self) -> StmtKind {
        self.next_token();
        let target =
            self.parse_assign_target(TargetOptions { with_namespace: true, ..Default::default() });
        if self.at_op("=") {
            self.next_token();
            let value = self.parse_tuple(TupleOptions::default());
            self.expect(TokenKind::BlockEnd, "");
            return StmtKind::Set(target, value);
        }
        let filter = if self.at_op("|") { self.parse_filter(None, false) } else { None };
        self.expect(TokenKind::BlockEnd, "");
        let body = self.subparse(Some(&["endset"]));
        self.end_tag();
        StmtKind::SetBlock(target, filter, body)
    }

    /// Parses "(a, b=1)" into parameter names and default values.
    fn parse_signature(&mut self) -> Vec<(Name, Option<Expr>)> {
        let mut params: Vec<(Name, Option<Expr>)> = Vec::new();
        self.expect(TokenKind::Operator, "(");
        while !self.at_op(")") && !self.failed() {
            if !params.is_empty() {
                self.expect(TokenKind::Operator, ",");
            }
            if self.at_op(")") {
                break;
            }
            let (name, line) = self.expect_name();
            let mut default = None;
            if self.at_op("=") {
                self.next_token();
                default = Some(self.parse_expr(true));
            } else if params.last().is_some_and(|(_, d)| d.is_some()) {
                self.fail(line, "non-default argument follows default argument");
            }
            params.push((name, default));
        }
        self.expect(TokenKind::Operator, ")");
        params
    }

    /// Records whether a macro body reads caller, varargs or kwargs, and stores it.
    fn add_macro(&mut self, mut m: MacroDef) -> MacroId {
        let mut uses = (false, false, false);
        let mut visit = |name: &str| match name {
            "caller" => uses.0 = true,
            "varargs" => uses.1 = true,
            "kwargs" => uses.2 = true,
            _ => {}
        };
        self.walk_stmts(&m.body, &mut visit);
        for (name, _) in &m.params {
            visit(name);
        }
        (m.uses_caller, m.uses_varargs, m.uses_kwargs) = uses;
        let id = MacroId(self.macros.len() as u32);
        self.macros.push(m);
        id
    }

    /// Calls visit with every name the statements read, skipping the bodies of nested
    /// macros and call blocks.
    fn walk_stmts(&self, stmts: &[Stmt], visit: &mut impl FnMut(&str)) {
        for s in stmts {
            match &s.kind {
                StmtKind::Data(..) | StmtKind::Break | StmtKind::Continue => {}
                StmtKind::Output(e) | StmtKind::Extends(e) | StmtKind::Do(e) => {
                    walk_names(e, visit)
                }
                StmtKind::Include { template: e, .. }
                | StmtKind::Import { template: e, .. }
                | StmtKind::FromImport { template: e, .. } => walk_names(e, visit),
                StmtKind::If(e, a, b) => {
                    walk_names(e, visit);
                    self.walk_stmts(a, visit);
                    self.walk_stmts(b, visit);
                }
                StmtKind::For(id) => {
                    let f = &self.loops[id.0 as usize];
                    for e in [&f.target, &f.iter].into_iter().chain(&f.filter) {
                        walk_names(e, visit);
                    }
                    self.walk_stmts(&f.body, visit);
                    self.walk_stmts(&f.else_body, visit);
                }
                StmtKind::Set(a, b) => {
                    walk_names(a, visit);
                    walk_names(b, visit);
                }
                StmtKind::SetBlock(t, f, body) => {
                    walk_names(t, visit);
                    f.iter().for_each(|e| walk_names(e, visit));
                    self.walk_stmts(body, visit);
                }
                StmtKind::Macro(id) => self.walk_defaults(*id, visit),
                StmtKind::CallBlock(id, call) => {
                    self.walk_defaults(*id, visit);
                    walk_names(call, visit);
                }
                StmtKind::FilterBlock(e, body) | StmtKind::Autoescape(e, body) => {
                    walk_names(e, visit);
                    self.walk_stmts(body, visit);
                }
                StmtKind::With(pairs, body) => {
                    pairs.iter().for_each(|(t, v)| {
                        walk_names(t, visit);
                        walk_names(v, visit);
                    });
                    self.walk_stmts(body, visit);
                }
                StmtKind::Block(id) => self.walk_stmts(&self.blocks[id.0 as usize].body, visit),
                StmtKind::Trans(t) => t.vars.iter().for_each(|(_, e)| walk_names(e, visit)),
            }
        }
    }

    fn walk_defaults(&self, id: MacroId, visit: &mut impl FnMut(&str)) {
        for (_, default) in &self.macros[id.0 as usize].params {
            default.iter().for_each(|e| walk_names(e, visit));
        }
    }

    fn parse_macro(&mut self) -> StmtKind {
        self.next_token();
        let (name, _) = self.expect_name();
        let params = self.parse_signature();
        self.expect(TokenKind::BlockEnd, "");
        let depth = std::mem::take(&mut self.loop_depth);
        let body = self.subparse(Some(&["endmacro"]));
        self.loop_depth = depth;
        self.end_tag();
        StmtKind::Macro(self.add_macro(MacroDef { name, params, body, ..MacroDef::default() }))
    }

    fn parse_call_block(&mut self) -> StmtKind {
        let line = self.next_token().line;
        let params = if self.at_op("(") { self.parse_signature() } else { Vec::new() };
        let call = self.parse_expr(true);
        if !self.failed() && !matches!(call.kind, ExprKind::Call(..)) {
            self.fail(line, "expected call");
        }
        self.expect(TokenKind::BlockEnd, "");
        let depth = std::mem::take(&mut self.loop_depth);
        let body = self.subparse(Some(&["endcall"]));
        self.loop_depth = depth;
        self.end_tag();
        let def = MacroDef { name: Rc::from("caller"), params, body, ..MacroDef::default() };
        StmtKind::CallBlock(self.add_macro(def), call)
    }

    fn parse_filter_block(&mut self) -> StmtKind {
        let line = self.next_token().line;
        let filter = self.parse_filter(None, true);
        self.expect(TokenKind::BlockEnd, "");
        let body = self.subparse(Some(&["endfilter"]));
        self.end_tag();
        let none = || Expr { line, kind: ExprKind::Const(crate::value::Value::None) };
        StmtKind::FilterBlock(filter.unwrap_or_else(none), body)
    }

    fn parse_with(&mut self) -> StmtKind {
        self.next_token();
        let mut pairs = Vec::new();
        while self.peek(0).kind != TokenKind::BlockEnd && !self.failed() {
            if !pairs.is_empty() {
                self.expect(TokenKind::Operator, ",");
            }
            let target = self.parse_assign_target(TargetOptions::default());
            self.expect(TokenKind::Operator, "=");
            pairs.push((target, self.parse_expr(true)));
        }
        self.expect(TokenKind::BlockEnd, "");
        let body = self.subparse(Some(&["endwith"]));
        self.end_tag();
        StmtKind::With(pairs, body)
    }

    fn parse_block(&mut self) -> StmtKind {
        let line = self.next_token().line;
        let (name, _) = self.expect_name();
        let scoped = self.skip_if_name("scoped");
        let required = self.skip_if_name("required");
        if self.failed() {
            return StmtKind::Break;
        }
        if self.blocks.iter().any(|b| b.name == name) {
            self.fail(line, format!("block {} defined twice", quoted(&name)));
            return StmtKind::Break;
        }
        self.expect(TokenKind::BlockEnd, "");
        let depth = std::mem::take(&mut self.loop_depth);
        let body = self.subparse(Some(&["endblock"]));
        self.loop_depth = depth;
        self.next_token();
        let t = self.peek(0).clone();
        if t.kind == TokenKind::Name {
            if *t.text == *name {
                self.next_token();
            } else {
                let got = quoted(&t.text);
                let message =
                    format!("mismatching name of block. Expected {}, got {got}", quoted(&name));
                self.fail(t.line, message);
            }
        }
        self.expect(TokenKind::BlockEnd, "");
        if required {
            for s in &body {
                let blank = match s.kind {
                    StmtKind::Data(a, b) => self.src[a..b].trim().is_empty(),
                    _ => false,
                };
                if !blank {
                    let line = s.line;
                    self.fail(line, "Required blocks can only contain comments or whitespace");
                }
            }
        }
        let id = BlockId(self.blocks.len() as u32);
        self.blocks.push(BlockDef { name, body, scoped, required });
        StmtKind::Block(id)
    }

    fn skip_if_name(&mut self, name: &str) -> bool {
        let found = self.at_name(name, 0);
        if found {
            self.next_token();
        }
        found
    }

    /// Parses extends, do and autoescape, which take one expression.
    fn parse_simple(&mut self) -> StmtKind {
        let t = self.next_token();
        if t.text == "do" {
            let e = self.parse_tuple(TupleOptions::default());
            self.expect(TokenKind::BlockEnd, "");
            return StmtKind::Do(e);
        }
        let e = self.parse_expr(true);
        self.expect(TokenKind::BlockEnd, "");
        if t.text == "extends" {
            return StmtKind::Extends(e);
        }
        let body = self.subparse(Some(&["endautoescape"]));
        self.end_tag();
        StmtKind::Autoescape(e, body)
    }

    /// Reads "with context" or "without context" into with_context.
    fn parse_context(&mut self, with_context: &mut bool) -> bool {
        if (self.at_name("with", 0) || self.at_name("without", 0)) && self.at_name("context", 1) {
            *with_context = self.next_token().text == "with";
            self.next_token();
            return true;
        }
        false
    }

    fn parse_include(&mut self) -> StmtKind {
        self.next_token();
        let template = self.parse_expr(true);
        let mut ignore_missing = false;
        if self.at_name("ignore", 0) && self.at_name("missing", 1) {
            self.next_token();
            self.next_token();
            ignore_missing = true;
        }
        let mut with_context = true;
        self.parse_context(&mut with_context);
        self.expect(TokenKind::BlockEnd, "");
        StmtKind::Include { template, ignore_missing, with_context }
    }

    fn parse_import(&mut self) -> StmtKind {
        self.next_token();
        let template = self.parse_expr(true);
        self.expect(TokenKind::Name, "as");
        let (name, _) = self.expect_name();
        let mut with_context = false;
        self.parse_context(&mut with_context);
        self.expect(TokenKind::BlockEnd, "");
        StmtKind::Import { template, name, with_context }
    }

    fn parse_from(&mut self) -> StmtKind {
        self.next_token();
        let template = self.parse_expr(true);
        self.expect(TokenKind::Name, "import");
        let mut names = Vec::new();
        let mut with_context = false;
        while !self.failed() {
            if !names.is_empty() {
                self.expect(TokenKind::Operator, ",");
            }
            if self.peek(0).kind != TokenKind::Name {
                self.expect(TokenKind::Name, "");
                break;
            }
            if self.parse_context(&mut with_context) {
                break;
            }
            let (name, line) = self.expect_name();
            if name.starts_with('_') {
                self.fail(line, "names starting with an underline can not be imported");
            }
            let mut alias = name.clone();
            if self.at_name("as", 0) {
                self.next_token();
                alias = self.expect_name().0;
            }
            names.push((name, alias));
            if self.parse_context(&mut with_context) || !self.at_op(",") {
                break;
            }
        }
        self.expect(TokenKind::BlockEnd, "");
        StmtKind::FromImport { template, names, with_context }
    }
}

fn can_assign(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Name(n) => {
            !matches!(&**n, "true" | "false" | "none" | "True" | "False" | "None")
        }
        ExprKind::Tuple(items) | ExprKind::List(items) => items.iter().all(can_assign),
        ExprKind::NsRef(..) => true,
        _ => false,
    }
}

fn kind_name(e: &Expr) -> &'static str {
    match e.kind {
        ExprKind::Const(_) => "Const",
        ExprKind::Call(..) => "Call",
        ExprKind::Getattr(..) => "Getattr",
        ExprKind::Getitem(..) => "Getitem",
        ExprKind::Binary(..) | ExprKind::Unary(..) => "BinExpr",
        _ => "expression",
    }
}
