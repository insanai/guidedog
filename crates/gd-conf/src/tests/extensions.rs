//! Tests of the settings of Sphinx's built-in extensions (GDS 0011).

use super::load_text;
use crate::*;

#[test]
fn every_built_in_extension_is_accepted() {
    let names = [
        "sphinx.ext.napoleon",
        "sphinx.ext.autosummary",
        "sphinx.ext.viewcode",
        "sphinx.ext.inheritance_diagram",
        "sphinx.ext.apidoc",
        "sphinx.ext.coverage",
        "sphinx.ext.duration",
        "sphinx.ext.imgconverter",
    ];
    let list = names.map(|n| format!("\"{n}\"")).join(", ");
    let (c, warnings, problem) = load_text(&format!("extensions = [{list}]\n"));
    assert_eq!(problem, "");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(names.iter().all(|n| c.has_extension(n)));
}

#[test]
fn defaults_are_sphinx_defaults() {
    let c = Config::default();
    assert!(
        c.ext.napoleon.google_docstring
            && c.ext.napoleon.use_param
            && c.ext.napoleon.attr_annotations
    );
    assert!(!c.ext.napoleon.use_ivar && !c.ext.napoleon.preprocess_types);
    assert_eq!(c.ext.autosummary.generate, FlagOrList::Flag(true));
    assert!(c.ext.autosummary.ignore_module_all && !c.ext.autosummary.imported_members);
    assert_eq!(c.ext.apidoc.max_depth, 4);
    assert_eq!(c.ext.apidoc.automodule_options, ["members", "show-inheritance", "undoc-members"]);
    assert!(c.ext.viewcode.follow_imported_members && !c.ext.viewcode.line_numbers);
    assert_eq!(c.ext.linkcode.url, "");
    assert!(c.ext.coverage.write_headline && c.ext.coverage.statistics_to_stdout);
    assert_eq!(c.ext.duration.n_slowest, 5);
    assert_eq!(c.ext.duration.write_json, "sphinx-reading-durations.json");
    assert_eq!(c.ext.duration.limit, None);
    assert_eq!(c.ext.image_converter.converter, "convert");
}

#[test]
fn napoleon_settings_are_read() {
    let (c, _, problem) = load_text(
        r#"napoleon_use_ivar = true
napoleon_custom_sections = ["Side Effects", ["Arguments", "params_style"]]
napoleon_type_aliases = {CustomType = "mypackage.CustomType"}
"#,
    );
    assert_eq!(problem, "");
    assert!(c.ext.napoleon.use_ivar);
    let sections = &c.ext.napoleon.custom_sections;
    assert_eq!(sections[0], NapoleonSection { name: "Side Effects".into(), like: "".into() });
    assert_eq!(sections[1].like, "params_style");
    assert_eq!(c.ext.napoleon.type_aliases["CustomType"], "mypackage.CustomType");
    let (_, _, bad) = load_text("napoleon_custom_sections = [[\"A\", \"B\", \"C\"]]\n");
    assert!(bad.contains("section name, or a name and the section"), "{bad}");
    let (_, _, typo) = load_text("napoleon_use_iver = true\n");
    assert!(typo.contains("Did you mean napoleon_use_ivar?"), "{typo}");
}

#[test]
fn autosummary_generate_is_a_flag_or_a_list() {
    let (c, _, problem) = load_text("autosummary_generate = [\"api.rst\"]\n");
    assert_eq!(problem, "");
    assert!(c.ext.autosummary.generate.is_on_for("api.rst"));
    assert!(!c.ext.autosummary.generate.is_on_for("index.rst"));
    let (c, _, _) = load_text("autosummary_generate = false\n");
    assert_eq!(c.ext.autosummary.generate, FlagOrList::Flag(false));
    let (_, _, bad) = load_text("autosummary_generate = 3\n");
    assert!(bad.contains("true, false, or a list of strings"), "{bad}");
}

#[test]
fn apidoc_modules_are_read_and_checked() {
    let (c, _, problem) = load_text(
        r#"apidoc_max_depth = 2

[[apidoc_modules]]
path = "../src/shapes"
destination = "api"
separate_modules = true
exclude_patterns = ["**/test*"]
"#,
    );
    assert_eq!(problem, "");
    let m = &c.ext.apidoc.modules[0];
    assert_eq!((m.path.as_str(), m.destination.as_str()), ("../src/shapes", "api"));
    assert_eq!(m.separate_modules, Some(true));
    assert_eq!(m.module_first, None);
    assert_eq!(c.ext.apidoc.max_depth, 2);
    let (_, _, missing) = load_text("[[apidoc_modules]]\npath = \"src\"\n");
    assert!(missing.contains("has no destination"), "{missing}");
    let (_, _, outside) =
        load_text("[[apidoc_modules]]\npath = \"src\"\ndestination = \"../api\"\n");
    assert!(outside.contains("not a folder inside the source folder"), "{outside}");
    let (_, _, typo) =
        load_text("[[apidoc_modules]]\npath = \"s\"\ndestination = \"a\"\nmax_dept = 3\n");
    assert!(typo.contains("Did you mean max_depth?"), "{typo}");
    let (_, _, negative) = load_text("apidoc_max_depth = -1\n");
    assert!(negative.contains("cannot be negative"), "{negative}");
}

#[test]
fn viewcode_import_is_the_old_name() {
    let (c, _, problem) = load_text("viewcode_import = false\nviewcode_line_numbers = true\n");
    assert_eq!(problem, "");
    assert!(!c.ext.viewcode.follow_imported_members);
    assert!(c.ext.viewcode.line_numbers);
}

#[test]
fn linkcode_url_placeholders_are_checked() {
    let url = "https://github.com/o/r/blob/{branch}/{path}#L{line}-L{end_line}";
    let (c, warnings, problem) =
        load_text(&format!("extensions = [\"sphinx.ext.linkcode\"]\nlinkcode_url = \"{url}\"\n"));
    assert_eq!(problem, "");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(c.ext.linkcode.url, url);
    let (_, _, bad) = load_text("linkcode_url = \"https://x/{file}\"\n");
    assert!(bad.contains("{file} is not a field of linkcode_url"), "{bad}");
    let (_, _, open) = load_text("linkcode_url = \"https://x/{path\"\n");
    assert!(open.contains("is not closed"), "{open}");
    let (_, warnings, _) = load_text("extensions = [\"sphinx.ext.linkcode\"]\n");
    assert_eq!(warnings, ["config.ext.linkcode"]);
}

#[test]
fn inheritance_attributes_must_be_graphviz_names() {
    let (c, _, problem) =
        load_text("[inheritance_graph_attrs]\nrankdir = \"LR\"\nfontsize = 14\n");
    assert_eq!(problem, "");
    assert_eq!(c.ext.inheritance.graph_attrs["fontsize"], "14");
    let (_, _, bad) = load_text("[inheritance_node_attrs]\n\"shape];x\" = \"box\"\n");
    assert!(bad.contains("not a Graphviz name"), "{bad}");
}

#[test]
fn coverage_patterns_compile() {
    let (c, _, problem) = load_text(
        r#"coverage_ignore_functions = ["main$", "_.*"]
coverage_c_regexes = {function = "^PyAPI_FUNC\\(.*\\)\\s+([^_][\\w_]+)"}

[coverage_ignore_c_items]
function = ["PyObject_.*"]
"#,
    );
    assert_eq!(problem, "");
    assert_eq!(c.ext.coverage.ignore_c_items["function"], ["PyObject_.*"]);
    let (_, _, bad) = load_text("coverage_ignore_classes = [\"(open\"]\n");
    assert!(bad.contains("does not compile"), "{bad}");
    let (_, _, kind) = load_text("coverage_ignore_c_items = {function = 3}\n");
    assert!(kind.contains("a table of lists of strings"), "{kind}");
}

#[test]
fn duration_settings_are_bounded() {
    let (c, _, problem) = load_text("duration_limit = 2.5\nduration_n_slowest = 0\n");
    assert_eq!(problem, "");
    assert_eq!(c.ext.duration.limit, Some(2.5));
    let (c, _, _) = load_text("duration_limit = 3\n");
    assert_eq!(c.ext.duration.limit, Some(3.0));
    let (_, _, negative) = load_text("duration_limit = -1\n");
    assert!(negative.contains("positive number of seconds"), "{negative}");
    let (_, _, count) = load_text("duration_n_slowest = -2\n");
    assert!(count.contains("cannot be negative"), "{count}");
    let (_, _, outside) = load_text("duration_write_json = \"../times.json\"\n");
    assert!(outside.contains("outside the output folder"), "{outside}");
    let (c, _, _) = load_text("duration_write_json = \"\"\n");
    assert_eq!(c.ext.duration.write_json, "");
}

#[test]
fn image_converter_settings_are_accepted() {
    let (c, warnings, problem) =
        load_text("image_converter = \"magick\"\nimage_converter_args = [\"convert\"]\n");
    assert_eq!(problem, "");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(c.ext.image_converter.converter_args, ["convert"]);
}
