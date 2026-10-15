//! Tests ported from lib/project/config_test.odin and trust_test.odin, plus the
//! rust_autoapi settings.

use std::path::{Path, PathBuf};

use super::*;

fn load_text(text: &str) -> (Config, Vec<String>, String) {
    let mut c = Config::new("/project", "/project/_build");
    let table = match gd_toml::parse(text) {
        Ok(t) => t,
        Err(e) => return (c, Vec::new(), e.message),
    };
    let mut warnings = Vec::new();
    let problem = apply_table(&mut c, &table, "conf.toml", &mut warnings, true)
        .err()
        .map(|p| format!("{}\n{}", p.message, p.hint))
        .unwrap_or_default();
    finish_config(&mut c);
    (c, warnings.into_iter().map(|w| w.code).collect(), problem)
}

#[test]
fn configuration_follows_conf_py() {
    let (c, warnings, problem) = load_text(
        r##"project = "Demo"
copyright = "2026, Ada"
author = "Ada"
version = "0.1"
release = "0.1.0"
extensions = ["sphinx.ext.todo"]
templates_path = ["_templates"]
exclude_patterns = ["_build", "Thumbs.db", ".DS_Store"]
language = "en"
html_theme = "alabaster"
html_static_path = ["_static"]
latex_elements = {papersize = "a4paper"}
source_suffix = {".rst" = "restructuredtext", ".md" = "markdown"}
numfig = true

[html_theme_options]
accent = "#126d78"

[extlinks]
issue = ["https://example.org/issues/%s", "issue %s"]
"##,
    );
    assert_eq!(problem, "");
    assert_eq!(c.project, "Demo");
    assert_eq!(c.html.title, "Demo 0.1.0 documentation");
    assert_eq!(c.source_suffix.len(), 2);
    assert_eq!(c.html.theme_options["accent"], "#126d78");
    assert_eq!(c.extlinks["issue"].1, "issue %s");
    assert_eq!(c.pdf.documents[0].file, "demo.pdf");
    assert!(c.numfig);
    // The theme is substituted, and _static does not exist here, which Sphinx warns about.
    assert_eq!(warnings.join(","), "config.ignored,config.theme,config.html");
}

#[test]
fn configuration_rejects_mistakes_with_directions() {
    let (_, _, typo) = load_text("projct = \"x\"\n");
    assert!(typo.contains("projct"));
    assert!(typo.contains("Did you mean project?"));
    let (_, _, kind) = load_text("numfig = \"yes\"\n");
    assert!(kind.contains("numfig must be true or false"));
    let (_, _, ext) = load_text("extensions = [\"sphinx.ext.napoleon\"]\n");
    assert!(ext.contains("sphinx.ext.napoleon"));
    let (_, _, theme) = load_text("html_theme = \"mytheme\"\n");
    assert!(theme.contains("mytheme"));
    assert_eq!(suggestion("projct"), "Did you mean project?");
    assert_eq!(suggestion("html_them"), "Did you mean html_theme?");
}

#[test]
fn numfig_format_keeps_the_other_kinds() {
    let (c, _, problem) = load_text("numfig_format = {figure = \"Abb. %s\"}\n");
    assert_eq!(problem, "");
    assert_eq!(c.numfig_format["figure"], "Abb. %s");
    assert_eq!(c.numfig_format["table"], "Table %s");
}

#[test]
fn defines_override_configuration() {
    let mut c = Config::new("/p", "/p/_build");
    let mut w = Vec::new();
    let mut define =
        |c: &mut Config, a: &str| apply_override(c, a, &mut w, true).map_err(|p| p.exit);
    assert_eq!(define(&mut c, "numfig=true"), Ok(()));
    assert_eq!(define(&mut c, "project=My Docs"), Ok(()));
    assert_eq!(define(&mut c, "exclude_patterns=[\"drafts\"]"), Ok(()));
    assert!(c.numfig);
    assert_eq!(c.project, "My Docs");
    assert_eq!(c.exclude_patterns, ["drafts"]);
    assert_eq!(define(&mut c, "novalue"), Err(2));
    // A text setting takes the value as written, as sphinx-build does: a version is not a
    // number, and a quoted value keeps its TOML form.
    assert_eq!(define(&mut c, "version=0.2.0"), Ok(()));
    assert_eq!(c.version, "0.2.0");
    assert_eq!(define(&mut c, "release=2026"), Ok(()));
    assert_eq!(c.release, "2026");
    assert_eq!(define(&mut c, "project=\"Quoted\""), Ok(()));
    assert_eq!(c.project, "Quoted");
    // sphinx-build -D takes 0 and 1 for a boolean setting.
    assert_eq!(define(&mut c, "numfig=0"), Ok(()));
    assert!(!c.numfig);
    assert_eq!(define(&mut c, "numfig=1"), Ok(()));
    assert!(c.numfig);
    assert_eq!(define(&mut c, "numfig=2"), Err(2));
}

#[test]
fn dotted_define_sets_one_key_of_a_table() {
    let mut c = Config::new("/p", "/p/_build");
    let mut w = Vec::new();
    for a in [
        "html_context={a = \"one\", t = {x = 1}}",
        "html_context.b=two",
        "html_context.t.y=2",
        "html_context.a=uno",
    ] {
        assert!(apply_override(&mut c, a, &mut w, true).is_ok(), "{a}");
    }
    let t = &c.html.context;
    assert_eq!(t.keys().collect::<Vec<_>>(), ["a", "t", "b"]);
    assert_eq!(t.get("a").and_then(|v| v.as_str()), Some("uno"));
    assert_eq!(t.get("b").and_then(|v| v.as_str()), Some("two"));
    let inner = t.get("t").and_then(|v| v.as_table()).unwrap();
    assert_eq!(inner.keys().collect::<Vec<_>>(), ["x", "y"]);
}

#[test]
fn settings_are_checked_after_the_overrides() {
    let table = gd_toml::parse("html_extra_path = [\"_editions\"]").unwrap();
    // load checks once the -D overrides are applied: one that drops the missing folder
    // leaves nothing to report.
    let mut c = Config::new("/no/such/project", "/no/such/project/_build");
    let mut w = Vec::new();
    assert!(apply_table(&mut c, &table, "conf.toml", &mut w, false).is_ok());
    assert!(apply_override(&mut c, "html_extra_path=[]", &mut w, false).is_ok());
    assert!(check_config(&mut c, "conf.toml", &mut w).is_ok());
    assert!(w.is_empty());
    let mut q = Config::new("/no/such/project", "/no/such/project/_build");
    assert!(apply_table(&mut q, &table, "conf.toml", &mut w, true).is_ok());
    assert_eq!(w.len(), 1);
}

// myst-parser 0.16.1's settings: myst_parser makes .md a document suffix, optional
// syntax is chosen by name, and a misspelled name is explained.
#[test]
fn myst_settings_follow_myst_parser() {
    let (c, _, problem) = load_text(
        "extensions = [\"myst_parser\"]\nmyst_enable_extensions = [\"colon_fence\", \
         \"dollarmath\"]\nmyst_heading_anchors = 2\nmyst_substitutions = {version = \"1.0\"}\n",
    );
    assert_eq!(problem, "");
    assert!(c.source_suffix.iter().any(|s| s == ".md"), "myst_parser adds .md");
    assert_eq!(c.myst.heading_anchors, 2);
    assert_eq!(c.myst.substitutions["version"], "1.0");
    assert!(c.myst.dmath_allow_labels && c.myst.footnote_transition);
    let (_, _, problem) = load_text("myst_enable_extensions = [\"dolarmath\"]\n");
    assert!(problem.contains("dolarmath"), "{problem}");
    assert!(problem.contains("Did you mean dollarmath?"), "{problem}");
}

#[test]
fn every_setting_is_listed_once() {
    let mut names: Vec<&str> = SETTINGS.iter().map(|s| s.name).collect();
    names.sort_unstable();
    let count = names.len();
    names.dedup();
    assert_eq!(names.len(), count);
    assert!(find_setting("master_doc").is_some());
    assert!(is_foreign_setting("latex_elements"));
}

#[test]
fn declared_types_and_patterns_are_read() {
    let (c, _, problem) = load_text(
        "nitpick_ignore_regex = [[\"py:.*\", \"foo\\\\..*\"]]\n\
         [[object_types]]\ndirective = \"django-admin\"\nrole = \"djadmin\"\n\
         name = \"first-word\"\n\
         [[crossref_types]]\ndirective = \"setting\"\nrole = \"setting\"\n\
         [directive_aliases]\ndjango-admin-option = \"option\"\n",
    );
    assert_eq!(problem, "");
    assert_eq!(c.object_types.len(), 2);
    assert!(c.object_types[0].first_word && !c.object_types[0].target);
    assert!(c.object_types[1].target);
    assert_eq!(
        c.declared_object_type("std:djadmin").map(|t| t.directive.as_str()),
        Some("django-admin")
    );
    assert_eq!(c.directive_aliases, [("django-admin-option".to_string(), "option".to_string())]);
    let (kind, target) = &c.nitpick_ignore_regex[0];
    assert!(kind.full_match("py:class") && target.full_match("foo.bar"));
    let (_, _, bad) = load_text("nitpick_ignore_regex = [[\"py:(\", \"x\"]]\n");
    assert!(bad.contains("is not a regular expression"), "{bad}");
    let (_, _, field) = load_text("[[crossref_types]]\ndirective = \"x\"\nprogram = true\n");
    assert!(field.contains("crossref_types has no field program"), "{field}");
}

#[test]
fn intersphinx_mapping_is_checked_as_sphinx_does() {
    let (c, _, problem) = load_text(
        "extensions = [\"sphinx.ext.intersphinx\"]\n[intersphinx_mapping]\n\
         python = [\"https://docs.python.org/3/\", \"\"]\n\
         click = [\"https://click.palletsprojects.com/\", [\"click.inv\", \"\"]]\n",
    );
    assert_eq!(problem, "");
    let names: Vec<&str> = c.intersphinx.mapping.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["click", "python"]);
    assert_eq!(c.intersphinx.mapping[0].locations, ["click.inv", ""]);
    let (_, _, twice) = load_text(
        "[intersphinx_mapping]\na = [\"https://x/\", \"\"]\nb = [\"https://x/\", \"\"]\n",
    );
    assert!(twice.contains("repeats the target URI"), "{twice}");
}

#[test]
fn autodoc_and_odin_choices_are_checked() {
    let (c, _, problem) = load_text(
        "[autodoc_default_options]\nmembers = true\nundoc-members = false\n\
         exclude-members = \"x\"\n",
    );
    assert_eq!(problem, "");
    assert_eq!(c.autodoc.default_options.len(), 2);
    assert!(c.autodoc.default_options[0].flag);
    let (_, _, order) = load_text("autodoc_member_order = \"random\"\n");
    assert!(order.contains("autodoc_member_order"), "{order}");
    let (_, _, odin) = load_text("odin_autoapi_root = \"../api\"\n");
    assert!(odin.contains("must name a folder inside the source folder"), "{odin}");
}

#[test]
fn rust_api_settings_are_checked_like_odin_ones() {
    let (c, warnings, problem) = load_text(
        "rust_autoapi_crates = [\"../crates/demo\"]\nrust_autoapi_options = [\"noindex\"]\n\
         rust_autoapi_member_order = \"alphabetical\"\n",
    );
    assert_eq!(problem, "");
    assert_eq!(c.rust_api.root, "rustapi");
    assert!(c.rust_api.add_toctree_entry && c.rust_api.generate_api_docs);
    assert_eq!(warnings, ["config.rust"]);
    assert!(readable_roots(&c).contains(&PathBuf::from("/crates/demo")));
    let (_, _, order) = load_text("rust_autoapi_member_order = \"groupwise\"\n");
    assert!(order.contains("not source or alphabetical"), "{order}");
    let (_, _, option) = load_text("rust_autoapi_options = [\"members\"]\n");
    assert!(option.contains("rust_autoapi_options lists \"members\""), "{option}");
    let (_, _, root) = load_text("rust_autoapi_root = \"\"\n");
    assert!(root.contains("rust_autoapi_root"), "{root}");
}

#[test]
fn intl_settings_are_checked() {
    let (_, _, target) = load_text("gettext_additional_targets = [\"literal\"]\n");
    assert!(target.contains("not a target"), "{target}");
    let (_, _, figure) = load_text("figure_language_filename = \"{root}.{lang}{ext}\"\n");
    assert!(figure.contains("{lang} is not a field"), "{figure}");
    assert!(check_figure_format("{{x}}{root}{ext}").is_ok());
    assert!(check_figure_format("{root").is_err());
    let (_, _, progress) = load_text("translation_progress_classes = \"both\"\n");
    assert!(progress.contains("translation_progress_classes is both"), "{progress}");
}

#[test]
fn plugin_settings_are_bounded() {
    let (_, _, timeout) = load_text("plugin_timeout = 0\n");
    assert!(timeout.contains("between 1 and 86400"), "{timeout}");
    let (c, _, problem) =
        load_text("plugins = [\"badge\"]\nplugin_configs = {badge = \"b.toml\"}\n");
    assert_eq!(problem, "");
    assert_eq!(c.plugins.ids, ["badge"]);
    assert_eq!(c.plugins.configs["badge"], "b.toml");
}

#[test]
fn mathjax_tables_become_json() {
    let (c, _, problem) = load_text("[mathjax3_config]\ntex = {inlineMath = [[\"$\", \"$\"]]}\n");
    assert_eq!(problem, "");
    assert_eq!(c.mathjax.v3_config, "{\"tex\": {\"inlineMath\": [[\"$\", \"$\"]]}}");
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gd-conf-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn projects_are_found_in_their_layouts() {
    let top = temp_dir("find");
    std::fs::create_dir_all(top.join("plain/sub")).unwrap();
    std::fs::write(top.join("plain/conf.toml"), "project = \"P\"\n").unwrap();
    let (source, build) = find_project(&top.join("plain/sub")).unwrap();
    assert_eq!((source.clone(), build), (top.join("plain"), top.join("plain/_build")));
    std::fs::create_dir_all(top.join("split/source")).unwrap();
    std::fs::create_dir_all(top.join("split/build")).unwrap();
    std::fs::write(top.join("split/source/conf.toml"), "").unwrap();
    let (source, build) = find_project(&top.join("split")).unwrap();
    assert_eq!((source, build), (top.join("split/source"), top.join("split/build")));
    let loaded = load_config(
        &top.join("plain"),
        &LoadOptions { overrides: vec!["version=1.2".into()], ..LoadOptions::default() },
    )
    .unwrap();
    assert_eq!(loaded.config.project, "P");
    assert_eq!(loaded.config.release, "1.2");
    let again = load_config(&top.join("plain"), &LoadOptions::default()).unwrap();
    assert_ne!(loaded.settings, again.settings, "-D changes the settings digest");
    let missing = load_config(Path::new("/"), &LoadOptions::default()).unwrap_err();
    assert_eq!((missing.code, missing.exit), ("project.missing", 2));
    let _ = std::fs::remove_dir_all(&top);
}

#[test]
fn untrusted_builds_read_only_below_the_source_folder() {
    let top = temp_dir("trust");
    let source = top.join("doc");
    std::fs::create_dir_all(source.join("_static")).unwrap();
    let mut c = Config::new(source.clone(), source.join("_build"));
    c.include_roots = vec!["..".into()];
    c.html.static_path = vec!["_static".into(), "../shared".into()];
    c.html.logo = "../logo.png".into();
    c.pdf.preamble = "book.typ".into();
    c.rust_api.crates = vec!["../crate".into()];
    let reports = confine_untrusted(&mut c);
    assert!(c.include_roots.is_empty() && c.pdf.preamble.is_empty() && c.html.logo.is_empty());
    assert_eq!(c.html.static_path, ["_static"]);
    assert!(c.rust_api.crates.is_empty());
    assert_eq!(c.pdf.packages, "offline");
    assert_eq!(reports.len(), 5);
    assert!(
        reports.iter().all(|r| r.code == "build.untrusted" && r.category == "build.untrusted")
    );
    let outside = unread_warning(
        "build.output",
        "FOLDER NOT COPIED",
        &top,
        "folder",
        gd_host::Confinement::Outside,
    )
    .unwrap();
    assert_eq!(outside.code, "build.output.outside");
    assert!(outside.message.contains("is outside the source folder"));
    let _ = std::fs::remove_dir_all(&top);
}
