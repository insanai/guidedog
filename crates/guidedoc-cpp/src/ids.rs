//! The ids of C++ declarations, as `get_id` gives them in Sphinx's C++ domain: version
//! 1 (Sphinx 1.2), and versions 2 to 4 after the Itanium C++ ABI's mangling. Each
//! function appends to `out` and returns `None` where Sphinx raises `NoOldIdError` (or
//! cannot map a name to an id); `out` then holds a partial id the caller drops.

use crate::ast::{Ast, Node, NodeId};
use crate::path::Element;

/// The prefix of a declaration's id in each version (index 0 unused).
pub(crate) const ID_PREFIX: [&str; 5] = ["", "", "_CPPv2", "_CPPv3", "_CPPv4"];

/// The newest id version.
pub const MAX_ID_VERSION: usize = 4;

/// The outcome of writing an id: `None` when there is none in the version.
pub(crate) type Done = Option<()>;

const FUNDAMENTAL_V1: [(&str, &str); 10] = [
    ("char", "c"),
    ("signed char", "c"),
    ("unsigned char", "C"),
    ("int", "i"),
    ("signed int", "i"),
    ("unsigned int", "U"),
    ("long", "l"),
    ("signed long", "l"),
    ("unsigned long", "L"),
    ("bool", "b"),
];

pub(crate) const SHORTHANDS_V1: [(&str, &str); 6] = [
    ("std::string", "ss"),
    ("std::ostream", "os"),
    ("std::istream", "is"),
    ("std::iostream", "ios"),
    ("std::vector", "v"),
    ("std::map", "m"),
];

const OPERATOR_V1: [(&str, &str); 42] = [
    ("new", "new-operator"),
    ("new[]", "new-array-operator"),
    ("delete", "delete-operator"),
    ("delete[]", "delete-array-operator"),
    ("~", "inv-operator"),
    ("+", "add-operator"),
    ("-", "sub-operator"),
    ("*", "mul-operator"),
    ("/", "div-operator"),
    ("%", "mod-operator"),
    ("&", "and-operator"),
    ("|", "or-operator"),
    ("^", "xor-operator"),
    ("=", "assign-operator"),
    ("+=", "add-assign-operator"),
    ("-=", "sub-assign-operator"),
    ("*=", "mul-assign-operator"),
    ("/=", "div-assign-operator"),
    ("%=", "mod-assign-operator"),
    ("&=", "and-assign-operator"),
    ("|=", "or-assign-operator"),
    ("^=", "xor-assign-operator"),
    ("<<", "lshift-operator"),
    (">>", "rshift-operator"),
    ("<<=", "lshift-assign-operator"),
    (">>=", "rshift-assign-operator"),
    ("==", "eq-operator"),
    ("!=", "neq-operator"),
    ("<", "lt-operator"),
    (">", "gt-operator"),
    ("<=", "lte-operator"),
    (">=", "gte-operator"),
    ("!", "not-operator"),
    ("&&", "sand-operator"),
    ("||", "sor-operator"),
    ("++", "inc-operator"),
    ("--", "dec-operator"),
    (",", "comma-operator"),
    ("->*", "pointer-by-pointer-operator"),
    ("->", "pointer-operator"),
    ("()", "call-operator"),
    ("[]", "subscript-operator"),
];

const FUNDAMENTAL_V2: [(&str, &str); 52] = [
    ("void", "v"),
    ("bool", "b"),
    ("char", "c"),
    ("signed char", "a"),
    ("unsigned char", "h"),
    ("wchar_t", "w"),
    ("char32_t", "Di"),
    ("char16_t", "Ds"),
    ("char8_t", "Du"),
    ("short", "s"),
    ("short int", "s"),
    ("signed short", "s"),
    ("signed short int", "s"),
    ("unsigned short", "t"),
    ("unsigned short int", "t"),
    ("int", "i"),
    ("signed", "i"),
    ("signed int", "i"),
    ("unsigned", "j"),
    ("unsigned int", "j"),
    ("long", "l"),
    ("long int", "l"),
    ("signed long", "l"),
    ("signed long int", "l"),
    ("unsigned long", "m"),
    ("unsigned long int", "m"),
    ("long long", "x"),
    ("long long int", "x"),
    ("signed long long", "x"),
    ("signed long long int", "x"),
    ("__int64", "x"),
    ("unsigned long long", "y"),
    ("unsigned long long int", "y"),
    ("__int128", "n"),
    ("signed __int128", "n"),
    ("unsigned __int128", "o"),
    ("float", "f"),
    ("double", "d"),
    ("long double", "e"),
    ("__float80", "e"),
    ("_Float64x", "e"),
    ("__float128", "g"),
    ("_Float128", "g"),
    ("_Complex float", "Cf"),
    ("_Complex double", "Cd"),
    ("_Complex long double", "Ce"),
    ("_Imaginary float", "f"),
    ("_Imaginary double", "d"),
    ("_Imaginary long double", "e"),
    ("auto", "Da"),
    ("decltype(auto)", "Dc"),
    ("std::nullptr_t", "Dn"),
];

pub(crate) const OPERATOR_V2: [(&str, &str); 56] = [
    ("new", "nw"),
    ("new[]", "na"),
    ("delete", "dl"),
    ("delete[]", "da"),
    ("~", "co"),
    ("compl", "co"),
    ("+", "pl"),
    ("-", "mi"),
    ("*", "ml"),
    ("/", "dv"),
    ("%", "rm"),
    ("&", "an"),
    ("bitand", "an"),
    ("|", "or"),
    ("bitor", "or"),
    ("^", "eo"),
    ("xor", "eo"),
    ("=", "aS"),
    ("+=", "pL"),
    ("-=", "mI"),
    ("*=", "mL"),
    ("/=", "dV"),
    ("%=", "rM"),
    ("&=", "aN"),
    ("and_eq", "aN"),
    ("|=", "oR"),
    ("or_eq", "oR"),
    ("^=", "eO"),
    ("xor_eq", "eO"),
    ("<<", "ls"),
    (">>", "rs"),
    ("<<=", "lS"),
    (">>=", "rS"),
    ("==", "eq"),
    ("!=", "ne"),
    ("not_eq", "ne"),
    ("<", "lt"),
    (">", "gt"),
    ("<=", "le"),
    (">=", "ge"),
    ("<=>", "ss"),
    ("!", "nt"),
    ("not", "nt"),
    ("&&", "aa"),
    ("and", "aa"),
    ("||", "oo"),
    ("or", "oo"),
    ("++", "pp"),
    ("--", "mm"),
    (",", "cm"),
    ("->*", "pm"),
    ("->", "pt"),
    ("()", "cl"),
    ("[]", "ix"),
    (".*", "ds"),
    ("?", "qu"),
];

const UNARY_V2: [(&str, &str); 10] = [
    ("++", "pp_"),
    ("--", "mm_"),
    ("*", "de"),
    ("&", "ad"),
    ("+", "ps"),
    ("-", "ng"),
    ("!", "nt"),
    ("not", "nt"),
    ("~", "co"),
    ("compl", "co"),
];

const CASTS_ID: [(&str, &str); 4] = [
    ("dynamic_cast", "dc"),
    ("static_cast", "sc"),
    ("const_cast", "cc"),
    ("reinterpret_cast", "rc"),
];

/// Looks a key up in one of the tables.
pub(crate) fn lookup(table: &[(&str, &'static str)], key: &str) -> Option<&'static str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

impl Ast<'_> {
    /// Returns the id of a name, type, template argument, or expression in `version`
    /// (1 to 4), as Sphinx's `get_id` gives it for that node; `None` where the version
    /// has none.
    pub fn node_id(&self, id: NodeId, version: usize) -> Option<String> {
        Ids { ast: self, version, elements: &[], placeholder: false }.piece(id)
    }
}

/// Writes the ids of one version.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ids<'t, 'a> {
    pub ast: &'t Ast<'a>,
    pub version: usize,
    /// The declared symbol's full nested name, from the root.
    pub elements: &'t [Element],
    /// Write the full name as `\x1d` modifiers `\x1d`, for the host to fill in.
    pub placeholder: bool,
}

impl Ids<'_, '_> {
    /// Returns the id of a node as a new string.
    pub fn piece(&self, id: NodeId) -> Option<String> {
        let mut out = String::new();
        self.id(id, &mut out)?;
        Some(out)
    }

    /// Writes the id of a name, expression, template argument, or type (a type's
    /// encoding, as `get_id` without an object type gives it).
    pub fn id(&self, id: NodeId, out: &mut String) -> Done {
        let v = self.version;
        match self.ast.node(id) {
            Node::Identifier(text) => identifier_id(text, v, out)?,
            Node::OpBuiltin(op) => {
                let table: &[_] = if v == 1 { &OPERATOR_V1 } else { &OPERATOR_V2 };
                out.push_str(lookup(table, op)?);
            }
            Node::OpLiteral(ident) => {
                if v == 1 {
                    return None;
                }
                out.push_str("li");
                self.id(*ident, out)?;
            }
            Node::OpType(ty) => {
                out.push_str(if v == 1 { "castto-" } else { "cv" });
                self.id(*ty, out)?;
                if v == 1 {
                    out.push_str("-operator");
                }
            }
            Node::NameElement { name, args, .. } => {
                self.id(*name, out)?;
                if let Some(args) = args {
                    self.id(*args, out)?;
                }
            }
            Node::NestedName { elements, .. } => self.nested_id(*elements, id, out)?,
            Node::TemplateArgs { args, is_pack } => self.template_args_id(*args, *is_pack, out)?,
            Node::ArgConstant(expr) => self.constant_id(id, *expr, out)?,
            Node::Type { specs, decl } => self.type_encoding(*specs, *decl, out)?,
            Node::TypeWithInit { ty, .. } => self.id(*ty, out)?, // as a parameter: the type.
            Node::ConstrainedInit { ty, .. } => {
                if v < 2 {
                    return None;
                }
                self.id(*ty, out)?;
            }
            Node::FunctionParam(arg) => match arg {
                Some(arg) => self.id(*arg, out)?,
                None => out.push('z'),
            },
            Node::Fundamental { .. }
            | Node::DecltypeAuto
            | Node::Decltype(_)
            | Node::TypeName { .. }
            | Node::DeclSpecs { .. } => self.specs_id(id, out)?,
            _ => self.expression_id(id, out)?,
        }
        Some(())
    }

    /// Version 1 joins the elements' ids with `::` (or uses a shorthand); later versions
    /// wrap several elements in `N...E`.
    fn nested_id(&self, elements: Option<NodeId>, name: NodeId, out: &mut String) -> Done {
        if self.version == 1 {
            if let Some(short) = lookup(&SHORTHANDS_V1, &self.ast.text(name, false)) {
                out.push_str(short);
                return Some(());
            }
            for (i, e) in self.ast.list(elements).enumerate() {
                if i > 0 {
                    out.push_str("::");
                }
                self.id(e, out)?;
            }
            return Some(());
        }
        let wrap = self.ast.count(elements) > 1;
        if wrap {
            out.push('N');
        }
        for e in self.ast.list(elements) {
            self.id(e, out)?;
        }
        if wrap {
            out.push('E');
        }
        Some(())
    }

    fn template_args_id(&self, args: Option<NodeId>, is_pack: bool, out: &mut String) -> Done {
        if self.version == 1 {
            out.push(':');
            for (i, a) in self.ast.list(args).enumerate() {
                if i > 0 {
                    out.push('.');
                }
                self.id(a, out)?;
            }
            out.push(':');
            return Some(());
        }
        out.push('I');
        let last = self.ast.last(args);
        for a in self.ast.list(args).take_while(|&a| Some(a) != last) {
            self.id(a, out)?;
        }
        if let Some(last) = last {
            if is_pack {
                out.push('J');
            }
            self.id(last, out)?;
            if is_pack {
                out.push('E');
            }
        }
        out.push('E');
        Some(())
    }

    fn constant_id(&self, arg: NodeId, expr: NodeId, out: &mut String) -> Done {
        match self.version {
            1 => out
                .extend(self.ast.text(arg, false).chars().map(|c| if c == ' ' { '-' } else { c })),
            2 => {
                out.push('X');
                self.ast.write_text(arg, false, out);
                out.push('E');
            }
            _ => {
                out.push('X');
                self.id(expr, out)?;
                out.push('E');
            }
        }
        Some(())
    }

    /// Declaration specifiers and trailing type specifiers.
    fn specs_id(&self, id: NodeId, out: &mut String) -> Done {
        let v1 = self.version == 1;
        match self.ast.node(id) {
            Node::Fundamental { canonical, .. } => {
                if v1 {
                    for (i, word) in canonical.split(' ').filter(|w| !w.is_empty()).enumerate() {
                        if i > 0 {
                            out.push('-');
                        }
                        out.push_str(lookup(&FUNDAMENTAL_V1, word).unwrap_or(word));
                    }
                } else {
                    out.push_str(lookup(&FUNDAMENTAL_V2, canonical)?);
                }
            }
            Node::DecltypeAuto => {
                if v1 {
                    return None;
                }
                out.push_str("Dc");
            }
            Node::Decltype(expr) => {
                if v1 {
                    return None;
                }
                out.push_str("DT");
                self.id(*expr, out)?;
                out.push('E');
            }
            Node::TypeName { name, .. } => self.id(*name, out)?,
            Node::DeclSpecs { trailing, .. } => {
                let (is_const, is_volatile) = self.ast.specs_cv(id);
                if v1 && let Some(trailing) = trailing {
                    self.id(*trailing, out)?;
                }
                if is_volatile {
                    out.push('V');
                }
                if is_const {
                    out.push(if v1 { 'C' } else { 'K' });
                }
                if let (false, Some(trailing)) = (v1, trailing) {
                    self.id(*trailing, out)?;
                }
            }
            _ => {}
        }
        Some(())
    }

    /// Version 1 writes the specifiers, pointer suffixes, and parameters (no function
    /// types); later versions the declarator's type around the specifiers' id.
    fn type_encoding(&self, specs: NodeId, decl: NodeId, out: &mut String) -> Done {
        if self.version == 1 {
            if self.ast.is_function_type(decl) {
                return None;
            }
            self.id(specs, out)?;
            self.ptr_suffix_id(decl, out)?;
            return self.param_id(decl, out);
        }
        let ret = self.piece(specs)?;
        self.type_id(decl, &ret, out)
    }

    /// Writes an expression's id (versions after 1).
    fn expression_id(&self, id: NodeId, out: &mut String) -> Done {
        match self.ast.node(id) {
            Node::LitPointer => out.push_str("LDnE"),
            Node::LitBool(value) => out.push_str(if *value { "L1E" } else { "L0E" }),
            Node::LitNumber(text) => {
                out.push('L');
                out.extend(text.chars().filter(|&c| c != '\''));
                out.push('E');
            }
            Node::LitString(text) => {
                out.push_str("LA");
                out.push_str(&text.len().saturating_sub(2).to_string());
                out.push_str("_KcE");
            }
            Node::LitChar { prefix, value, .. } => {
                out.push_str(match *prefix {
                    "u" => "Ds",
                    "U" => "Di",
                    "L" => "w",
                    _ => "c",
                });
                out.push_str(&value.to_string());
            }
            Node::LitUser { literal, suffix } => {
                out.push_str("clL_Zli");
                self.id(*suffix, out)?;
                out.push('E');
                self.id(*literal, out)?;
                out.push('E');
            }
            Node::This => out.push_str("fpT"),
            Node::Fold { left, op, right } => self.fold_id(id, *left, op, *right, out)?,
            Node::Paren(inner) | Node::IdExpr(inner) => self.id(*inner, out)?,
            Node::Postfix { prefix, ops } => self.postfix_id(*prefix, *ops, out)?,
            Node::ExplicitCast { cast, ty, expr } => {
                out.push_str(lookup(&CASTS_ID, cast)?);
                self.id(*ty, out)?;
                self.id(*expr, out)?;
            }
            Node::TypeId { operand, is_type } => {
                out.push_str(if *is_type { "ti" } else { "te" });
                self.id(*operand, out)?;
            }
            Node::Unary { op, operand } => {
                out.push_str(lookup(&UNARY_V2, op)?);
                self.id(*operand, out)?;
            }
            Node::SizeofPack(x) => self.prefixed_id("sZ", *x, out)?,
            Node::SizeofType(x) => self.prefixed_id("st", *x, out)?,
            Node::SizeofExpr(x) => self.prefixed_id("sz", *x, out)?,
            Node::Alignof(x) => self.prefixed_id("at", *x, out)?,
            Node::NoexceptExpr(x) => self.prefixed_id("nx", *x, out)?,
            _ => return self.expression_id_more(id, out),
        }
        Some(())
    }

    fn prefixed_id(&self, prefix: &str, id: NodeId, out: &mut String) -> Done {
        out.push_str(prefix);
        self.id(id, out)
    }

    fn expression_id_more(&self, id: NodeId, out: &mut String) -> Done {
        match self.ast.node(id) {
            Node::New { ty, init, .. } => {
                out.push_str("nw_");
                self.id(*ty, out)?;
                match init {
                    Some(init) => self.id(*init, out)?,
                    None => out.push('E'),
                }
            }
            Node::Delete { is_array, operand, .. } => {
                self.prefixed_id(if *is_array { "da" } else { "dl" }, *operand, out)?;
            }
            Node::Cast { ty, operand } => {
                out.push_str("cv");
                self.id(*ty, out)?;
                self.id(*operand, out)?;
            }
            Node::BinOp { first, rest } => self.bin_op_id(*first, *rest, out)?,
            Node::Comma(items) => {
                if self.version < 2 {
                    return None;
                }
                for item in self.ast.list(*items) {
                    if self.ast.next(item).is_some() {
                        out.push_str("cm");
                    }
                    self.id(item, out)?;
                }
            }
            Node::Conditional { cond, then, other } => {
                if self.version < 2 {
                    return None;
                }
                out.push_str("qu");
                self.id(*cond, out)?;
                self.id(*then, out)?;
                self.id(*other, out)?;
            }
            Node::BracedInit { items, .. } | Node::ParenList(items) => {
                let braced = matches!(self.ast.node(id), Node::BracedInit { .. });
                out.push_str(if braced { "il" } else { "pi" });
                for item in self.ast.list(*items) {
                    self.id(item, out)?;
                }
                out.push('E');
            }
            Node::Assignment { left, op, right } => {
                out.push_str(lookup(&OPERATOR_V2, op)?);
                self.id(*left, out)?;
                self.id(*right, out)?;
            }
            Node::Fallback(text) => out.push_str(text),
            Node::PackExpansion(expr) => self.prefixed_id("sp", *expr, out)?,
            _ => return None,
        }
        Some(())
    }

    /// Each operand after the first puts its operator before the operand it follows,
    /// as Sphinx's `ASTBinOpExpr.get_id` does.
    fn bin_op_id(&self, first: NodeId, rest: Option<NodeId>, out: &mut String) -> Done {
        if self.version < 2 {
            return None;
        }
        let mut prev = first;
        for item in self.ast.list(rest) {
            let Node::BinOperand { op, expr } = *self.ast.node(item) else { return None };
            out.push_str(lookup(&OPERATOR_V2, op)?);
            self.id(prev, out)?;
            prev = expr;
        }
        self.id(prev, out)
    }

    fn fold_id(
        &self,
        id: NodeId,
        left: Option<NodeId>,
        op: &str,
        right: Option<NodeId>,
        out: &mut String,
    ) -> Done {
        match self.version {
            0..=2 => return None,
            3 => {
                self.ast.write_text(id, false, out);
                return Some(());
            }
            _ => {}
        }
        out.push_str(match (left, right) {
            (None, _) => "fl",
            (_, None) => "fr",
            _ => "fL",
        });
        out.push_str(lookup(&OPERATOR_V2, op)?);
        for side in [left, right].into_iter().flatten() {
            self.id(side, out)?;
        }
        Some(())
    }

    /// Writes the operations from the first: each wraps the id before it.
    fn postfix_id(&self, prefix: NodeId, ops: Option<NodeId>, out: &mut String) -> Done {
        let mut current = self.piece(prefix)?;
        for op in self.ast.list(ops) {
            let node = self.ast.node(op);
            let mut next = String::from(match node {
                Node::PfArray(_) => "ix",
                Node::PfMember(_) => "dt",
                Node::PfMemberPtr(_) => "pt",
                Node::PfInc => "pp",
                Node::PfDec => "mm",
                Node::PfCall(_) => "cl",
                _ => "",
            });
            next.push_str(&current);
            match *node {
                Node::PfArray(x) | Node::PfMember(x) | Node::PfMemberPtr(x) => {
                    self.id(x, &mut next)?;
                }
                Node::PfCall(list) => {
                    let items = match *self.ast.node(list) {
                        Node::ParenList(items) | Node::BracedInit { items, .. } => items,
                        _ => None,
                    };
                    for item in self.ast.list(items) {
                        self.id(item, &mut next)?;
                    }
                    next.push('E');
                }
                _ => {}
            }
            current = next;
        }
        out.push_str(&current);
        Some(())
    }
}

/// Writes an identifier's id; anonymous entities have none before version 3.
fn identifier_id(name: &str, v: usize, out: &mut String) -> Done {
    let anonymous = name.starts_with('@');
    if anonymous && v < 3 {
        return None;
    }
    if v == 1 {
        out.push_str(if name == "size_t" { "s" } else { name });
    } else if name == "std" {
        out.push_str("St");
    } else if name.starts_with('~') {
        out.push_str("D0");
    } else if anonymous {
        out.push_str("Ut");
        out.push_str(&(name.len() - 1).to_string());
        out.push('_');
        out.push_str(&name[1..]);
    } else {
        out.push_str(&name.len().to_string());
        out.push_str(name);
    }
    Some(())
}
