//! Annotations as `typing.get_type_hints` gives them to autodoc: of functions, modules,
//! and classes along their MRO, evaluated, or kept as written when one fails.

use std::sync::Arc;

use gd_pyscan::{ClassRef, DefKind, FunctionRef, Module, ModuleId, NodeId, Scope, UnparseMode};
use gd_pyscan::{str_value, unparse};

use crate::types::{Eval, TypeMode, eval_string, eval_type, stringify};
use crate::{Generator, Severity};

/// One annotation as autodoc writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Annotated {
    /// A parameter, `"return"`, or an attribute.
    pub(crate) name: String,
    pub(crate) text: String,
}

/// Where an annotation comes from.
#[derive(Debug, Clone)]
pub(crate) enum AnnotationSource {
    /// An annotation written in the source.
    Node(NodeId),
    /// Text: a `# type:` comment, or what the analyser unparsed from `self.x: T`.
    Text(String),
}

/// One entry of an `__annotations__`, with the module its names resolve in.
#[derive(Debug, Clone)]
pub(crate) struct ClassAnnotation {
    pub(crate) name: String,
    pub(crate) module: ModuleId,
    pub(crate) source: AnnotationSource,
}

impl ClassAnnotation {
    fn new(name: &str, module: ModuleId, source: AnnotationSource) -> ClassAnnotation {
        ClassAnnotation { name: name.to_string(), module, source }
    }
}

/// Whether a module has `from __future__ import annotations`, which keeps every
/// annotation as the string of its expression.
pub(crate) fn future_annotations(m: &Module) -> bool {
    m.scope.defs.iter().any(|d| {
        d.kind == DefKind::ImportFrom && &*d.target == "__future__" && &*d.member == "annotations"
    })
}

/// An annotation as `__annotations__` holds it when `get_type_hints` failed: the string
/// Python made of it (from `__future__`), a string written as such, or the object it
/// named when the function was defined.
pub(crate) fn raw_annotation(
    g: &mut Generator<'_>,
    m: &Module,
    n: NodeId,
    mode: TypeMode,
) -> String {
    let mut node = m.node(n);
    if node.kind() == "type" {
        node = node.named_child(0);
    }
    if future_annotations(m) {
        return strip_quotes(&unparse(node, UnparseMode::Python)).to_string();
    }
    if let Some(s) = str_value(node) {
        return strip_quotes(&s).to_string();
    }
    let mut e = Eval::default();
    if let Some(v) = eval_type(g, &mut e, m, node.id()) {
        note_approximate(g, &e);
        return stringify(&v, mode);
    }
    unparse(node, UnparseMode::Python)
}

/// `stringify_annotation`'s unquoting of a double forward reference.
fn strip_quotes(s: &str) -> &str {
    s.strip_prefix('\'').and_then(|t| t.strip_suffix('\'')).unwrap_or(s)
}

/// Reports that an evaluation named an object outside the roots by its import path.
pub(crate) fn note_approximate(g: &mut Generator<'_>, e: &Eval) {
    if !e.approximate || e.outside.is_empty() {
        return;
    }
    g.report(
        Severity::Note,
        "autodoc.outside",
        "TYPE OUTSIDE THE SOURCE PATHS",
        format!(
            "I name {} by the path it is imported from; Sphinx names it by the module \
                 that defines it, which may differ.",
            e.outside
        ),
        "Add the folder holding that package to autodoc_source_paths in conf.toml for exact \
         names.",
    );
}

/// `typing.get_type_hints(function)` as `inspect.signature` applies it: every
/// annotation evaluated, or, when one fails, all of them as written.
pub(crate) fn function_hints(
    g: &mut Generator<'_>,
    f: FunctionRef,
    mode: TypeMode,
    bound: bool,
) -> Vec<Annotated> {
    let list = function_annotations(g, f, bound);
    evaluate_all(g, &list, mode)
}

/// Evaluates every annotation of a list, or writes them all as written when one fails.
fn evaluate_all(
    g: &mut Generator<'_>,
    list: &[ClassAnnotation],
    mode: TypeMode,
) -> Vec<Annotated> {
    let mut e = Eval::default();
    let mut values = Vec::with_capacity(list.len());
    for a in list {
        match eval_class_annotation(g, &mut e, a) {
            Some(v) => values.push(v),
            None => {
                let raw = list.iter().map(|a| Annotated {
                    name: a.name.clone(),
                    text: raw_class_annotation(g, a, mode),
                });
                return raw.collect();
            }
        }
    }
    note_approximate(g, &e);
    list.iter()
        .zip(&values)
        .map(|(a, v)| Annotated { name: a.name.clone(), text: stringify(v, mode) })
        .collect()
}

/// A function's `__annotations__` as autodoc sees them: those written in the signature,
/// then (`autodoc_use_type_comments`) those of a `# type:` comment for parameters that
/// have none, matched after self when bound.
pub(crate) fn function_annotations(
    g: &Generator<'_>,
    f: FunctionRef,
    bound: bool,
) -> Vec<ClassAnnotation> {
    let type_comments = g.config.autodoc_use_type_comments;
    let m = g.project.module(f.module);
    let function = m.function(f.kind);
    let args = &function.type_args;
    let suppressed = !type_comments || args.is_empty() || (args.len() == 1 && args[0] == "...");
    let mut out = Vec::new();
    for (i, p) in function.params.iter().enumerate() {
        let comment_arg = if bound { i.checked_sub(1) } else { Some(i) }
            .and_then(|j| args.get(j))
            .filter(|_| function.type_comment && !suppressed);
        let source = match (p.annotation, comment_arg) {
            (Some(n), _) => AnnotationSource::Node(n),
            (None, Some(arg)) => AnnotationSource::Text(arg.clone()),
            (None, None) if !p.type_comment.is_empty() && type_comments => {
                AnnotationSource::Text(p.type_comment.clone())
            }
            (None, None) => continue,
        };
        out.push(ClassAnnotation::new(&p.name, f.module, source));
    }
    let returns = match function.returns {
        Some(n) => Some(AnnotationSource::Node(n)),
        None if function.type_comment && type_comments => {
            Some(AnnotationSource::Text(function.type_returns.clone()))
        }
        None => None,
    };
    if let Some(source) = returns {
        out.push(ClassAnnotation::new("return", f.module, source));
    }
    out
}

/// One name's annotation among annotations.
pub(crate) fn annotation_of<'a>(list: &'a [Annotated], name: &str) -> Option<&'a str> {
    list.iter().find(|a| a.name == name).map(|a| a.text.as_str())
}

/// The annotations a module or class body holds at run time, in order.
fn annotated_defs(scope: &Scope) -> impl Iterator<Item = (&str, NodeId)> {
    scope.defs.iter().filter_map(|d| {
        let kept = d.runtime && matches!(d.kind, DefKind::Assign | DefKind::Annotation);
        d.annotation.filter(|_| kept).map(|a| (&*d.name, a))
    })
}

/// `get_type_hints(module)`: the module's annotations evaluated, or as written when one
/// fails.
pub(crate) fn module_hints(g: &mut Generator<'_>, id: ModuleId, mode: TypeMode) -> Vec<Annotated> {
    let m = Arc::clone(g.project.module(id));
    let mut list: Vec<ClassAnnotation> = annotated_defs(&m.scope)
        .map(|(name, n)| ClassAnnotation::new(name, id, AnnotationSource::Node(n)))
        .collect();
    // Merged as for classes: the analyser's annotations (type comments) join them.
    if g.merged.contains(&m.name) {
        for key in m.info.annotation_order.iter().filter(|k| k.scope.is_empty()) {
            if !list.iter().any(|a| a.name == key.name) {
                let text = m.info.annotations[key].clone();
                list.push(ClassAnnotation::new(&key.name, id, AnnotationSource::Text(text)));
            }
        }
    }
    let mut out = Vec::new();
    let mut e = Eval::default();
    for a in &list {
        let Some(v) = eval_class_annotation(g, &mut e, a) else {
            out.clear();
            for x in &list {
                let text = raw_class_annotation(g, x, mode);
                set_hint(&mut out, &x.name, text);
            }
            return out;
        };
        set_hint(&mut out, &a.name, stringify(&v, mode));
    }
    note_approximate(g, &e);
    out
}

fn set_hint(list: &mut Vec<Annotated>, name: &str, text: String) {
    match list.iter_mut().find(|a| a.name == name) {
        Some(a) => a.text = text,
        None => list.push(Annotated { name: name.to_string(), text }),
    }
}

/// `get_type_hints(cls)`: the annotations of every class along the MRO, evaluated in
/// their modules, the most derived winning; when one fails, the class's own annotations
/// as written.
pub(crate) fn class_hints(g: &mut Generator<'_>, c: ClassRef, mode: TypeMode) -> Vec<Annotated> {
    let (order, _) = g.project.mro(c);
    let mut out = Vec::new();
    let mut e = Eval::default();
    for class in order.iter().rev().filter_map(|e| e.class) {
        for a in class_annotations(g, class) {
            let Some(v) = eval_class_annotation(g, &mut e, &a) else {
                out.clear();
                for x in class_annotations(g, c) {
                    let text = raw_class_annotation(g, &x, mode);
                    set_hint(&mut out, &x.name, text);
                }
                return out;
            };
            set_hint(&mut out, &a.name, stringify(&v, mode));
        }
    }
    note_approximate(g, &e);
    out
}

/// A class's `__annotations__`: from its body, and, once autodoc merged them
/// (`_ensure_annotations_from_type_comments`), those the analyser found on self in
/// `__init__`, kept as the text Sphinx unparsed.
pub(crate) fn class_annotations(g: &Generator<'_>, c: ClassRef) -> Vec<ClassAnnotation> {
    let m = g.project.module(c.module);
    let class = m.class(c.class);
    let mut out: Vec<ClassAnnotation> = annotated_defs(&class.scope)
        .map(|(name, n)| ClassAnnotation::new(name, c.module, AnnotationSource::Node(n)))
        .collect();
    if !g.merged.contains(&m.name) {
        return out;
    }
    for key in m.info.annotation_order.iter().filter(|k| *k.scope == *class.qualname) {
        if !out.iter().any(|a| a.name == key.name) {
            let text = m.info.annotations[key].clone();
            out.push(ClassAnnotation::new(&key.name, c.module, AnnotationSource::Text(text)));
        }
    }
    out
}

fn eval_class_annotation(
    g: &mut Generator<'_>,
    e: &mut Eval,
    a: &ClassAnnotation,
) -> Option<crate::types::TypeValue> {
    let m = Arc::clone(g.project.module(a.module));
    match &a.source {
        AnnotationSource::Node(n) => eval_type(g, e, &m, Some(*n)),
        AnnotationSource::Text(text) => eval_string(g, e, &m, text),
    }
}

fn raw_class_annotation(g: &mut Generator<'_>, a: &ClassAnnotation, mode: TypeMode) -> String {
    match &a.source {
        AnnotationSource::Node(n) => {
            let m = Arc::clone(g.project.module(a.module));
            raw_annotation(g, &m, *n, mode)
        }
        AnnotationSource::Text(text) => text.clone(),
    }
}

/// `_get_render_mode`.
pub(crate) fn render_mode(g: &Generator<'_>) -> TypeMode {
    if g.config.autodoc_typehints_format == crate::TypehintsFormat::Short {
        TypeMode::Smart
    } else {
        TypeMode::FullyQualifiedExceptTyping
    }
}

/// The mode `_stringify_signature_to_parts` uses.
pub(crate) fn signature_mode(g: &Generator<'_>) -> TypeMode {
    if g.config.autodoc_typehints_format == crate::TypehintsFormat::Short {
        TypeMode::Smart
    } else {
        TypeMode::FullyQualified
    }
}
