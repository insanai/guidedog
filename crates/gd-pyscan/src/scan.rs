//! Scanning records what a module or class body binds, in source order: definitions,
//! assignments, and imports, looking into if, try, with, for, and while blocks as the
//! code would run. Bindings under `if TYPE_CHECKING:` are kept but marked as not at run
//! time; except clauses are skipped, as Sphinx's analyser skips them.

use crate::literal::{compile_cleandoc, str_value};
use crate::syntax::Node;
use crate::{All, Class, ClassId, Def, DefKind, Function, FunctionId, Keyword, Module};
use crate::{Param, ParamKind, Scope};

struct Scanner<'a> {
    package: &'a str,
    classes: Vec<Class>,
    functions: Vec<Function>,
}

/// One body being scanned: its bindings, the class it belongs to, and whether its code
/// runs (false under `if TYPE_CHECKING:`).
struct Body {
    defs: Vec<Def>,
    class: Option<ClassId>,
    runtime: bool,
}

pub(crate) fn scan_module(m: &mut Module) {
    let package = crate::parent_package(&m.name, m.kind).to_string();
    let mut s = Scanner { package: &package, classes: Vec::new(), functions: Vec::new() };
    let root = m.syntax.root();
    let mut body = Body { defs: Vec::new(), class: None, runtime: true };
    s.scan_block(&mut body, root);
    let (classes, functions) = (s.classes, s.functions);
    m.doc = body_docstring(root);
    m.scope = Scope { defs: body.defs };
    m.classes = classes;
    m.functions = functions;
    m.all = read_all(root);
}

/// The docstring of a module, class, or function body: its first statement when that
/// is a str literal alone.
pub(crate) fn body_docstring(body: Node<'_>) -> Option<String> {
    let first = body.named_children().find(|n| n.kind() != "comment")?;
    if first.kind() != "expression_statement" || first.named_child_count() != 1 {
        return None;
    }
    str_value(first.named_child(0)).map(|v| compile_cleandoc(&v))
}

impl<'a> Scanner<'a> {
    fn scan_block(&mut self, b: &mut Body, block: Node<'a>) {
        for n in block.named_children() {
            self.scan_statement(b, n);
        }
    }

    fn scan_statement(&mut self, b: &mut Body, n: Node<'a>) {
        match n.kind() {
            "expression_statement" => {
                for e in n.named_children().filter(|e| e.kind() == "assignment") {
                    scan_assignment(b, n, e);
                }
            }
            "function_definition" | "class_definition" => self.scan_definition(b, n, n, vec![]),
            "decorated_definition" => {
                let definition = n.field("definition");
                if !definition.is_null() {
                    self.scan_definition(b, n, definition, decorators_of(n));
                }
            }
            "import_statement" => scan_import(b, n),
            "import_from_statement" | "future_import_statement" => {
                scan_import_from(b, n, self.package)
            }
            "if_statement" => self.scan_if(b, n),
            "try_statement" => {
                for part in n.named_children() {
                    match part.kind() {
                        "block" => self.scan_block(b, part),
                        "else_clause" | "finally_clause" => self.scan_block(b, clause_body(part)),
                        _ => {}
                    }
                }
            }
            "with_statement" | "while_statement" => self.scan_block(b, n.field("body")),
            "for_statement" => {
                self.scan_block(b, n.field("body"));
                let alt = n.field("alternative");
                if !alt.is_null() {
                    self.scan_block(b, clause_body(alt));
                }
            }
            "type_alias_statement" => {
                let name = n.field_text("left");
                let value = n.field("right").id();
                add_def(
                    b,
                    Def {
                        name: name.into(),
                        kind: DefKind::TypeAlias,
                        node: n.id(),
                        value,
                        line: n.line(),
                        ..Def::default()
                    },
                );
            }
            _ => {}
        }
    }

    fn scan_if(&mut self, b: &mut Body, n: Node<'a>) {
        let checking = is_type_checking(n.field("condition"));
        let saved = b.runtime;
        b.runtime = saved && !checking;
        self.scan_block(b, n.field("consequence"));
        b.runtime = saved;
        for alt in n.named_children() {
            match alt.kind() {
                "elif_clause" => self.scan_block(b, alt.field("consequence")),
                "else_clause" => self.scan_block(b, clause_body(alt)),
                _ => {}
            }
        }
    }

    fn qualname_in(&self, b: &Body, name: &str) -> String {
        match b.class {
            Some(c) => format!("{}.{name}", self.classes[c.0 as usize].qualname),
            None => name.to_string(),
        }
    }

    fn scan_definition(
        &mut self,
        b: &mut Body,
        statement: Node<'a>,
        n: Node<'a>,
        decorators: Vec<crate::NodeId>,
    ) {
        let name = n.field_text("name");
        let qualname = self.qualname_in(b, name);
        if n.kind() == "class_definition" {
            let id = ClassId(self.classes.len() as u32);
            let body = n.field("body");
            let (bases, keywords) = read_bases(n.field("superclasses"));
            self.classes.push(Class {
                name: name.into(),
                qualname: qualname.into(),
                node: n.id(),
                bases,
                keywords,
                decorators,
                doc: body_docstring(body),
                scope: Scope::default(),
                outer: b.class,
            });
            let mut inner = Body { defs: Vec::new(), class: Some(id), runtime: b.runtime };
            self.scan_block(&mut inner, body);
            self.classes[id.0 as usize].scope = Scope { defs: inner.defs };
            add_def(
                b,
                Def {
                    name: name.into(),
                    kind: DefKind::Class,
                    node: statement.id(),
                    class: Some(id),
                    line: n.line(),
                    ..Def::default()
                },
            );
            return;
        }
        let mut f = function_of(n, &qualname);
        f.decorators = decorators;
        f.class = b.class;
        let id = FunctionId(self.functions.len() as u32);
        self.functions.push(f);
        add_def(
            b,
            Def {
                name: name.into(),
                kind: DefKind::Function,
                node: statement.id(),
                function: Some(id),
                line: n.line(),
                ..Def::default()
            },
        );
    }
}

/// The block of an else, finally, or except clause.
pub(crate) fn clause_body(clause: Node<'_>) -> Node<'_> {
    let body = clause.field("body");
    if !body.is_null() {
        return body;
    }
    clause.named_children().find(|n| n.kind() == "block").unwrap_or(body)
}

/// Recognises `TYPE_CHECKING`, `typing.TYPE_CHECKING`, and `t.TYPE_CHECKING`.
pub(crate) fn is_type_checking(condition: Node<'_>) -> bool {
    let s = condition.text();
    s == "TYPE_CHECKING" || s.ends_with(".TYPE_CHECKING")
}

/// The decorator expressions of a `decorated_definition`.
pub(crate) fn decorators_of(decorated: Node<'_>) -> Vec<crate::NodeId> {
    decorated
        .named_children()
        .filter(|n| n.kind() == "decorator" && n.named_child_count() > 0)
        .filter_map(|n| n.named_child(0).id())
        .collect()
}

/// Records a binding of the body; its runtime flag is the body's.
fn add_def(b: &mut Body, mut d: Def) {
    d.runtime = b.runtime;
    b.defs.push(d);
}

fn read_bases(args: Node<'_>) -> (Vec<crate::NodeId>, Vec<Keyword>) {
    let mut bases = Vec::new();
    let mut keywords = Vec::new();
    for a in args.named_children() {
        match a.kind() {
            "keyword_argument" => keywords
                .push(Keyword { name: a.field_text("name").into(), value: a.field("value").id() }),
            "comment" | "list_splat" | "dictionary_splat" => {}
            _ => bases.extend(a.id()),
        }
    }
    (bases, keywords)
}

/// Reads a `function_definition` or `lambda`: its parameters as written.
pub(crate) fn function_of(n: Node<'_>, qualname: &str) -> Function {
    let mut f = Function { qualname: qualname.into(), node: n.id(), ..Function::default() };
    f.is_lambda = n.kind() == "lambda";
    if f.is_lambda {
        f.name = "<lambda>".into();
    } else {
        f.name = n.field_text("name").into();
        f.is_async = n.child(0).kind() == "async";
        f.returns = n.field("return_type").id();
        f.doc = body_docstring(n.field("body"));
        read_type_comment(&mut f, n);
    }
    read_params(&mut f, n.field("parameters"));
    f
}

/// Reads a function's `# type: (args) -> result` comment, which tree-sitter places
/// between the header's colon and the body.
fn read_type_comment(f: &mut Function, n: Node<'_>) {
    for c in n.named_children().filter(|c| c.kind() == "comment") {
        let Some(spec) = type_comment_text(c.text()) else { continue };
        let (Some(open), Some(arrow)) = (spec.find('('), spec.rfind("->")) else { return };
        let Some(close) = spec[..arrow].rfind(')') else { return };
        if close < open {
            return;
        }
        f.type_comment = true;
        f.type_args = split_top(&spec[open + 1..close])
            .map(|part| part.trim().trim_start_matches('*'))
            .filter(|arg| !arg.is_empty())
            .map(str::to_string)
            .collect();
        f.type_returns = spec[arrow + 2..].trim().to_string();
        return;
    }
}

/// What follows `# type:` in a comment, unless it is `ignore`.
pub(crate) fn type_comment_text(comment: &str) -> Option<&str> {
    let t = comment.trim();
    let t = t.strip_prefix('#').unwrap_or(t).trim();
    let t = t.strip_prefix("type:")?.trim();
    if t.is_empty() || t.starts_with("ignore") {
        return None;
    }
    Some(t)
}

/// Splits at commas outside brackets; a blank last part is left out.
fn split_top(s: &str) -> impl Iterator<Item = &str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in s.bytes().enumerate() {
        match c {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => depth -= 1,
            b',' if depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if !s[start..].trim().is_empty() {
        parts.push(&s[start..]);
    }
    parts.into_iter()
}

fn read_params(f: &mut Function, params: Node<'_>) {
    let mut keyword_only = false;
    for n in params.named_children() {
        let kind =
            if keyword_only { ParamKind::KeywordOnly } else { ParamKind::PositionalOrKeyword };
        let mut p = Param { kind, ..Param::default() };
        match n.kind() {
            "identifier" => p.name = n.text().into(),
            "typed_parameter" => {
                p.annotation = n.field("type").id();
                splat(n.named_child(0), &mut p, &mut keyword_only);
            }
            "default_parameter" | "typed_default_parameter" => {
                p.name = n.field_text("name").into();
                p.annotation = n.field("type").id();
                p.default = n.field("value").id();
            }
            "list_splat_pattern" | "dictionary_splat_pattern" => {
                splat(n, &mut p, &mut keyword_only)
            }
            "keyword_separator" => {
                keyword_only = true;
                continue;
            }
            "positional_separator" => {
                for q in f.params.iter_mut().filter(|q| q.kind == ParamKind::PositionalOrKeyword) {
                    q.kind = ParamKind::PositionalOnly;
                }
                continue;
            }
            "comment" => {
                // "# type: T" after a parameter (and its comma) is that parameter's type.
                if let (Some(t), Some(last)) = (type_comment_text(n.text()), f.params.last_mut()) {
                    last.type_comment = t.to_string();
                }
                continue;
            }
            _ => continue,
        }
        f.params.push(p);
    }
}

fn splat(n: Node<'_>, p: &mut Param, keyword_only: &mut bool) {
    match n.kind() {
        "list_splat_pattern" => {
            p.kind = ParamKind::VarPositional;
            p.name = n.named_child(0).text().into();
            *keyword_only = true;
        }
        "dictionary_splat_pattern" => {
            p.kind = ParamKind::VarKeyword;
            p.name = n.named_child(0).text().into();
        }
        _ => p.name = n.text().into(),
    }
}

fn scan_assignment(b: &mut Body, statement: Node<'_>, n: Node<'_>) {
    let (left, right, annotation) = (n.field("left"), n.field("right"), n.field("type"));
    let kind = if !right.is_null() || annotation.is_null() {
        DefKind::Assign
    } else {
        DefKind::Annotation
    };
    // a = b = value: the innermost right side is the value of every target.
    let mut value = right;
    while value.kind() == "assignment" {
        value = value.field("right");
    }
    let target = Target { statement, value, kind };
    bind_targets(b, &target, left, annotation.id(), false);
    if right.kind() == "assignment" {
        scan_assignment(b, statement, right);
    }
}

struct Target<'a> {
    statement: Node<'a>,
    value: Node<'a>,
    kind: DefKind,
}

/// Binds the names of an assignment target; the parts of an unpacking are `tuple`
/// bindings, and only a plain name keeps the annotation.
fn bind_targets(
    b: &mut Body,
    t: &Target<'_>,
    target: Node<'_>,
    annotation: Option<crate::NodeId>,
    tuple: bool,
) {
    match target.kind() {
        "identifier" => add_def(
            b,
            Def {
                name: target.text().into(),
                kind: t.kind,
                node: t.statement.id(),
                value: t.value.id(),
                annotation,
                tuple,
                line: t.statement.line(),
                ..Def::default()
            },
        ),
        "pattern_list" | "tuple_pattern" | "list_pattern" | "expression_list" | "tuple"
        | "list" => {
            for part in target.named_children() {
                bind_targets(b, t, part, None, true);
            }
        }
        "list_splat_pattern" => bind_targets(b, t, target.named_child(0), None, true),
        _ => {}
    }
}

fn import_def(name: &str, kind: DefKind, n: Node<'_>, target: &str, member: &str) -> Def {
    Def {
        name: name.into(),
        kind,
        node: n.id(),
        target: target.into(),
        member: member.into(),
        line: n.line(),
        ..Def::default()
    }
}

fn scan_import(b: &mut Body, n: Node<'_>) {
    for name in n.named_children() {
        match name.kind() {
            "dotted_name" => {
                // import a.b binds a, the top package.
                let full = name.text();
                let top = full.split('.').next().unwrap_or(full);
                add_def(b, import_def(top, DefKind::Import, n, top, ""));
            }
            "aliased_import" => {
                let alias = name.field_text("alias");
                add_def(b, import_def(alias, DefKind::Import, n, name.field_text("name"), ""));
            }
            _ => {}
        }
    }
}

fn scan_import_from(b: &mut Body, n: Node<'_>, package: &str) {
    let module_name = n.field("module_name");
    let from = if module_name.is_null() {
        "__future__".to_string()
    } else {
        absolute_module(module_name, package)
    };
    for child in n.children() {
        let field = child.field_name();
        let def = match (child.kind(), field) {
            ("wildcard_import", _) => import_def("*", DefKind::ImportFrom, n, &from, "*"),
            ("dotted_name", Some("name")) => {
                let name = child.text();
                import_def(name, DefKind::ImportFrom, n, &from, name)
            }
            ("aliased_import", Some("name")) => {
                let (alias, name) = (child.field_text("alias"), child.field_text("name"));
                import_def(alias, DefKind::ImportFrom, n, &from, name)
            }
            _ => continue,
        };
        add_def(b, def);
    }
}

/// Turns the module of a from-import, relative or not, into a dotted name.
pub(crate) fn absolute_module(n: Node<'_>, package: &str) -> String {
    if n.kind() != "relative_import" {
        return n.text().to_string();
    }
    let mut dots = 0;
    let mut rest = "";
    for part in n.children() {
        match part.kind() {
            "import_prefix" => dots = part.text().trim().len(),
            "dotted_name" => rest = part.text(),
            _ => {}
        }
    }
    let mut base = package;
    for _ in 1..dots {
        base = base.rfind('.').map_or("", |i| &base[..i]);
    }
    match (base.is_empty(), rest.is_empty()) {
        (_, true) => base.to_string(),
        (true, false) => rest.to_string(),
        (false, false) => format!("{base}.{rest}"),
    }
}

/// Reads `__all__` from the module's top-level statements, in order: an assignment of a
/// list or tuple of strings, `+=` of one, and `.append` and `.extend` calls. Anything
/// else (a computed value) leaves it defined but unknown.
fn read_all(root: Node<'_>) -> All {
    let mut all = All::default();
    walk_all(&mut all, root);
    all
}

fn walk_all(all: &mut All, block: Node<'_>) {
    for n in block.named_children() {
        match n.kind() {
            "expression_statement" => all_statement(all, n.named_child(0)),
            "if_statement" if !is_type_checking(n.field("condition")) => {
                walk_all(all, n.field("consequence"))
            }
            "try_statement" => walk_all(all, n.field("body")),
            _ => {}
        }
    }
}

fn all_statement(all: &mut All, e: Node<'_>) {
    match e.kind() {
        "assignment" if e.field_text("left") == "__all__" => {
            all.names.clear();
            all.defined = true;
            all.known = add_all_names(all, e.field("right"));
        }
        "augmented_assignment" if e.field_text("left") == "__all__" => {
            all.known = all.known && add_all_names(all, e.field("right"));
        }
        "call" => {
            let args = e.field("arguments");
            match e.field_text("function") {
                "__all__.extend" => {
                    all.known = all.known && add_all_names(all, args.named_child(0))
                }
                "__all__.append" => {
                    let name = str_value(args.named_child(0));
                    all.known = all.known && name.is_some();
                    all.names.extend(name.map(|n| n.into_owned()));
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn add_all_names(all: &mut All, value: Node<'_>) -> bool {
    match value.kind() {
        "list" | "tuple" | "expression_list" => {
            for item in value.named_children().filter(|i| i.kind() != "comment") {
                let Some(name) = str_value(item) else { return false };
                all.names.push(name.into_owned());
            }
            true
        }
        "parenthesized_expression" => add_all_names(all, value.named_child(0)),
        _ => false,
    }
}
