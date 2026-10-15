//! Docstrings follow autodoc's `_get_docstring_lines`, `sphinx.util.inspect.getdoc`, and
//! `prepare_docstring`. A value's docstring is its class's: known for instances of
//! classes in the roots, reported otherwise.

use std::sync::Arc;

use gd_pyscan::{ClassRef, DefKind, DefRef, FunctionRef, ModuleKind, Object, ObjectKind};
use gd_pyscan::{Wrapper, prepare_docstring, str_value};

use crate::dataclasses::named_tuple_field;
use crate::loader::is_descriptor;
use crate::props::{Props, Sentinel, attribute_comment};
use crate::values::call_of_class;
use crate::{ClassContent, Generator, ObjType, Severity};

/// Docstrings, each as lines; `None` is `_get_docstring_lines`' None (no docstring
/// event), which differs from an empty list.
pub(crate) type Docstrings = Option<Vec<Vec<String>>>;

/// `__doc__` depends on something outside the roots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Unknown;

/// `obj.__doc__` as far as the source tells.
fn own_doc(g: &mut Generator<'_>, obj: &Object) -> Result<Option<String>, Unknown> {
    match obj.kind {
        ObjectKind::Function => {
            if obj.doc.is_some() {
                return Ok(obj.doc.clone());
            }
            if obj.partial_method {
                return Ok(None);
            }
            Ok(obj.function.and_then(|f| function_doc(g, f)))
        }
        ObjectKind::Class => Ok(obj.class.and_then(|c| g.project.class(c).doc.clone())),
        ObjectKind::Module => Ok(obj.module.and_then(|m| g.project.module(m).doc.clone())),
        ObjectKind::Value => {
            if let Some(own) = instance_doc(g, obj) {
                return Ok(own);
            }
            match call_of_class(g, obj) {
                Some(c) => Ok(g.project.class(c).doc.clone()),
                None => Err(Unknown),
            }
        }
        _ => Err(Unknown),
    }
}

fn function_doc(g: &Generator<'_>, f: FunctionRef) -> Option<String> {
    g.project.module(f.module).function(f.kind).doc.clone()
}

/// The `__doc__` an instance sets for itself: its class's `__init__` assigns a
/// parameter to `self.__doc__` (as descriptor factories do), and the call passes a
/// string for it. `Some(doc)` when the instance sets it (to a string, or to None).
pub(crate) fn instance_doc(g: &mut Generator<'_>, o: &Object) -> Option<Option<String>> {
    let c = call_of_class(g, o)?;
    let init = g.project.class_attr(c, "__init__");
    let f = init.function.filter(|_| init.kind == ObjectKind::Function)?;
    let fm = Arc::clone(g.project.module(f.module));
    let function = fm.function(f.kind);
    let param = doc_parameter(&fm, &function)?;
    // The argument's position, after self.
    let index =
        function.params.iter().rposition(|q| *q.name == *param).and_then(|i| i.checked_sub(1));
    let om = Arc::clone(g.project.module(o.module?));
    let mut positional = 0;
    for a in om.node(o.value).field("arguments").named_children() {
        match a.kind() {
            "comment" | "list_splat" | "dictionary_splat" => {}
            "keyword_argument" => {
                if a.field_text("name") == param {
                    return str_value(a.field("value")).map(|s| Some(s.into_owned()));
                }
            }
            _ => {
                if Some(positional) == index {
                    return str_value(a).map(|s| Some(s.into_owned()));
                }
                positional += 1;
            }
        }
    }
    let default = function.params.get(index? + 1)?.default?;
    let default = fm.node(default);
    match str_value(default) {
        Some(text) => Some(Some(text.into_owned())),
        None => (default.kind() == "none").then_some(None),
    }
}

/// The parameter `__init__` stores in `self.__doc__`.
fn doc_parameter<'m>(m: &'m gd_pyscan::Module, f: &gd_pyscan::Function) -> Option<&'m str> {
    let target = format!("{}.__doc__", f.params.first()?.name);
    let body = m.node(f.node).field("body");
    body.named_children()
        .filter(|s| s.kind() == "expression_statement")
        .map(|s| s.named_child(0))
        .filter(|e| e.kind() == "assignment")
        .find_map(|e| {
            let left = e.field("left");
            let right = e.field("right");
            let assigns = left.kind() == "attribute" && left.text() == target;
            (assigns && right.kind() == "identifier").then(|| right.text())
        })
}

/// `sphinx.util.inspect.getdoc`: the object's docstring, or with `allow_inherited` the
/// docstring of the same name along the MRO of `cls`.
pub(crate) fn getdoc(
    g: &mut Generator<'_>,
    obj: &Object,
    allow_inherited: bool,
    cls: Option<ClassRef>,
    name: &str,
) -> Result<Option<String>, Unknown> {
    let inherits = cls.filter(|_| !name.is_empty());
    if let Some(cls) = inherits.filter(|_| obj.wrapper == Wrapper::Classmethod)
        && let Some(doc) = classmethod_doc(g, cls, name, allow_inherited)
    {
        return Ok(doc);
    }
    let doc = own_doc(g, obj)?;
    if let (None, true, Some(cls)) = (&doc, allow_inherited, inherits) {
        let (order, _) = g.project.mro(cls);
        for class in order.iter().filter_map(|e| e.class) {
            let scope = &g.project.class(class).scope;
            let Some(i) = scope.find_def_index(name) else { continue };
            if scope.defs[i].kind == DefKind::Annotation {
                continue;
            }
            let d = DefRef { module: class.module, class: Some(class.class), index: i as u32 };
            let found = g.project.def_object(d);
            if let Ok(Some(inherited)) = own_doc(g, &found) {
                return Ok(Some(inherited));
            }
        }
    }
    Ok(doc)
}

/// The docstring a classmethod has along the MRO: the first definition with one, or
/// the first definition at all without `allow_inherited`.
fn classmethod_doc(
    g: &mut Generator<'_>,
    cls: ClassRef,
    name: &str,
    allow_inherited: bool,
) -> Option<Option<String>> {
    let (order, _) = g.project.mro(cls);
    for class in order.iter().filter_map(|e| e.class) {
        let m = g.project.module(class.module);
        let Some(d) = m.class(class.class).scope.find_def(name) else { continue };
        let (DefKind::Function, Some(f)) = (d.kind, d.function) else { continue };
        let doc = &m.functions[f.0 as usize].doc;
        if doc.is_some() || !allow_inherited {
            return Some(doc.clone());
        }
    }
    None
}

/// `object.__init__.__doc__`.
const OBJECT_INIT_DOC: &str = "Initialize self.  See help(type(self)) for accurate signature.";
/// `object.__new__.__doc__`.
const OBJECT_NEW_DOC: &str =
    "Create and return a new object.  See help(type) for accurate signature.";

fn prepared(g: &Generator<'_>, doc: &str) -> Vec<String> {
    prepare_docstring(doc, g.config.tab_width)
}

/// A known, non-empty docstring, or none.
fn non_empty(doc: Result<Option<String>, Unknown>) -> Option<String> {
    doc.ok().flatten().filter(|d| !d.is_empty())
}

/// `_get_docstring_lines`.
pub(crate) fn docstring_lines(
    g: &mut Generator<'_>,
    p: &Props,
    class_doc_from: ClassContent,
) -> Docstrings {
    let inherit = g.config.autodoc_inherit_docstrings;
    let parent_class = p.parent_class();
    match p.obj_type {
        ObjType::Class | ObjType::Exception => class_docstrings(g, p, class_doc_from),
        ObjType::Method => {
            let Some(doc) = non_empty(getdoc(g, &p.obj, inherit, parent_class, &p.object_name))
            else {
                return Some(Vec::new());
            };
            let object_doc = match p.name() {
                "__init__" => OBJECT_INIT_DOC,
                "__new__" => OBJECT_NEW_DOC,
                _ => "",
            };
            if doc == object_doc {
                return Some(Vec::new());
            }
            Some(vec![prepared(g, &doc)])
        }
        ObjType::Data => {
            if let Some(lines) = module_attr_doc(g, &p.module_name, p.name()) {
                return Some(vec![lines]);
            }
            if p.sentinel == Sentinel::Uninitialized {
                return Some(Vec::new());
            }
            value_docstring(g, p, parent_class)
        }
        ObjType::Attribute => attribute_docstrings(g, p, parent_class),
        ObjType::Type => module_attr_doc(g, &p.module_name, p.name()).map(|lines| vec![lines]),
        ObjType::Module | ObjType::Function | ObjType::Decorator | ObjType::Property => {
            let doc = non_empty(getdoc(g, &p.obj, inherit, parent_class, &p.object_name));
            Some(doc.map(|d| vec![prepared(g, &d)]).unwrap_or_default())
        }
    }
}

/// The analyser's non-empty documentation of a module-level name.
fn module_attr_doc(g: &mut Generator<'_>, module: &str, name: &str) -> Option<Vec<String>> {
    let id = g.project.load_module(module)?;
    let lines = g.project.module(id).info.attr_doc("", name)?;
    (!lines.is_empty()).then(|| lines.to_vec())
}

/// `getdoc` of a value: its class's docstring.
fn value_docstring(g: &mut Generator<'_>, p: &Props, cls: Option<ClassRef>) -> Docstrings {
    let inherit = g.config.autodoc_inherit_docstrings;
    match getdoc(g, &p.obj, inherit, cls, &p.object_name) {
        Err(Unknown) => {
            if matches!(p.obj.kind, ObjectKind::Value | ObjectKind::External) {
                g.report(
                    Severity::Note,
                    "autodoc.docstring",
                    "DOCSTRING KNOWN ONLY AT RUN TIME",
                    format!(
                        "{} has no #: comment or docstring after it; Sphinx would show \
                             the docstring of its value's type, which I cannot read.",
                        p.full_name()
                    ),
                    "Document it with a #: comment above the assignment.",
                );
            }
            Some(Vec::new())
        }
        Ok(Some(doc)) if !doc.is_empty() => Some(vec![prepared(g, &doc)]),
        Ok(_) => Some(Vec::new()),
    }
}

fn class_docstrings(g: &mut Generator<'_>, p: &Props, class_doc_from: ClassContent) -> Docstrings {
    // A TypeVar's docstring is TypeVar's own, which autodoc leaves out.
    if p.alias_kind != crate::typevars::AliasKind::None {
        return Some(Vec::new());
    }
    if p.doc_as_attr() {
        return class_variable_comment(g, p).then(Vec::new);
    }
    let c = p.obj_class()?;
    let mut docs: Vec<String> =
        g.project.class(c).doc.iter().filter(|d| !d.is_empty()).cloned().collect();
    if class_doc_from != ClassContent::Class {
        let init_doc = constructor_doc(g, c, "__init__", OBJECT_INIT_DOC)
            .or_else(|| constructor_doc(g, c, "__new__", OBJECT_NEW_DOC));
        if let Some(init_doc) = init_doc {
            if class_doc_from == ClassContent::Init {
                docs.clear();
            }
            docs.push(init_doc);
        }
    }
    Some(docs.iter().map(|d| prepared(g, d)).collect())
}

/// The docstring of a class's `__init__` or `__new__`, unless it is `object`'s.
fn constructor_doc(
    g: &mut Generator<'_>,
    c: ClassRef,
    name: &str,
    object_doc: &str,
) -> Option<String> {
    let method = g.project.class_attr(c, name);
    if !method.is_found() {
        return None;
    }
    let cls = (name == "__init__").then_some(c);
    let inherit = g.config.autodoc_inherit_docstrings;
    let doc = non_empty(getdoc(g, &method, inherit, cls, name))?;
    (doc != object_doc).then_some(doc)
}

/// Whether the analyser documented a class bound to another name, where it is bound.
pub(crate) fn class_variable_comment(g: &mut Generator<'_>, p: &Props) -> bool {
    module_attr_doc(g, &p.module_name, &p.dotted_parts()).is_some()
}

fn attribute_docstrings(g: &mut Generator<'_>, p: &Props, cls: Option<ClassRef>) -> Docstrings {
    if let Some(cls) = cls
        && let Some(lines) = attribute_comment(g, cls, p.name()).filter(|l| !l.is_empty())
    {
        return Some(vec![lines]);
    }
    if let Some(index) = named_tuple_field(g, p) {
        return Some(vec![vec![format!("Alias for field number {index}"), String::new()]]);
    }
    match p.sentinel {
        Sentinel::Slots => {
            let slots = cls.and_then(|c| g.project.slots(c)).unwrap_or_default();
            let docs = slots.iter().filter(|s| s.name == p.name()).filter_map(|s| s.doc.as_ref());
            return Some(docs.map(|d| prepare_docstring(d, 8)).collect());
        }
        Sentinel::RuntimeInstance | Sentinel::Uninitialized | Sentinel::InstanceAttr => {
            return None;
        }
        Sentinel::None => {}
    }
    // An attribute descriptor's docstring is its own (its class's); other values'
    // would be their type's, which autodoc does not show.
    if !is_descriptor(g, &p.obj) {
        return None;
    }
    match own_doc(g, &p.obj) {
        Ok(Some(doc)) if !doc.is_empty() => Some(vec![prepared(g, &doc)]),
        _ => Some(Vec::new()),
    }
}

/// `_docstring_lines_for_props`: analyser documentation first when it has the object, a
/// blank line after each docstring.
pub(crate) fn finish_docstrings(
    g: &mut Generator<'_>,
    p: &Props,
    docstrings: Docstrings,
    parent_modname: &str,
) -> Vec<String> {
    let mut docstrings = docstrings;
    let documents_attrs = matches!(p.obj_type, ObjType::Data | ObjType::Attribute | ObjType::Type);
    if !documents_attrs && !p.parts.is_empty() {
        let real = if p.obj_type.is_class() {
            p.module_name.clone()
        } else if parent_modname.is_empty() {
            p.canonical_module_name().to_string()
        } else {
            parent_modname.to_string()
        };
        if let Some(lines) = analyser_doc(g, &real, p) {
            docstrings = Some(vec![lines]);
        }
    }
    let Some(mut docstrings) = docstrings else { return Vec::new() };
    if docstrings.is_empty() {
        docstrings.push(Vec::new());
    }
    let mut out = Vec::new();
    for d in docstrings {
        let d = crate::process::process_docstring(g, p, d);
        let needs_blank = d.last().is_some_and(|l| !l.is_empty());
        out.extend(d);
        if needs_blank {
            out.push(String::new());
        }
    }
    out
}

/// The analyser's documentation of the object in module `real`.
fn analyser_doc(g: &mut Generator<'_>, real: &str, p: &Props) -> Option<Vec<String>> {
    let id = g.project.load_module(real)?;
    let m = g.project.module(id);
    if m.kind == ModuleKind::Native {
        return None;
    }
    let scope = p.parts[..p.parts.len() - 1].join(".");
    m.info.attr_doc(&scope, p.name()).map(<[String]>::to_vec)
}
