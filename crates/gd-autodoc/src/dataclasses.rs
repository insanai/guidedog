//! Dataclasses and NamedTuple classes get a constructor Python generates: its
//! parameters are the class's annotated fields, in order (a dataclass's bases first),
//! with their defaults. That generated signature is what autodoc shows; here it is made
//! from the field declarations.

use std::sync::Arc;

use gd_pyscan::{ClassRef, DefKind, Module, ModuleId, NodeId, ObjectKind};

use crate::hints::{raw_annotation, signature_mode};
use crate::props::Props;
use crate::types::{Eval, eval_type, stringify};
use crate::typevars::call_arguments;
use crate::values::describe;
use crate::{Generator, Severity};

/// Which constructor Python generates for a class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Generated {
    Dataclass,
    NamedTuple,
}

/// The constructor Python generates for a class, if it does.
pub(crate) fn generated_constructor(g: &mut Generator<'_>, c: ClassRef) -> Option<Generated> {
    let m = Arc::clone(g.project.module(c.module));
    let class = m.class(c.class);
    let outer = class.outer.map(|class| ClassRef { module: c.module, class });
    for &dec in &class.decorators {
        let node = m.node(dec);
        let target = if node.kind() == "call" { node.field("function") } else { node };
        let Some(target) = target.id() else { continue };
        let reference = g.project.reference(&m, outer, target);
        if reference.kind != ObjectKind::External || reference.path != "dataclasses.dataclass" {
            continue;
        }
        if class.scope.find_def("__init__").is_some() {
            return None;
        }
        let (_, keywords) =
            if node.kind() == "call" { call_arguments(node) } else { Default::default() };
        let no_init = keywords
            .iter()
            .any(|k| k.field_text("name") == "init" && k.field("value").kind() == "false");
        return if no_init { None } else { Some(Generated::Dataclass) };
    }
    let named_tuple = g.project.base_objects(c).iter().any(|b| {
        b.kind == ObjectKind::External
            && (b.path == "typing.NamedTuple" || b.path == "typing_extensions.NamedTuple")
    });
    named_tuple.then_some(Generated::NamedTuple)
}

/// One init field of a dataclass or NamedTuple.
#[derive(Debug, Clone)]
pub(crate) struct Field {
    pub(crate) name: String,
    module: ModuleId,
    annotation: Option<NodeId>,
    /// The value, or a `field()` call's default.
    default: Option<NodeId>,
    /// `field(default_factory=...)`: shown as `<factory>`.
    factory: bool,
}

/// A dataclass's or NamedTuple's init fields: annotated names of the class and (for
/// dataclasses) its dataclass bases, without `ClassVar` ones or `init=False` ones.
pub(crate) fn fields(g: &mut Generator<'_>, c: ClassRef, kind: Generated) -> Vec<Field> {
    let classes: Vec<ClassRef> = match kind {
        Generated::Dataclass => {
            let (order, _) = g.project.mro(c);
            let mut classes = Vec::new();
            for class in order.iter().rev().filter_map(|e| e.class) {
                if generated_constructor(g, class).is_some() {
                    classes.push(class);
                }
            }
            classes
        }
        Generated::NamedTuple => vec![c],
    };
    let mut out: Vec<Field> = Vec::new();
    for k in classes {
        let m = Arc::clone(g.project.module(k.module));
        for d in &m.class(k.class).scope.defs {
            let Some(annotation) = d.annotation else { continue };
            let declared = matches!(d.kind, DefKind::Assign | DefKind::Annotation);
            if !d.runtime || !declared || m.text(annotation).contains("ClassVar") {
                continue;
            }
            let mut f = Field {
                name: d.name.to_string(),
                module: k.module,
                annotation: Some(annotation),
                default: d.value,
                factory: false,
            };
            if !field_call(g, &m, &mut f) {
                continue;
            }
            match out.iter_mut().find(|x| x.name == f.name) {
                Some(x) => *x = f,
                None => out.push(f),
            }
        }
    }
    out
}

/// Reads `field(default=..., default_factory=..., init=...)`; false for a field the
/// constructor does not take.
fn field_call(g: &mut Generator<'_>, m: &Module, f: &mut Field) -> bool {
    let value = m.node(f.default);
    let Some(function) = value.field("function").id().filter(|_| value.kind() == "call") else {
        return true;
    };
    let callee = g.project.reference(m, None, function);
    if callee.kind != ObjectKind::External || !callee.path.ends_with("dataclasses.field") {
        return true;
    }
    f.default = None;
    for k in call_arguments(value).1 {
        let v = k.field("value");
        match k.field_text("name") {
            "default" => f.default = v.id(),
            "default_factory" => f.factory = true,
            "init" if v.kind() == "false" => return false,
            _ => {}
        }
    }
    true
}

/// Writes a generated constructor's parameters.
pub(crate) fn generated_signature(
    g: &mut Generator<'_>,
    p: &Props,
    c: ClassRef,
    kind: Generated,
) -> String {
    let show = g.config.autodoc_typehints.in_signature();
    let list = fields(g, c, kind);
    let types = field_types(g, &list);
    let mut parts = Vec::with_capacity(list.len());
    for (f, annotation) in list.iter().zip(&types) {
        let mut b = f.name.clone();
        let annotated = show && f.annotation.is_some();
        if annotated {
            b.push_str(": ");
            b.push_str(annotation);
        }
        if f.factory || f.default.is_some() {
            b.push_str(if annotated { " = " } else { "=" });
            if f.factory {
                b.push_str("<factory>");
            } else {
                let m = Arc::clone(g.project.module(f.module));
                let (text, exact) = describe(g, &m, Some(c), f.default);
                if !exact {
                    note_default(g, p, &f.name, &text);
                }
                b.push_str(&text);
            }
        }
        parts.push(b);
    }
    format!("({})", parts.join(", "))
}

/// The fields' annotations: the generated `__init__` holds them, and `get_type_hints`
/// evaluates them all, or leaves them all as written when one fails.
fn field_types(g: &mut Generator<'_>, list: &[Field]) -> Vec<String> {
    let mode = signature_mode(g);
    let mut e = Eval::default();
    let mut types = Vec::with_capacity(list.len());
    for f in list {
        let Some(annotation) = f.annotation else {
            types.push(String::new());
            continue;
        };
        let m = Arc::clone(g.project.module(f.module));
        match eval_type(g, &mut e, &m, Some(annotation)) {
            Some(v) => types.push(stringify(&v, mode)),
            None => return raw_types(g, list),
        }
    }
    types
}

fn raw_types(g: &mut Generator<'_>, list: &[Field]) -> Vec<String> {
    let mode = signature_mode(g);
    list.iter()
        .map(|f| match f.annotation {
            Some(annotation) => {
                let m = Arc::clone(g.project.module(f.module));
                raw_annotation(g, &m, annotation, mode)
            }
            None => String::new(),
        })
        .collect()
}

/// The index of the NamedTuple field an attribute is: a descriptor whose docstring is
/// `Alias for field number N`.
pub(crate) fn named_tuple_field(g: &mut Generator<'_>, p: &Props) -> Option<usize> {
    let c = p.parent_class()?;
    if p.parts.is_empty() || generated_constructor(g, c) != Some(Generated::NamedTuple) {
        return None;
    }
    fields(g, c, Generated::NamedTuple).iter().position(|f| f.name == p.last_part())
}

fn note_default(g: &mut Generator<'_>, p: &Props, name: &str, text: &str) {
    g.report(
        Severity::Note,
        "autodoc.default",
        "DEFAULT KNOWN ONLY AT RUN TIME",
        format!(
            "The default of {name} in {} is made when the module runs; I show it as \
                 written: {text}.",
            p.full_name()
        ),
        "Set autodoc_preserve_defaults = true to show defaults as written.",
    );
}
