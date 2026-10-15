//! The odin domain's generating directives and API pages (odin.odin). The domain is
//! always on; `odin:autopackage` and the other generating directives read Odin source
//! below `odin_autoapi_dirs` and `odin_collections` (gd-odindoc, which parses without
//! the compiler) and answer with the reStructuredText the domain reads. Every Odin file
//! read is a dependency of the document; a document whose package was not found is read
//! again every build. With `odin_autoapi_generate_api_docs`, each package below
//! `odin_autoapi_dirs` gets a page of its own, `<odin_autoapi_root>/<package>`, and the
//! packages an index, `<odin_autoapi_root>/index`: generated documents that never appear
//! in the source folder.

use std::fmt::Write as _;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use gd_conf::Config;
use gd_env::{Dependency, SourceFile};
use gd_odindoc::{DirectiveOption, FileReader, MemberOrder, Project, Request, Root};
use guidedoc::Severity;
use guidedoc_sphinx::{AutodocProvider, AutodocRequest, AutodocResult};

use crate::api::{generated_source, rst_escape, title_block};
use crate::files::SourceFiles;
use crate::messages::{NO_ODIN_ROOTS, host_message, static_message};

/// The dependency that stands for "a package was not found".
const MISSING: &str = "<odin: a package was not found>";

/// Serves the odin generating directives of one document's read. The directives of a
/// document share one gd-odindoc project, made at the first, so each package is
/// analysed once per document rather than once per directive; the files are read once
/// per document and recorded here.
#[derive(Debug)]
pub(crate) struct OdinHost {
    roots: Vec<Root>,
    member_order: MemberOrder,
    files: SourceFiles,
    /// The document's project; `None` until the first directive.
    project: Mutex<Option<Project>>,
}

impl OdinHost {
    /// The host for one document.
    pub(crate) fn new(c: &Config) -> OdinHost {
        let roots = odin_roots(c);
        let files = odin_files(&roots);
        OdinHost { files, roots, member_order: member_order(c), project: Mutex::new(None) }
    }

    /// Adds the Odin files read to `deps`.
    pub(crate) fn dependencies(&self, deps: &mut Vec<Dependency>) {
        self.files.dependencies(MISSING, deps);
    }
}

impl AutodocProvider for OdinHost {
    fn generate(&self, request: &AutodocRequest<'_>) -> AutodocResult {
        let name = format!("<{} {}>", request.directive, request.argument);
        let mut out = AutodocResult { name, ..Default::default() };
        if self.roots.is_empty() {
            out.messages.push(static_message(NO_ODIN_ROOTS, None));
            return out;
        }
        let mut slot = self.project.lock().unwrap_or_else(PoisonError::into_inner);
        let project = slot.get_or_insert_with(|| {
            let reader = Box::new(ConfinedReader { files: self.files.clone() });
            Project::new(self.roots.clone(), reader)
        });
        let config = gd_odindoc::Config { member_order: self.member_order, ..Default::default() };
        let odin_request = Request {
            directive: request.directive.to_owned(),
            argument: request.argument.to_owned(),
            options: request
                .options
                .iter()
                .map(|o| DirectiveOption::new(o.name, &o.value))
                .collect(),
            current: request.module.to_owned(),
        };
        let generated = gd_odindoc::document(project, &config, &odin_request);
        drop(slot);
        for m in &generated.messages {
            let severity = match m.severity {
                gd_odindoc::Severity::Note => Severity::Note,
                gd_odindoc::Severity::Warning => Severity::Warning,
                gd_odindoc::Severity::Error => {
                    self.files.note_missing();
                    Severity::Error
                }
            };
            out.messages.push(host_message(severity, &m.code, &m.title, &m.text, &m.hint));
        }
        if !generated.lines.is_empty() {
            out.text = generated.lines.join("\n");
            out.text.push('\n');
        }
        out
    }
}

/// gd-odindoc's file reader over a document's confined, recorded files.
struct ConfinedReader {
    files: SourceFiles,
}

impl FileReader for ConfinedReader {
    fn read(&mut self, path: &str) -> Option<String> {
        self.files.read(Path::new(path))
    }

    fn list(&mut self, dir: &str) -> Option<Vec<String>> {
        self.files.list(Path::new(dir))
    }
}

/// The folders packages are found in: `odin_autoapi_dirs`, whose packages are named by
/// their path below the folder (`readers/sphinx`), then `odin_collections` in name
/// order, whose packages are named as Odin imports them (`core:fmt`). Paths are relative
/// to the source folder.
pub(crate) fn odin_roots(c: &Config) -> Vec<Root> {
    let o = &c.odin_api;
    let dir = |d: &str| gd_conf::source_path(c, d).display().to_string();
    let dirs = o.dirs.iter().map(|d| Root { dir: dir(d), collection: String::new() });
    let collections =
        o.collections.iter().map(|(name, d)| Root { dir: dir(d), collection: name.clone() });
    dirs.chain(collections).collect()
}

fn odin_files(roots: &[Root]) -> SourceFiles {
    let dirs: Vec<std::path::PathBuf> = roots.iter().map(|r| r.dir.clone().into()).collect();
    SourceFiles::new(&dirs)
}

fn member_order(c: &Config) -> MemberOrder {
    match c.odin_api.member_order.as_str() {
        "alphabetical" => MemberOrder::Alphabetical,
        "groupwise" => MemberOrder::Groupwise,
        _ => MemberOrder::Source,
    }
}

/// What the API index shows of a package.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Summary {
    id: String,
    synopsis: String,
    platform: String,
}

/// The API pages of `odin_autoapi_dirs` as generated documents: a page per package that
/// generates it with `odin:autopackage`, and an index listing every package with its
/// synopsis. Empty when the pages are off or no folder holds a package.
pub(crate) fn odin_api_sources(c: &Config) -> Vec<SourceFile> {
    let o = &c.odin_api;
    if !o.generate_api_docs || o.dirs.is_empty() {
        return Vec::new();
    }
    let roots: Vec<Root> = odin_roots(c).into_iter().filter(|r| r.collection.is_empty()).collect();
    let reader = Box::new(ConfinedReader { files: odin_files(&roots) });
    let mut project = Project::new(roots, reader);
    let mut packages = Vec::new();
    for package in project.list_packages() {
        if let Some(p) = project.load_package(&package.id) {
            let (id, synopsis, platform) = (p.id.clone(), p.synopsis.clone(), p.platform.clone());
            packages.push(Summary { id, synopsis, platform });
        }
    }
    if packages.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<SourceFile> = packages
        .iter()
        .map(|p| {
            let docname = format!("{}/{}", o.root, page_name(&p.id));
            generated_source(c, &docname, package_page(c, &p.id), Vec::new())
        })
        .collect();
    let index = format!("{}/index", o.root);
    out.push(generated_source(c, &index, index_page(&packages), Vec::new()));
    out
}

/// A package's page below `odin_autoapi_root`: its path, with a collection's `:` as a
/// folder.
fn page_name(id: &str) -> String {
    id.replace(':', "/")
}

/// A package page: its path as the title, then `odin:autopackage` with
/// `odin_autoapi_options` and `odin_autoapi_member_order`.
fn package_page(c: &Config, id: &str) -> String {
    let mut page = title_block(&rst_escape(id));
    let _ = writeln!(page, ".. odin:autopackage:: {id}");
    for option in &c.odin_api.options {
        let _ = writeln!(page, "   :{option}:");
    }
    let _ = writeln!(page, "   :member-order: {}", c.odin_api.member_order);
    page
}

/// The index: the packages in a hidden toctree, then each with its synopsis.
fn index_page(packages: &[Summary]) -> String {
    let mut page = String::from("API Reference\n=============\n\n.. toctree::\n   :hidden:\n\n");
    for p in packages {
        let _ = writeln!(page, "   {}", page_name(&p.id));
    }
    page.push('\n');
    for p in packages {
        let _ = write!(page, "- :odin:pkg:`{}`", p.id);
        if !p.synopsis.is_empty() {
            let _ = write!(page, " -- {}", rst_escape(&p.synopsis));
        }
        if !p.platform.is_empty() {
            let _ = write!(page, " ({})", rst_escape(&p.platform));
        }
        page.push('\n');
    }
    page
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_name_their_package() {
        let c = Config::new("/p/docs", "/p/docs/_build");
        let page = package_page(&c, "core:fmt");
        assert_eq!(
            page,
            "core:fmt\n========\n\n.. odin:autopackage:: core:fmt\n   :members:\n   \
             :undoc-members:\n   :member-order: source\n"
        );
        assert_eq!(page_name("core:fmt"), "core/fmt");
        let index = index_page(&[Summary {
            id: "shapes".into(),
            synopsis: "Measures *figures*.".into(),
            platform: String::new(),
        }]);
        assert!(index.contains(
            "   :hidden:\n\n   shapes\n\n- :odin:pkg:`shapes` -- Measures \\*figures\\*.\n"
        ));
    }
}
