//! What a build reads and refuses to read: the registry and the trust policy.

#![allow(clippy::unwrap_used)]

mod common;

use common::{TempDir, assert_built, build, load, report_with};
use gd_project::{BuilderKind, LoadOptions};
use guidedoc::Registry;

fn registry(readers: Vec<guidedoc::Reader>) -> Registry {
    Registry { readers, renderers: vec![guidedoc_html::renderer()] }
}

fn load_with(
    dir: &std::path::Path,
    registry: Registry,
) -> Result<gd_project::Project, gd_host::Problem> {
    gd_project::load(dir, &LoadOptions { registry: Some(registry), ..Default::default() })
}

/// load refuses a registry a conversion would refuse, and a build refuses to read
/// documents the registry has no reader for, instead of reading them with another one.
#[test]
fn builds_need_a_valid_registry_with_their_readers() {
    let dir = TempDir::new("registry");
    dir.put("conf.toml", "project = \"Demo\"\nextensions = [\"myst_parser\"]\n");
    dir.put("index.rst", "Demo\n====\n\n.. toctree::\n\n   page\n");
    dir.put("page.md", "# Page\n");
    let sphinx = guidedoc_sphinx::READER;
    let twice = load_with(&dir.0, registry(vec![sphinx, sphinx])).unwrap_err();
    assert_eq!(twice.code, "core.registry.invalid");
    assert_ne!(twice.exit, 0);
    // No myst reader: the Markdown page is not read as something else.
    let partial = load_with(&dir.0, registry(vec![guidedoc_rst::reader(), sphinx])).unwrap();
    let r = build(&partial, BuilderKind::Html);
    let problem = r.problem.as_ref().unwrap();
    assert_eq!((problem.title, problem.path.as_str()), ("NO READER FOR THE DOCUMENTS", "page.md"));
    assert_eq!(r.read, 0);
    assert!(!dir.exists("_build/html/index.html"));
    // Without the sphinx reader, reStructuredText is not read as plain Docutils.
    std::fs::remove_file(dir.path("page.md")).unwrap();
    assert_built(&build(&partial, BuilderKind::Html));
    let only_rst = load_with(&dir.0, registry(vec![guidedoc_rst::reader()])).unwrap();
    let r = build(&only_rst, BuilderKind::Html);
    let problem = r.problem.as_ref().unwrap();
    assert_eq!(
        (problem.title, problem.path.as_str()),
        ("NO READER FOR THE DOCUMENTS", "index.rst")
    );
}

/// A folder conf.toml names outside the readable folders is reported as outside, once;
/// a link inside a static folder that leads out is reported as a link.
#[test]
fn unread_static_files_say_why() {
    let top = TempDir::new("unread");
    top.put("shared/site.css", "body {}\n");
    top.put("secret.txt", "secret\n");
    top.put(
        "Doc/conf.toml",
        "project = \"Demo\"\nhtml_static_path = [\"_static\"]\nhtml_extra_path = [\"../shared\"]\n",
    );
    top.put("Doc/index.rst", "Demo\n====\n\nText.\n");
    top.put("Doc/_static/own.css", "p {}\n");
    #[cfg(unix)]
    std::os::unix::fs::symlink(top.path("secret.txt"), top.path("Doc/_static/secret.txt"))
        .unwrap();
    let r = build(&load(&top.path("Doc")), BuilderKind::Html);
    assert_built(&r);
    let outside = report_with(&r, "build.output.outside").unwrap();
    assert!(outside.contains("is outside the source folder"), "{outside}");
    #[cfg(unix)]
    assert!(report_with(&r, "build.output.link").unwrap().contains("is a link"));
    let outsides = r.reports.iter().filter(|rep| rep.code == "build.output.outside").count();
    assert_eq!(outsides, 1); // the folder, not each of its files.
    assert!(top.exists("Doc/_build/html/_static/own.css"));
    assert!(!top.exists("Doc/_build/html/_static/secret.txt"));
    assert!(!top.exists("Doc/_build/html/site.css"));
}

/// A templates_path folder outside the readable folders is not used, with a warning, and
/// is no page's input: changing a template in it rewrites no page.
#[test]
fn unread_templates_are_reported_and_not_inputs() {
    let top = TempDir::new("templates");
    let layout = "{% extends \"!layout.html\" %}{% block footer %}OUTSIDE{% endblock %}";
    top.put("tpl/layout.html", layout);
    top.put("Doc/conf.toml", "project = \"Demo\"\ntemplates_path = [\"../tpl\"]\n");
    top.put("Doc/index.rst", "Demo\n====\n\nText.\n");
    let project = load(&top.path("Doc"));
    let r = build(&project, BuilderKind::Html);
    assert_built(&r);
    let message = report_with(&r, "build.template.outside").unwrap();
    assert!(message.contains("templates_path folder"), "{message}");
    assert!(!top.read("Doc/_build/html/index.html").contains("OUTSIDE"));
    top.put("tpl/layout.html", &format!("{layout} "));
    let again = build(&project, BuilderKind::Html);
    assert_built(&again);
    assert_eq!(again.written, 0);
    assert!(report_with(&again, "build.template.outside").is_some());
}
