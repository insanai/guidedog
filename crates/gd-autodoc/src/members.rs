//! Members follow autodoc's `_gather_members`: the members `dir()` lists, annotated
//! names, and the attributes the analyser documented; each kept or skipped as
//! `_should_keep_member` decides; documented as the object type of highest priority;
//! and ordered alphabetically, by group, or by source.

use std::rc::Rc;

use gd_pyscan::{ClassRef, DefKind, ModuleId, ModuleKind, MroEntry, Object, ObjectKind};
use gd_pyscan::{Wrapper, prepare_docstring, split_lines};

use crate::dataclasses::{Generated, fields, generated_constructor};
use crate::docstrings::{getdoc, instance_doc};
use crate::hints::class_annotations;
use crate::options::contains;
use crate::props::{Props, Sentinel, annotates, load_object, outside_bases};
use crate::typehints::field_name_of;
use crate::typevars::{AliasKind, alias_kind};
use crate::values::is_builtin_type;
use crate::{ClassSignature, Generator, MemberOrder, ObjType, Severity};

/// One member found, before it is loaded.
#[derive(Debug, Clone, Default)]
struct Member {
    name: String,
    obj: Object,
    sentinel: Sentinel,
    /// The analyser's (or a slot's) documentation of it.
    docstring: Vec<String>,
    /// The class whose `__dict__` holds it, when known.
    class: Option<ClassRef>,
    /// Defined only by `object`.
    object_member: bool,
}

impl Member {
    fn new(name: &str, obj: Object) -> Member {
        Member { name: name.to_string(), obj, ..Member::default() }
    }

    fn instance_attr(name: &str, class: Option<ClassRef>, docstring: Vec<String>) -> Member {
        Member {
            name: name.to_string(),
            sentinel: Sentinel::InstanceAttr,
            class,
            docstring,
            ..Member::default()
        }
    }
}

/// A member loaded for documenting.
#[derive(Debug, Clone)]
pub(crate) struct Loaded {
    pub(crate) props: Props,
    pub(crate) is_attr: bool,
    pub(crate) indent: String,
}

/// The attributes every class inherits from `object`.
const OBJECT_ATTRS: &[&str] = &[
    "__class__",
    "__delattr__",
    "__dir__",
    "__doc__",
    "__eq__",
    "__format__",
    "__ge__",
    "__getattribute__",
    "__getstate__",
    "__gt__",
    "__hash__",
    "__init__",
    "__init_subclass__",
    "__le__",
    "__lt__",
    "__ne__",
    "__new__",
    "__reduce__",
    "__reduce_ex__",
    "__repr__",
    "__setattr__",
    "__sizeof__",
    "__str__",
    "__subclasshook__",
];

/// The attributes every module has.
const MODULE_ATTRS: &[&str] = &[
    "__builtins__",
    "__cached__",
    "__doc__",
    "__file__",
    "__loader__",
    "__name__",
    "__package__",
    "__spec__",
];

/// `_gather_members`: the members of a module or class to document, loaded and sorted.
pub(crate) fn gather_members(
    g: &mut Generator<'_>,
    p: &Props,
    want_all: bool,
    indent: &str,
    parent_modname: &str,
    analyzer: Option<ModuleId>,
) -> Vec<Loaded> {
    let member_indent =
        if p.obj_type == ObjType::Module { indent.to_string() } else { format!("{indent}   ") };
    let saved_module = std::mem::replace(&mut g.autodoc_module, p.module_name.clone());
    let saved_class = g.autodoc_class.clone();
    if let Some(first) = p.parts.first() {
        g.autodoc_class = first.clone();
    }
    let found = members_to_document(g, p, want_all);
    let namespace = if p.obj_type == ObjType::Module { String::new() } else { p.dotted_parts() };
    let mut out = Vec::new();
    for m in &found {
        let has_attr_doc =
            analyzer.is_some_and(|a| g.project.module(a).info.has_attr_doc(&namespace, &m.name));
        if !keep_member(g, p, m, want_all, has_attr_doc) {
            continue;
        }
        let is_attr = m.sentinel == Sentinel::InstanceAttr || has_attr_doc;
        let Some(t) = member_type(g, p, m, is_attr) else { continue };
        // An object imported from outside the roots is skipped by the __module__ check
        // (document_members) before anything is read of it.
        if checks_module(g, p, want_all) && !is_attr && m.obj.kind == ObjectKind::External {
            continue;
        }
        let dotted = if p.parts.is_empty() {
            m.name.clone()
        } else {
            format!("{}.{}", p.dotted_parts(), m.name)
        };
        let name = format!("{}::{dotted}", p.module_name);
        if let Some(props) = load_object(g, &name, t, parent_modname, true) {
            out.push(Loaded { props, is_attr, indent: member_indent.clone() });
        }
    }
    g.autodoc_module = saved_module;
    g.autodoc_class = saved_class;
    let order = g.options.member_order.unwrap_or(g.config.autodoc_member_order);
    sort_members(g, &mut out, order, p, analyzer);
    out
}

/// Whether members of a module must be defined in it (`members_check_module`): all of a
/// module's members, without `__all__` to say which.
pub(crate) fn checks_module(g: &Generator<'_>, p: &Props, want_all: bool) -> bool {
    if p.obj_type != ObjType::Module || !want_all {
        return false;
    }
    (g.options.ignore_module_all || p.all.is_none()) && !g.options.imported_members
}

/// Whether the options ask for a member by name (or all of them).
fn wanted(g: &Generator<'_>, p: &Props, want_all: bool, name: &str) -> bool {
    let o = &g.options;
    if want_all {
        return match &p.all {
            Some(all) if p.obj_type == ObjType::Module && !o.ignore_module_all => {
                contains(all, name)
            }
            _ => true,
        };
    }
    if o.members.includes(name) && !o.members.all {
        return true;
    }
    for n in [&o.private_members, &o.special_members] {
        if n.given && !n.all && contains(&n.names, name) {
            return true;
        }
    }
    if g.config.autodoc_class_signature == ClassSignature::Separated && p.obj_type.is_class() {
        return name == "__new__" || name == "__init__";
    }
    false
}

/// `_get_members_to_document`.
fn members_to_document(g: &mut Generator<'_>, p: &Props, want_all: bool) -> Vec<Member> {
    let mut found = Vec::new();
    if p.obj_type == ObjType::Module {
        if let Some(m) = p.module {
            module_members(g, p, m, want_all, &mut found);
        }
    } else if let Some(c) = p.obj_class() {
        class_members(g, p, c, want_all, &mut found);
        if want_all && !g.options.inherited_members.given {
            found.retain(|m| m.class == Some(c));
        }
    }
    if !want_all && !g.options.members.all {
        for name in g.options.members.names.clone() {
            if found.iter().any(|m| m.name == name) {
                continue;
            }
            g.report(
                Severity::Warning,
                "autodoc.member",
                "MEMBER NOT FOUND",
                format!(
                    "attribute {name} is listed in :members: but is missing as it was not \
                         found in object {}.",
                    p.full_name()
                ),
                "Check the spelling, or whether the member is defined where the source shows \
                 it.",
            );
        }
    }
    found
}

/// Adds `name` to a list of names unless it is there.
fn add_name(names: &mut Vec<String>, name: &str) {
    if !contains(names, name) {
        names.push(name.to_string());
    }
}

/// `dir(module)`: its runtime names, sorted.
fn module_names(g: &mut Generator<'_>, id: ModuleId) -> Vec<String> {
    let m = Rc::clone(g.project.module(id));
    let mut names = Vec::new();
    let bound =
        |d: &&gd_pyscan::Def| d.runtime && d.kind != DefKind::Annotation && &*d.name != "*";
    for d in m.scope.defs.iter().filter(bound) {
        add_name(&mut names, &d.name);
    }
    let wildcards = m
        .scope
        .defs
        .iter()
        .filter(|d| d.runtime && d.kind == DefKind::ImportFrom && &*d.member == "*");
    for d in wildcards {
        let Some(source) = g.project.load_module(&d.target) else { continue };
        let source_module = Rc::clone(g.project.module(source));
        for e in source_module.scope.defs.iter().filter(bound) {
            if g.project.exported(source, &e.name) {
                add_name(&mut names, &e.name);
            }
        }
    }
    for n in MODULE_ATTRS {
        add_name(&mut names, n);
    }
    if m.kind == ModuleKind::Package {
        add_name(&mut names, "__path__");
    }
    names.sort();
    names
}

fn module_members(
    g: &mut Generator<'_>,
    p: &Props,
    id: ModuleId,
    want_all: bool,
    found: &mut Vec<Member>,
) {
    let m = Rc::clone(g.project.module(id));
    let module_doc = |name: &str| m.info.attr_doc("", name).map(<[String]>::to_vec);
    for name in module_names(g, id) {
        if !wanted(g, p, want_all, &name) {
            continue;
        }
        let obj = g.project.module_attr(id, &name);
        if obj.is_found() {
            let mut member = Member::new(&name, obj);
            member.docstring = module_doc(&name).unwrap_or_default();
            found.push(member);
        }
    }
    for d in m.scope.defs.iter().filter(|d| d.runtime && d.kind == DefKind::Annotation) {
        if found.iter().any(|m| *m.name == *d.name) || !wanted(g, p, want_all, &d.name) {
            continue;
        }
        let doc = module_doc(&d.name).unwrap_or_default();
        found.push(Member::instance_attr(&d.name, None, doc));
    }
}

/// The names a class body puts in its `__dict__`.
fn class_dict_names(g: &Generator<'_>, c: ClassRef) -> Vec<String> {
    let mut names = Vec::new();
    for d in &g.project.class(c).scope.defs {
        if d.runtime && d.kind != DefKind::Annotation {
            add_name(&mut names, &d.name);
        }
    }
    for n in ["__module__", "__doc__", "__firstlineno__", "__static_attributes__"] {
        add_name(&mut names, n);
    }
    names
}

fn class_members(
    g: &mut Generator<'_>,
    p: &Props,
    c: ClassRef,
    want_all: bool,
    found: &mut Vec<Member>,
) {
    let (order, complete) = g.project.mro(c);
    if !complete && (g.options.inherited_members.given || !want_all) {
        note_outside_bases(g, p, c);
    }
    let mut all = Vec::new();
    for e in order.iter() {
        match e.class {
            Some(class) => {
                for n in class_dict_names(g, class) {
                    add_name(&mut all, &n);
                }
            }
            None if e.path == "builtins.object" => {
                for n in OBJECT_ATTRS {
                    add_name(&mut all, n);
                }
            }
            None => {}
        }
    }
    all.sort();
    slot_members(g, p, c, want_all, found);
    named_tuple_members(g, p, c, want_all, found);
    let own = class_dict_names(g, c);
    for name in &all {
        let Some(unmangled) = unmangle(g, c, &order, name) else { continue };
        if !wanted(g, p, want_all, &unmangled) || found.iter().any(|m| m.name == unmangled) {
            continue;
        }
        let obj = g.project.class_attr(c, name);
        let mut member = Member::new(&unmangled, obj);
        member.object_member = !member.obj.is_found();
        if contains(&own, name) {
            member.class = Some(c);
        }
        found.push(member);
    }
    for class in order.iter().filter_map(|e| e.class) {
        annotated_members(g, p, &order, class, want_all, found);
        analyser_members(g, p, c, class, want_all, found);
    }
}

fn slot_members(
    g: &mut Generator<'_>,
    p: &Props,
    c: ClassRef,
    want_all: bool,
    found: &mut Vec<Member>,
) {
    for s in g.project.slots(c).unwrap_or_default() {
        if !wanted(g, p, want_all, &s.name) || found.iter().any(|m| m.name == s.name) {
            continue;
        }
        let mut member = Member {
            sentinel: Sentinel::Slots,
            class: Some(c),
            ..Member::new(&s.name, Object::default())
        };
        if let Some(doc) = &s.doc {
            member.docstring = split_lines(doc).into_iter().map(str::to_string).collect();
        }
        found.push(member);
    }
}

/// Each field of a NamedTuple is a descriptor in the class's `__dict__`, documented by
/// its number.
fn named_tuple_members(
    g: &mut Generator<'_>,
    p: &Props,
    c: ClassRef,
    want_all: bool,
    found: &mut Vec<Member>,
) {
    if generated_constructor(g, c) != Some(Generated::NamedTuple) {
        return;
    }
    for (i, f) in fields(g, c, Generated::NamedTuple).iter().enumerate() {
        if !wanted(g, p, want_all, &f.name) || found.iter().any(|m| m.name == f.name) {
            continue;
        }
        let value =
            Object { kind: ObjectKind::Value, module: Some(c.module), ..Object::default() };
        let mut member = Member::new(&f.name, value);
        member.class = Some(c);
        member.docstring = vec![format!("Alias for field number {i}")];
        found.push(member);
    }
}

/// The names a class along the MRO annotates without assigning.
fn annotated_members(
    g: &mut Generator<'_>,
    p: &Props,
    order: &[MroEntry],
    class: ClassRef,
    want_all: bool,
    found: &mut Vec<Member>,
) {
    let qualname = g.project.class(class).qualname.to_string();
    for a in class_annotations(g, class) {
        let Some(name) = unmangle(g, class, order, &a.name) else { continue };
        if found.iter().any(|m| m.name == name) || !wanted(g, p, want_all, &name) {
            continue;
        }
        let info = &g.project.module(class.module).info;
        let doc = info.attr_doc(&qualname, &name).map(<[String]>::to_vec).unwrap_or_default();
        found.push(Member::instance_attr(&name, Some(class), doc));
    }
}

/// The attributes the analyser documented in a class along the MRO.
fn analyser_members(
    g: &mut Generator<'_>,
    p: &Props,
    c: ClassRef,
    class: ClassRef,
    want_all: bool,
    found: &mut Vec<Member>,
) {
    let m = Rc::clone(g.project.module(class.module));
    let qualname = &m.class(class.class).qualname;
    for key in m.info.attr_order.iter().filter(|k| *k.scope == **qualname) {
        let doc = &m.info.attr_docs[key];
        match found.iter_mut().find(|m| m.name == key.name) {
            None => {
                if wanted(g, p, want_all, &key.name) {
                    found.push(Member::instance_attr(&key.name, Some(class), doc.clone()));
                }
            }
            Some(member) => {
                let inherits = class == c || g.config.autodoc_inherit_docstrings;
                if !doc.is_empty() && member.docstring.is_empty() && inherits {
                    member.docstring = doc.clone();
                }
            }
        }
    }
}

fn note_outside_bases(g: &mut Generator<'_>, p: &Props, c: ClassRef) {
    let outside = outside_bases(g, c);
    if outside.is_empty() {
        return;
    }
    g.report(
        Severity::Note,
        "autodoc.members",
        "MEMBERS OUTSIDE THE SOURCE PATHS",
        format!(
            "{} derives from {outside}, outside the source paths; members it inherits \
                 from there are not documented.",
            p.full_name()
        ),
        "Add the folder holding that package to autodoc_source_paths in conf.toml.",
    );
}

/// autodoc's `unmangle`: `_Class__name` is `__name` for the class itself, and hidden
/// (`None`) when a base class mangled it.
fn unmangle(g: &Generator<'_>, c: ClassRef, order: &[MroEntry], name: &str) -> Option<String> {
    if name.ends_with("__") {
        return Some(name.to_string());
    }
    let prefix = format!("_{}__", g.project.class(c).name);
    if let Some(rest) = name.strip_prefix(&prefix) {
        return Some(format!("__{rest}"));
    }
    let mangled_by_base = order
        .iter()
        .filter_map(|e| e.class)
        .any(|class| name.starts_with(&format!("_{}__", g.project.class(class).name)));
    (!mangled_by_base).then(|| name.to_string())
}

/// `_should_keep_member`.
fn keep_member(
    g: &mut Generator<'_>,
    p: &Props,
    m: &Member,
    want_all: bool,
    has_attr_doc: bool,
) -> bool {
    let mut doc_text = String::new();
    let (mut metadata_private, mut metadata_public) = (false, false);
    if let Some(text) = member_doc(g, p, m) {
        (doc_text, metadata_private, metadata_public) = separate_metadata(&text);
    }
    if doc_text.is_empty() && !g.options.undoc_members {
        note_inherited_doc(g, p, m);
    }
    let o = &g.options;
    let has_doc = !doc_text.is_empty() || o.undoc_members;
    let is_private = if metadata_private {
        true
    } else if metadata_public {
        false
    } else {
        m.name.starts_with('_')
    };
    if o.exclude_members.given && contains(&o.exclude_members.names, &m.name) {
        return false;
    }
    if !want_all {
        return has_doc || has_attr_doc;
    }
    let filtered = filtered_inherited(g, p, m);
    let o = &g.options;
    if special_name(&m.name) {
        if m.name == "__doc__" || filtered {
            return false;
        }
        if o.special_members.includes(&m.name) {
            return has_doc;
        }
        let separated = g.config.autodoc_class_signature == ClassSignature::Separated;
        let constructor = m.name == "__new__" || m.name == "__init__";
        return separated && p.obj_type != ObjType::Module && constructor && has_doc;
    }
    if is_private {
        if !(has_attr_doc || has_doc) || !o.private_members.given || (has_doc && filtered) {
            return false;
        }
        return o.private_members.includes(&m.name);
    }
    has_attr_doc || (!filtered && has_doc)
}

/// Reports a method without a docstring in a class with bases outside the roots:
/// Sphinx would look for the docstring there (`autodoc_inherit_docstrings`).
fn note_inherited_doc(g: &mut Generator<'_>, p: &Props, m: &Member) {
    let Some(c) = p.obj_class() else { return };
    let plain_method =
        m.obj.kind == ObjectKind::Function && !special_name(&m.name) && !m.name.starts_with('_');
    if !g.config.autodoc_inherit_docstrings || !plain_method {
        return;
    }
    let mut outside = outside_bases(g, c);
    if outside.is_empty() {
        let (order, _) = g.project.mro(c);
        if let Some(e) =
            order.iter().rev().find(|e| e.class.is_none() && e.path != "builtins.object")
        {
            outside = e.path.clone();
        }
    }
    if outside.is_empty() {
        return;
    }
    g.report(
        Severity::Note,
        "autodoc.docstring",
        "DOCSTRING MAY BE INHERITED",
        format!(
            "{}.{} has no docstring, and its class derives from {outside}, which I cannot \
                 read; Sphinx would document it with the docstring it inherits from there, if \
                 any.",
            p.full_name(),
            m.name
        ),
        "Add the folder holding that package to autodoc_source_paths in conf.toml, or give \
         the method a docstring.",
    );
}

/// Matches `^__\S+__$`.
fn special_name(name: &str) -> bool {
    name.len() >= 5
        && name.starts_with("__")
        && name.ends_with("__")
        && !name.contains([' ', '\t', '\n'])
}

/// The docstring `_should_keep_member` judges: the member's own documentation, else
/// `getdoc` of the member unless it is its class's docstring.
fn member_doc(g: &mut Generator<'_>, p: &Props, m: &Member) -> Option<String> {
    if !m.docstring.is_empty() {
        return Some(m.docstring.join("\n"));
    }
    if m.sentinel != Sentinel::None || m.object_member {
        return None;
    }
    match m.obj.kind {
        // An instance's docstring is its class's, which autodoc discards, unless the
        // instance sets its own.
        ObjectKind::Value => instance_doc(g, &m.obj).flatten(),
        ObjectKind::External | ObjectKind::Builtin => None,
        _ => {
            let inherit = g.config.autodoc_inherit_docstrings;
            getdoc(g, &m.obj, inherit, p.obj_class(), &m.name).ok().flatten()
        }
    }
}

/// `sphinx.util.docstrings.separate_metadata`: `:meta ...:` fields are taken out of the
/// text; returns the text and whether it said private or public.
pub(crate) fn separate_metadata(s: &str) -> (String, bool, bool) {
    let (mut private, mut public) = (false, false);
    let mut kept = Vec::new();
    let mut in_other = false;
    for l in prepare_docstring(s, 8) {
        if l.trim().is_empty() {
            in_other = false;
            kept.push(l);
            continue;
        }
        match field_name_of(&l) {
            Some(name) if !in_other => {
                if let Some(meta) = name.strip_prefix("meta ") {
                    match meta.trim() {
                        "private" => private = true,
                        "public" => public = true,
                        _ => {}
                    }
                    continue;
                }
                kept.push(l);
            }
            _ => {
                in_other = true;
                kept.push(l);
            }
        }
    }
    (kept.join("\n"), private, public)
}

/// `_is_filtered_inherited_member`.
fn filtered_inherited(g: &mut Generator<'_>, p: &Props, m: &Member) -> bool {
    let Some(c) = p.obj_class() else { return false };
    let (order, _) = g.project.mro(c);
    let inherited = &g.options.inherited_members.names;
    let mut seen = false;
    for e in order.iter() {
        let defines = entry_defines(g, e, &m.name);
        seen |= defines;
        let name = match e.class {
            Some(class) => g.project.class(class).name.to_string(),
            None => short_name(&e.path).to_string(),
        };
        if contains(inherited, &name) && e.class != Some(c) && seen {
            return true;
        }
        if m.class.is_some() && e.class == m.class {
            return false;
        }
        if defines {
            return false;
        }
        if let Some(class) = e.class
            && annotates(g, class, &m.name)
        {
            return false;
        }
    }
    false
}

/// The last part of a dotted path.
pub(crate) fn short_name(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or(path)
}

fn entry_defines(g: &Generator<'_>, e: &MroEntry, name: &str) -> bool {
    match e.class {
        None => e.path == "builtins.object" && OBJECT_ATTRS.contains(&name),
        Some(class) => {
            let d = g.project.class(class).scope.find_def(name);
            d.is_some_and(|d| d.kind != DefKind::Annotation)
        }
    }
}

/// `_best_object_type_for_member`; `None` for a member that is not documented.
fn member_type(g: &mut Generator<'_>, p: &Props, m: &Member, is_attr: bool) -> Option<ObjType> {
    let in_class = p.obj_type != ObjType::Module;
    let callable = if in_class { ObjType::Method } else { ObjType::Function };
    if matches!(m.sentinel, Sentinel::InstanceAttr | Sentinel::Slots) {
        return Some(if in_class { ObjType::Attribute } else { ObjType::Data });
    }
    let o = &m.obj;
    if is_attr && alias_kind(g, o) != AliasKind::None {
        return Some(ObjType::Class);
    }
    let fallback = if in_class {
        Some(ObjType::Attribute)
    } else if is_attr {
        Some(ObjType::Data)
    } else {
        None
    };
    match o.kind {
        ObjectKind::Class => {
            let c = o.class?;
            Some(if g.project.is_exception(c) { ObjType::Exception } else { ObjType::Class })
        }
        ObjectKind::Function => match o.wrapper {
            Wrapper::Property | Wrapper::CachedProperty if in_class => Some(ObjType::Property),
            Wrapper::Property | Wrapper::CachedProperty => fallback,
            _ => Some(callable),
        },
        ObjectKind::Builtin => {
            let name = o.path.trim_start_matches("builtins.");
            if gd_pyscan::is_builtin_exception(name) {
                Some(ObjType::Exception)
            } else if is_builtin_type(name) {
                Some(ObjType::Class)
            } else {
                Some(callable)
            }
        }
        ObjectKind::None => m.object_member.then_some(callable),
        ObjectKind::External => {
            // Outside the roots only the name tells: a capitalised one is taken for a
            // class.
            let name = short_name(&o.path);
            let capitalised = name.len() > 1
                && name.starts_with(|c: char| c.is_ascii_uppercase())
                && name.to_uppercase() != name;
            if capitalised { Some(ObjType::Class) } else { fallback }
        }
        ObjectKind::Module | ObjectKind::Value => fallback,
    }
}

/// `_groupwise_order_key`.
fn group_key(p: &Props) -> u8 {
    match p.obj_type {
        ObjType::Exception => 10,
        ObjType::Class => 20,
        ObjType::Method if p.is_classmethod => 48,
        ObjType::Method if p.is_staticmethod => 49,
        ObjType::Method => 50,
        ObjType::Property | ObjType::Attribute => 60,
        ObjType::Function | ObjType::Decorator => 30,
        ObjType::Data => 40,
        ObjType::Type => 70,
        ObjType::Module => 0,
    }
}

/// `_sort_members`; sorts are stable, as Python's are.
fn sort_members(
    g: &Generator<'_>,
    members: &mut [Loaded],
    order: MemberOrder,
    p: &Props,
    analyzer: Option<ModuleId>,
) {
    match order {
        MemberOrder::Groupwise => {
            members.sort_by_cached_key(|m| (group_key(&m.props), m.props.full_name()))
        }
        MemberOrder::Bysource => {
            if let Some(all) = p.all.as_ref().filter(|a| {
                p.obj_type == ObjType::Module && !g.options.ignore_module_all && !a.is_empty()
            }) {
                members.sort_by_cached_key(|m| {
                    let dotted = m.props.dotted_parts();
                    all.iter().position(|n| *n == dotted).unwrap_or(all.len())
                });
            } else if let Some(a) = analyzer {
                let tagorder = &g.project.module(a).info.tagorder;
                if !tagorder.is_empty() {
                    members.sort_by_cached_key(|m| {
                        tagorder.get(&m.props.dotted_parts()).copied().unwrap_or(tagorder.len())
                    });
                }
            }
        }
        MemberOrder::Alphabetical => members.sort_by_cached_key(|m| m.props.full_name()),
    }
}
