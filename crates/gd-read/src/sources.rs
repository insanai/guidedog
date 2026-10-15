//! sphinx.ext.viewcode and sphinx.ext.linkcode, while reading: where each Python object
//! a document describes is defined. The source scan (gd-pyscan) finds the object, the
//! module that defines it and the lines of its definition, without running anything.
//! What it finds becomes the document's `SourceLocation`s, and the Python files read
//! become its dependencies, so editing one reads the document again. The link pass adds
//! the `[source]` links from them, and the HTML builders write the source pages.

use std::collections::BTreeSet;
use std::path::PathBuf;

use gd_conf::Config;
use gd_env::{DocInfo, SourceLocation};
use gd_pyscan::{Module, NodeId, Object, ObjectKind, Project};
use gd_report::Report;
use guidedoc::{AttributeKey, Document, NodeKind};

use crate::files::SourceFiles;

/// Whether reading records source locations: viewcode or linkcode is listed.
pub(crate) fn wanted(c: &Config) -> bool {
    c.has_extension("sphinx.ext.viewcode") || c.has_extension("sphinx.ext.linkcode")
}

/// One Python object description: its module as written, its name within it, and the
/// id of its signature.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Described {
    module: String,
    fullname: String,
    anchor: String,
}

/// Finds where the Python objects `doc` describes are defined, and records them and the
/// files read in `d`. A module the scan cannot find is a note in `reports`, once per
/// module.
pub(crate) fn locate_objects(
    c: &Config,
    doc: Document<'_>,
    d: &mut DocInfo,
    reports: &mut Vec<Report>,
) {
    let described = python_descriptions(doc);
    if described.is_empty() {
        return;
    }
    let roots = gd_conf::autodoc_roots(c);
    let files = SourceFiles::new(&roots);
    let reader = files.clone();
    let reader = Box::new(move |path: &std::path::Path| reader.read(path));
    let Ok(mut project) = Project::with_reader(&roots, reader) else { return };
    let follow = c.ext.viewcode.follow_imported_members;
    let mut missing = BTreeSet::new();
    for object in &described {
        if project.load_module(&object.module).is_none() {
            missing.insert(object.module.as_str());
            continue;
        }
        if let Some(location) = locate(&mut project, object, follow) {
            d.locations.push(location);
        }
    }
    for module in missing {
        reports.push(missing_module(&d.relative, module, &roots));
    }
    // Nothing is noted missing, so no placeholder dependency is added.
    files.dependencies("", &mut d.deps);
}

/// The `py` object descriptions of `doc` with a module, one per name and description,
/// as viewcode links them.
fn python_descriptions(doc: Document<'_>) -> Vec<Described> {
    let mut out: Vec<Described> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut desc_end = None;
    for i in 0..doc.node_count() {
        let id = guidedoc::NodeId::from_index(i);
        match doc.kind(id) {
            NodeKind::Desc => {
                names.clear();
                let python = doc.custom_attribute(id, "domain") == Some("py");
                desc_end = python.then(|| doc.subtree_end(id));
            }
            NodeKind::DescSignature if desc_end.is_some_and(|end| id < end) => {
                let module = doc.custom_attribute(id, "module").unwrap_or("");
                let fullname = doc.custom_attribute(id, "fullname").unwrap_or("");
                let anchor = doc.attribute(id, AttributeKey::Id).unwrap_or("");
                if module.is_empty() || fullname.is_empty() || names.iter().any(|n| n == fullname)
                {
                    continue;
                }
                names.push(fullname.to_owned());
                out.push(Described {
                    module: module.to_owned(),
                    fullname: fullname.to_owned(),
                    anchor: anchor.to_owned(),
                });
            }
            _ => {}
        }
    }
    out
}

/// Where `object` is defined: a class or function whose qualified name in its module is
/// the name described, as Sphinx's `ModuleAnalyzer.find_tags` names them. Without
/// `follow`, only an object its own module defines.
fn locate(project: &mut Project, object: &Described, follow: bool) -> Option<SourceLocation> {
    let m = project.load_module(&object.module)?;
    let mut found = Object::of_module(m);
    for part in object.fullname.split('.') {
        found = project.get_attr(&found, part);
    }
    let (module, qualname, node, decorators) = match found.kind {
        ObjectKind::Class => {
            let c = found.class?;
            let class = project.class(c);
            (c.module, class.qualname.to_string(), class.node, class.decorators.clone())
        }
        ObjectKind::Function => {
            let f = found.function?;
            let function = project.module(f.module).function(f.kind).into_owned();
            (f.module, function.qualname.to_string(), function.node, function.decorators)
        }
        _ => return None,
    };
    let defined = project.module(module);
    if qualname != object.fullname || (!follow && defined.name != object.module) {
        return None;
    }
    let (line, end_line) = definition_lines(defined, node?, &decorators);
    Some(SourceLocation {
        domain: "py".to_owned(),
        module: object.module.clone(),
        defined_in: defined.name.clone(),
        fullname: object.fullname.clone(),
        anchor: object.anchor.clone(),
        path: defined.path.clone(),
        line,
        end_line,
    })
}

/// The first and last line of a definition, counted from 1: from its first decorator
/// to the last line of its body that holds more than white space.
fn definition_lines(module: &Module, node: NodeId, decorators: &[NodeId]) -> (u32, u32) {
    let n = module.node(node);
    let first = decorators.iter().map(|&d| module.node(d).line()).chain([n.line()]).min();
    let source = module.syntax.source();
    let end = n.end_byte().min(source.len());
    let body = source.get(..end).unwrap_or("").trim_end();
    let last = 1 + body.bytes().filter(|&b| b == b'\n').count();
    (first.unwrap_or(1), u32::try_from(last).unwrap_or(u32::MAX))
}

/// `viewcode.source`, or `viewcode.limit` when the module's file is larger than a
/// source read may be.
fn missing_module(relative: &str, module: &str, roots: &[PathBuf]) -> Report {
    let too_large = module_files(module, roots)
        .into_iter()
        .find(|path| path.metadata().is_ok_and(|m| m.len() > gd_host::MAX_SOURCE_BYTES as u64));
    let report = match too_large {
        Some(path) => Report::warning(
            "viewcode.limit",
            "SOURCE FILE TOO LARGE",
            &format!(
                "The module {module} ({}) is larger than {} MiB, the most a source read may be. \
                 Its objects have no [source] link.",
                path.display(),
                gd_host::MAX_SOURCE_BYTES >> 20
            ),
        )
        .hint("Split the module, or leave it out of the documented modules."),
        None => Report::note(
            "viewcode.source",
            "MODULE SOURCE NOT FOUND",
            &format!(
                "The module {module} is not below autodoc_source_paths, so its objects have \
                 no [source] link."
            ),
        )
        .hint("Add the folder holding the package to autodoc_source_paths in conf.toml."),
    };
    Report { path: relative.to_owned(), category: "viewcode".to_owned(), ..report }
}

/// The files a module may be read from: `a/b.py` and `a/b/__init__.py` below each root.
fn module_files(module: &str, roots: &[PathBuf]) -> Vec<PathBuf> {
    let relative: PathBuf = module.split('.').collect();
    let mut out = Vec::new();
    for root in roots {
        let base = root.join(&relative);
        out.push(base.with_extension("py"));
        out.push(base.join("__init__.py"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(source: &str) -> (tempdir::Dir, Project, gd_pyscan::ModuleId) {
        let dir = tempdir::Dir::new();
        std::fs::write(dir.path.join("shapes.py"), source).unwrap();
        let mut project = Project::new(&[&dir.path]).unwrap();
        let id = project.load_module("shapes").unwrap();
        (dir, project, id)
    }

    mod tempdir {
        pub struct Dir {
            pub path: std::path::PathBuf,
        }
        impl Dir {
            pub fn new() -> Dir {
                use std::sync::atomic::{AtomicU32, Ordering};
                static NEXT: AtomicU32 = AtomicU32::new(0);
                let n = NEXT.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir()
                    .join(format!("gd-read-sources-{}-{n}", std::process::id()));
                std::fs::create_dir_all(&path).unwrap();
                Dir { path }
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.path);
            }
        }
    }

    fn described(module: &str, fullname: &str) -> Described {
        Described { module: module.into(), fullname: fullname.into(), anchor: String::new() }
    }

    #[test]
    fn a_definition_runs_from_its_first_decorator_to_its_last_line() {
        let source = "import functools\n\n@functools.cache\ndef area(x):\n    return x\n\n\n\
                      class Square:\n    def side(self):\n        return 1\n";
        let (_dir, mut project, _) = module(source);
        let area = locate(&mut project, &described("shapes", "area"), true).unwrap();
        assert_eq!((area.line, area.end_line), (3, 5));
        let square = locate(&mut project, &described("shapes", "Square"), true).unwrap();
        assert_eq!((square.line, square.end_line), (8, 10));
        let side = locate(&mut project, &described("shapes", "Square.side"), true).unwrap();
        assert_eq!((side.line, side.end_line, side.defined_in.as_str()), (9, 10, "shapes"));
        assert!(locate(&mut project, &described("shapes", "missing"), true).is_none());
    }

    #[test]
    fn a_reexport_links_to_its_module_only_when_followed() {
        let dir = tempdir::Dir::new();
        let package = dir.path.join("pkg");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(package.join("__init__.py"), "from .impl import Thing\n").unwrap();
        std::fs::write(package.join("impl.py"), "class Thing:\n    pass\n").unwrap();
        let mut project = Project::new(&[&dir.path]).unwrap();
        let thing = locate(&mut project, &described("pkg", "Thing"), true).unwrap();
        assert_eq!((thing.module.as_str(), thing.defined_in.as_str()), ("pkg", "pkg.impl"));
        assert!(locate(&mut project, &described("pkg", "Thing"), false).is_none());
    }
}
