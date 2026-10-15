//! `generate_autosummary_docs`: a stub page for each entry of an `autosummary`
//! directive with `:toctree:`, rendered from the templates `autosummary/base.rst`,
//! `module.rst` and `class.rst` (the project's under `templates_path` first). Stub
//! pages are read for directives of their own, so `:recursive:` walks a package.
//! They are generated documents, never files.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gd_autodoc::{Session, Stub, StubOptions};
use gd_conf::Config;
use gd_env::{Dependency, SourceFile, SourceKind};
use gd_jinja::{Environment, ErrorKind, Loader, Options, Value};
use gd_report::Report;
use gd_toml::Data;

use super::scan::{Entry, find_entries};
use crate::api::generated_source;
use crate::files::SourceFiles;

/// The most stub pages one build generates.
const MAX_STUBS: usize = 10_000;
/// How deeply stub pages may generate more stub pages.
const MAX_DEPTH: usize = 64;

/// Sphinx's built-in autosummary templates.
const BUILTIN_TEMPLATES: &[(&str, &str)] = &[
    ("autosummary/base.rst", include_str!("templates/base.rst")),
    ("autosummary/class.rst", include_str!("templates/class.rst")),
    ("autosummary/module.rst", include_str!("templates/module.rst")),
];

/// `posixpath.normpath` of a relative document path: `.` and `a/..` resolved.
pub(crate) fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|p| *p != "..") => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    parts.join("/")
}

/// The folder of a document name: `api/index` is in `api`, `index` in `""`.
fn folder_of(docname: &str) -> &str {
    docname.rsplit_once('/').map_or("", |(folder, _)| folder)
}

/// A document whose autosummary directives generate stub pages (`autosummary_generate`).
fn generates(c: &Config, source: &SourceFile) -> bool {
    use gd_conf::FlagOrList;
    if source.generated.is_some() || source.kind != SourceKind::Rst {
        return false;
    }
    match &c.ext.autosummary.generate {
        FlagOrList::Flag(on) => *on,
        FlagOrList::List(files) => files.iter().any(|f| {
            *f == source.relative
                || *f == source.docname
                || f.trim_start_matches("./") == source.relative
        }),
    }
}

/// The stub pages the autosummary directives of `taken` (the source folder's
/// documents) ask for, as generated documents. A stub whose name a document of
/// `taken` already has is left out (`autosummary.exists`); what could not be made is
/// reported (`autosummary.import`, `autosummary.template`, `autosummary.toctree`,
/// `autosummary.limit`).
pub(crate) fn autosummary_sources(
    c: &Config,
    taken: &[SourceFile],
    reports: &mut Vec<Report>,
) -> Vec<SourceFile> {
    if !super::is_on(c) {
        return Vec::new();
    }
    let sources: Vec<&SourceFile> = taken.iter().filter(|s| generates(c, s)).collect();
    if sources.is_empty() {
        return Vec::new();
    }
    let roots = crate::autodoc::python_roots(c);
    let files = SourceFiles::new(&roots);
    let reader_files = files.clone();
    let reader = Box::new(move |path: &std::path::Path| reader_files.read(path));
    let Ok(project) = gd_pyscan::Project::with_reader(&roots, reader) else { return Vec::new() };
    let mut g = Generation {
        config: c,
        autodoc: crate::autodoc::autodoc_config(c),
        session: Session::new(project),
        templates: Templates::new(c),
        taken,
        out: Vec::new(),
        reports,
        queue: Vec::new(),
    };
    for source in sources {
        let Ok(text) = std::fs::read_to_string(&source.path) else { continue };
        g.scan(&text, &source.docname, &source.relative, 0);
    }
    g.run();
    let mut deps = Vec::new();
    files.dependencies("<autosummary: an object was not found>", &mut deps);
    deps.extend(g.templates.dependencies());
    let mut out = g.out;
    for page in &mut out {
        if let Some(generated) = page.generated.as_mut() {
            generated.deps.clone_from(&deps);
        }
    }
    out
}

/// An entry waiting for its stub page: where it was found and how deep.
#[derive(Clone)]
struct Pending {
    entry: Entry,
    /// The document it was found in, for reports.
    relative: String,
    depth: usize,
}

/// One build's stub generation.
struct Generation<'a> {
    config: &'a Config,
    autodoc: gd_autodoc::Config,
    session: Session,
    templates: Templates,
    taken: &'a [SourceFile],
    out: Vec<SourceFile>,
    reports: &'a mut Vec<Report>,
    queue: Vec<Pending>,
}

impl Generation<'_> {
    /// Queues the entries of a document's text.
    fn scan(&mut self, text: &str, docname: &str, relative: &str, depth: usize) {
        let lines: Vec<&str> = text.lines().collect();
        let session = &mut self.session;
        let mut docstring = |name: &str| gd_autodoc::module_docstring(session, name);
        let mut entries = find_entries(&lines, None, folder_of(docname), &mut docstring);
        entries.sort();
        entries.dedup();
        let relative = relative.to_owned();
        let pending = entries.into_iter().filter(|e| e.folder.is_some());
        self.queue.extend(pending.map(|entry| Pending {
            entry,
            relative: relative.clone(),
            depth,
        }));
    }

    /// Makes the queued stub pages, and those their own directives ask for.
    fn run(&mut self) {
        let mut next = 0;
        while next < self.queue.len() {
            if self.out.len() >= MAX_STUBS {
                self.limit(&format!("more than {MAX_STUBS} stub pages"));
                return;
            }
            let Pending { entry, relative, depth } = self.queue[next].clone();
            next += 1;
            if depth > MAX_DEPTH {
                self.limit(&format!("stub pages nested more than {MAX_DEPTH} levels deep"));
                continue;
            }
            if let Some(page) = self.make(&entry, &relative) {
                let text = page.generated.as_ref().map(|g| g.text.clone()).unwrap_or_default();
                let (docname, relative) = (page.docname.clone(), page.relative.clone());
                self.out.push(page);
                self.scan(&text, &docname, &relative, depth + 1);
            }
        }
    }

    /// The stub page of one entry, or `None` with what went wrong reported.
    fn make(&mut self, entry: &Entry, relative: &str) -> Option<SourceFile> {
        let folder = entry.folder.as_deref().unwrap_or("");
        let map = &self.config.ext.autosummary.filename_map;
        let file = map.get(&entry.name).unwrap_or(&entry.name);
        let docname = normalize(&format!("{folder}/{file}"));
        if docname.is_empty() || docname == ".." || docname.starts_with("../") {
            self.warn(
                relative,
                "autosummary.toctree",
                "STUB FOLDER OUTSIDE THE SOURCE FOLDER",
                format!("The :toctree: folder of {} leads outside the source folder.", entry.name),
                "Give :toctree: a folder inside the source folder, such as generated.",
            );
            return None;
        }
        if self.taken.iter().any(|s| s.docname == docname) {
            self.note_exists(&docname);
            return None;
        }
        if self.out.iter().any(|s| s.docname == docname) {
            return None;
        }
        let options = StubOptions {
            imported_members: self.config.ext.autosummary.imported_members,
            ignore_module_all: self.config.ext.autosummary.ignore_module_all,
            recursive: entry.recursive,
        };
        let stub = match gd_autodoc::stub(&mut self.session, &self.autodoc, &entry.name, options) {
            Ok(stub) => stub,
            Err(m) => {
                self.warn(
                    relative,
                    "autosummary.import",
                    "STUB OBJECT NOT FOUND",
                    m.text,
                    &m.hint,
                );
                return None;
            }
        };
        let context = &self.config.ext.autosummary.context;
        match self.templates.render(&entry.template, &stub, context) {
            Ok(text) => Some(generated_source(self.config, &docname, text, Vec::new())),
            Err(e) => {
                let text = format!("The stub page of {} could not be made: {e}", entry.name);
                self.warn(
                    relative,
                    "autosummary.template",
                    "STUB TEMPLATE FAILED",
                    text,
                    "Correct the template; the report above shows the line.",
                );
                None
            }
        }
    }

    fn warn(&mut self, relative: &str, code: &str, title: &str, message: String, hint: &str) {
        let report = Report::warning(code, title, &message).hint(hint);
        self.reports.push(Report {
            path: relative.to_owned(),
            category: code.to_owned(),
            ..report
        });
    }

    fn note_exists(&mut self, docname: &str) {
        let message = format!(
            "The source folder has a document named {docname}, the name of a stub page \
             autosummary would generate; the document is kept."
        );
        let report = Report::note("autosummary.exists", "STUB PAGE WRITTEN BY HAND", &message)
            .hint("Nothing to do, unless the page should be generated: then remove the file.");
        self.reports.push(Report { category: "autosummary.exists".to_owned(), ..report });
    }

    fn limit(&mut self, what: &str) {
        let message = format!("autosummary stopped generating at {what}; the rest are left out.");
        let report = Report::warning("autosummary.limit", "TOO MANY STUB PAGES", &message)
            .hint("Remove :recursive: from the largest packages, or list fewer entries.");
        self.reports.push(Report { category: "autosummary.limit".to_owned(), ..report });
    }
}

/// The templates of stub pages: the project's `templates_path` folders, then Sphinx's.
struct Templates {
    env: Environment,
    /// The project's template files read, for the pages' dependencies.
    read: Rc<RefCell<Vec<PathBuf>>>,
}

impl Templates {
    fn new(c: &Config) -> Templates {
        let dirs: Vec<PathBuf> =
            c.templates_path.iter().map(|p| gd_conf::source_path(c, p)).collect();
        let read = Rc::new(RefCell::new(Vec::new()));
        let seen = Rc::clone(&read);
        let loader = Loader::custom(move |name: &str| {
            if let Some(builtin) = name.strip_prefix('!') {
                return builtin_template(builtin);
            }
            for dir in &dirs {
                let path = dir.join(name);
                if name.split('/').any(|p| p == "..") {
                    return None;
                }
                if let Ok(text) = std::fs::read_to_string(&path) {
                    seen.borrow_mut().push(path);
                    return Some(text);
                }
            }
            builtin_template(name)
        });
        let mut env = Environment::new(Options { loader: Some(loader), ..Options::default() });
        env.add_filter("escape", |call| {
            Ok(Value::from(rst_escape(&gd_jinja::to_str(&call.args[0]))))
        });
        env.add_filter("e", |call| Ok(Value::from(rst_escape(&gd_jinja::to_str(&call.args[0])))));
        env.add_filter("underline", |call| {
            let title = gd_jinja::to_str(&call.args[0]);
            let line = call.arg(1, "line");
            let line = if line.is_undefined() {
                "=".to_owned()
            } else {
                gd_jinja::to_str(&line).into_owned()
            };
            Ok(Value::from(format!("{title}\n{}", line.repeat(title.chars().count()))))
        });
        Templates { env, read }
    }

    /// The text of the stub page `stub`: `template` when the project has it, else the
    /// template of the object's type, else `autosummary/base.rst`.
    fn render(
        &mut self,
        template: &str,
        stub: &Stub,
        context: &gd_toml::Table,
    ) -> Result<String, gd_jinja::Error> {
        let names = [template.to_owned(), format!("autosummary/{}.rst", stub.objtype)];
        let mut chosen = None;
        for name in names.iter().filter(|n| !n.is_empty()) {
            match self.env.load_template(name) {
                Ok(t) => {
                    chosen = Some(t);
                    break;
                }
                Err(e) if e.kind == ErrorKind::NotFound => continue,
                Err(e) => return Err(e),
            }
        }
        let template = match chosen {
            Some(t) => t,
            None => self.env.load_template("autosummary/base.rst")?,
        };
        template.render(&stub_context(stub, context))
    }

    fn dependencies(&self) -> Vec<Dependency> {
        let read = self.read.borrow();
        let mut paths: Vec<&PathBuf> = read.iter().collect();
        paths.sort();
        paths.dedup();
        paths
            .into_iter()
            .filter_map(|p| {
                Some(Dependency {
                    path: p.clone(),
                    digest: gd_env::file_digest(p)?,
                    included: false,
                })
            })
            .collect()
    }
}

fn builtin_template(name: &str) -> Option<String> {
    BUILTIN_TEMPLATES.iter().find(|(n, _)| *n == name).map(|(_, text)| (*text).to_owned())
}

/// Sphinx's `rst.escape`: ASCII punctuation but `.` escaped, and a leading `.`.
fn rst_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for (i, c) in s.chars().enumerate() {
        if (c.is_ascii_punctuation() && c != '.') || (i == 0 && c == '.') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn strings(items: &[String]) -> Value {
    Value::list(items.iter().map(|s| Value::from(s.as_str())))
}

/// The template variables: `autosummary_context`, then Sphinx's names.
fn stub_context(stub: &Stub, context: &gd_toml::Table) -> Value {
    let ctx = Value::dict();
    for (key, value) in context.iter() {
        ctx.set(key, toml_value(value));
    }
    for (name, value) in [
        ("fullname", &stub.fullname),
        ("module", &stub.module),
        ("objname", &stub.objname),
        ("name", &stub.name),
        ("objtype", &stub.objtype),
    ] {
        ctx.set(name, Value::from(value.as_str()));
    }
    ctx.set("underline", Value::from("=".repeat(stub.fullname.chars().count())));
    if let Some(class) = &stub.class {
        ctx.set("class", Value::from(class.as_str()));
    }
    for (name, list) in [
        ("members", &stub.members),
        ("inherited_members", &stub.inherited_members),
        ("functions", &stub.functions),
        ("all_functions", &stub.all_functions),
        ("classes", &stub.classes),
        ("all_classes", &stub.all_classes),
        ("exceptions", &stub.exceptions),
        ("all_exceptions", &stub.all_exceptions),
        ("attributes", &stub.attributes),
        ("all_attributes", &stub.all_attributes),
        ("modules", &stub.modules),
        ("all_modules", &stub.all_modules),
        ("methods", &stub.methods),
        ("all_methods", &stub.all_methods),
    ] {
        ctx.set(name, strings(list));
    }
    ctx
}

fn toml_value(v: &gd_toml::Value) -> Value {
    match &v.data {
        Data::String(s) => Value::from(s.as_str()),
        Data::Datetime(d) => Value::from(d.to_string()),
        Data::Integer(i) => Value::from(*i),
        Data::Float(f) => Value::from(*f),
        Data::Boolean(b) => Value::from(*b),
        Data::Array(items) => Value::list(items.iter().map(toml_value)),
        Data::Table(table) => {
            let out = Value::dict();
            for (key, value) in table.iter() {
                out.set(key, toml_value(value));
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_normalized_as_posixpath_does() {
        assert_eq!(normalize("api/./generated//pkg.mod"), "api/generated/pkg.mod");
        assert_eq!(normalize("api/../generated/x"), "generated/x");
        assert_eq!(normalize("../x"), "../x");
        assert_eq!(folder_of("api/index"), "api");
        assert_eq!(folder_of("index"), "");
    }

    #[test]
    fn the_builtin_templates_render_a_module() {
        let c = Config::new("/p/docs", "/p/docs/_build");
        let mut t = Templates::new(&c);
        let stub = Stub {
            fullname: "pkg.mod".into(),
            module: "pkg".into(),
            objname: "mod".into(),
            name: "mod".into(),
            objtype: "module".into(),
            functions: vec!["area".into()],
            ..Stub::default()
        };
        let text = t.render("", &stub, &gd_toml::Table::new()).expect("renders");
        assert!(text.starts_with("pkg.mod\n=======\n\n.. automodule:: pkg.mod"), "{text}");
        assert!(text.contains(".. rubric:: Functions") && text.contains("      area"), "{text}");
    }
}
