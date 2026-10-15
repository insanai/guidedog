//! C++ declarations read as Sphinx's C++ domain reads them (`sphinx/domains/cpp`, Sphinx
//! 9.1): a recursive-descent parser builds the tree of a declaration, from which come
//! the ids of every id version (1 to 4), the names and display strings, and the symbol
//! path the declaration declares. The crate is std-only and knows nothing of documents:
//! the Sphinx layer (`guidedoc-sphinx`) turns its plain data into nodes.
//!
//! The one-call paths:
//!
//! - `parse_declaration` then `Declaration::symbol` for a `cpp:*` directive's signature;
//! - `parse_xref_target` for a cross-reference's target;
//! - `namespace_path` for `cpp:namespace` and `cpp:namespace-push`;
//! - `parse_expression_or_type` for `:cpp:expr:` and `:cpp:texpr:`.
//!
//! Every parse is bounded: nesting by `Limits::max_depth`, machine stack by
//! `Limits::stack_bytes` (a stack guard measures it), and backtracking by
//! `STEPS_PER_BYTE` steps per byte of the definition. Past any of them the parse fails
//! with an `ErrorKind::Limit`, never a stack overflow.

mod ast;
mod error;
mod ids;
mod ids_decl;
mod lex;
mod parse_decl;
mod parse_expr;
mod parse_top;
mod parser;
mod path;
mod query;
mod stack;
mod text;

pub use ast::{Ast, Directive, List, ListIter, Node, NodeId, ObjectType, ParamQual, SimpleSpecs};
pub use error::{CppError, ErrorKind, Stop};
pub use ids::MAX_ID_VERSION;
pub use parser::STEPS_PER_BYTE;
pub use path::{
    Element, FIELD, NO_ID, decode_path, display_name, encode_path, full_name, path_count,
};
pub use text::is_spaced_operator;

use ids::Ids;
use parser::Parser;

/// The bounds of one parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// How deep templates, declarators, and expressions may nest.
    pub max_depth: usize,
    /// The machine stack the parse may use, in bytes. A caller that runs inside a
    /// guarded recursion of its own passes what remains of its budget.
    pub stack_bytes: usize,
}

impl Default for Limits {
    /// Returns the engine's defaults: depth 256 and 512 KiB of stack.
    fn default() -> Limits {
        Limits { max_depth: 256, stack_bytes: 512 << 10 }
    }
}

/// A parsed declaration: its tree and the `Declaration` node at its root.
#[derive(Debug, Clone, PartialEq)]
pub struct Declaration<'a> {
    /// The tree; it borrows the definition.
    pub ast: Ast<'a>,
    /// The `Node::Declaration`.
    pub root: NodeId,
}

/// What a declaration declares, as `CPPObject.handle_signature` and
/// `add_target_and_index` compute it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Symbol {
    /// The declaration's ids in versions 1 to 4 (index 0 is version 1), each with its
    /// `_CPPvN` prefix; version 4 is always there. Sphinx adds them newest first.
    pub ids: [Option<String>; MAX_ID_VERSION],
    /// The symbol's path from the root: the scope's elements, then the declared
    /// name's; the last element's `decl` is the newest id.
    pub elements: Vec<Element>,
    /// `str()` of the full nested name (`fullname`).
    pub full_name: String,
    /// The display string of the full nested name (`[anonymous]` for `@...`).
    pub display_name: String,
    /// The encoded path (`cpp:path`).
    pub path: String,
    /// The names of the symbol's own template parameters (`cpp:tparams`).
    pub template_params: Vec<String>,
    /// The names of a function's parameters (`cpp:fparams`).
    pub function_params: Vec<String>,
}

impl Symbol {
    /// Returns the newest (version 4) id.
    pub fn newest_id(&self) -> &str {
        self.ids[MAX_ID_VERSION - 1].as_deref().unwrap_or("")
    }
}

/// One element of a cross-reference target's name.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct XrefName {
    /// The identifier or operator, as `str()` gives it.
    pub ident: String,
    /// Its template arguments, or `""`.
    pub args: String,
}

/// How the host looks a C++ cross-reference up, as `_resolve_xref_inner` parses its
/// target.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct XrefTarget {
    /// The name's elements.
    pub names: Vec<XrefName>,
    /// Written with `::` before it.
    pub is_rooted: bool,
    /// The template parameter lists before the name, as `str()` gives them.
    pub template_decls: Vec<String>,
    /// The target is a name (with `()` allowed after it), not a whole declaration.
    pub is_shorthand: bool,
    /// For a whole function declaration: its newest id with the full name left as
    /// `\x1d` modifiers `\x1d` (`cpp:query`).
    pub query: Option<String>,
    /// For a whole function declaration: the version 4 id of the name it declares
    /// (`cpp:own`).
    pub own: Option<String>,
}

impl XrefTarget {
    /// Returns the names one per line, identifier and arguments apart by `FIELD`
    /// (`cpp:name`).
    pub fn encoded_names(&self) -> String {
        let lines: Vec<String> =
            self.names.iter().map(|n| format!("{}{FIELD}{}", n.ident, n.args)).collect();
        lines.join("\n")
    }

    /// Returns the template parameter lists one per line (`cpp:tdecls`).
    pub fn encoded_template_decls(&self) -> String {
        self.template_decls.join("\n")
    }
}

/// A parsed `:cpp:expr:` or `:cpp:texpr:` text: an expression, or else a type.
#[derive(Debug, Clone, PartialEq)]
pub struct Expression<'a> {
    /// The tree; it borrows the text.
    pub ast: Ast<'a>,
    /// The expression or `Type` node.
    pub root: NodeId,
}

/// Runs a parse over `text` with white space trimmed, requiring it to consume the
/// whole text. It returns the tree and the root, or the error, its span relative to
/// `text`.
fn parse_whole<'a>(
    text: &'a str,
    what: &str,
    limits: &Limits,
    production: impl FnOnce(&mut Parser<'a>) -> parser::PResult<NodeId>,
) -> Result<(Ast<'a>, NodeId), CppError> {
    let def = lex::trim_space(text);
    let offset = def.as_ptr() as usize - text.as_ptr() as usize;
    let span = offset..offset + def.len();
    let mut p = Parser::new(def, limits);
    let parsed = production(&mut p);
    let at_end = parsed.is_ok() && {
        p.skip_ws();
        p.is_eof()
    };
    if let Some(mut error) = p.stop_error() {
        error.span = span;
        return Err(error);
    }
    match parsed {
        Ok(root) if at_end && !def.is_empty() => Ok((p.into_ast(), root)),
        _ => Err(CppError::invalid(what, span)),
    }
}

/// Parses a declaration written with `directive` (Sphinx's
/// `DefinitionParser.parse_declaration`). The definition is trimmed of white space and
/// must be one declaration; the tree borrows it.
///
/// Fails with `ErrorKind::Invalid` when it is not a declaration of that kind or names
/// nothing, and with `ErrorKind::Limit` when it nests too deeply or costs too much.
pub fn parse_declaration<'a>(
    definition: &'a str,
    directive: Directive,
    limits: &Limits,
) -> Result<Declaration<'a>, CppError> {
    let what = format!("{} declaration", directive.object_type().name());
    let (ast, root) = parse_whole(definition, &what, limits, |p| p.declaration(directive))?;
    if ast.declared_name(root).is_none() {
        return Err(CppError::invalid(&what, 0..definition.len()));
    }
    Ok(Declaration { ast, root })
}

impl Declaration<'_> {
    /// Returns what the declaration declares.
    pub fn object_type(&self) -> ObjectType {
        match *self.ast.node(self.root) {
            Node::Declaration { object, .. } => object,
            _ => ObjectType::Function,
        }
    }

    /// Returns the directive the declaration was written with.
    pub fn directive(&self) -> Directive {
        match *self.ast.node(self.root) {
            Node::Declaration { directive, .. } => directive,
            _ => Directive::Function,
        }
    }

    /// Returns the `NestedName` the declaration declares.
    pub fn name(&self) -> Option<NodeId> {
        self.ast.declared_name(self.root)
    }

    /// Returns the template parameter lists of its prefix.
    pub fn template_lists(&self) -> List {
        self.ast.template_lists(self.root)
    }

    /// Returns the declaration's id in `version` (1 to 4), with its `_CPPvN` prefix,
    /// for the symbol whose full path is `elements`; `None` when the version has none.
    pub fn id(&self, version: usize, elements: &[Element]) -> Option<String> {
        let ids = Ids { ast: &self.ast, version, elements, placeholder: false };
        let mut out = String::new();
        ids.declaration_id(self.root, true, &mut out)?;
        Some(out)
    }

    /// Computes the symbol the declaration declares in the scope whose encoded path is
    /// `scope_path` (`""` for the root): its ids, path, names, and parameter names.
    ///
    /// Fails with `ErrorKind::NoId` when there is no version 4 id (Sphinx then warns
    /// that the signature is invalid).
    pub fn symbol(&self, scope_path: &str) -> Result<Symbol, CppError> {
        let span = 0..self.ast.definition().len();
        let name = self.name().ok_or_else(|| CppError::invalid("declaration", span.clone()))?;
        let mut elements = decode_path(scope_path);
        elements.extend(self.ast.name_elements(name, self.template_lists()));
        let ids = [1, 2, 3, 4].map(|v| self.id(v, &elements));
        let Some(newest) = ids[MAX_ID_VERSION - 1].clone() else {
            return Err(CppError::no_id(span));
        };
        let templated = elements.last().is_some_and(|e| !e.params.is_empty());
        if let Some(last) = elements.last_mut() {
            last.decl = newest;
        }
        let mut path = String::new();
        encode_path(&elements, &mut path);
        Ok(Symbol {
            full_name: full_name(&elements),
            display_name: display_name(&elements),
            template_params: if templated { self.template_param_names() } else { Vec::new() },
            function_params: self.function_param_names(),
            ids,
            elements,
            path,
        })
    }

    /// The names of the parameters of the last template parameter list (the symbol
    /// takes it).
    fn template_param_names(&self) -> Vec<String> {
        let lists = self.template_lists();
        let Some(last) = self.ast.last(lists) else { return Vec::new() };
        let params = self.ast.template_params(last);
        self.ast.list(params).filter_map(|p| self.ast.param_name(p)).map(String::from).collect()
    }

    /// The names of a function's parameters, as `str()` gives them.
    fn function_param_names(&self) -> Vec<String> {
        let Node::Declaration { object: ObjectType::Function, decl, .. } =
            *self.ast.node(self.root)
        else {
            return Vec::new();
        };
        let Node::Type { decl: d, .. } = *self.ast.node(decl) else { return Vec::new() };
        let Some(Node::ParamQual(q)) = self.ast.function_params(d).map(|q| self.ast.node(q))
        else {
            return Vec::new();
        };
        let mut names = Vec::new();
        for param in self.ast.list(q.params) {
            let Node::FunctionParam(Some(arg)) = *self.ast.node(param) else { continue };
            let Some(name) = self.ast.declared_name(arg) else { continue };
            let text = self.ast.text(name, false);
            if !text.is_empty() {
                names.push(text);
            }
        }
        names
    }
}

/// Returns the path of the symbol a namespace directive's argument names below the
/// scope whose encoded path is `base` (`add_name`), or `""` for the root (`NULL`, `0`,
/// `nullptr`).
///
/// Fails with `ErrorKind::Invalid` when the argument is not a name, and with
/// `ErrorKind::Limit` when it nests too deeply.
pub fn namespace_path(base: &str, argument: &str, limits: &Limits) -> Result<String, CppError> {
    let def = lex::trim_space(argument);
    if matches!(def, "NULL" | "0" | "nullptr") {
        return Ok(String::new());
    }
    let (ast, root) = parse_whole(argument, "namespace name", limits, Parser::namespace)?;
    let Node::Namespace { name, .. } = *ast.node(root) else {
        return Err(CppError::invalid("namespace name", 0..argument.len()));
    };
    let mut elements = decode_path(base);
    elements.extend(ast.name_elements(name, ast.template_lists(root)));
    let mut path = String::new();
    encode_path(&elements, &mut path);
    Ok(path)
}

/// Parses a C++ cross-reference's target as the C++ domain's `_resolve_xref_inner`
/// does: a (template-prefixed) name, `()` allowed after it, or else a whole function
/// declaration. `parens` appends `()` (the `func` and `any` roles).
///
/// Fails with `ErrorKind::Invalid` when the target does not parse (Sphinx warns and
/// leaves it unresolved: `cpp:unparsed`), and with `ErrorKind::Limit` when it nests too
/// deeply or costs too much.
pub fn parse_xref_target(
    target: &str,
    parens: bool,
    limits: &Limits,
) -> Result<XrefTarget, CppError> {
    let mut def = lex::trim_space(target).to_string();
    if parens {
        def.push_str("()");
    }
    let mut shorthand = false;
    let (ast, root) = parse_whole(&def, "cross-reference target", limits, |p| {
        let (root, short) = p.xref()?;
        shorthand = short;
        Ok(root)
    })?;
    let (name, prefix) = match *ast.node(root) {
        Node::Namespace { name, prefix } => (Some(name), prefix),
        Node::Declaration { prefix, .. } => (ast.declared_name(root), prefix),
        _ => (None, None),
    };
    let Some(name) = name else {
        return Err(CppError::invalid("cross-reference target", 0..target.len()));
    };
    let mut x = XrefTarget { is_shorthand: shorthand, ..XrefTarget::default() };
    if let Node::NestedName { is_rooted, elements } = *ast.node(name) {
        x.is_rooted = is_rooted;
        for e in ast.list(elements) {
            let Node::NameElement { name, args, .. } = *ast.node(e) else { continue };
            let args = args.map(|a| ast.text(a, false)).unwrap_or_default();
            x.names.push(XrefName { ident: ast.text(name, false), args });
        }
    }
    let lists = match prefix.map(|p| ast.node(p)) {
        Some(Node::TemplatePrefix { lists, is_instantiation: false }) => *lists,
        _ => None,
    };
    x.template_decls = ast.list(lists).map(|t| ast.text(t, false)).collect();
    if shorthand {
        return Ok(x);
    }
    let ids = Ids { ast: &ast, version: MAX_ID_VERSION, elements: &[], placeholder: true };
    let mut query = String::new();
    if ids.declaration_id(root, true, &mut query).is_none() {
        return Ok(x);
    }
    x.query = Some(query);
    let own = ast.name_elements(name, lists);
    x.own = own.last().and_then(|e| e.id(MAX_ID_VERSION)).map(String::from);
    Ok(x)
}

/// Parses the text of a `:cpp:expr:` or `:cpp:texpr:` role as Sphinx's `CPPExprRole`
/// does: as an expression, or else as a type. The text is trimmed; the tree borrows it.
///
/// Fails with `ErrorKind::Invalid` when it parses as neither, or only with the
/// fallback scanner (the role then writes nothing), and with `ErrorKind::Limit` when
/// it nests too deeply.
pub fn parse_expression_or_type<'a>(
    text: &'a str,
    limits: &Limits,
) -> Result<Expression<'a>, CppError> {
    let (ast, root) = parse_whole(text, "expression or type", limits, Parser::expression_or_type)?;
    if ast.is_fallback_used() {
        return Err(CppError::invalid("expression or type", 0..text.len()));
    }
    Ok(Expression { ast, root })
}
