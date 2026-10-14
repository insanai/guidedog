//! Annotations are evaluated as `typing.get_type_hints` evaluates them, then written as
//! `sphinx.util.typing.stringify_annotation` and `restify` write the objects they name.
//! A name that is not bound when the module has run (one imported only for type
//! checkers) fails the evaluation, as Python's `NameError` does; the caller then keeps
//! the annotations as written, as Sphinx does.

use std::rc::Rc;

use gd_pyscan::{Module, NodeId, Object, ObjectKind, UnparseMode, str_value, unparse};

use crate::Generator;

/// An evaluated annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TypeValue {
    NoneType,
    Ellipsis,
    /// A class: its module and qualname.
    Class {
        module: String,
        name: String,
    },
    /// A typing special form or alias, bare (module `typing`).
    Typing {
        name: String,
    },
    /// `origin[args]`.
    Generic {
        origin: Box<TypeValue>,
        args: Vec<TypeValue>,
    },
    /// Arguments joined by `|`.
    Union(Vec<TypeValue>),
    /// `[A, B]`, the parameters of a Callable.
    ArgList(Vec<TypeValue>),
    /// A value inside `Literal[...]`: its repr; `module` is set for an enum member.
    Literal {
        text: String,
        module: String,
    },
    /// A TypeVar, ParamSpec, or NewType (`newtype`, which keeps its module even in
    /// typing).
    TypeVar {
        module: String,
        name: String,
        newtype: bool,
    },
    /// Something only known by its text.
    Other(String),
}

impl TypeValue {
    fn class(module: &str, name: &str) -> TypeValue {
        TypeValue::Class { module: module.to_string(), name: name.to_string() }
    }

    fn literal(text: String) -> TypeValue {
        TypeValue::Literal { text, module: String::new() }
    }
}

/// Bounds the nodes one evaluation visits. Depth 40 cuts alias cycles, but aliases that
/// each use the next twice (`T0 = tuple[T1, T1]`, ...) double the work at every level.
/// Past it the evaluation fails, and the annotation is kept as written.
pub(crate) const MAX_EVAL_STEPS: usize = 10_000;

const MAX_EVAL_DEPTH: usize = 40;

/// One evaluation's state.
#[derive(Debug, Default)]
pub(crate) struct Eval {
    /// An object outside the source roots stands in by the path it was imported from,
    /// whose real module may differ.
    pub(crate) approximate: bool,
    depth: usize,
    /// Nodes evaluated (`MAX_EVAL_STEPS`).
    steps: usize,
    /// The first such path, for the note.
    pub(crate) outside: String,
    /// Names resolve among builtins alone, as in a function whose `__globals__` is
    /// empty (one made by a decorator outside the roots).
    pub(crate) builtins_only: bool,
}

/// Evaluates an annotation node of module `m`. `None` where Python would raise (a name
/// not bound at run time, an unknown attribute of a module in the roots), and past the
/// evaluation's depth or steps.
pub(crate) fn eval_type(
    g: &mut Generator<'_>,
    e: &mut Eval,
    m: &Module,
    n: Option<NodeId>,
) -> Option<TypeValue> {
    e.depth += 1;
    e.steps += 1;
    let out = match n {
        Some(id) if e.depth <= MAX_EVAL_DEPTH && e.steps <= MAX_EVAL_STEPS => {
            eval_node(g, e, m, id)
        }
        _ => None,
    };
    e.depth -= 1;
    out
}

fn eval_node(g: &mut Generator<'_>, e: &mut Eval, m: &Module, id: NodeId) -> Option<TypeValue> {
    let n = m.node(id);
    match n.kind() {
        "type" | "parenthesized_expression" => eval_type(g, e, m, n.named_child(0).id()),
        "none" => Some(TypeValue::NoneType),
        "ellipsis" => Some(TypeValue::Ellipsis),
        "identifier" | "attribute" => eval_reference(g, e, m, id),
        "subscript" | "generic_type" => eval_subscript(g, e, m, id),
        "binary_operator" => {
            if n.field_text("operator") != "|" {
                return None;
            }
            let a = eval_type(g, e, m, n.field("left").id())?;
            let b = eval_type(g, e, m, n.field("right").id())?;
            Some(make_union(vec![a, b]))
        }
        "string" | "concatenated_string" => {
            let s = str_value(n)?;
            eval_string(g, e, m, &s)
        }
        "list" => {
            let mut args = Vec::new();
            for item in n.named_children().filter(|i| i.kind() != "comment") {
                args.push(eval_type(g, e, m, item.id())?);
            }
            Some(TypeValue::ArgList(args))
        }
        "integer" | "float" | "true" | "false" => {
            Some(TypeValue::literal(unparse(n, UnparseMode::SphinxRepr)))
        }
        _ => {
            e.approximate = true;
            Some(TypeValue::Other(unparse(n, UnparseMode::Python)))
        }
    }
}

/// Evaluates a forward reference: the string parsed as an expression of `m`.
pub(crate) fn eval_string(
    g: &mut Generator<'_>,
    e: &mut Eval,
    m: &Module,
    s: &str,
) -> Option<TypeValue> {
    let (snippet, node) = g.project.parse_expression(m, s)?;
    eval_type(g, e, &snippet, Some(node))
}

fn eval_reference(
    g: &mut Generator<'_>,
    e: &mut Eval,
    m: &Module,
    id: NodeId,
) -> Option<TypeValue> {
    if e.builtins_only {
        let mut root = m.node(id);
        while root.kind() == "attribute" {
            root = root.field("object");
        }
        if !gd_pyscan::is_builtin(root.text()) {
            return None;
        }
    }
    let o = g.project.reference(m, None, id);
    type_of_object(g, e, &o)
}

fn type_of_object(g: &mut Generator<'_>, e: &mut Eval, o: &Object) -> Option<TypeValue> {
    match o.kind {
        ObjectKind::Class => {
            let c = o.class?;
            let module = &g.project.module(c.module).name;
            Some(TypeValue::class(module, &g.project.class(c).qualname))
        }
        ObjectKind::Builtin => {
            Some(TypeValue::class("builtins", o.path.trim_start_matches("builtins.")))
        }
        ObjectKind::External => Some(external_type(e, &o.path)),
        ObjectKind::Value => alias_type(g, e, o),
        ObjectKind::Function => {
            let f = o.function?;
            let m = g.project.module(f.module);
            let qualname = m.function(f.kind).qualname.to_string();
            Some(TypeValue::class(&m.name, &qualname))
        }
        ObjectKind::Module => Some(TypeValue::Other(g.project.module(o.module?).name.clone())),
        ObjectKind::None => None,
    }
}

/// An object outside the roots, known by the path it was imported from.
pub(crate) fn external_type(e: &mut Eval, path: &str) -> TypeValue {
    let (module, name) = path.rsplit_once('.').unwrap_or(("", path));
    if module == "typing" || module == "typing_extensions" {
        return match name {
            "AnyStr" => {
                TypeValue::TypeVar { module: "typing".into(), name: name.into(), newtype: false }
            }
            "Text" => TypeValue::class("builtins", "str"),
            _ => TypeValue::Typing { name: name.into() },
        };
    }
    if let Some((module, name)) = fixed_path(path).and_then(|f| f.rsplit_once('.')) {
        return TypeValue::class(module, name);
    }
    if !stdlib_module(module) {
        e.approximate = true;
        if e.outside.is_empty() {
            e.outside = path.to_string();
        }
    }
    TypeValue::class(module, name)
}

/// Evaluates what an assignment in the roots binds: a type alias, a TypeVar, or a
/// NewType.
fn alias_type(g: &mut Generator<'_>, e: &mut Eval, o: &Object) -> Option<TypeValue> {
    let m = Rc::clone(g.project.module(o.module?));
    let value = o.value.filter(|_| !o.tuple);
    let Some(value) = value else {
        e.approximate = true;
        let name = o.def.map(|d| g.project.def(d).name.to_string()).unwrap_or_default();
        return Some(TypeValue::Other(name));
    };
    let node = m.node(value);
    if node.kind() != "call" {
        return eval_type(g, e, &m, Some(value));
    }
    let callee = match node.field("function").id() {
        Some(f) => g.project.reference(&m, None, f),
        None => Object::default(),
    };
    let path = if callee.kind == ObjectKind::External { callee.path.as_str() } else { "" };
    let first = node.field("arguments").named_child(0);
    let name = str_value(first).map(|s| s.into_owned()).unwrap_or_default();
    let module = m.name.clone();
    match path {
        "typing.TypeVar"
        | "typing_extensions.TypeVar"
        | "typing.ParamSpec"
        | "typing_extensions.ParamSpec"
        | "typing.TypeVarTuple" => Some(TypeValue::TypeVar { module, name, newtype: false }),
        "typing.NewType" | "typing_extensions.NewType" => {
            Some(TypeValue::TypeVar { module, name, newtype: true })
        }
        _ => {
            e.approximate = true;
            Some(TypeValue::Other(unparse(node, UnparseMode::Python)))
        }
    }
}

fn eval_subscript(
    g: &mut Generator<'_>,
    e: &mut Eval,
    m: &Module,
    id: NodeId,
) -> Option<TypeValue> {
    let (value, subs) = subscript_ids(m, id);
    let origin = eval_type(g, e, m, value)?;
    if matches!(&origin, TypeValue::Typing { name } if name == "Literal") {
        let args = subs.iter().map(|&s| literal_arg(g, m, s)).collect();
        return Some(TypeValue::Generic { origin: Box::new(origin), args });
    }
    let mut args = Vec::with_capacity(subs.len());
    for s in subs {
        args.push(eval_type(g, e, m, Some(s))?);
    }
    if let TypeValue::Typing { name } = &origin {
        match name.as_str() {
            "Optional" => {
                args.push(TypeValue::NoneType);
                return Some(make_union(args));
            }
            "Union" => return Some(make_union(args)),
            _ => {}
        }
    }
    Some(TypeValue::Generic { origin: Box::new(origin), args })
}

/// The subscripted value and the subscripts of a `subscript` or `generic_type` node.
fn subscript_ids(m: &Module, id: NodeId) -> (Option<NodeId>, Vec<NodeId>) {
    let n = m.node(id);
    if n.kind() == "generic_type" {
        let params = n.named_child(1).named_children().filter_map(|p| p.id()).collect();
        return (n.named_child(0).id(), params);
    }
    let subs = n.children().filter(|c| c.field_name() == Some("subscript"));
    (n.field("value").id(), subs.filter_map(|c| c.id()).collect())
}

fn literal_arg(g: &mut Generator<'_>, m: &Module, id: NodeId) -> TypeValue {
    let n = m.node(id);
    if n.kind() == "attribute" {
        // An enum member: Class.member.
        let object = n.field("object").id();
        let o = object.map(|o| g.project.reference(m, None, o)).unwrap_or_default();
        if let (ObjectKind::Class, Some(c)) = (o.kind, o.class) {
            let text = format!("{}.{}", g.project.class(c).qualname, n.field_text("attribute"));
            let module = g.project.module(c.module).name.clone();
            return TypeValue::Literal { text, module };
        }
    }
    TypeValue::literal(unparse(n, UnparseMode::SphinxRepr))
}

/// `typing.Union`: nested unions flattened, duplicates dropped, one argument left alone.
pub(crate) fn make_union(args: Vec<TypeValue>) -> TypeValue {
    let mut flat: Vec<TypeValue> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for a in args {
        let parts = match a {
            TypeValue::Union(parts) => parts,
            other => vec![other],
        };
        for p in parts {
            let key = stringify(&p, TypeMode::Smart);
            if !seen.contains(&key) {
                seen.push(key);
                flat.push(p);
            }
        }
    }
    if flat.len() == 1 {
        return flat.remove(0);
    }
    TypeValue::Union(flat)
}

/// How annotations name the classes they use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeMode {
    /// `~module.Name`: shown as Name, linked to module.Name.
    Smart,
    /// `module.Name`, but typing names bare.
    FullyQualifiedExceptTyping,
    /// `module.Name`, `typing.Name`.
    FullyQualified,
}

/// `stringify_annotation` for an evaluated annotation.
pub(crate) fn stringify(v: &TypeValue, mode: TypeMode) -> String {
    match v {
        TypeValue::NoneType => "None".into(),
        TypeValue::Ellipsis => "...".into(),
        TypeValue::Literal { text, .. } | TypeValue::Other(text) => text.clone(),
        TypeValue::Class { module, name } if module == "builtins" => name.clone(),
        TypeValue::Class { module, name } => format!("{}{name}", prefix(mode, module)),
        TypeValue::Typing { name } => match mode {
            TypeMode::FullyQualifiedExceptTyping => name.clone(),
            TypeMode::FullyQualified => format!("typing.{name}"),
            TypeMode::Smart => format!("~typing.{name}"),
        },
        TypeValue::TypeVar { module, name, newtype } => {
            if module == "typing" && !newtype {
                name.clone()
            } else {
                format!("{}{name}", prefix(mode, module))
            }
        }
        TypeValue::Union(args) => join_types(args, " | ", mode),
        TypeValue::ArgList(args) => format!("[{}]", join_types(args, ", ", mode)),
        TypeValue::Generic { origin, args } => stringify_generic(origin, args, mode),
    }
}

fn prefix(mode: TypeMode, module: &str) -> String {
    if mode == TypeMode::Smart { format!("~{module}.") } else { format!("{module}.") }
}

fn join_types(args: &[TypeValue], sep: &str, mode: TypeMode) -> String {
    args.iter().map(|a| stringify(a, mode)).collect::<Vec<_>>().join(sep)
}

fn stringify_generic(origin: &TypeValue, args: &[TypeValue], mode: TypeMode) -> String {
    let head = stringify(origin, mode);
    if all_system_type_vars(args) {
        return head;
    }
    let callable = match origin {
        TypeValue::Typing { name } => name == "Callable",
        TypeValue::Class { module, name } => module == "collections.abc" && name == "Callable",
        _ => false,
    };
    if let (true, Some((last, params))) = (callable, args.split_last()) {
        let inner = match params {
            [TypeValue::ArgList(list)] => join_types(list, ", ", mode),
            _ => join_types(params, ", ", mode),
        };
        return format!("{head}[[{inner}], {}]", stringify(last, mode));
    }
    format!("{head}[{}]", join_types(args, ", ", mode))
}

/// `sphinx.util.typing.restify`: a reST cross-reference to a type, for `Bases:` and
/// `alias of`.
pub(crate) fn restify(v: &TypeValue, mode: TypeMode) -> String {
    let tilde = if mode == TypeMode::Smart { "~" } else { "" };
    match v {
        TypeValue::NoneType => ":py:obj:`None`".into(),
        TypeValue::Ellipsis => "...".into(),
        // An enum member restifies as a reference to it.
        TypeValue::Literal { text, module } if module.is_empty() => text.clone(),
        TypeValue::Literal { text, module } => format!(":py:attr:`~{module}.{text}`"),
        TypeValue::Other(text) => text.clone(),
        TypeValue::Class { module, name } if module == "builtins" => format!(":py:class:`{name}`"),
        TypeValue::Class { module, name } => format!(":py:class:`{tilde}{module}.{name}`"),
        TypeValue::Typing { name } => {
            let role = if name == "Any" || special_form(name) { "obj" } else { "class" };
            format!(":py:{role}:`~typing.{name}`")
        }
        TypeValue::TypeVar { module, name, .. } => format!(":py:obj:`{tilde}{module}.{name}`"),
        TypeValue::Union(args) => restify_list(args, " | ", mode),
        TypeValue::ArgList(args) => format!("[{}]", restify_list(args, ", ", mode)),
        TypeValue::Generic { origin, args } => {
            if all_system_type_vars(args) {
                return restify(origin, mode);
            }
            format!("{}\\ [{}]", restify(origin, mode), restify_list(args, ", ", mode))
        }
    }
}

fn restify_list(args: &[TypeValue], sep: &str, mode: TypeMode) -> String {
    args.iter().map(|a| restify(a, mode)).collect::<Vec<_>>().join(sep)
}

/// `is_system_TypeVar` for every argument: typing's own TypeVars (as in `Dict[KT, VT]`)
/// are not written.
fn all_system_type_vars(args: &[TypeValue]) -> bool {
    !args.is_empty()
        && args.iter().all(|a| {
            matches!(a, TypeValue::TypeVar { module, newtype: false, .. } if module == "typing")
        })
}

/// typing's `_SpecialForm` names, which restify links as objects.
fn special_form(name: &str) -> bool {
    matches!(
        name,
        "Any"
            | "NoReturn"
            | "Never"
            | "Self"
            | "LiteralString"
            | "ClassVar"
            | "Final"
            | "Union"
            | "Optional"
            | "Literal"
            | "TypeAlias"
            | "Concatenate"
            | "TypeGuard"
            | "TypeIs"
            | "Required"
            | "NotRequired"
            | "ReadOnly"
            | "Unpack"
    )
}

/// The documented path of standard classes whose `__module__` is private, where a
/// source imports the private one.
fn fixed_path(path: &str) -> Option<&'static str> {
    Some(match path {
        "_io.BytesIO" => "io.BytesIO",
        "_io.StringIO" => "io.StringIO",
        "pathlib._local.Path" => "pathlib.Path",
        "json.decoder.JSONDecoder" => "json.JSONDecoder",
        "json.encoder.JSONEncoder" => "json.JSONEncoder",
        "_contextvars.ContextVar" => "contextvars.ContextVar",
        _ => return None,
    })
}

/// Top-level modules of Python's standard library, whose classes keep the module they
/// are imported from (so the path is exact).
const STDLIB: &[&str] = &[
    "abc",
    "argparse",
    "array",
    "ast",
    "asyncio",
    "base64",
    "binascii",
    "bisect",
    "builtins",
    "bz2",
    "calendar",
    "cgi",
    "cmath",
    "codecs",
    "collections",
    "concurrent",
    "configparser",
    "contextlib",
    "contextvars",
    "copy",
    "csv",
    "ctypes",
    "dataclasses",
    "datetime",
    "decimal",
    "difflib",
    "email",
    "enum",
    "errno",
    "fnmatch",
    "fractions",
    "functools",
    "gc",
    "getpass",
    "gettext",
    "glob",
    "gzip",
    "hashlib",
    "heapq",
    "hmac",
    "html",
    "http",
    "importlib",
    "inspect",
    "io",
    "ipaddress",
    "itertools",
    "json",
    "keyword",
    "locale",
    "logging",
    "lzma",
    "math",
    "mimetypes",
    "multiprocessing",
    "numbers",
    "operator",
    "os",
    "pathlib",
    "pickle",
    "platform",
    "pprint",
    "queue",
    "random",
    "re",
    "secrets",
    "select",
    "selectors",
    "shlex",
    "shutil",
    "signal",
    "socket",
    "socketserver",
    "sqlite3",
    "ssl",
    "stat",
    "statistics",
    "string",
    "struct",
    "subprocess",
    "sys",
    "tarfile",
    "tempfile",
    "textwrap",
    "threading",
    "time",
    "timeit",
    "token",
    "tokenize",
    "traceback",
    "types",
    "typing",
    "typing_extensions",
    "unicodedata",
    "unittest",
    "urllib",
    "uuid",
    "warnings",
    "weakref",
    "wsgiref",
    "xml",
    "zipfile",
    "zlib",
    "zoneinfo",
];

/// Whether a module is part of Python's standard library.
pub(crate) fn stdlib_module(module: &str) -> bool {
    let top = module.split('.').next().unwrap_or(module);
    STDLIB.contains(&top)
}
