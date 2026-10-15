//! The parsed form of a template. Expressions and statements are trees of `Box`es; the
//! definitions a render refers back to (macros, blocks, for loops) live in side tables
//! of the [`Compiled`] template and are named by id.

use std::rc::Rc;

use crate::value::Value;

/// An identifier: shared, because renders store variables under it.
pub(crate) type Name = Rc<str>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MacroId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BlockId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LoopId(pub u32);

#[derive(Debug)]
pub(crate) struct Expr {
    pub line: usize,
    pub kind: ExprKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnaryOp {
    Not,
    Neg,
    Pos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BinOp {
    And,
    Or,
    Add,
    Sub,
    Mul,
    Div,
    FloorDiv,
    Mod,
    Pow,
}

impl BinOp {
    pub(crate) fn symbol(self) -> &'static str {
        match self {
            BinOp::And => "and",
            BinOp::Or => "or",
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::FloorDiv => "//",
            BinOp::Mod => "%",
            BinOp::Pow => "**",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    In,
    NotIn,
}

impl CmpOp {
    pub(crate) fn symbol(self) -> &'static str {
        match self {
            CmpOp::Eq => "==",
            CmpOp::Ne => "!=",
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
            CmpOp::In => "in",
            CmpOp::NotIn => "not in",
        }
    }
}

/// The arguments of a call, filter or test: `(a, name=b, *c, **d)`.
#[derive(Debug, Default)]
pub(crate) struct Args {
    pub args: Vec<Expr>,
    pub kwargs: Vec<(Name, Expr)>,
    pub star: Option<Expr>,
    pub dstar: Option<Expr>,
}

#[derive(Debug)]
pub(crate) enum ExprKind {
    Const(Value),
    Name(Name),
    List(Vec<Expr>),
    Tuple(Vec<Expr>),
    Dict(Vec<(Expr, Expr)>),
    Getattr(Box<Expr>, Name),
    Getitem(Box<Expr>, Box<Expr>),
    /// `a:b:c`, each part optional; only inside a subscript.
    Slice(Option<Box<Expr>>, Option<Box<Expr>>, Option<Box<Expr>>),
    Call(Box<Expr>, Box<Args>),
    /// `a | name(args)`; the operand is None in filter blocks and `set` blocks.
    Filter(Option<Box<Expr>>, Name, Box<Args>),
    Test(Box<Expr>, Name, Box<Args>),
    Unary(UnaryOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Concat(Vec<Expr>),
    Compare(Box<Expr>, Vec<(CmpOp, Expr)>),
    /// `then if test else otherwise`.
    Cond {
        test: Box<Expr>,
        then: Box<Expr>,
        otherwise: Option<Box<Expr>>,
    },
    /// `ns.attr` as an assignment target.
    NsRef(Name, Name),
}

#[derive(Debug)]
pub(crate) struct Stmt {
    pub line: usize,
    pub kind: StmtKind,
}

#[derive(Debug)]
pub(crate) enum StmtKind {
    /// Template text: a byte range of the compiled template's source.
    Data(usize, usize),
    Output(Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    For(LoopId),
    Set(Expr, Expr),
    SetBlock(Expr, Option<Expr>, Vec<Stmt>),
    Macro(MacroId),
    CallBlock(MacroId, Expr),
    FilterBlock(Expr, Vec<Stmt>),
    With(Vec<(Expr, Expr)>, Vec<Stmt>),
    Block(BlockId),
    Extends(Expr),
    Include {
        template: Expr,
        ignore_missing: bool,
        with_context: bool,
    },
    Import {
        template: Expr,
        name: Name,
        with_context: bool,
    },
    FromImport {
        template: Expr,
        names: Vec<(Name, Name)>,
        with_context: bool,
    },
    Autoescape(Expr, Vec<Stmt>),
    Do(Expr),
    Break,
    Continue,
    Trans(Box<Trans>),
}

/// `for target in iter if filter [recursive]: body else else_body`.
#[derive(Debug)]
pub(crate) struct ForDef {
    pub target: Expr,
    pub iter: Expr,
    pub filter: Option<Expr>,
    pub recursive: bool,
    pub body: Vec<Stmt>,
    pub else_body: Vec<Stmt>,
}

/// A macro, or the body of a call block (named "caller").
#[derive(Debug, Default)]
pub(crate) struct MacroDef {
    pub name: Name,
    pub params: Vec<(Name, Option<Expr>)>,
    pub body: Vec<Stmt>,
    pub uses_caller: bool,
    pub uses_varargs: bool,
    pub uses_kwargs: bool,
}

#[derive(Debug)]
pub(crate) struct BlockDef {
    pub name: Name,
    pub body: Vec<Stmt>,
    pub scoped: bool,
    pub required: bool,
}

/// `{% trans %}`: the singular (and plural) text with `%(name)s` placeholders.
#[derive(Debug, Default)]
pub(crate) struct Trans {
    pub singular: String,
    pub plural: Option<String>,
    /// The index in vars of the count that chooses the plural.
    pub count: Option<usize>,
    pub vars: Vec<(Name, Expr)>,
}

/// A parsed template, shared by the environment's cache and the renders using it.
#[derive(Debug)]
pub(crate) struct Compiled {
    pub name: Rc<str>,
    pub source: Box<str>,
    pub body: Vec<Stmt>,
    pub macros: Vec<MacroDef>,
    pub blocks: Vec<BlockDef>,
    pub loops: Vec<ForDef>,
    pub autoescape: bool,
}

impl Compiled {
    pub(crate) fn macro_def(&self, id: MacroId) -> &MacroDef {
        &self.macros[id.0 as usize]
    }

    pub(crate) fn block(&self, id: BlockId) -> &BlockDef {
        &self.blocks[id.0 as usize]
    }

    pub(crate) fn loop_def(&self, id: LoopId) -> &ForDef {
        &self.loops[id.0 as usize]
    }

    pub(crate) fn data(&self, start: usize, end: usize) -> &str {
        &self.source[start..end]
    }
}

/// Calls visit with every name an expression reads.
pub(crate) fn walk_names(e: &Expr, visit: &mut impl FnMut(&str)) {
    let args = |a: &Args, visit: &mut dyn FnMut(&Expr)| {
        a.args.iter().chain(a.kwargs.iter().map(|(_, e)| e)).for_each(&mut *visit);
        a.star.iter().chain(a.dstar.iter()).for_each(visit);
    };
    match &e.kind {
        ExprKind::Const(_) => {}
        ExprKind::Name(n) => visit(n),
        ExprKind::List(xs) | ExprKind::Tuple(xs) | ExprKind::Concat(xs) => {
            xs.iter().for_each(|x| walk_names(x, visit));
        }
        ExprKind::Dict(pairs) => pairs.iter().for_each(|(k, v)| {
            walk_names(k, visit);
            walk_names(v, visit);
        }),
        ExprKind::Getattr(a, _) | ExprKind::Unary(_, a) => walk_names(a, visit),
        ExprKind::Getitem(a, b) | ExprKind::Binary(_, a, b) => {
            walk_names(a, visit);
            walk_names(b, visit);
        }
        ExprKind::Slice(a, b, c) => {
            [a, b, c].into_iter().flatten().for_each(|x| walk_names(x, visit));
        }
        ExprKind::Call(a, xs) | ExprKind::Test(a, _, xs) => {
            walk_names(a, visit);
            args(xs, &mut |x| walk_names(x, visit));
        }
        ExprKind::Filter(a, _, xs) => {
            a.iter().for_each(|x| walk_names(x, visit));
            args(xs, &mut |x| walk_names(x, visit));
        }
        ExprKind::Compare(a, rest) => {
            walk_names(a, visit);
            rest.iter().for_each(|(_, x)| walk_names(x, visit));
        }
        ExprKind::Cond { test, then, otherwise } => {
            walk_names(test, visit);
            walk_names(then, visit);
            otherwise.iter().for_each(|x| walk_names(x, visit));
        }
        ExprKind::NsRef(ns, _) => visit(ns),
    }
}
