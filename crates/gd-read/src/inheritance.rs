//! sphinx.ext.inheritance_diagram. The reader hands each `inheritance-diagram`
//! directive to this host, which finds the classes with the source scan (never running
//! the code) and answers with a `graphviz` directive holding the graph, which the link
//! pass draws like any graph, and, for text, the list of classes and their bases. The
//! Python files read are the document's dependencies.

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use gd_conf::{Config, InheritanceConfig};
use gd_env::Dependency;
use gd_pyscan::Project;
use guidedoc::{Message, Severity, Status};
use guidedoc_sphinx::{AutodocProvider, AutodocRequest, AutodocResult};

use crate::files::SourceFiles;
use crate::inheritance_graph::{
    ClassInfo, GraphRequest, add_subclasses, class_info, find_classes, generate_dot,
};
use crate::messages::{PYTHON_GRAMMAR, static_message};

/// A name that is neither a class nor a module of the scan.
const CLASS_NOT_FOUND: Message = Message {
    code: "inheritance.class",
    severity: Severity::Warning,
    status: Status::Ok,
    title: "CLASS FOR INHERITANCE DIAGRAM NOT FOUND",
    message: "No class or module named {0} is below autodoc_source_paths, so the diagram \
              leaves it out.",
    hint: "Write the class's full name (package.module.Class), or add the folder holding \
           its package to autodoc_source_paths.",
};

/// A diagram with more classes than it may hold.
const TOO_MANY_CLASSES: Message = Message {
    code: "inheritance.limit",
    severity: Severity::Warning,
    status: Status::Ok,
    title: "INHERITANCE DIAGRAM TOO LARGE",
    message: "The diagram would hold more than {0} classes, so it is not drawn.",
    hint: "Name fewer classes, or stop the bases early with :top-classes:.",
};

/// Serves the inheritance diagrams of one document's read.
#[derive(Debug)]
pub(crate) struct InheritanceHost {
    settings: InheritanceConfig,
    roots: Vec<PathBuf>,
    files: SourceFiles,
    /// The document's scan, made at the first diagram.
    project: Mutex<Option<Project>>,
}

impl InheritanceHost {
    /// The host for one document, or `None` when the extension is not listed.
    pub(crate) fn new(c: &Config) -> Option<InheritanceHost> {
        if !c.has_extension("sphinx.ext.inheritance_diagram") {
            return None;
        }
        let roots = gd_conf::autodoc_roots(c);
        let files = SourceFiles::new(&roots);
        let settings = c.ext.inheritance.clone();
        Some(InheritanceHost { settings, roots, files, project: Mutex::new(None) })
    }

    /// Adds the Python files read to `deps`.
    pub(crate) fn dependencies(&self, deps: &mut Vec<Dependency>) {
        self.files.dependencies("<inheritance: a class was not found>", deps);
    }
}

impl AutodocProvider for InheritanceHost {
    fn generate(&self, request: &AutodocRequest<'_>) -> AutodocResult {
        let mut out = AutodocResult {
            name: format!("<inheritance-diagram {}>", request.argument),
            ..Default::default()
        };
        let mut slot = self.project.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.is_none() {
            let files = self.files.clone();
            let reader = Box::new(move |path: &std::path::Path| files.read(path));
            match Project::with_reader(&self.roots, reader) {
                Ok(project) => *slot = Some(project),
                Err(e) => {
                    out.messages.push(static_message(PYTHON_GRAMMAR, Some(&e.to_string())));
                    return out;
                }
            }
        }
        let Some(project) = slot.as_mut() else { return out };
        let graph = GraphRequest {
            parts: option(request, "parts").parse().unwrap_or(0),
            private_bases: request.options.iter().any(|o| o.name == "private-bases"),
            top_classes: option(request, "top-classes")
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
            include_subclasses: request.options.iter().any(|o| o.name == "include-subclasses"),
            settings: Some(&self.settings),
        };
        let names: Vec<&str> = request.argument.split_whitespace().collect();
        let mut classes = Vec::new();
        for name in &names {
            match find_classes(project, name, request.module) {
                Some(found) => classes.extend(found),
                None => {
                    self.files.note_missing();
                    out.messages.push(static_message(CLASS_NOT_FOUND, Some(name)));
                }
            }
        }
        if graph.include_subclasses {
            add_subclasses(project, &self.roots, &mut classes);
        }
        let info = match class_info(project, &classes, &graph) {
            Ok(info) => info,
            Err(limit) => {
                out.messages.push(static_message(TOO_MANY_CLASSES, Some(&limit.to_string())));
                return out;
            }
        };
        if info.is_empty() {
            return out;
        }
        let content = names.join(", ");
        let dot = generate_dot(&info, &content, &graph);
        out.text = diagram_text(&dot, &content, option(request, "caption"), &info);
        out
    }
}

/// An option's value, or "".
fn option<'r>(request: &'r AutodocRequest<'_>, name: &str) -> &'r str {
    request.options.iter().find(|o| o.name == name).map_or("", |o| o.value.as_str())
}

/// The reStructuredText the directive stands for: the drawing for HTML and the book,
/// and the classes with their bases as a list for text.
fn diagram_text(dot: &str, content: &str, caption: &str, info: &[ClassInfo]) -> String {
    let mut out = String::from(".. only:: not text\n\n   .. graphviz::\n");
    out.push_str("      :class: inheritance graphviz-adaptive\n");
    out.push_str(&format!("      :alt: Inheritance diagram of {content}\n"));
    if !caption.is_empty() {
        out.push_str(&format!("      :caption: {caption}\n"));
    }
    out.push('\n');
    for line in dot.lines() {
        out.push_str("      ");
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("\n.. only:: text\n\n");
    if !caption.is_empty() {
        out.push_str(&format!("   {caption}\n\n"));
    }
    for class in info {
        out.push_str(&format!("   - ``{}``", class.fullname));
        if !class.bases.is_empty() {
            let bases: Vec<String> = class.bases.iter().map(|b| format!("``{b}``")).collect();
            out.push_str(&format!(": {}", bases.join(", ")));
        }
        out.push('\n');
    }
    out
}
