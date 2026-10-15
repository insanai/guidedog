//! The graph of an `inheritance-diagram`: the classes named, their bases up to the
//! roots (or to `:top-classes:`), and with `:include-subclasses:` the classes that
//! derive from them, all found by the source scan. It is written as the DOT Sphinx's
//! `InheritanceGraph.generate_dot` writes, with each node's URL left as a reference
//! the link pass resolves (`gd-xref:py:class:<name>`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use gd_conf::{Digest, InheritanceConfig};
use gd_pyscan::{ClassRef, ModuleId, Object, ObjectKind, Project};

/// The most classes one diagram may hold.
pub(crate) const MAX_CLASSES: usize = 1_000;

/// The most modules `:include-subclasses:` loads to find subclasses.
const MAX_MODULES: usize = 10_000;

/// The prefix of a node URL the link pass resolves as a `py:class` reference.
pub(crate) const XREF_URL: &str = "gd-xref:py:class:";

/// What the directive asks for.
#[derive(Debug, Clone, Default)]
pub(crate) struct GraphRequest<'a> {
    /// `:parts:`: how many dotted parts of a name to show (negative: how many to drop).
    pub parts: i64,
    /// `:private-bases:`.
    pub private_bases: bool,
    /// `:top-classes:`, full names.
    pub top_classes: Vec<String>,
    /// `:include-subclasses:`.
    pub include_subclasses: bool,
    /// `inheritance_alias` and the attribute tables.
    pub settings: Option<&'a InheritanceConfig>,
}

/// One class of the graph.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ClassInfo {
    /// The name shown (`:parts:` and aliases applied).
    pub shown: String,
    /// The full name, aliases applied.
    pub fullname: String,
    /// The shown names of its bases.
    pub bases: Vec<String>,
    /// The first line of its docstring.
    pub tooltip: String,
}

/// The graph's classes, or `Err` when it would hold more than [`MAX_CLASSES`].
pub(crate) fn class_info(
    project: &mut Project,
    classes: &[ClassRef],
    request: &GraphRequest<'_>,
) -> Result<Vec<ClassInfo>, usize> {
    let mut seen: BTreeMap<String, ClassInfo> = BTreeMap::new();
    let mut todo: Vec<Node> = classes.iter().map(|&c| Node::Scanned(c)).collect();
    todo.reverse();
    while let Some(node) = todo.pop() {
        let full = node_name(project, &node);
        if seen.contains_key(&full) || (!request.private_bases && is_private(&full)) {
            continue;
        }
        if seen.len() >= MAX_CLASSES {
            return Err(MAX_CLASSES);
        }
        let mut info = ClassInfo {
            shown: class_name(&full, request.parts, request),
            fullname: class_name(&full, 0, request),
            bases: Vec::new(),
            tooltip: String::new(),
        };
        if let Node::Scanned(c) = node {
            info.tooltip = first_line(project.class(c).doc.as_deref());
            if !request.top_classes.contains(&info.fullname) {
                for base in bases(project, c) {
                    let name = node_name(project, &base);
                    if !request.private_bases && is_private(&name) {
                        continue;
                    }
                    info.bases.push(class_name(&name, request.parts, request));
                    todo.push(base);
                }
            }
        }
        seen.insert(full, info);
    }
    let mut out: Vec<ClassInfo> = seen.into_values().collect();
    out.sort();
    Ok(out)
}

/// A class of the graph: one the scan read, or one from outside the source folders,
/// known by its name only.
#[derive(Debug, Clone)]
enum Node {
    Scanned(ClassRef),
    Outside(String),
}

/// The bases of `c` that are drawn: classes, but not Python's builtins.
fn bases(project: &mut Project, c: ClassRef) -> Vec<Node> {
    let mut out = Vec::new();
    for base in project.base_objects(c) {
        match (base.kind, base.class) {
            (ObjectKind::Class, Some(b)) => out.push(Node::Scanned(b)),
            (ObjectKind::External, _) if !base.path.is_empty() => {
                out.push(Node::Outside(base.path));
            }
            _ => {}
        }
    }
    out
}

/// The full name of a class: its module and qualified name.
fn node_name(project: &Project, node: &Node) -> String {
    match node {
        Node::Scanned(c) => {
            let module = &project.module(c.module).name;
            format!("{module}.{}", project.class(*c).qualname)
        }
        Node::Outside(path) => path.clone(),
    }
}

fn is_private(fullname: &str) -> bool {
    fullname.rsplit('.').next().is_some_and(|name| name.starts_with('_'))
}

/// Sphinx's `class_name`: the last `parts` dotted parts (or all but the first
/// `-parts`), then `inheritance_alias`.
fn class_name(fullname: &str, parts: i64, request: &GraphRequest<'_>) -> String {
    let names: Vec<&str> = fullname.split('.').collect();
    let keep = usize::try_from(parts.unsigned_abs()).unwrap_or(usize::MAX);
    let start = match parts {
        0 => 0,
        p if p > 0 => names.len().saturating_sub(keep),
        _ => keep.min(names.len()),
    };
    let result = names[start..].join(".");
    let alias = request.settings.and_then(|s| s.alias.get(&result));
    alias.cloned().unwrap_or(result)
}

/// A docstring's first line, as a tooltip.
fn first_line(doc: Option<&str>) -> String {
    let doc = doc.unwrap_or("").trim();
    doc.lines().next().unwrap_or("").trim().to_owned()
}

/// The classes a module defines at its top level, as `inspect` finds them in its
/// namespace.
pub(crate) fn module_classes(project: &Project, m: ModuleId) -> Vec<ClassRef> {
    let module = project.module(m);
    let top = |(i, c): (usize, &gd_pyscan::Class)| {
        let class = gd_pyscan::ClassId(u32::try_from(i).ok()?);
        (c.outer.is_none() && !c.qualname.contains('.')).then_some(ClassRef { module: m, class })
    };
    module.classes.iter().enumerate().filter_map(top).collect()
}

/// Adds to `classes` every class of the scan that derives from one of them, after
/// loading the modules of their top-level packages below `roots`.
pub(crate) fn add_subclasses(
    project: &mut Project,
    roots: &[std::path::PathBuf],
    classes: &mut Vec<ClassRef>,
) {
    let packages: BTreeSet<String> = classes
        .iter()
        .map(|c| project.module(c.module).name.split('.').next().unwrap_or("").to_owned())
        .collect();
    let mut loaded = 0;
    for package in &packages {
        for root in roots {
            load_package(project, &root.join(package), package, &mut loaded, 0);
        }
    }
    let mut all = Vec::new();
    for i in 0..project.module_count() {
        let m = ModuleId(u32::try_from(i).unwrap_or(u32::MAX));
        all.extend(module_classes_all(project, m));
    }
    let mut changed = true;
    while changed {
        changed = false;
        for &c in &all {
            if classes.contains(&c) {
                continue;
            }
            let derives = project.base_objects(c).iter().any(|b| {
                b.kind == ObjectKind::Class && b.class.is_some_and(|b| classes.contains(&b))
            });
            if derives && classes.len() < MAX_CLASSES {
                classes.push(c);
                changed = true;
            }
        }
    }
}

/// Every class a module defines, nested ones too.
fn module_classes_all(project: &Project, m: ModuleId) -> Vec<ClassRef> {
    let count = project.module(m).classes.len();
    (0..count)
        .filter_map(|i| u32::try_from(i).ok())
        .map(|i| ClassRef { module: m, class: gd_pyscan::ClassId(i) })
        .collect()
}

/// Loads the modules of the package folder `dir` (named `name`), at most
/// [`MAX_MODULES`] in all and 64 folders deep.
fn load_package(project: &mut Project, dir: &Path, name: &str, loaded: &mut usize, depth: usize) {
    if depth > 64 || *loaded >= MAX_MODULES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut names: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    names.sort();
    for path in names {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else { continue };
        if path.is_dir() {
            if path.join("__init__.py").is_file() {
                load_package(project, &path, &format!("{name}.{stem}"), loaded, depth + 1);
            }
        } else if path.extension().is_some_and(|e| e == "py") && *loaded < MAX_MODULES {
            *loaded += 1;
            let module =
                if stem == "__init__" { name.to_owned() } else { format!("{name}.{stem}") };
            project.load_module(&module);
        }
    }
}

/// Finds a class or module by name as Sphinx's `import_classes` does: relative to the
/// current module first, then as written; a module gives the classes it defines.
pub(crate) fn find_classes(
    project: &mut Project,
    name: &str,
    module: &str,
) -> Option<Vec<ClassRef>> {
    if !module.is_empty()
        && let Some(found) = find_target(project, &format!("{module}.{name}"))
    {
        return Some(found);
    }
    find_target(project, name)
}

fn find_target(project: &mut Project, name: &str) -> Option<Vec<ClassRef>> {
    if let Some(m) = project.load_module(name) {
        return Some(module_classes(project, m));
    }
    let (module, attribute) = name.rsplit_once('.')?;
    let m = project.load_module(module)?;
    let found: Object = project.get_attr(&Object::of_module(m), attribute);
    match (found.kind, found.class) {
        (ObjectKind::Class, Some(c)) => Some(vec![c]),
        _ => None,
    }
}

/// The DOT of the graph, as Sphinx writes it, named after what was asked for.
pub(crate) fn generate_dot(
    classes: &[ClassInfo],
    content: &str,
    request: &GraphRequest<'_>,
) -> String {
    let hex =
        Digest::of_parts(&[content.as_bytes(), request.parts.to_string().as_bytes()]).to_string();
    let name = format!("inheritance{}", &hex[hex.len().saturating_sub(10)..]);
    let mut graph: BTreeMap<String, String> =
        [("rankdir", "LR"), ("size", "\"8.0, 12.0\""), ("bgcolor", "transparent")]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect();
    let mut node: BTreeMap<String, String> = [
        ("shape", "box"),
        ("fontsize", "10"),
        ("height", "0.25"),
        ("fontname", "\"Vera Sans, DejaVu Sans, Liberation Sans, Arial, Helvetica, sans\""),
        ("style", "\"setlinewidth(0.5),filled\""),
        ("fillcolor", "white"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect();
    let mut edge: BTreeMap<String, String> =
        [("arrowsize", "0.5"), ("style", "\"setlinewidth(0.5)\"")]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect();
    if let Some(s) = request.settings {
        for (table, extra) in
            [(&mut graph, &s.graph_attrs), (&mut node, &s.node_attrs), (&mut edge, &s.edge_attrs)]
        {
            for (k, v) in extra {
                table.insert(k.clone(), dot_value(v));
            }
        }
    }
    let mut out = format!("digraph {name} {{\n");
    for (k, v) in &graph {
        out.push_str(&format!("{k}={v};\n"));
    }
    let edge_attrs = join_attrs(&edge);
    for class in classes {
        let mut attrs = node.clone();
        attrs.insert("URL".to_owned(), format!("\"{XREF_URL}{}\"", escape(&class.fullname)));
        attrs.insert("target".to_owned(), "\"_top\"".to_owned());
        if !class.tooltip.is_empty() {
            attrs.insert("tooltip".to_owned(), format!("\"{}\"", escape(&class.tooltip)));
        }
        out.push_str(&format!("  \"{}\" [{}];\n", escape(&class.shown), join_attrs(&attrs)));
        for base in &class.bases {
            out.push_str(&format!(
                "  \"{}\" -> \"{}\" [{edge_attrs}];\n",
                escape(base),
                escape(&class.shown)
            ));
        }
    }
    out.push_str("}\n");
    out
}

/// `k=v` pairs joined by commas, in name order (upper case first, as Python sorts).
fn join_attrs(attrs: &BTreeMap<String, String>) -> String {
    let pairs: Vec<String> = attrs.iter().map(|(k, v)| format!("{k}={v}")).collect();
    pairs.join(",")
}

/// A value from conf.toml as DOT: a plain word or number as it is, a quoted string
/// kept, anything else quoted, so a value cannot end the attribute list.
fn dot_value(value: &str) -> String {
    let plain = !value.is_empty()
        && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-');
    let quoted = value.len() >= 2
        && value.starts_with('"')
        && value.ends_with('"')
        && !value[1..value.len() - 1].replace("\\\"", "").contains('"')
        && !value.ends_with("\\\"");
    if plain || quoted { value.to_owned() } else { format!("\"{}\"", escape(value)) }
}

/// Escapes `"` and `\` for a quoted DOT string.
fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "class Base:\n    \"\"\"The base.\n\n    More.\"\"\"\n\n\
                          class _Hidden(Base):\n    pass\n\n\
                          class Child(_Hidden, Exception):\n    pass\n\n\
                          class Leaf(Child):\n    pass\n";

    fn project() -> (std::path::PathBuf, Project) {
        let dir = std::env::temp_dir().join(format!("gd-read-inherit-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("pkg")).unwrap();
        std::fs::write(dir.join("pkg/__init__.py"), "").unwrap();
        std::fs::write(dir.join("pkg/tree.py"), SOURCE).unwrap();
        let project = Project::new(&[&dir]).unwrap();
        (dir, project)
    }

    fn names(info: &[ClassInfo]) -> Vec<(&str, Vec<&str>)> {
        info.iter()
            .map(|c| (c.shown.as_str(), c.bases.iter().map(String::as_str).collect()))
            .collect()
    }

    #[test]
    fn bases_are_walked_as_sphinx_walks_them() {
        let (_dir, mut p) = project();
        let leaf = find_classes(&mut p, "Leaf", "pkg.tree").unwrap();
        let info = class_info(&mut p, &leaf, &GraphRequest::default()).unwrap();
        // _Hidden is private and Exception a builtin: neither is drawn.
        assert_eq!(
            names(&info),
            [("pkg.tree.Child", vec![]), ("pkg.tree.Leaf", vec!["pkg.tree.Child"])]
        );
        let private = GraphRequest { private_bases: true, parts: 1, ..GraphRequest::default() };
        let info = class_info(&mut p, &leaf, &private).unwrap();
        assert_eq!(
            names(&info),
            [
                ("Base", vec![]),
                ("Child", vec!["_Hidden"]),
                ("Leaf", vec!["Child"]),
                ("_Hidden", vec!["Base"]),
            ]
        );
        assert_eq!(info[0].tooltip, "The base.");
        let top = GraphRequest {
            private_bases: true,
            top_classes: vec!["pkg.tree.Child".into()],
            ..GraphRequest::default()
        };
        assert_eq!(class_info(&mut p, &leaf, &top).unwrap().len(), 2);
    }

    #[test]
    fn a_module_gives_its_classes_and_subclasses_can_be_added() {
        let (dir, mut p) = project();
        assert_eq!(find_classes(&mut p, "pkg.tree", "").unwrap().len(), 4);
        assert!(find_classes(&mut p, "pkg.nothing", "").is_none());
        let mut base = find_classes(&mut p, "pkg.tree.Base", "").unwrap();
        add_subclasses(&mut p, &[dir], &mut base);
        assert_eq!(base.len(), 4);
    }

    #[test]
    fn the_dot_is_sphinxs_with_resolvable_urls() {
        let (_dir, mut p) = project();
        let leaf = find_classes(&mut p, "pkg.tree.Leaf", "").unwrap();
        let mut settings = InheritanceConfig::default();
        settings.graph_attrs.insert("rankdir".into(), "TB".into());
        settings.node_attrs.insert("color".into(), "x]; evil [".into());
        settings.alias.insert("pkg.tree.Leaf".into(), "Leaf".into());
        let request = GraphRequest { settings: Some(&settings), ..GraphRequest::default() };
        let info = class_info(&mut p, &leaf, &request).unwrap();
        let dot = generate_dot(&info, "pkg.tree.Leaf", &request);
        assert!(dot.starts_with("digraph inheritance"));
        assert!(dot.contains("bgcolor=transparent;\nrankdir=TB;\nsize=\"8.0, 12.0\";\n"));
        assert!(dot.contains(
            "  \"Leaf\" [URL=\"gd-xref:py:class:Leaf\",color=\"x]; evil [\",fillcolor=white,"
        ));
        assert!(dot.contains(
            "  \"pkg.tree.Child\" -> \"Leaf\" [arrowsize=0.5,style=\"setlinewidth(0.5)\"];"
        ));
        assert!(dot.ends_with("}\n"));
    }
}
