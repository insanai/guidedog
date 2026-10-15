//! `generate_autosummary_content`'s namespace: what a stub page's template is told
//! about an object (its names, and the lists of its functions, classes, methods,
//! attributes and submodules), found in the source scan.

use std::sync::Arc;

use gd_pyscan::{ClassRef, DefKind, ModuleId, ModuleKind, Object, ObjectKind, Wrapper};

use super::find::{Found, find_object, object_type};
use super::generator;
use crate::{Config, Generator, Message, ObjType, Request, Session};

/// How a stub's lists are chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StubOptions {
    /// `autosummary_imported_members`: list what a module imports too.
    pub imported_members: bool,
    /// `autosummary_ignore_module_all`: list every member, not only `__all__`.
    pub ignore_module_all: bool,
    /// `:recursive:`: list a package's submodules.
    pub recursive: bool,
}

impl Default for StubOptions {
    fn default() -> StubOptions {
        StubOptions { imported_members: false, ignore_module_all: true, recursive: false }
    }
}

/// The template variables of one stub page, with Sphinx's names. A list is empty when
/// the object's kind has no such list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Stub {
    /// `fullname`: the name the entry gave.
    pub fullname: String,
    /// `module`: the module it was found in.
    pub module: String,
    /// `objname`: its name within that module.
    pub objname: String,
    /// `name`: its last part (a class's whole name within its module).
    pub name: String,
    /// `objtype`: `module`, `class`, `function`, ...
    pub objtype: String,
    /// `class`: the class of a method or attribute.
    pub class: Option<String>,
    /// `members`: `dir()` of the object.
    pub members: Vec<String>,
    /// `inherited_members`: a class's members it does not define itself.
    pub inherited_members: Vec<String>,
    /// `functions` and `all_functions` (with private names).
    pub functions: Vec<String>,
    /// `all_functions`.
    pub all_functions: Vec<String>,
    /// `classes`.
    pub classes: Vec<String>,
    /// `all_classes`.
    pub all_classes: Vec<String>,
    /// `exceptions`.
    pub exceptions: Vec<String>,
    /// `all_exceptions`.
    pub all_exceptions: Vec<String>,
    /// `attributes`.
    pub attributes: Vec<String>,
    /// `all_attributes`.
    pub all_attributes: Vec<String>,
    /// `modules`: a package's submodules, with `:recursive:`.
    pub modules: Vec<String>,
    /// `all_modules`.
    pub all_modules: Vec<String>,
    /// `methods`: public methods and `__init__`.
    pub methods: Vec<String>,
    /// `all_methods`.
    pub all_methods: Vec<String>,
    /// The object is a package, whose submodules a recursive stub lists.
    pub is_package: bool,
}

/// The template variables for the stub page of `name`, found in the source scan with
/// no prefix; `autosummary.import` when it is not found.
pub fn stub(
    session: &mut Session,
    config: &Config,
    name: &str,
    options: StubOptions,
) -> Result<Stub, Message> {
    let req = Request::default();
    let mut g = generator(session, config, &req);
    let Some(found) = find_object(&mut g, name, &[String::new()], &mut Vec::new()) else {
        super::report_missing(&mut g, name, &[String::new()]);
        return Err(g.out.messages.remove(0));
    };
    let obj_type = object_type(&mut g, &found);
    let qualname = found.real_name.replace(&format!("{}.", found.modname), "");
    let mut out = Stub {
        fullname: found.real_name.clone(),
        module: found.modname.clone(),
        objname: qualname.clone(),
        objtype: obj_type.name().to_owned(),
        ..Stub::default()
    };
    out.name = if obj_type == ObjType::Class {
        qualname.clone()
    } else {
        qualname.rsplit('.').next().unwrap_or(&qualname).to_owned()
    };
    if matches!(obj_type, ObjType::Method | ObjType::Attribute | ObjType::Property) {
        out.class =
            Some(qualname.rsplit_once('.').map_or(qualname.as_str(), |(c, _)| c).to_owned());
    }
    match (obj_type, &found.obj) {
        (ObjType::Module, o) if o.module.is_some() => {
            module_lists(&mut g, &found, options, &mut out)
        }
        (ObjType::Class, o) if o.class.is_some() => class_lists(&mut g, &found, &mut out),
        _ => {}
    }
    Ok(out)
}

/// A member's `__module__`, when it has one: where a class or function is defined, a
/// module's own name, the module of an object from outside the roots.
fn defining_module(g: &Generator<'_>, o: &Object) -> Option<String> {
    match o.kind {
        ObjectKind::Class => o.class.map(|c| g.project.module(c.module).name.clone()),
        ObjectKind::Function => o.function.map(|f| g.project.module(f.module).name.clone()),
        ObjectKind::Module => o.module.map(|m| g.project.module(m).name.clone()),
        ObjectKind::External => o.path.rsplit_once('.').map(|(m, _)| m.to_owned()),
        ObjectKind::Builtin => Some("builtins".to_owned()),
        ObjectKind::Value | ObjectKind::None => None,
    }
}

/// `members_of`: `dir(module)`, or `__all__` when it is respected and defined.
fn module_member_names(g: &mut Generator<'_>, id: ModuleId, ignore_all: bool) -> Vec<String> {
    let all = &g.project.module(id).all;
    if !ignore_all && all.defined {
        return all.names.clone();
    }
    crate::members::module_names(g, id)
}

fn module_lists(g: &mut Generator<'_>, found: &Found, options: StubOptions, out: &mut Stub) {
    let Some(id) = found.obj.module else { return };
    let module = Arc::clone(g.project.module(id));
    let respect_all = !options.ignore_module_all && module.all.defined;
    let names = module_member_names(g, id, options.ignore_module_all);
    let mut values = Vec::with_capacity(names.len());
    for name in names {
        let value = g.project.module_attr(id, &name);
        let ty = object_type(g, &as_member(&value, Object::of_module(id), &module.name));
        values.push((name, value, ty));
    }
    for (name, value, _) in &values {
        let imported = if module.info.has_attr_doc("", name) {
            false
        } else if value.kind == ObjectKind::Module {
            true
        } else {
            defining_module(g, value).is_some_and(|m| m != module.name)
        };
        if options.imported_members || !imported || respect_all {
            out.members.push(name.clone());
        }
    }
    let imported = options.imported_members || respect_all;
    for (target, all, kind) in [
        (&mut out.functions, &mut out.all_functions, ObjType::Function),
        (&mut out.classes, &mut out.all_classes, ObjType::Class),
        (&mut out.exceptions, &mut out.all_exceptions, ObjType::Exception),
    ] {
        for (name, value, _) in values.iter().filter(|v| v.2 == kind) {
            if imported || defining_module(g, value).as_deref() == Some(module.name.as_str()) {
                all.push(name.clone());
                if !name.starts_with('_') {
                    target.push(name.clone());
                }
            }
        }
    }
    for key in module.info.attr_order.iter().filter(|k| k.scope.is_empty()) {
        if out.members.contains(&key.name) && !out.all_attributes.contains(&key.name) {
            out.all_attributes.push(key.name.clone());
            if !key.name.starts_with('_') {
                out.attributes.push(key.name.clone());
            }
        }
    }
    out.is_package = module.kind == ModuleKind::Package;
    if out.is_package && options.recursive {
        submodule_lists(g, &module, &values, respect_all, out);
    }
}

/// A module member as `_get_documenter` sees it, with the module as its parent.
fn as_member(value: &Object, parent: Object, modname: &str) -> Found {
    Found {
        real_name: String::new(),
        obj: value.clone(),
        parent: Some(parent),
        modname: modname.to_owned(),
        instance_attr: false,
    }
}

/// `_get_modules`: a package's submodules, in file name order, those the package's
/// own members hide left out.
fn submodule_lists(
    g: &mut Generator<'_>,
    module: &gd_pyscan::Module,
    values: &[(String, Object, ObjType)],
    respect_all: bool,
    out: &mut Stub,
) {
    let mut skip: Vec<String> = [&out.all_functions, &out.all_classes, &out.all_exceptions]
        .into_iter()
        .flatten()
        .chain(&out.all_attributes)
        .cloned()
        .collect();
    let mut public_members = None;
    if respect_all {
        for (name, _, _) in values.iter().filter(|v| v.2 == ObjType::Module) {
            out.all_modules.push(name.clone());
            out.modules.push(name.clone());
            skip.push(name.clone());
        }
        public_members = Some(module.all.names.clone());
    }
    for name in package_entries(module) {
        if skip.contains(&name) {
            continue;
        }
        if g.project.load_module(&format!("{}.{name}", module.name)).is_none() {
            continue;
        }
        let public = match &public_members {
            Some(all) => all.contains(&name),
            None => !name.starts_with('_'),
        };
        out.all_modules.push(name.clone());
        if public {
            out.modules.push(name);
        }
    }
}

/// `pkgutil.iter_modules(package.__path__)`: the module files and packages in the
/// package's folder, sorted, without `__init__` and `__main__`.
fn package_entries(module: &gd_pyscan::Module) -> Vec<String> {
    let Some(dir) = module.path.parent() else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut names = Vec::new();
    for entry in entries.flatten() {
        let file = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let name = if path.is_dir() {
            if !path.join("__init__.py").is_file() {
                continue;
            }
            file
        } else {
            match file.strip_suffix(".py").or_else(|| file.strip_suffix(".pyi")) {
                Some(stem) => stem.to_owned(),
                None => continue,
            }
        };
        let identifier = name.chars().all(|c| c.is_alphanumeric() || c == '_');
        if identifier && name != "__init__" && name != "__main__" && !names.contains(&name) {
            names.push(name);
        }
    }
    names.sort();
    names
}

fn class_lists(g: &mut Generator<'_>, found: &Found, out: &mut Stub) {
    let Some(c) = found.obj.class else { return };
    let own = crate::members::class_dict_names(g, c);
    let mut names = class_dir(g, c);
    out.members = names.clone();
    out.inherited_members = names.iter().filter(|n| !own.contains(n)).cloned().collect();
    names.extend(annotated_names(g, c).into_iter().filter(|n| !out.members.contains(n)));
    for name in names {
        let kind = class_member_kind(g, c, &name);
        let (all, public) = match kind {
            MemberKind::Method => (&mut out.all_methods, &mut out.methods),
            MemberKind::Attribute => (&mut out.all_attributes, &mut out.attributes),
            MemberKind::Other => continue,
        };
        all.push(name.clone());
        if !name.starts_with('_') || (kind == MemberKind::Method && name == "__init__") {
            public.push(name);
        }
    }
}

/// `dir(cls)`: the names of the class and its bases, sorted.
fn class_dir(g: &mut Generator<'_>, c: ClassRef) -> Vec<String> {
    let (order, _) = g.project.mro(c);
    let mut names: Vec<String> = Vec::new();
    for e in order.iter() {
        let more: Vec<String> = match e.class {
            Some(class) => {
                let mut own = crate::members::class_dict_names(g, class);
                own.extend(["__dict__".to_owned(), "__weakref__".to_owned()]);
                own
            }
            None if e.path == "builtins.object" => {
                crate::members::OBJECT_ATTRS.iter().map(|s| (*s).to_owned()).collect()
            }
            None => Vec::new(),
        };
        for n in more {
            if !names.contains(&n) {
                names.push(n);
            }
        }
    }
    names.sort();
    names
}

/// Names only annotated in the class or its bases, and instance attributes the
/// analyser documented (`self.x` with a `#:` comment), in that order.
fn annotated_names(g: &mut Generator<'_>, c: ClassRef) -> Vec<String> {
    let (order, _) = g.project.mro(c);
    let mut names = Vec::new();
    for class in order.iter().filter_map(|e| e.class) {
        let m = g.project.module(class.module);
        let cls = m.class(class.class);
        for d in cls.scope.defs.iter().filter(|d| d.kind == DefKind::Annotation) {
            names.push(d.name.to_string());
        }
        let qualname = cls.qualname.to_string();
        for key in m.info.attr_order.iter().filter(|k| k.scope == qualname) {
            names.push(key.name.clone());
        }
    }
    let mut seen = Vec::new();
    names.retain(|n| {
        let new = !seen.contains(n);
        seen.push(n.clone());
        new
    });
    names
}

/// How `_get_documenter` sorts a class member for the stub's lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemberKind {
    Method,
    Attribute,
    Other,
}

fn class_member_kind(g: &mut Generator<'_>, c: ClassRef, name: &str) -> MemberKind {
    let value = g.project.class_attr(c, name);
    match value.kind {
        ObjectKind::Function => match value.wrapper {
            Wrapper::Property | Wrapper::CachedProperty => MemberKind::Attribute,
            _ => MemberKind::Method,
        },
        ObjectKind::Class => MemberKind::Other,
        ObjectKind::Value | ObjectKind::External => MemberKind::Attribute,
        ObjectKind::Builtin | ObjectKind::Module => MemberKind::Other,
        ObjectKind::None => match name {
            "__class__" => MemberKind::Other,
            "__doc__"
            | "__module__"
            | "__dict__"
            | "__weakref__"
            | "__firstlineno__"
            | "__static_attributes__" => MemberKind::Attribute,
            _ if crate::members::OBJECT_ATTRS.contains(&name) => MemberKind::Method,
            _ => MemberKind::Attribute,
        },
    }
}
