//! conf.py and guidedog.toml migration, the one-call path, and extension harvesting.

#![allow(clippy::unwrap_used)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use gd_migrate::{
    Destination, MigrateOptions, check_conf_toml, convert_conf_py, convert_guidedog_toml, migrate,
};

const CONF_PY: &str = r#"# Configuration file for the Sphinx documentation builder.
import os
import sys
from datetime import date
sys.path.insert(0, os.path.abspath('..'))

project = 'Widget'
author = "Ada & Co"
copyright = f'2019-{date.today().year}, {author}'
version = build_tag = '2.1'
release = version + '.3'
master_doc = 'contents'

extensions = [
    'sphinx.ext.todo',
    'sphinx.ext.autodoc',
    'sphinx.ext.napoleon',  # needs Python
    "sphinx.ext.graphviz",
]
extensions.append('sphinx.ext.ifconfig')
templates_path = ['_templates']
exclude_patterns = ['_build', 'Thumbs.db', '.DS_Store']
todo_include_todos = True
numfig = True
numfig_secnum_depth = 2
rst_prolog = """
.. |product| replace:: Widget\\n
"""
html_theme = 'sphinx_rtd_theme'
html_theme_options = {'navigation_depth': 3, 'sticky': True}
html_static_path = ['_static']
html_css_files = [('custom.css', {'media': 'print'}), 'extra.css']
extlinks = {'issue': ('https://example.org/issues/%s', 'issue %s'),
            'pr': ('https://example.org/pull/%s', None)}
nitpick_ignore = [('py:class', 'Widget')]
html_context = {}
html_context['display_github'] = True
html_additional_pages = {'index': 'indexcontent.html'}
language = None
latex_elements = {'papersize': 'letterpaper', 'preamble': r'\usepackage{x}'}
latex_documents = [(master_doc, 'widget.tex', 'Widget Manual', author, 'manual')]
html_last_updated_fmt = os.environ.get('FMT', '%b %d')
_helper = 3

if os.environ.get('READTHEDOCS'):
    html_theme = 'furo'

def setup(app):
    app.add_css_file('x.css')
"#;

fn expect_line(toml: &str, line: &str) {
    assert!(toml.contains(line), "missing {line:?} in\n{toml}");
}

fn codes(notes: &[gd_report::Report]) -> HashMap<&str, usize> {
    let mut codes = HashMap::new();
    for n in notes {
        *codes.entry(n.code.as_str()).or_default() += 1;
    }
    codes
}

#[test]
fn conf_py_becomes_loadable_conf_toml() {
    let c = convert_conf_py(CONF_PY, Path::new("conf.py"), Some(2026));
    let toml = &c.toml;
    check_conf_toml(toml).unwrap();
    expect_line(toml, r#"project = "Widget""#);
    expect_line(toml, r#"copyright = "2019-2026, Ada & Co""#);
    expect_line(toml, r#"release = "2.1.3""#);
    expect_line(toml, r#"version = "2.1""#);
    expect_line(toml, r#"root_doc = "contents""#);
    expect_line(toml, "    \"sphinx.ext.graphviz\",\n    \"sphinx.ext.ifconfig\",\n]");
    expect_line(toml, r#"html_theme = "sphinx_rtd_theme""#);
    expect_line(toml, r#"html_css_files = ["custom.css", "extra.css"]"#);
    expect_line(toml, r#"pr = ["https://example.org/pull/%s", "https://example.org/pull/%s"]"#);
    expect_line(toml, r#"nitpick_ignore = [["py:class", "Widget"]]"#);
    expect_line(toml, "display_github = true");
    expect_line(toml, "[html_additional_pages]\nindex = \"indexcontent.html\"");
    expect_line(toml, r#"pdf_paper_size = "letter""#);
    expect_line(toml, r#"file = "widget.pdf""#);
    expect_line(toml, r#"root = "contents""#);
    expect_line(toml, "rst_prolog = '''\n\n.. |product| replace:: Widget\\n\n'''");
    expect_line(toml, "# html_last_updated_fmt = os.environ.get('FMT', '%b %d')");
    assert!(!toml.contains(r#""sphinx.ext.napoleon""#), "napoleon is left out");
    expect_line(toml, r#""sphinx.ext.autodoc""#);
    expect_line(toml, r#"autodoc_source_paths = [".."]"#);
    assert!(!toml.contains("_helper"), "private helpers are left out");
    let codes = codes(&c.notes);
    assert_eq!(codes.get("migrate.extension"), Some(&1));
    assert_eq!(codes.get("migrate.computed"), Some(&1));
    assert_eq!(codes.get("migrate.code"), Some(&1));
    assert_eq!(codes.get("migrate.setup"), Some(&1));
    assert_eq!(codes.get("migrate.path"), Some(&1));
}

#[test]
fn notes_point_at_the_line_with_an_excerpt() {
    let c = convert_conf_py(CONF_PY, Path::new("conf.py"), Some(2026));
    let computed = c.notes.iter().find(|n| n.code == "migrate.computed").unwrap();
    assert_eq!(computed.line, 42);
    let excerpt = computed.excerpt.as_ref().unwrap();
    assert!(excerpt.text.starts_with("html_last_updated_fmt"));
    assert!(!computed.hints.is_empty());
    assert_eq!(
        c.attention,
        c.notes.iter().filter(|n| n.severity == gd_report::Severity::Warning).count()
    );
}

#[test]
fn guidedog_toml_migrates_to_its_source_root() {
    let old = r#"[project]
title = "Old"
version = "0.1"
[source]
roots = ["docs"]
entry = "docs/start.rst"
order = ["docs/a.rst"]
[pdf]
paper = "us-letter"
"#;
    let (file, want) = if cfg!(windows) {
        (r"C:\p\guidedog.toml", PathBuf::from(r"C:\p\docs"))
    } else {
        ("/p/guidedog.toml", PathBuf::from("/p/docs"))
    };
    let (c, dir) = convert_guidedog_toml(old, Path::new(file)).unwrap();
    assert_eq!(dir, want);
    check_conf_toml(&c.toml).unwrap();
    assert!(c.toml.contains(r#"root_doc = "start""#), "{}", c.toml);
    assert!(c.toml.contains(r#"pdf_paper_size = "letter""#), "{}", c.toml);
    assert_eq!(c.attention, 1);
}

/// A dictionary with a computed entry keeps its literal entries, as CPython's extlinks do.
#[test]
fn dictionaries_keep_their_literal_entries() {
    let conf = "branch = compute()\nextlinks = {\n\
                \x20   'pypi': ('https://pypi.org/project/%s/', '%s'),\n\
                \x20   'source': (f'https://github.com/x/tree/{branch}/%s', '%s'),\n}\n";
    let c = convert_conf_py(conf, Path::new("conf.py"), Some(2026));
    check_conf_toml(&c.toml).unwrap();
    assert!(c.toml.contains(r#"pypi = ["https://pypi.org/project/%s/", "%s"]"#), "{}", c.toml);
    assert!(!c.toml.contains("source = "), "{}", c.toml);
    assert!(c.notes.iter().any(|n| n.title == "SOME ENTRIES NOT CONVERTED"));
}

/// intersphinx_mapping migrates with None written as "", which conf.toml reads back.
#[test]
fn intersphinx_mapping_migrates() {
    let conf = "extensions = ['sphinx.ext.intersphinx']\nintersphinx_mapping = {\n\
                \x20   'python': ('https://docs.python.org/3/', None),\n\
                \x20   'click': ('https://click.palletsprojects.com/', ('click.inv', None)),\n}\n\
                intersphinx_disabled_reftypes = ['*']\n";
    let c = convert_conf_py(conf, Path::new("conf.py"), Some(2026));
    check_conf_toml(&c.toml).unwrap();
    for line in [
        r#"extensions = ["sphinx.ext.intersphinx"]"#,
        r#"python = ["https://docs.python.org/3/", ""]"#,
        r#"click = ["https://click.palletsprojects.com/", ["click.inv", ""]]"#,
        r#"intersphinx_disabled_reftypes = ["*"]"#,
    ] {
        expect_line(&c.toml, line);
    }
}

#[test]
fn unknown_names_get_a_suggestion() {
    let c = convert_conf_py("projct = 'x'\n", Path::new("conf.py"), Some(2026));
    let n = c.notes.iter().find(|n| n.code == "migrate.unknown").unwrap();
    assert_eq!(n.hints, ["Did you mean project?"]);
}

/// A fresh folder under the system's temporary folder, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("gd-migrate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn project_extensions_declare_object_types() {
    let dir = TempDir::new("ext");
    std::fs::create_dir_all(dir.0.join("_ext")).unwrap();
    std::fs::write(
        dir.0.join("_ext/widgets.py"),
        "from sphinx.domains.std import Cmdoption\n\n\
         def setup(app):\n\
         \x20   app.add_object_type('confval', 'confval', 'pair: %s; setting')\n\
         \x20   app.add_crossref_type(directivename='term2', rolename='t2')\n\
         \x20   app.add_directive('cmd-option', Cmdoption)\n\
         \x20   app.add_role('kbd2', kbd_role)\n",
    )
    .unwrap();
    let conf = "extensions = ['widgets']\n";
    let c = convert_conf_py(conf, &dir.0.join("conf.py"), Some(2026));
    check_conf_toml(&c.toml).unwrap();
    expect_line(
        &c.toml,
        "[[object_types]]\ndirective = \"confval\"\nrole = \"confval\"\n\
         index = \"pair: %s; setting\"",
    );
    expect_line(&c.toml, "[[crossref_types]]\ndirective = \"term2\"\nrole = \"t2\"");
    expect_line(&c.toml, "[directive_aliases]\ncmd-option = \"option\"");
    let read = c.notes.iter().find(|n| n.code == "migrate.extension.read").unwrap();
    assert!(read.message.contains("declares 3 object type(s)"), "{}", read.message);
    assert!(read.message.contains("add_role(kbd2)"), "{}", read.message);
}

#[test]
fn migrate_writes_beside_conf_py_and_refuses_to_overwrite() {
    let dir = TempDir::new("write");
    std::fs::write(dir.0.join("conf.py"), "project = 'Demo'\n").unwrap();
    let options = MigrateOptions { input: dir.0.clone(), year: Some(2026), ..Default::default() };
    let done = migrate(&options).unwrap();
    assert_eq!(done.output.as_deref(), Some(dir.0.join("conf.toml").as_path()));
    assert_eq!(done.conversion.converted, 1);
    let again = migrate(&options).unwrap_err();
    assert_eq!(again.problem.code, "migrate.exists");
    let forced = migrate(&MigrateOptions { force: true, ..options.clone() });
    assert!(forced.is_ok());
    let dry = migrate(&MigrateOptions { output: Destination::Nowhere, ..options }).unwrap();
    assert!(dry.output.is_none());
    assert!(dry.conversion.toml.contains("project = \"Demo\""));
}

#[test]
fn migrate_without_input_explains() {
    let dir = TempDir::new("missing");
    let failure = migrate(&MigrateOptions { input: dir.0.clone(), ..Default::default() });
    assert_eq!(failure.unwrap_err().problem.code, "migrate.missing");
}
