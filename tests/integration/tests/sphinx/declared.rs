//! Object types declared in conf.toml, as Sphinx extensions register them with
//! add_crossref_type, add_object_type and add_directive; and `guidedog migrate` reading
//! those declarations from an extension without running it.

use guidedog_tests::{Scratch, guidedog, has, lacks};

use crate::{build, in_order, project};

const DECLARED_CONF: &str = r#"project = "Demo"

[[crossref_types]]
directive = "setting"
role = "setting"
index = "pair: %s; setting"

[[object_types]]
directive = "django-admin"
role = "djadmin"
index = "pair: %s; django-admin command"
name = "first-word"
display = "django-admin %s"
program = true

[directive_aliases]
django-admin-option = "option"
awaitablefunction = "py:function"
"#;

/// Targets and descriptions get Sphinx's ids, roles reach them, index entries are made,
/// and options belong to the program.
#[test]
fn declared_object_types_follow_sphinx() {
    let index = "Demo\n====\n\n.. toctree::\n\n   ref\n\nSee :setting:`DEBUG`, \
        :djadmin:`migrate`, :option:`migrate --fake`, and :py:func:`fetch`.\n";
    let reference = "Reference\n=========\n\n.. setting:: DEBUG\n\n``DEBUG``\n---------\n\n\
        Default: ``False``\n\n.. django-admin:: migrate [app_label]\n\n   Runs migrations.\n\n\
        .. django-admin-option:: --fake\n\n   Marks them run.\n\n\
        .. awaitablefunction:: fetch(url)\n\n   Fetches.\n";
    let dir = project("declared", DECLARED_CONF, &[("index.rst", index), ("ref.rst", reference)]);
    build(&dir.path, 0, &["html", "-W", "-n"]);
    let index = dir.read("_build/html/index.html");
    for href in [
        "ref.html#setting-DEBUG",
        "ref.html#django-admin-migrate",
        "ref.html#cmdoption-migrate-fake",
        "ref.html#fetch",
    ] {
        has(&index, &format!("href=\"{href}\""));
    }
    let reference = dir.read("_build/html/ref.html");
    has(&reference, "id=\"setting-DEBUG\"");
    has(&reference, "django-admin migrate [app_label]");
    let genindex = dir.read("_build/html/genindex.html");
    has(&genindex, "DEBUG");
    has(&genindex, "setting");
    let r = build(&dir.path, 0, &["html"]);
    lacks(&r.err, "UNKNOWN");
}

/// `guidedog migrate` reads a project extension's setup() without running it, and
/// writes what it declares into conf.toml; what it cannot carry over is named.
#[test]
fn migrate_reads_extension_declarations() {
    let dir = Scratch::new("declared-migrate");
    dir.write("conf.py", "project = \"D\"\nextensions = [\"djangodocs\"]\n");
    dir.write("_ext/djangodocs.py", "from sphinx.domains.std import Cmdoption\n\n\
        def setup(app):\n    app.add_crossref_type(\n        directivename=\"setting\",\n        \
        rolename=\"setting\",\n        indextemplate=\"pair: %s; setting\",\n    )\n    \
        app.add_object_type('django-admin', 'djadmin', 'pair: %s; command',\n        \
        parse_node=parse_django_admin_node)\n    \
        app.add_directive(\"django-admin-option\", Cmdoption)\n    \
        app.add_role(\"sourcefile\", sourcefile)\n");
    let r = guidedog(&dir.path, 0, &["migrate", "conf.py", "--color=never"]);
    in_order(
        &dir.read("conf.toml"),
        &[
            "[[crossref_types]]",
            "directive = \"setting\"",
            "role = \"setting\"",
            "index = \"pair: %s; setting\"",
            "[[object_types]]",
            "directive = \"django-admin\"",
            "role = \"djadmin\"",
            "[directive_aliases]",
            "django-admin-option = \"option\"",
        ],
    );
    has(&r.err, "parse_django_admin_node");
    has(&r.err, "add_role(sourcefile)");
}

/// Declarations are checked like every other setting.
#[test]
fn declared_object_types_are_checked() {
    let conf = "project = \"D\"\n[[crossref_types]]\ndirective = \"setting\"\nprogram = true\n";
    let dir = project("declared-checked", conf, &[("index.rst", "Demo\n====\n")]);
    let r = build(&dir.path, 2, &["html"]);
    has(&format!("{}{}", r.out, r.err), "crossref_types has no field program");
    dir.write("conf.toml", "project = \"D\"\n[[object_types]]\nrole = \"x\"\n");
    let r = build(&dir.path, 2, &["html"]);
    has(&format!("{}{}", r.out, r.err), "has no directive");
}
