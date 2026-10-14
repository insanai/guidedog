//! What autodoc knows of one object (`_ItemProperties`), and `_load_object_by_name`:
//! importing the object a name gives, as far as the source shows.

use gd_pyscan::{ClassRef, DefKind, ModuleId, ModuleKind, Object, ObjectKind};

use crate::docstrings::{docstring_lines, finish_docstrings};
use crate::loader::make_props;
use crate::names::{ParsedName, parse_name, quote};
use crate::signatures::format_signatures;
use crate::typevars::AliasKind;
use crate::{Generator, ObjType, Severity};

/// autodoc's markers of attributes that have no value to import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Sentinel {
    #[default]
    None,
    /// `INSTANCE_ATTR`: found only by the analyser (member discovery).
    InstanceAttr,
    /// `RUNTIME_INSTANCE_ATTRIBUTE`: set in `__init__`.
    RuntimeInstance,
    /// `UNINITIALIZED_ATTR`: annotated, never assigned.
    Uninitialized,
    /// `SLOTS_ATTR`: named in `__slots__`.
    Slots,
}

/// One annotation of an object, for `autodoc_typehints = "description"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hint {
    pub(crate) name: String,
    pub(crate) text: String,
}

/// `_ItemProperties` and its subclasses: what autodoc knows of one object.
#[derive(Debug, Clone)]
pub(crate) struct Props {
    pub(crate) obj_type: ObjType,
    pub(crate) module_name: String,
    pub(crate) parts: Vec<String>,
    pub(crate) docstring_lines: Vec<String>,
    pub(crate) signatures: Vec<String>,
    pub(crate) obj: Object,
    pub(crate) sentinel: Sentinel,
    pub(crate) parent: Object,
    pub(crate) object_name: String,
    /// `__module__`; `""` is None.
    pub(crate) obj_module: String,
    /// Classes: `__name__`; `""` is None.
    pub(crate) obj_name: String,
    pub(crate) obj_qualname: String,
    /// Restified, for `Bases:`.
    pub(crate) bases: Vec<String>,
    /// The constructor method a class signature came from.
    pub(crate) signature_method: String,
    /// The class (or metaclass) that defines it.
    pub(crate) signature_class: Option<ClassRef>,
    pub(crate) is_async: bool,
    pub(crate) is_classmethod: bool,
    pub(crate) is_staticmethod: bool,
    pub(crate) is_abstract: bool,
    pub(crate) property_type: Option<String>,
    pub(crate) type_annotation: Option<String>,
    /// The value's repr, when it is known.
    pub(crate) value_repr: Option<String>,
    pub(crate) generic_alias: bool,
    /// The `alias of` target of a generic alias, TypeVar, or NewType, restified.
    pub(crate) alias_text: String,
    /// A TypeVar or NewType documented as a class.
    pub(crate) alias_kind: AliasKind,
    /// The docstring has `:meta hide-value:`.
    pub(crate) hide_value: bool,
    /// Modules: `__all__`, when it is known.
    pub(crate) all: Option<Vec<String>>,
    /// Modules: the module.
    pub(crate) module: Option<ModuleId>,
    /// Annotations for `autodoc_typehints = "description"`, when recorded.
    pub(crate) hints: Option<Vec<Hint>>,
}

impl Props {
    /// Properties of an imported object, before its kind fills in the rest.
    pub(crate) fn new(im: Imported, obj_type: ObjType) -> Props {
        Props {
            obj_type,
            module_name: im.module_name,
            parts: im.parts,
            docstring_lines: Vec::new(),
            signatures: Vec::new(),
            obj: im.obj,
            sentinel: im.sentinel,
            parent: im.parent,
            object_name: im.object_name,
            obj_module: String::new(),
            obj_name: String::new(),
            obj_qualname: String::new(),
            bases: Vec::new(),
            signature_method: String::new(),
            signature_class: None,
            is_async: false,
            is_classmethod: false,
            is_staticmethod: false,
            is_abstract: false,
            property_type: None,
            type_annotation: None,
            value_repr: None,
            generic_alias: false,
            alias_text: String::new(),
            alias_kind: AliasKind::None,
            hide_value: false,
            all: None,
            module: im.module,
            hints: None,
        }
    }

    pub(crate) fn full_name(&self) -> String {
        if self.obj_type == ObjType::Module {
            return self.module_name.clone();
        }
        format!("{}.{}", self.module_name, self.dotted_parts())
    }

    pub(crate) fn dotted_parts(&self) -> String {
        self.parts.join(".")
    }

    pub(crate) fn name(&self) -> &str {
        if self.obj_type == ObjType::Module {
            return self.module_name.rsplit('.').next().unwrap_or(&self.module_name);
        }
        self.last_part()
    }

    /// The last of the dotted parts, or `""` when there are none.
    pub(crate) fn last_part(&self) -> &str {
        self.parts.last().map_or("", String::as_str)
    }

    pub(crate) fn canonical_module_name(&self) -> &str {
        if self.obj_module.is_empty() { &self.module_name } else { &self.obj_module }
    }

    /// A class documented under another name is documented as an alias.
    pub(crate) fn doc_as_attr(&self) -> bool {
        if !self.obj_type.is_class() {
            return false;
        }
        self.obj_name.is_empty() || self.last_part() != self.obj_name
    }

    pub(crate) fn canonical_full_name(&self) -> Option<String> {
        let module = self.canonical_module_name();
        let qualname =
            if self.obj_qualname.is_empty() { &self.obj_name } else { &self.obj_qualname };
        if module.is_empty() || qualname.is_empty() || qualname.contains("<locals>") {
            return None;
        }
        Some(format!("{module}.{qualname}"))
    }

    /// The class the parent object is, if it is one.
    pub(crate) fn parent_class(&self) -> Option<ClassRef> {
        self.parent.class.filter(|_| self.parent.kind == ObjectKind::Class)
    }

    /// The class the object is, if it is one.
    pub(crate) fn obj_class(&self) -> Option<ClassRef> {
        self.obj.class.filter(|_| self.obj.kind == ObjectKind::Class)
    }
}

/// `_ImportedObject`.
#[derive(Debug, Clone, Default)]
pub(crate) struct Imported {
    pub(crate) module: Option<ModuleId>,
    pub(crate) module_name: String,
    pub(crate) parts: Vec<String>,
    pub(crate) parent: Object,
    pub(crate) object_name: String,
    pub(crate) obj: Object,
    pub(crate) sentinel: Sentinel,
}

/// `_load_object_by_name`: imports the object a name gives, and gathers its properties,
/// docstring, and signature. `None` when it cannot be imported (reported unless
/// `quiet`).
pub(crate) fn load_object(
    g: &mut Generator<'_>,
    name: &str,
    t: ObjType,
    parent_modname: &str,
    quiet: bool,
) -> Option<Props> {
    let parsed = parse_name(g, name, t)?;
    let im = import_object(g, &parsed, t, name, quiet)?;
    let mut props = make_props(g, im, t)?;
    let class_doc_from = g.options.class_doc_from.unwrap_or(g.config.autoclass_content);
    let mut docstrings = docstring_lines(g, &props, class_doc_from);
    props.hide_value = docstrings.iter().flatten().flatten().any(|l| hide_value_line(l));
    format_signatures(g, &mut props, &parsed, &mut docstrings);
    props.docstring_lines = finish_docstrings(g, &props, docstrings, parent_modname);
    Some(props)
}

/// Whether a docstring line is `:meta hide-value:`.
fn hide_value_line(l: &str) -> bool {
    let Some(after_meta) = l.strip_prefix(":meta") else { return false };
    let rest = after_meta.trim_start();
    if rest.len() == after_meta.len() {
        return false;
    }
    match rest.strip_prefix("hide-value:") {
        Some(after) => after.is_empty() || after.starts_with(' '),
        None => false,
    }
}

/// `_import_object`: the module, with parents tried when a dotted name is not a module,
/// then getattr along the path; data and attributes that have no value (annotated
/// only, or set in `__init__`) are found through the analyser.
fn import_object(
    g: &mut Generator<'_>,
    parsed: &ParsedName,
    t: ObjType,
    written: &str,
    quiet: bool,
) -> Option<Imported> {
    let mut im = Imported {
        module_name: parsed.module.clone(),
        parts: parsed.parts.clone(),
        ..Imported::default()
    };
    let mut walk = parsed.parts.clone();
    let module = loop {
        if let Some(id) = g.project.load_module(&im.module_name) {
            if g.project.module(id).kind != ModuleKind::Native {
                break id;
            }
            if !quiet {
                report_native(g, id, written);
            }
            return None;
        }
        let Some((parent, last)) = im.module_name.rsplit_once('.') else {
            if !quiet {
                report_no_module(g, &parsed.module, written);
            }
            return None;
        };
        walk.insert(0, last.to_string());
        im.module_name = parent.to_string();
    };
    im.module = Some(module);
    let mut obj = Object::of_module(module);
    for (i, attr) in walk.iter().enumerate() {
        im.parent = obj.clone();
        let found = g.project.get_attr(&obj, &mangle(g, &obj, attr));
        if !found.is_found() {
            if i == walk.len() - 1 && declared_without_value(g, &mut im, t, attr) {
                im.object_name.clear();
                return Some(im);
            }
            if !quiet {
                report_no_attribute(g, &im.module_name, &walk, i, &obj, written);
            }
            return None;
        }
        obj = found;
        im.object_name = attr.clone();
    }
    im.obj = obj;
    Some(im)
}

/// `_import_data_declaration` and `_import_attribute_declaration`.
fn declared_without_value(
    g: &mut Generator<'_>,
    im: &mut Imported,
    t: ObjType,
    name: &str,
) -> bool {
    match (t, im.parent.kind) {
        (ObjType::Data, ObjectKind::Module) => {
            let Some(m) = im.parent.module else { return false };
            let annotated = g.project.module(m).scope.find_def(name);
            if !annotated.is_some_and(|d| d.kind == DefKind::Annotation) {
                return false;
            }
            im.sentinel = Sentinel::Uninitialized;
            im.obj = Object::default();
            true
        }
        (ObjType::Attribute, ObjectKind::Class) => {
            let Some(c) = im.parent.class else { return false };
            let slots = g.project.slots(c).unwrap_or_default();
            im.sentinel = if slots.iter().any(|s| s.name == name) {
                Sentinel::Slots
            } else if runtime_instance_attribute(g, c, name) {
                Sentinel::RuntimeInstance
            } else if annotated_in_class(g, c, name) {
                Sentinel::Uninitialized
            } else {
                return false;
            };
            true
        }
        _ => false,
    }
}

/// `_is_runtime_instance_attribute`: the analyser of a class along the MRO documents
/// the name, or saw it set in `__init__`.
fn runtime_instance_attribute(g: &mut Generator<'_>, c: ClassRef, name: &str) -> bool {
    if attribute_comment(g, c, name).is_some() {
        return true;
    }
    let (order, _) = g.project.mro(c);
    order.iter().filter_map(|e| e.class).any(|class| {
        let key = format!("{}.{name}", g.project.class(class).qualname);
        g.project.module(class.module).info.tagorder.contains_key(&key)
    })
}

/// `_get_attribute_comment`: the analyser's documentation of `name` in the first class
/// along the MRO that has one.
pub(crate) fn attribute_comment(
    g: &mut Generator<'_>,
    c: ClassRef,
    name: &str,
) -> Option<Vec<String>> {
    let (order, _) = g.project.mro(c);
    order.iter().filter_map(|e| e.class).find_map(|class| {
        let qualname = &g.project.class(class).qualname;
        let info = &g.project.module(class.module).info;
        info.attr_doc(qualname, name).map(<[String]>::to_vec)
    })
}

fn annotated_in_class(g: &mut Generator<'_>, c: ClassRef, name: &str) -> bool {
    let (order, _) = g.project.mro(c);
    order.iter().filter_map(|e| e.class).any(|class| annotates(g, class, name))
}

/// Whether a class body annotates `name` at run time.
pub(crate) fn annotates(g: &Generator<'_>, c: ClassRef, name: &str) -> bool {
    g.project
        .class(c)
        .scope
        .defs
        .iter()
        .any(|d| &*d.name == name && d.runtime && d.annotation.is_some())
}

/// `_mangle_name`: `__private` names of a class are `_Class__private`.
fn mangle(g: &Generator<'_>, obj: &Object, name: &str) -> String {
    match obj.class.filter(|_| obj.kind == ObjectKind::Class) {
        Some(c) if name.starts_with("__") && !name.ends_with("__") => {
            format!("_{}{name}", g.project.class(c).name)
        }
        _ => name.to_string(),
    }
}

fn report_no_module(g: &mut Generator<'_>, module: &str, written: &str) {
    let roots: Vec<String> = g.project.roots().iter().map(|r| r.display().to_string()).collect();
    let roots = if roots.is_empty() { "(none)".to_string() } else { roots.join(", ") };
    let text = format!(
        "autodoc: failed to import {}: no module named {} in the source paths I looked in: \
         {roots}.",
        quote(written),
        quote(module)
    );
    g.report(
        Severity::Warning,
        "autodoc.import",
        "PYTHON MODULE NOT FOUND",
        text,
        "Check the name, or add the folder that holds the package to autodoc_source_paths \
         in conf.toml.",
    );
}

fn report_native(g: &mut Generator<'_>, id: ModuleId, written: &str) {
    let m = g.project.module(id);
    let (name, path) = (quote(&m.name), m.path.display().to_string());
    if m.too_deep {
        let text = format!(
            "autodoc: {} is in {name} ({path}), whose syntax nests too deeply to analyse \
             (see pyscan.MAX_SYNTAX_DEPTH).",
            quote(written)
        );
        g.report(
            Severity::Warning,
            "autodoc.too_deep",
            "SOURCE NESTS TOO DEEPLY",
            text,
            "Split the deeply nested expression, such as a long chain of + operands.",
        );
        return;
    }
    let text = format!(
        "autodoc: {} is in {name}, which is compiled ({path}) or unreadable; Guidedog reads \
         Python source and never imports modules.",
        quote(written)
    );
    g.report(
        Severity::Warning,
        "autodoc.native",
        "COMPILED MODULE CANNOT BE READ",
        text,
        "Document it with py: directives written by hand, or provide a .py source.",
    );
}

fn report_no_attribute(
    g: &mut Generator<'_>,
    module: &str,
    walk: &[String],
    at: usize,
    owner: &Object,
    written: &str,
) {
    let missing = &walk[at];
    let place = if at > 0 { format!("{module}.{}", walk[..at].join(".")) } else { module.into() };
    let mut text = format!(
        "autodoc: failed to import {}: {} has no attribute {}",
        quote(written),
        quote(&place),
        quote(missing)
    );
    let outside_hint = "Add the folder holding that package to autodoc_source_paths in conf.toml.";
    let mut hint = "Check the name and the module it is defined or re-exported in.";
    match (owner.kind, owner.class, owner.module) {
        (ObjectKind::Class, Some(c), _) => {
            let outside = outside_bases(g, c);
            if !outside.is_empty() {
                text.push_str(&format!(
                    " that I can see; it derives from {outside}, which is outside the source paths"
                ));
                hint = outside_hint;
            }
        }
        (ObjectKind::External, _, _) => {
            text = format!(
                "autodoc: failed to import {}: {} is outside the source paths, so its \
                 attributes are unknown",
                quote(written),
                quote(&owner.path)
            );
            hint = outside_hint;
        }
        (ObjectKind::Module, _, Some(m)) => {
            let path = g.project.module(m).path.display().to_string();
            text.push_str(&format!(" (looked in {path})"));
        }
        _ => {}
    }
    text.push('.');
    g.report(Severity::Warning, "autodoc.import", "PYTHON OBJECT NOT FOUND", text, hint);
}

/// Names the classes of an MRO that lie outside the source roots.
pub(crate) fn outside_bases(g: &mut Generator<'_>, c: ClassRef) -> String {
    let (order, _) = g.project.mro(c);
    let outside: Vec<&str> = order
        .iter()
        .filter(|e| e.class.is_none() && !e.path.starts_with("builtins."))
        .map(|e| e.path.as_str())
        .collect();
    outside.join(", ")
}
