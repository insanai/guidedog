//! `import_by_name` and `_get_documenter`: finding the object an autosummary entry
//! names, from the source scan, and the kind of object it is.

use gd_pyscan::{Object, ObjectKind, Wrapper};

use crate::{Generator, ObjType};

/// An object an entry named.
#[derive(Debug, Clone)]
pub(crate) struct Found {
    /// The name it was found under, with the prefix that found it.
    pub(crate) real_name: String,
    pub(crate) obj: Object,
    /// The module or class holding it; `None` for a module.
    pub(crate) parent: Option<Object>,
    /// The module it was found in.
    pub(crate) modname: String,
    /// An instance attribute, known only from the analyser (`INSTANCE_ATTR`).
    pub(crate) instance_attr: bool,
}

/// `get_import_prefixes_from_env`: the current class, the current module, then none.
pub(crate) fn import_prefixes(module: &str, class: &str) -> Vec<String> {
    let mut prefixes = Vec::new();
    if !class.is_empty() {
        prefixes.push(if module.is_empty() {
            class.to_owned()
        } else {
            format!("{module}.{class}")
        });
    }
    if !module.is_empty() {
        prefixes.push(module.to_owned());
    }
    prefixes.push(String::new());
    prefixes
}

/// `import_by_name`, then `import_ivar_by_name`: `name` under the first prefix that
/// finds it. A prefix the name already starts with is skipped, as Sphinx skips it
/// (`autosummary.import_cycle`), and named in `cycles`.
pub(crate) fn find_object(
    g: &mut Generator<'_>,
    name: &str,
    prefixes: &[String],
    cycles: &mut Vec<String>,
) -> Option<Found> {
    if let Some(found) = find_plain(g, name, prefixes, cycles) {
        return Some(found);
    }
    let (head, attr) = name.rsplit_once('.')?;
    let owner = find_plain(g, head, prefixes, &mut Vec::new())?;
    let class = owner.obj.class.filter(|_| owner.obj.kind == ObjectKind::Class)?;
    let (order, _) = g.project.mro(class);
    for c in order.iter().filter_map(|e| e.class) {
        let m = g.project.module(c.module);
        let known = m.info.attr_docs.keys().any(|k| k.name == attr)
            || m.info.annotations.keys().any(|k| k.name == attr);
        if known {
            return Some(Found {
                real_name: format!("{}.{attr}", owner.real_name),
                obj: Object::default(),
                parent: Some(owner.obj),
                modname: owner.modname,
                instance_attr: true,
            });
        }
    }
    None
}

fn find_plain(
    g: &mut Generator<'_>,
    name: &str,
    prefixes: &[String],
    cycles: &mut Vec<String>,
) -> Option<Found> {
    for prefix in prefixes {
        if !prefix.is_empty() && name.starts_with(&format!("{prefix}.")) {
            cycles.push(prefix.clone());
            continue;
        }
        let prefixed =
            if prefix.is_empty() { name.to_owned() } else { format!("{prefix}.{name}") };
        if let Some((obj, parent, modname)) = import_name(g, &prefixed) {
            return Some(Found {
                real_name: prefixed,
                obj,
                parent,
                modname,
                instance_attr: false,
            });
        }
    }
    None
}

/// `_import_by_name`: `MODULE.OBJ` first, then the longest module prefix and the
/// attributes after it.
fn import_name(g: &mut Generator<'_>, name: &str) -> Option<(Object, Option<Object>, String)> {
    let parts: Vec<&str> = name.split('.').collect();
    if parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    if parts.len() > 1 {
        let modname = parts[..parts.len() - 1].join(".");
        if let Some(id) = g.project.load_module(&modname) {
            let module = Object::of_module(id);
            let obj = g.project.get_attr(&module, parts[parts.len() - 1]);
            if obj.is_found() {
                return Some((obj, Some(module), modname));
            }
        }
    }
    let (j, id) = (1..=parts.len()).rev().find_map(|j| {
        let id = g.project.load_module(&parts[..j].join("."))?;
        Some((j, id))
    })?;
    let modname = parts[..j].join(".");
    let mut obj = Object::of_module(id);
    let mut parent = None;
    for part in &parts[j..] {
        let next = g.project.get_attr(&obj, part);
        if !next.is_found() {
            return None;
        }
        parent = Some(std::mem::replace(&mut obj, next));
    }
    Some((obj, parent, modname))
}

/// `_get_documenter`: the autodoc object type that documents `found`.
pub(crate) fn object_type(g: &mut Generator<'_>, found: &Found) -> ObjType {
    let in_class = found.parent.as_ref().is_some_and(|p| p.kind == ObjectKind::Class);
    if found.instance_attr {
        return ObjType::Attribute;
    }
    let o = &found.obj;
    match o.kind {
        ObjectKind::Module => ObjType::Module,
        ObjectKind::Class => match o.class {
            Some(c) if g.project.is_exception(c) => ObjType::Exception,
            _ => ObjType::Class,
        },
        ObjectKind::Function => match (o.wrapper, in_class) {
            (Wrapper::Property | Wrapper::CachedProperty, true) => ObjType::Property,
            (Wrapper::Property | Wrapper::CachedProperty, false) => ObjType::Data,
            (_, true) => ObjType::Method,
            (_, false) => ObjType::Function,
        },
        ObjectKind::Builtin => {
            let name = o.path.trim_start_matches("builtins.");
            if gd_pyscan::is_builtin_exception(name) {
                ObjType::Exception
            } else if crate::values::is_builtin_type(name) {
                ObjType::Class
            } else if in_class {
                ObjType::Method
            } else {
                ObjType::Function
            }
        }
        ObjectKind::Value | ObjectKind::External | ObjectKind::None => {
            if in_class {
                ObjType::Attribute
            } else {
                ObjType::Data
            }
        }
    }
}
