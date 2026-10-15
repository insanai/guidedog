//! What sphinx.ext.coverage inspects in a module, from the source scan: its public
//! functions and classes (those it defines), each class's public methods, and whether
//! each has a docstring; and a package's submodules.

use std::sync::Arc;

use gd_pyscan::{ClassRef, ModuleKind, ObjectKind, Wrapper};

use crate::{Config, Request, Session};

/// A function or method, and whether it has a docstring.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CoveredObject {
    /// Its name within its module or class.
    pub name: String,
    /// It has a non-empty docstring.
    pub has_doc: bool,
}

/// A class, whether it has a docstring, and its own public methods.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CoveredClass {
    /// Its name within its module.
    pub name: String,
    /// It has a non-empty docstring.
    pub has_doc: bool,
    /// The methods its body defines, sorted.
    pub methods: Vec<CoveredObject>,
}

/// The public functions and classes a module defines, sorted by name.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModuleObjects {
    /// Its functions.
    pub functions: Vec<CoveredObject>,
    /// Its classes.
    pub classes: Vec<CoveredClass>,
}

/// `inspect.getmembers(module)` as the coverage builder filters it: the public functions
/// and classes defined in module `name`; `None` when it is not in the source scan.
pub fn module_objects(session: &mut Session, name: &str) -> Option<ModuleObjects> {
    let id = session.project.load_module(name)?;
    let config = Config::default();
    let req = Request::default();
    let mut g = crate::autosummary::generator(session, &config, &req);
    let mut out = ModuleObjects::default();
    for member in crate::members::module_names(&mut g, id) {
        if member.starts_with('_') {
            continue;
        }
        let obj = g.project.module_attr(id, &member);
        match (obj.kind, obj.function, obj.class) {
            (ObjectKind::Function, Some(f), _) if f.module == id => {
                let doc = g.project.module(f.module).function(f.kind).doc.clone();
                out.functions.push(CoveredObject { name: member, has_doc: non_empty(doc) });
            }
            (ObjectKind::Class, _, Some(c)) if c.module == id => {
                let has_doc = non_empty(g.project.class(c).doc.clone());
                let methods = class_methods(&mut g, c);
                out.classes.push(CoveredClass { name: member, has_doc, methods });
            }
            _ => {}
        }
    }
    Some(out)
}

fn non_empty(doc: Option<String>) -> bool {
    doc.is_some_and(|d| !d.trim().is_empty())
}

/// The public functions and methods a class body defines: properties are not methods.
fn class_methods(g: &mut crate::Generator<'_>, c: ClassRef) -> Vec<CoveredObject> {
    let mut out = Vec::new();
    let mut names = crate::members::class_dict_names(g, c);
    names.sort();
    for name in names.into_iter().filter(|n| !n.starts_with('_')) {
        let obj = g.project.class_attr(c, &name);
        let property = matches!(obj.wrapper, Wrapper::Property | Wrapper::CachedProperty);
        let (ObjectKind::Function, Some(f)) = (obj.kind, obj.function) else { continue };
        if property {
            continue;
        }
        let doc = g.project.module(f.module).function(f.kind).doc.clone();
        out.push(CoveredObject { name, has_doc: non_empty(doc) });
    }
    out
}

/// `_load_modules`: module `name` and, for a package, its submodules, recursively, in
/// no particular order; `None` when `name` is not in the source scan. `ignored` says
/// which modules (and their submodules) are left out; at most `limit` names are given.
pub fn package_modules(
    session: &mut Session,
    name: &str,
    ignored: &dyn Fn(&str) -> bool,
    limit: usize,
) -> Option<Vec<String>> {
    if ignored(name) {
        return Some(Vec::new());
    }
    let id = session.project.load_module(name)?;
    let mut out = vec![name.to_owned()];
    let mut stack = vec![Arc::clone(session.project.module(id))];
    while let Some(module) = stack.pop() {
        if module.kind != ModuleKind::Package {
            continue;
        }
        for sub in crate::autosummary::package_entries(&module) {
            if sub == "__main__" || out.len() >= limit {
                continue;
            }
            let full = format!("{}.{sub}", module.name);
            if ignored(&full) || ignored(&sub) {
                continue;
            }
            let Some(sid) = session.project.load_module(&full) else { continue };
            out.push(full);
            stack.push(Arc::clone(session.project.module(sid)));
        }
    }
    Some(out)
}
