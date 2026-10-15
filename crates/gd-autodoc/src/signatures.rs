//! Signatures follow autodoc's `_format_signatures` and inspect's `stringify_signature`:
//! the parameters of the function (or of the constructor a class is called through),
//! with defaults as repr would write them, or as written with
//! `autodoc_preserve_defaults`, and annotations evaluated as `get_type_hints` evaluates
//! them.

use std::sync::Arc;

use gd_pyscan::{ClassRef, DefKind, FunctionId, FunctionRef, Module, ModuleId, NodeId};
use gd_pyscan::{ObjectKind, Param, ParamKind, UnparseMode, Wrapper, prepare_docstring};
use gd_pyscan::{try_unparse, unparse};

use crate::dataclasses::{generated_constructor, generated_signature};
use crate::docstrings::Docstrings;
use crate::hints::{annotation_of, function_hints, signature_mode};
use crate::names::{ParsedName, match_signature};
use crate::props::{Hint, Props};
use crate::types::{Eval, TypeMode, eval_string, stringify};
use crate::values::{describe, is_enum};
use crate::{ClassSignature, Generator, ObjType, Severity, Typehints};

/// A signature: its parameters in parentheses, and its return annotation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Formatted {
    args: String,
    retann: String,
}

impl Formatted {
    fn line(&self) -> String {
        if self.retann.is_empty() {
            self.args.clone()
        } else {
            format!("{} -> {}", self.args, self.retann)
        }
    }
}

/// `_format_signatures`: fills `p.signatures`, from the directive, the docstring, or
/// the object, and records annotations for `autodoc_typehints = "description"`.
pub(crate) fn format_signatures(
    g: &mut Generator<'_>,
    p: &mut Props,
    parsed: &ParsedName,
    docstrings: &mut Docstrings,
) {
    if p.obj_type.is_class()
        && (p.doc_as_attr() || g.config.autodoc_class_signature == ClassSignature::Separated)
    {
        return;
    }
    let mut sigs = Vec::new();
    if let Some(args) = &parsed.args {
        let retann = parsed.retann.clone().unwrap_or_default();
        sigs.push(Formatted { args: args.clone(), retann });
    }
    let reads_docstring = !matches!(p.obj_type, ObjType::Module | ObjType::Data | ObjType::Type);
    if sigs.is_empty()
        && g.config.autodoc_docstring_signature
        && reads_docstring
        && let Some(docstrings) = docstrings.as_mut()
    {
        signatures_from_docstrings(g, p, docstrings, &mut sigs);
    }
    if sigs.is_empty()
        && let Some(s) = signature_from_object(g, p)
    {
        sigs.push(s);
    }
    if matches!(p.obj_type, ObjType::Attribute | ObjType::Property) {
        for s in &mut sigs {
            s.args.clear();
        }
    }
    record_hints(g, p);
    if !reads_docstring {
        sigs.truncate(1);
    }
    overloaded_signatures(g, p, &mut sigs);
    p.signatures = sigs.iter().map(Formatted::line).collect();
}

/// `_extract_signatures_from_docstrings`: leading lines of the first docstring that
/// match the object's name with a signature.
fn signatures_from_docstrings(
    g: &mut Generator<'_>,
    p: &Props,
    docstrings: &mut [Vec<String>],
    sigs: &mut Vec<Formatted>,
) {
    let mut valid = vec![p.name().to_string()];
    if let Some(c) = p.obj_class().filter(|_| p.obj_type.is_class()) {
        valid.push("__init__".into());
        let (order, _) = g.project.mro(c);
        for e in order.iter() {
            match e.class {
                Some(class) => valid.push(g.project.class(class).name.to_string()),
                None => valid.push(crate::members::short_name(&e.path).to_string()),
            }
        }
    }
    for doclines in docstrings.iter_mut() {
        let mut j = 0;
        for (k, line) in doclines.iter().enumerate() {
            j = k;
            if line.is_empty() {
                break;
            }
            let stripped = line.trim_end_matches('\\').trim_end();
            let Some(m) = match_signature(stripped) else { break };
            if !valid.iter().any(|v| v == m.base) {
                break;
            }
            let mut retann = m.retann.unwrap_or("");
            if p.obj_type.is_class() && retann == "None" {
                retann = "";
            }
            sigs.push(Formatted { args: m.args.unwrap_or("").into(), retann: retann.into() });
        }
        if !sigs.is_empty() {
            if !matches!(p.obj_type, ObjType::Data | ObjType::Attribute | ObjType::Module) {
                let rest = doclines[j..].join("\n");
                *doclines = prepare_docstring(&rest, g.config.tab_width);
            }
            return;
        }
    }
}

/// What a signature is read from.
#[derive(Debug, Clone)]
enum SignatureSource {
    /// A function; `bound` drops its first parameter, and `given` leading parameters
    /// `functools.partial` gave are left out.
    Function { function: FunctionRef, bound: bool, given: usize },
    /// A signature Python knows without a function: `()`, `(*values)`.
    Fixed(String),
}

fn signature_source(g: &mut Generator<'_>, p: &mut Props) -> Option<SignatureSource> {
    let o = &p.obj;
    let function = o.function.filter(|_| o.kind == ObjectKind::Function);
    match p.obj_type {
        // autofunction on a class documents it as a callable: its constructor's
        // signature, as inspect.signature gives it.
        ObjType::Function | ObjType::Decorator if o.kind == ObjectKind::Class => {
            let c = o.class?;
            constructor(g, p, c)
        }
        ObjType::Function | ObjType::Decorator => {
            let bound = o.wrapper == Wrapper::Classmethod && p.parent.kind == ObjectKind::Class;
            Some(SignatureSource::Function { function: function?, bound, given: o.partial_args })
        }
        ObjType::Method => {
            let bound = o.wrapper != Wrapper::Staticmethod;
            Some(SignatureSource::Function { function: function?, bound, given: o.partial_args })
        }
        ObjType::Class | ObjType::Exception => {
            let c = p.obj_class()?;
            constructor(g, p, c)
        }
        _ => None,
    }
}

/// What calling a class runs, as autodoc's `_get_object_for_signature` finds it: a
/// metaclass `__call__`, then `__new__`, then `__init__`, each only when defined in the
/// source roots.
fn constructor(g: &mut Generator<'_>, p: &mut Props, c: ClassRef) -> Option<SignatureSource> {
    // Enum classes give their own __signature__: (*values) in Python 3.12 and 3.13.
    if is_enum(g, c) {
        if enum_has_members(g, c) {
            return Some(SignatureSource::Fixed("(*values)".into()));
        }
        // An Enum without members is called to make one: the functional API.
        return Some(SignatureSource::Fixed(
            "(new_class_name, /, names, *, module=None, qualname=None, type=None, start=1, \
             boundary=None)"
                .into(),
        ));
    }
    if let Some(kind) = generated_constructor(g, c) {
        return Some(SignatureSource::Fixed(generated_signature(g, p, c, kind)));
    }
    if let Some(source) = metaclass_call(g, p, c) {
        return Some(source);
    }
    for name in ["__new__", "__init__"] {
        let m = g.project.class_attr(c, name);
        if let (ObjectKind::Function, Some(function)) = (m.kind, m.function) {
            p.signature_method = name.into();
            p.signature_class = Some(c);
            return Some(SignatureSource::Function { function, bound: true, given: 0 });
        }
    }
    inherited_constructor(g, p, c)
}

/// A metaclass's `__call__`, when the source defines one.
fn metaclass_call(g: &mut Generator<'_>, p: &mut Props, c: ClassRef) -> Option<SignatureSource> {
    let m = Arc::clone(g.project.module(c.module));
    let class = m.class(c.class);
    let outer = class.outer.map(|class| ClassRef { module: c.module, class });
    for kw in class.keywords.iter().filter(|k| &*k.name == "metaclass") {
        let Some(value) = kw.value else { continue };
        let meta = g.project.reference(&m, outer, value);
        let (ObjectKind::Class, Some(meta)) = (meta.kind, meta.class) else { continue };
        let call = g.project.class_attr(meta, "__call__");
        if let (ObjectKind::Function, Some(function)) = (call.kind, call.function) {
            p.signature_method = "__call__".into();
            p.signature_class = Some(meta);
            return Some(SignatureSource::Function { function, bound: true, given: 0 });
        }
    }
    None
}

/// No constructor in the roots: `list` and `tuple` take `(iterable=(), /)`; other
/// builtin bases such as `dict` or `Exception` have no signature Python can give, nor
/// does a TypedDict; `object`'s own gives `()`.
fn inherited_constructor(
    g: &mut Generator<'_>,
    p: &Props,
    c: ClassRef,
) -> Option<SignatureSource> {
    let (order, complete) = g.project.mro(c);
    if !complete {
        let outside = order.iter().find(|e| {
            e.class.is_none() && !e.path.starts_with("builtins.") && !abstract_base(&e.path)
        });
        if let Some(e) = outside {
            g.report(
                Severity::Note,
                "autodoc.signature",
                "CONSTRUCTOR OUTSIDE THE SOURCE PATHS",
                format!(
                    "The signature of {} comes from {}, outside the source paths, so I \
                         show none.",
                    p.full_name(),
                    e.path
                ),
                "Add the folder holding that package to autodoc_source_paths in conf.toml.",
            );
            return None;
        }
    }
    for e in order.iter() {
        if e.path.ends_with("typing.TypedDict") {
            return None;
        }
        if e.class.is_some() || e.path == "builtins.object" || abstract_base(&e.path) {
            continue;
        }
        if e.path == "builtins.list" || e.path == "builtins.tuple" {
            return Some(SignatureSource::Fixed("(iterable=(), /)".into()));
        }
        return None;
    }
    Some(SignatureSource::Fixed("()".into()))
}

/// Whether an Enum class body assigns a member.
fn enum_has_members(g: &Generator<'_>, c: ClassRef) -> bool {
    g.project
        .class(c)
        .scope
        .defs
        .iter()
        .any(|d| d.runtime && d.kind == DefKind::Assign && !d.name.starts_with('_'))
}

/// A standard base class that adds no constructor: the `collections.abc` and `typing`
/// protocols, and `abc.ABC`.
fn abstract_base(path: &str) -> bool {
    path.starts_with("collections.abc.")
        || path.starts_with("typing.")
        || path.starts_with("typing_extensions.")
        || path == "abc.ABC"
}

fn signature_from_object(g: &mut Generator<'_>, p: &mut Props) -> Option<Formatted> {
    let (function, bound, given) = match signature_source(g, p)? {
        SignatureSource::Fixed(args) => return Some(Formatted { args, retann: String::new() }),
        SignatureSource::Function { function, bound, given } => (function, bound, given),
    };
    let m = Arc::clone(g.project.module(function.module));
    let f = m.function(function.kind);
    let params = visible_params(&f.params, bound, given);
    if p.obj_type == ObjType::Decorator && params.len() == 1 {
        return Some(Formatted::default());
    }
    let show = g.config.autodoc_typehints.in_signature();
    let returns = !p.obj_type.is_class() && p.obj.kind != ObjectKind::Class;
    let mut s = stringify_signature(g, p, function, params, show && returns, show, bound);
    if g.config.strip_signature_backslash {
        s.args = s.args.replace('\\', "\\\\");
        s.retann = s.retann.replace('\\', "\\\\");
    }
    Some(s)
}

/// The parameters a signature shows: without self when bound, and without those
/// `functools.partial` gave.
fn visible_params(params: &[Param], bound: bool, given: usize) -> &[Param] {
    let mut params = params;
    let takes_self = params.first().is_some_and(|q| {
        !matches!(
            q.kind,
            ParamKind::VarPositional | ParamKind::KeywordOnly | ParamKind::VarKeyword
        )
    });
    if bound && takes_self {
        params = &params[1..];
    }
    for _ in 0..given {
        match params.first() {
            Some(q)
                if matches!(
                    q.kind,
                    ParamKind::PositionalOnly | ParamKind::PositionalOrKeyword
                ) =>
            {
                params = &params[1..];
            }
            _ => break,
        }
    }
    params
}

/// Writes a parameter list as `_stringify_signature_to_parts` does, with each
/// parameter's annotation and default from the given closures.
fn write_params(params: &[Param], mut write: impl FnMut(&mut String, usize, &Param)) -> String {
    let mut args = Vec::with_capacity(params.len() + 2);
    let mut last = ParamKind::PositionalOrKeyword;
    for (i, q) in params.iter().enumerate() {
        if i > 0 && last == ParamKind::PositionalOnly && q.kind != ParamKind::PositionalOnly {
            args.push("/".to_string());
        }
        let opens_keywords =
            i == 0 || matches!(last, ParamKind::PositionalOrKeyword | ParamKind::PositionalOnly);
        if q.kind == ParamKind::KeywordOnly && opens_keywords {
            args.push("*".to_string());
        }
        let mut b = match q.kind {
            ParamKind::VarPositional => "*".to_string(),
            ParamKind::VarKeyword => "**".to_string(),
            _ => String::new(),
        };
        b.push_str(&q.name);
        write(&mut b, i, q);
        args.push(b);
        last = q.kind;
    }
    if !params.is_empty() && last == ParamKind::PositionalOnly {
        args.push("/".to_string());
    }
    format!("({})", args.join(", "))
}

/// `_stringify_signature_to_parts`.
fn stringify_signature(
    g: &mut Generator<'_>,
    p: &Props,
    f: FunctionRef,
    params: &[Param],
    show_return: bool,
    show_annotation: bool,
    bound: bool,
) -> Formatted {
    let hints = function_hints(g, f, signature_mode(g), bound);
    let defaults = default_texts(g, p, f, params);
    let args = write_params(params, |b, i, q| {
        let annotation = annotation_of(&hints, &q.name).filter(|_| show_annotation);
        if let Some(annotation) = annotation {
            b.push_str(": ");
            b.push_str(annotation);
        }
        if let (Some(_), Some(default)) = (q.default, &defaults[i]) {
            b.push_str(if annotation.is_some() { " = " } else { "=" });
            b.push_str(default);
        }
    });
    let retann = if show_return { annotation_of(&hints, "return").unwrap_or("") } else { "" };
    Formatted { args, retann: retann.to_string() }
}

/// Each parameter's default: as written with `autodoc_preserve_defaults` (Sphinx's
/// unparser for defaults spanning lines), else as `object_description` writes it.
fn default_texts(
    g: &mut Generator<'_>,
    p: &Props,
    f: FunctionRef,
    params: &[Param],
) -> Vec<Option<String>> {
    let m = Arc::clone(g.project.module(f.module));
    if g.config.autodoc_preserve_defaults
        && let Some(texts) = preserved_defaults(g, p, &m, params)
    {
        return texts;
    }
    let owner = m.function(f.kind).class.map(|class| ClassRef { module: f.module, class });
    let mut out = Vec::with_capacity(params.len());
    for q in params {
        let Some(default) = q.default else {
            out.push(None);
            continue;
        };
        let (text, exact) = describe(g, &m, owner, Some(default));
        if !exact {
            g.report(
                Severity::Note,
                "autodoc.default",
                "DEFAULT KNOWN ONLY AT RUN TIME",
                format!(
                    "The default of {} in {} is made when the module runs; I show it as \
                         written: {text}.",
                    q.name,
                    p.full_name()
                ),
                "Set autodoc_preserve_defaults = true to show every default as written.",
            );
        }
        out.push(Some(text));
    }
    out
}

/// The defaults as written; `None`, with a warning, when Sphinx's unparser refuses one.
fn preserved_defaults(
    g: &mut Generator<'_>,
    p: &Props,
    m: &Module,
    params: &[Param],
) -> Option<Vec<Option<String>>> {
    let mut out = Vec::with_capacity(params.len());
    for q in params {
        let Some(default) = q.default else {
            out.push(None);
            continue;
        };
        let node = m.node(default);
        if node.start_row() == node.end_row() {
            out.push(Some(node.text().to_string()));
            continue;
        }
        let (text, ok) = try_unparse(node, UnparseMode::SphinxRepr);
        if !ok {
            g.report(
                Severity::Warning,
                "autodoc.default",
                "DEFAULT VALUE NOT PRESERVED",
                format!(
                    "Failed to parse a default argument value for {}: {}.",
                    p.full_name(),
                    q.name
                ),
                "Nothing to do: Sphinx cannot write this expression back either; it shows its repr.",
            );
            return None;
        }
        out.push(Some(text));
    }
    Some(out)
}

/// `_record_typehints`, for `autodoc_typehints = "description"` or `"both"`.
fn record_hints(g: &mut Generator<'_>, p: &mut Props) {
    if !g.config.autodoc_typehints.in_description() {
        return;
    }
    let Some(SignatureSource::Function { function, bound, given }) = signature_source(g, p) else {
        return;
    };
    let hints = function_hints(g, function, signature_mode(g), bound);
    let m = Arc::clone(g.project.module(function.module));
    let f = m.function(function.kind);
    let visible = visible_params(&f.params, bound, given);
    let drops_self = p.obj_type.is_class()
        || (p.obj.kind == ObjectKind::Function && p.obj.wrapper == Wrapper::Classmethod);
    let kept = hints.into_iter().filter(|a| {
        !drops_self || a.name == "return" || visible.iter().any(|q| *q.name == a.name)
    });
    p.hints = Some(kept.map(|a| Hint { name: a.name, text: a.text }).collect());
}

/// Replaces a function's signature with its `@overload` variants.
fn overloaded_signatures(g: &mut Generator<'_>, p: &Props, sigs: &mut Vec<Formatted>) {
    if g.config.autodoc_typehints == Typehints::Hidden {
        return;
    }
    match p.obj_type {
        ObjType::Function | ObjType::Decorator | ObjType::Method => {
            let Some(id) = g.project.load_module(p.canonical_module_name()) else { return };
            let m = Arc::clone(g.project.module(id));
            let Some(overloads) = m.info.overloads.get(&p.dotted_parts()) else { return };
            let drop_self = p.obj_type == ObjType::Method && !p.is_staticmethod;
            *sigs =
                overloads.iter().map(|&f| overload_signature(g, p, id, f, drop_self)).collect();
        }
        ObjType::Class | ObjType::Exception => {
            let (Some(signature_class), Some(c)) = (p.signature_class, p.obj_class()) else {
                return;
            };
            // __call__ is looked up along the metaclass's MRO, the others along the class's.
            let from = if p.signature_method == "__call__" { signature_class } else { c };
            class_overloads(g, p, from, sigs);
        }
        _ => {}
    }
}

fn class_overloads(g: &mut Generator<'_>, p: &Props, from: ClassRef, sigs: &mut Vec<Formatted>) {
    let (order, _) = g.project.mro(from);
    for class in order.iter().filter_map(|e| e.class) {
        let m = Arc::clone(g.project.module(class.module));
        let key = format!("{}.{}", m.class(class.class).qualname, p.signature_method);
        if let Some(overloads) = m.info.overloads.get(&key) {
            *sigs = overloads
                .iter()
                .map(|&f| {
                    let mut s = overload_signature(g, p, class.module, f, true);
                    s.retann.clear();
                    s
                })
                .collect();
            return;
        }
        if m.info.tagorder.contains_key(&key) {
            return;
        }
    }
}

/// `signature_from_ast` of an `@overload`, with its annotations evaluated one by one
/// and `...` defaults taken from the implementation.
fn overload_signature(
    g: &mut Generator<'_>,
    p: &Props,
    module: ModuleId,
    f: FunctionId,
    drop_first: bool,
) -> Formatted {
    let m = Arc::clone(g.project.module(module));
    let overload = &m.functions[f.0 as usize];
    let params = if drop_first && !overload.params.is_empty() {
        &overload.params[1..]
    } else {
        &overload.params[..]
    };
    let mode = signature_mode(g);
    let args = write_params(params, |b, _, q| {
        if let Some(annotation) = q.annotation {
            b.push_str(": ");
            b.push_str(&overload_annotation(g, &m, annotation, mode, p));
        }
        if let Some(default) = q.default {
            b.push_str(if q.annotation.is_some() { " = " } else { "=" });
            b.push_str(&overload_default(g, p, &m, default, &q.name));
        }
    });
    let retann = match overload.returns {
        Some(r) => overload_annotation(g, &m, r, mode, p),
        None => String::new(),
    };
    Formatted { args, retann }
}

/// An `@overload`'s default, with `...` replaced by the implementation's default of
/// the same parameter (`_merge_default_value`).
fn overload_default(
    g: &mut Generator<'_>,
    p: &Props,
    m: &Module,
    default: NodeId,
    name: &str,
) -> String {
    let written = unparse(m.node(default), UnparseMode::SphinxRepr);
    let Some(implementation) = p.obj.function.filter(|_| p.obj.kind == ObjectKind::Function)
    else {
        return written;
    };
    if written != "..." {
        return written;
    }
    let im = Arc::clone(g.project.module(implementation.module));
    let f = im.function(implementation.kind);
    let Some(i) = f.params.iter().position(|q| *q.name == *name && q.default.is_some()) else {
        return written;
    };
    let texts = default_texts(g, p, implementation, &f.params[i..=i]);
    texts.into_iter().next().flatten().unwrap_or(written)
}

/// `evaluate_signature` for one annotation of an `@overload`: the text Sphinx's
/// unparser made of it, evaluated in the `__globals__` of the implementation as
/// imported, which a decorator's wrapper function takes from the decorator's module.
fn overload_annotation(
    g: &mut Generator<'_>,
    m: &Arc<Module>,
    n: NodeId,
    mode: TypeMode,
    p: &Props,
) -> String {
    let text = unparse(m.node(n), UnparseMode::SphinxRepr);
    let mut e = Eval::default();
    let mut globals = Arc::clone(m);
    if let Some(implementation) = p.obj.function.filter(|_| p.obj.kind == ObjectKind::Function) {
        globals = Arc::clone(g.project.module(implementation.module));
        if let Some(found) = decorator_globals(g, implementation, &mut e) {
            globals = found;
        }
    }
    match eval_string(g, &mut e, &globals, &text) {
        Some(v) => stringify(&v, mode),
        None => text,
    }
}

/// The module whose globals the implementation's first decorator gives its wrapper;
/// outside the roots, the wrapper's globals hold builtins only.
fn decorator_globals(
    g: &mut Generator<'_>,
    implementation: FunctionRef,
    e: &mut Eval,
) -> Option<Arc<Module>> {
    let im = Arc::clone(g.project.module(implementation.module));
    let f = im.function(implementation.kind);
    let first = im.node(*f.decorators.first()?);
    let target = if first.kind() == "call" { first.field("function") } else { first };
    let owner = f.class.map(|class| ClassRef { module: implementation.module, class });
    let reference = g.project.reference(&im, owner, target.id()?);
    match reference.kind {
        ObjectKind::Function => {
            let found: FunctionRef = reference.function?;
            Some(Arc::clone(g.project.module(found.module)))
        }
        ObjectKind::External | ObjectKind::Builtin => {
            e.builtins_only = reference.path != "builtins.staticmethod"
                && reference.path != "builtins.classmethod";
            None
        }
        _ => None,
    }
}
