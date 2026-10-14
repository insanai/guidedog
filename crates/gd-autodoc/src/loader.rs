//! `_make_props_from_imported_object`: the properties each kind of object has.

use std::rc::Rc;

use gd_pyscan::{ClassRef, Object, ObjectKind, UnparseMode, Wrapper, unparse};

use crate::dataclasses::named_tuple_field;
use crate::hints::{
    Annotated, annotation_of, class_hints, function_hints, module_hints, note_approximate,
    render_mode,
};
use crate::props::{Imported, Props, Sentinel};
use crate::types::{Eval, TypeValue, eval_type, restify};
use crate::typevars::{AliasKind, alias_kind, typevar_props};
use crate::values::{call_of_class, describe};
use crate::{Generator, ObjType, Severity};

/// `_make_props_from_imported_object`; `None`, reported, when the object is not what
/// the directive documents.
pub(crate) fn make_props(g: &mut Generator<'_>, im: Imported, t: ObjType) -> Option<Props> {
    let mut p = Props::new(im, t);
    let ok = match t {
        ObjType::Module => module_props(g, &mut p),
        ObjType::Class | ObjType::Exception => class_props(g, &mut p),
        ObjType::Function | ObjType::Decorator | ObjType::Method => function_props(g, &mut p),
        ObjType::Property => property_props(g, &mut p),
        ObjType::Data | ObjType::Attribute => {
            assignment_props(g, &mut p);
            true
        }
        ObjType::Type => {
            if let (ObjectKind::Value, Some(m)) = (p.obj.kind, p.obj.module) {
                p.obj_module = g.project.module(m).name.clone();
            }
            true
        }
    };
    ok.then_some(p)
}

fn module_props(g: &mut Generator<'_>, p: &mut Props) -> bool {
    let (ObjectKind::Module, Some(id)) = (p.obj.kind, p.obj.module) else { return false };
    let m = Rc::clone(g.project.module(id));
    p.module = Some(id);
    p.obj_module = m.name.clone();
    if m.all.defined && m.all.known {
        p.all = Some(m.all.names.clone());
    } else if m.all.defined {
        g.report(
            Severity::Note,
            "autodoc.all",
            "__ALL__ IS COMPUTED",
            format!(
                "I cannot read __all__ of {} without running it, so its members are \
                     chosen as if it had none.",
                m.name
            ),
            "Write __all__ as a list of strings, or use :ignore-module-all: or :members: with \
             names.",
        );
    }
    true
}

fn class_props(g: &mut Generator<'_>, p: &mut Props) -> bool {
    match p.obj.kind {
        ObjectKind::Class => {}
        ObjectKind::External | ObjectKind::Builtin => {
            // A class from outside the roots (or a builtin) bound to another name is
            // documented as an alias of the path it was imported from.
            let (module, name) = p.obj.path.rsplit_once('.').unwrap_or(("", &p.obj.path));
            (p.obj_module, p.obj_name) = (module.to_string(), name.to_string());
            if p.doc_as_attr() {
                return true;
            }
            let path = p.obj.path.clone();
            report_outside(g, p, &path);
            return false;
        }
        ObjectKind::Value if alias_kind(g, &p.obj) != AliasKind::None => {
            let kind = alias_kind(g, &p.obj);
            typevar_props(g, p, kind);
            return true;
        }
        _ => {
            g.report(
                Severity::Warning,
                "autodoc.kind",
                "NOT A CLASS",
                format!("{} is not a class as far as the source shows.", p.full_name()),
                "Use the autodoc directive for what it is, such as autodata or autofunction.",
            );
            return false;
        }
    }
    let Some(c) = p.obj.class else { return false };
    let m = Rc::clone(g.project.module(c.module));
    let class = m.class(c.class);
    p.obj_module = m.name.clone();
    p.obj_name = class.name.to_string();
    p.obj_qualname = class.qualname.to_string();
    let subscripted =
        class.bases.iter().any(|&b| matches!(m.node(b).kind(), "subscript" | "generic_type"));
    let mode = render_mode(g);
    let mut e = Eval::default();
    for &b in &class.bases {
        let Some(mut v) = eval_type(g, &mut e, &m, Some(b)) else {
            p.bases.push(unparse(m.node(b), UnparseMode::Python));
            continue;
        };
        if !subscripted && let TypeValue::Generic { origin, .. } = v {
            v = *origin;
        }
        let generic_base =
            matches!(&v, TypeValue::Typing { name } if name == "Generic" || name == "Protocol");
        if !subscripted && generic_base {
            continue;
        }
        p.bases.push(restify(&v, mode));
    }
    if p.bases.is_empty() {
        p.bases.push(":py:class:`object`".into());
    }
    note_approximate(g, &e);
    true
}

pub(crate) fn report_outside(g: &mut Generator<'_>, p: &Props, path: &str) {
    g.report(
        Severity::Warning,
        "autodoc.outside",
        "OBJECT OUTSIDE THE SOURCE PATHS",
        format!(
            "{} is {path}, which is defined outside the source paths, so I cannot read it.",
            p.full_name()
        ),
        "Add the folder holding that package to autodoc_source_paths in conf.toml.",
    );
}

fn function_props(g: &mut Generator<'_>, p: &mut Props) -> bool {
    match p.obj.kind {
        ObjectKind::Function => {}
        ObjectKind::External => {
            let path = p.obj.path.clone();
            report_outside(g, p, &path);
            return false;
        }
        ObjectKind::Value => {
            // A function made at run time, such as by a decorator factory: documented
            // without the signature only running it would give.
            let Some(m) = p.obj.module else { return false };
            let m = Rc::clone(g.project.module(m));
            let made = unparse(m.node(p.obj.value), UnparseMode::Python);
            g.report(
                Severity::Note,
                "autodoc.signature",
                "FUNCTION MADE AT RUN TIME",
                format!(
                    "{} is made by {made} when the module runs, so I cannot give its \
                         signature.",
                    p.full_name()
                ),
                "Write the signature in the directive: .. autofunction:: name(args)",
            );
            p.obj_module = m.name.clone();
            return true;
        }
        _ => {
            g.report(
                Severity::Warning,
                "autodoc.kind",
                "NOT A FUNCTION",
                format!("{} is not a function as far as the source shows.", p.full_name()),
                "Use the autodoc directive for what it is, such as autodata or autoclass.",
            );
            return false;
        }
    }
    let Some(f) = p.obj.function else { return false };
    let m = Rc::clone(g.project.module(f.module));
    let function = m.function(f.kind);
    p.obj_module = m.name.clone();
    p.obj_name = function.name.to_string();
    p.obj_qualname = function.qualname.to_string();
    p.is_async = function.is_async;
    p.is_abstract = p.obj.is_abstract;
    let in_class = p.parent.kind == ObjectKind::Class;
    match p.obj.wrapper {
        Wrapper::Staticmethod => p.is_staticmethod = in_class || p.obj_type == ObjType::Method,
        Wrapper::Classmethod => p.is_classmethod = in_class || p.obj_type == ObjType::Method,
        Wrapper::Unknown => note_unknown_decorator(g, p),
        _ => {}
    }
    if p.obj_type != ObjType::Method {
        // A function found through a class is a staticmethod or bound classmethod.
        p.is_staticmethod = p.is_staticmethod && in_class;
    }
    true
}

fn note_unknown_decorator(g: &mut Generator<'_>, p: &Props) {
    g.report(
        Severity::Note,
        "autodoc.decorator",
        "DECORATOR OUTSIDE THE SOURCE PATHS",
        format!(
            "{} is decorated with {}, which is outside the source paths; I document the \
                 function it decorates, as if the decorator kept it.",
            p.full_name(),
            p.obj.decorator
        ),
        "If the decorator makes something else, document it by hand with py: directives.",
    );
}

fn property_props(g: &mut Generator<'_>, p: &mut Props) -> bool {
    let property = matches!(p.obj.wrapper, Wrapper::Property | Wrapper::CachedProperty);
    let Some(f) = p.obj.function.filter(|_| p.obj.kind == ObjectKind::Function && property) else {
        return false;
    };
    p.is_abstract = p.obj.is_abstract;
    p.is_classmethod = p.obj.class_property;
    match p.parent_class() {
        Some(c) => {
            let class = g.project.class(c);
            p.obj_name = class.name.to_string();
            p.obj_qualname = class.qualname.to_string();
            p.obj_module = g.project.module(c.module).name.clone();
        }
        None => p.obj_module = p.module_name.clone(),
    }
    let mode = render_mode(g);
    let hints = function_hints(g, f, mode, false);
    p.property_type = annotation_of(&hints, "return").map(str::to_string);
    true
}

fn assignment_props(g: &mut Generator<'_>, p: &mut Props) {
    let mode = render_mode(g);
    merge_annotations(g, &p.parent);
    let hints: Vec<Annotated> = match (p.parent.kind, p.parent.module, p.parent.class) {
        (ObjectKind::Module, Some(m), _) => module_hints(g, m, mode),
        (ObjectKind::Class, _, Some(c)) => class_hints(g, c, mode),
        _ => Vec::new(),
    };
    p.type_annotation = annotation_of(&hints, p.last_part()).map(str::to_string);
    if let (ObjectKind::Module, Some(m)) = (p.parent.kind, p.parent.module) {
        p.obj_module = g.project.module(m).name.clone();
    }
    if p.sentinel != Sentinel::None || named_tuple_field(g, p).is_some() {
        return; // no value; a NamedTuple field is a descriptor.
    }
    let o = p.obj.clone();
    match o.kind {
        ObjectKind::Value => value_props(g, p, &o),
        ObjectKind::Class => {
            let Some(c) = o.class else { return };
            let module = &g.project.module(c.module).name;
            p.value_repr = Some(format!("<class '{module}.{}'>", g.project.class(c).qualname));
        }
        ObjectKind::Function if o.wrapper == Wrapper::Unknown => note_value(g, p, &o.decorator),
        ObjectKind::Function => {
            let Some(f) = o.function else { return };
            let qualname = g.project.module(f.module).function(f.kind).qualname.to_string();
            p.value_repr = Some(format!("<function {qualname}>"));
        }
        ObjectKind::Module => {
            let name = o.module.map(|m| g.project.module(m).name.clone()).unwrap_or_default();
            note_value(g, p, &name);
        }
        _ => note_value(g, p, &o.path),
    }
}

/// The value of an assignment: an alias, nothing for a descriptor, or its description.
fn value_props(g: &mut Generator<'_>, p: &mut Props, o: &Object) {
    if generic_alias(g, p, o) || is_descriptor(g, o) {
        return; // an attribute descriptor shows no value.
    }
    let Some(m) = o.module else { return };
    let m = Rc::clone(g.project.module(m));
    let (text, exact) = describe(g, &m, o.owner, o.value);
    if exact {
        p.value_repr = Some(text);
    } else {
        note_value(g, p, &text);
    }
    if let Some(c) = call_of_class(g, o) {
        p.obj_module = g.project.module(c.module).name.clone();
    }
}

/// `_ensure_annotations_from_type_comments`: from now on, the classes of the parent's
/// module (or of each module along a class's MRO) have the annotations the analyser
/// found (`self.name: type` in `__init__`) among their own.
fn merge_annotations(g: &mut Generator<'_>, parent: &Object) {
    if !g.config.autodoc_use_type_comments {
        return;
    }
    match (parent.kind, parent.module, parent.class) {
        (ObjectKind::Module, Some(m), _) => {
            let name = g.project.module(m).name.clone();
            g.merged.insert(name);
        }
        (ObjectKind::Class, _, Some(c)) => {
            let (order, _) = g.project.mro(c);
            for class in order.iter().filter_map(|e| e.class) {
                let name = g.project.module(class.module).name.clone();
                g.merged.insert(name);
            }
        }
        _ => {}
    }
}

/// `inspect.isattributedescriptor` for an instance of a class in the roots: its class
/// defines `__get__`, `__set__`, or `__delete__`.
pub(crate) fn is_descriptor(g: &mut Generator<'_>, o: &Object) -> bool {
    let Some(c) = call_of_class(g, o) else { return false };
    let (order, _) = g.project.mro(c);
    order.iter().filter_map(|e| e.class).any(|class: ClassRef| {
        let scope = &g.project.class(class).scope;
        ["__get__", "__set__", "__delete__"].iter().any(|n| scope.find_def(n).is_some())
    })
}

fn note_value(g: &mut Generator<'_>, p: &Props, what: &str) {
    g.report(
        Severity::Note,
        "autodoc.value",
        "VALUE KNOWN ONLY AT RUN TIME",
        format!(
            "The value of {} ({what}) is made when the module runs, so I leave out its \
                 :value:.",
            p.full_name()
        ),
        "Add :no-value: to the directive, or :annotation: with the text to show.",
    );
}

/// `inspect.isgenericalias`: an assignment of a subscripted typing or builtin generic,
/// documented as "alias of".
fn generic_alias(g: &mut Generator<'_>, p: &mut Props, o: &Object) -> bool {
    let Some(m) = o.module else { return false };
    let m = Rc::clone(g.project.module(m));
    if !matches!(m.node(o.value).kind(), "subscript" | "generic_type") {
        return false;
    }
    let mut e = Eval::default();
    let Some(v) = eval_type(g, &mut e, &m, o.value) else { return false };
    let is_alias = match &v {
        // typing.Union[...] and Optional[...] are generic aliases too.
        TypeValue::Union(_) => true,
        TypeValue::Generic { origin, .. } => match origin.as_ref() {
            TypeValue::Typing { .. } => true,
            TypeValue::Class { module, .. } => module == "builtins" || module == "collections.abc",
            _ => false,
        },
        _ => false,
    };
    if is_alias {
        p.generic_alias = true;
        p.alias_text = restify(&v, render_mode(g));
    }
    is_alias
}
