//! The settings table: every conf.toml key, its kind, and where it is stored.
//!
//! Each entry's [`Store`] is a typed accessor, so a setting can only be stored into a
//! field of the type its kind reads, and that is checked when the crate compiles.

use std::collections::BTreeMap;

use crate::config::{Config, FlagOrName};

/// What a setting's value is, as conf.toml writes it. gd-migrate checks a conf.py
/// value against it before writing the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingKind {
    /// A string in quotes.
    String,
    /// `true` or `false`.
    Bool,
    /// A whole number.
    Int,
    /// A list of strings, or one string.
    Strings,
    /// A table of strings (numbers and booleans are kept as text).
    Table,
    /// A suffix, a list of suffixes, or Sphinx's `{".rst" = "restructuredtext"}`.
    Suffixes,
    /// `pdf_documents`: a list of tables.
    PdfDocuments,
    /// `extlinks`: a table of two-string lists.
    Extlinks,
    /// A list of string pairs.
    Pairs,
    /// A list of pairs of regular expressions, compiled when loaded.
    PatternPairs,
    /// `intersphinx_mapping`.
    Intersphinx,
    /// A table, kept as the JSON Python's `json.dumps` writes for it.
    Json,
    /// A table, kept as parsed, in the order its keys are defined.
    Values,
    /// A string that may be unset, which differs from `""` (None and `''` in conf.py).
    OptionalString,
    /// True, false, or a name.
    FlagOrName,
    /// `[[object_types]]`.
    ObjectTypes,
    /// `[[crossref_types]]`.
    CrossrefTypes,
    /// `[directive_aliases]`.
    DirectiveAliases,
    /// `autodoc_default_options`: option names to values or `true`.
    AutodocOptions,
}

/// Where a setting's value goes: a typed field, or a reader of its own for the kinds
/// whose values need more than one field.
#[derive(Clone, Copy)]
pub(crate) enum Store {
    String(fn(&mut Config) -> &mut String),
    Bool(fn(&mut Config) -> &mut bool),
    Int(fn(&mut Config) -> &mut i64),
    Strings(fn(&mut Config) -> &mut Vec<String>),
    Table(fn(&mut Config) -> &mut BTreeMap<String, String>),
    Pairs(fn(&mut Config) -> &mut Vec<(String, String)>),
    Values(fn(&mut Config) -> &mut gd_toml::Table),
    OptionalString(fn(&mut Config) -> &mut Option<String>),
    FlagOrName(fn(&mut Config) -> &mut FlagOrName),
    /// The kind's own reader (`custom.rs`).
    Custom,
}

/// One conf.toml key: its name, its kind, and where it is stored.
#[derive(Clone, Copy)]
pub struct Setting {
    /// The key, as conf.py names it.
    pub name: &'static str,
    /// What its value is.
    pub kind: SettingKind,
    pub(crate) store: Store,
}

impl std::fmt::Debug for Setting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Setting").field("name", &self.name).field("kind", &self.kind).finish()
    }
}

const fn text(name: &'static str, field: fn(&mut Config) -> &mut String) -> Setting {
    Setting { name, kind: SettingKind::String, store: Store::String(field) }
}

const fn flag(name: &'static str, field: fn(&mut Config) -> &mut bool) -> Setting {
    Setting { name, kind: SettingKind::Bool, store: Store::Bool(field) }
}

const fn int(name: &'static str, field: fn(&mut Config) -> &mut i64) -> Setting {
    Setting { name, kind: SettingKind::Int, store: Store::Int(field) }
}

const fn list(name: &'static str, field: fn(&mut Config) -> &mut Vec<String>) -> Setting {
    Setting { name, kind: SettingKind::Strings, store: Store::Strings(field) }
}

const fn table(
    name: &'static str,
    field: fn(&mut Config) -> &mut BTreeMap<String, String>,
) -> Setting {
    Setting { name, kind: SettingKind::Table, store: Store::Table(field) }
}

const fn json(name: &'static str, field: fn(&mut Config) -> &mut String) -> Setting {
    Setting { name, kind: SettingKind::Json, store: Store::String(field) }
}

const fn custom(name: &'static str, kind: SettingKind) -> Setting {
    Setting { name, kind, store: Store::Custom }
}

const fn either(name: &'static str, field: fn(&mut Config) -> &mut FlagOrName) -> Setting {
    Setting { name, kind: SettingKind::FlagOrName, store: Store::FlagOrName(field) }
}

/// Every conf.toml key Guidedog reads, in the order the Sphinx documentation lists them.
pub static SETTINGS: &[Setting] = &[
    text("project", |c| &mut c.project),
    text("author", |c| &mut c.author),
    text("copyright", |c| &mut c.copyright),
    text("version", |c| &mut c.version),
    text("release", |c| &mut c.release),
    text("today", |c| &mut c.today),
    text("today_fmt", |c| &mut c.today_fmt),
    text("language", |c| &mut c.language),
    text("root_doc", |c| &mut c.root_doc),
    text("master_doc", |c| &mut c.root_doc),
    Setting {
        name: "source_suffix",
        kind: SettingKind::Suffixes,
        store: Store::Strings(|c| &mut c.source_suffix),
    },
    list("exclude_patterns", |c| &mut c.exclude_patterns),
    list("include_patterns", |c| &mut c.include_patterns),
    list("templates_path", |c| &mut c.templates_path),
    list("extensions", |c| &mut c.extensions),
    list("plugins", |c| &mut c.plugins.ids),
    table("plugin_configs", |c| &mut c.plugins.configs),
    int("plugin_timeout", |c| &mut c.plugins.timeout),
    text("rst_prolog", |c| &mut c.rst_prolog),
    text("rst_epilog", |c| &mut c.rst_epilog),
    text("default_role", |c| &mut c.default_role),
    text("primary_domain", |c| &mut c.primary_domain),
    text("highlight_language", |c| &mut c.highlight_language),
    text("pygments_style", |c| &mut c.pygments_style),
    text("pygments_dark_style", |c| &mut c.pygments_dark_style),
    flag("numfig", |c| &mut c.numfig),
    table("numfig_format", |c| &mut c.numfig_format),
    int("numfig_secnum_depth", |c| &mut c.numfig_secnum_depth),
    flag("math_number_all", |c| &mut c.math_number_all),
    text("math_eqref_format", |c| &mut c.math_eqref_format),
    flag("math_numfig", |c| &mut c.math_numfig),
    flag("nitpicky", |c| &mut c.nitpicky),
    Setting {
        name: "nitpick_ignore",
        kind: SettingKind::Pairs,
        store: Store::Pairs(|c| &mut c.nitpick_ignore),
    },
    text("mathjax_path", |c| &mut c.mathjax.path),
    table("mathjax_options", |c| &mut c.mathjax.options),
    list("mathjax_inline", |c| &mut c.mathjax.inline),
    list("mathjax_display", |c| &mut c.mathjax.display),
    json("mathjax3_config", |c| &mut c.mathjax.v3_config),
    json("mathjax4_config", |c| &mut c.mathjax.v4_config),
    text("mathjax_config_path", |c| &mut c.mathjax.config_path),
    custom("intersphinx_mapping", SettingKind::Intersphinx),
    int("intersphinx_cache_limit", |c| &mut c.intersphinx.cache_limit),
    int("intersphinx_timeout", |c| &mut c.intersphinx.timeout),
    list("intersphinx_disabled_reftypes", |c| &mut c.intersphinx.disabled_reftypes),
    text("intersphinx_resolve_self", |c| &mut c.intersphinx.resolve_self),
    list("odin_autoapi_dirs", |c| &mut c.odin_api.dirs),
    table("odin_collections", |c| &mut c.odin_api.collections),
    text("odin_autoapi_root", |c| &mut c.odin_api.root),
    list("odin_autoapi_options", |c| &mut c.odin_api.options),
    text("odin_autoapi_member_order", |c| &mut c.odin_api.member_order),
    flag("odin_autoapi_add_toctree_entry", |c| &mut c.odin_api.add_toctree_entry),
    flag("odin_autoapi_generate_api_docs", |c| &mut c.odin_api.generate_api_docs),
    list("rust_autoapi_crates", |c| &mut c.rust_api.crates),
    text("rust_autoapi_root", |c| &mut c.rust_api.root),
    list("rust_autoapi_options", |c| &mut c.rust_api.options),
    text("rust_autoapi_member_order", |c| &mut c.rust_api.member_order),
    flag("rust_autoapi_add_toctree_entry", |c| &mut c.rust_api.add_toctree_entry),
    flag("rust_autoapi_generate_api_docs", |c| &mut c.rust_api.generate_api_docs),
    list("autodoc_source_paths", |c| &mut c.autodoc.source_paths),
    text("autoclass_content", |c| &mut c.autodoc.autoclass_content),
    text("autodoc_class_signature", |c| &mut c.autodoc.class_signature),
    custom("autodoc_default_options", SettingKind::AutodocOptions),
    flag("autodoc_docstring_signature", |c| &mut c.autodoc.docstring_signature),
    flag("autodoc_inherit_docstrings", |c| &mut c.autodoc.inherit_docstrings),
    text("autodoc_member_order", |c| &mut c.autodoc.member_order),
    list("autodoc_mock_imports", |c| &mut c.autodoc.mock_imports),
    flag("autodoc_preserve_defaults", |c| &mut c.autodoc.preserve_defaults),
    text("autodoc_typehints", |c| &mut c.autodoc.typehints),
    text("autodoc_typehints_description_target", |c| &mut c.autodoc.typehints_description_target),
    text("autodoc_typehints_format", |c| &mut c.autodoc.typehints_format),
    table("autodoc_type_aliases", |c| &mut c.autodoc.type_aliases),
    flag("autodoc_use_type_comments", |c| &mut c.autodoc.use_type_comments),
    flag("autodoc_warningiserror", |c| &mut c.autodoc.warningiserror),
    flag("python_display_short_literal_types", |c| {
        &mut c.autodoc.python_display_short_literal_types
    }),
    flag("strip_signature_backslash", |c| &mut c.autodoc.strip_signature_backslash),
    custom("nitpick_ignore_regex", SettingKind::PatternPairs),
    flag("show_authors", |c| &mut c.show_authors),
    flag("add_function_parentheses", |c| &mut c.add_function_parentheses),
    flag("add_module_names", |c| &mut c.add_module_names),
    flag("toc_object_entries", |c| &mut c.toc_object_entries),
    text("toc_object_entries_show_parents", |c| &mut c.toc_object_entries_show_parents),
    flag("trim_footnote_reference_space", |c| &mut c.trim_footnote_reference_space),
    flag("smartquotes", |c| &mut c.smartquotes),
    flag("keep_warnings", |c| &mut c.keep_warnings),
    list("suppress_warnings", |c| &mut c.suppress_warnings),
    flag("autosectionlabel_prefix_document", |c| &mut c.autosectionlabel_prefix_document),
    int("autosectionlabel_maxdepth", |c| &mut c.autosectionlabel_maxdepth),
    flag("todo_include_todos", |c| &mut c.todo_include_todos),
    flag("trim_doctest_flags", |c| &mut c.trim_doctest_flags),
    text("manpages_url", |c| &mut c.manpages_url),
    flag("option_emphasise_placeholders", |c| &mut c.option_emphasise_placeholders),
    custom("extlinks", SettingKind::Extlinks),
    flag("extlinks_detect_hardcoded_links", |c| &mut c.extlinks_detect_hardcoded_links),
    custom("object_types", SettingKind::ObjectTypes),
    custom("crossref_types", SettingKind::CrossrefTypes),
    custom("directive_aliases", SettingKind::DirectiveAliases),
    text("html_theme", |c| &mut c.html.theme),
    table("html_theme_options", |c| &mut c.html.theme_options),
    text("html_title", |c| &mut c.html.title),
    text("html_short_title", |c| &mut c.html.short_title),
    text("html_logo", |c| &mut c.html.logo),
    text("html_favicon", |c| &mut c.html.favicon),
    list("html_static_path", |c| &mut c.html.static_path),
    list("html_extra_path", |c| &mut c.html.extra_path),
    list("include_roots", |c| &mut c.include_roots),
    list("html_css_files", |c| &mut c.html.css_files),
    list("html_js_files", |c| &mut c.html.js_files),
    Setting {
        name: "html_last_updated_fmt",
        kind: SettingKind::OptionalString,
        store: Store::OptionalString(|c| &mut c.html.last_updated_fmt),
    },
    flag("html_permalinks", |c| &mut c.html.permalinks),
    text("html_permalinks_icon", |c| &mut c.html.permalinks_icon),
    flag("html_use_index", |c| &mut c.html.use_index),
    flag("html_domain_indices", |c| &mut c.html.domain_indices),
    list("modindex_common_prefix", |c| &mut c.html.modindex_common_prefix),
    flag("html_split_index", |c| &mut c.html.split_index),
    flag("html_copy_source", |c| &mut c.html.copy_source),
    flag("html_show_sourcelink", |c| &mut c.html.show_sourcelink),
    text("html_sourcelink_suffix", |c| &mut c.html.sourcelink_suffix),
    flag("html_show_copyright", |c| &mut c.html.show_copyright),
    flag("html_show_sphinx", |c| &mut c.html.show_sphinx),
    text("html_baseurl", |c| &mut c.html.baseurl),
    text("html_file_suffix", |c| &mut c.html.file_suffix),
    text("html_link_suffix", |c| &mut c.html.link_suffix),
    text("html_secnumber_suffix", |c| &mut c.html.secnumber_suffix),
    Setting {
        name: "html_context",
        kind: SettingKind::Values,
        store: Store::Values(|c| &mut c.html.context),
    },
    table("html_additional_pages", |c| &mut c.html.additional_pages),
    custom("pdf_documents", SettingKind::PdfDocuments),
    text("pdf_paper_size", |c| &mut c.pdf.paper_size),
    text("pdf_logo", |c| &mut c.pdf.logo),
    text("pdf_toplevel_sectioning", |c| &mut c.pdf.toplevel_sectioning),
    text("pdf_show_urls", |c| &mut c.pdf.show_urls),
    text("pdf_preamble", |c| &mut c.pdf.preamble),
    text("pdf_packages", |c| &mut c.pdf.packages),
    text("pdf_fonts", |c| &mut c.pdf.fonts),
    list("pdf_font_paths", |c| &mut c.pdf.font_paths),
    text("graphviz_output_format", |c| &mut c.graphviz.output_format),
    list("graphviz_dot_args", |c| &mut c.graphviz.dot_args),
    text("graphviz_dot", |c| &mut c.graphviz.dot),
    text("typst", |c| &mut c.pdf.typst),
    list("myst_enable_extensions", |c| &mut c.myst.enable_extensions),
    int("myst_heading_anchors", |c| &mut c.myst.heading_anchors),
    flag("myst_dmath_allow_labels", |c| &mut c.myst.dmath_allow_labels),
    flag("myst_dmath_allow_space", |c| &mut c.myst.dmath_allow_space),
    flag("myst_dmath_allow_digits", |c| &mut c.myst.dmath_allow_digits),
    flag("myst_dmath_double_inline", |c| &mut c.myst.dmath_double_inline),
    flag("myst_linkify_fuzzy_links", |c| &mut c.myst.linkify_fuzzy_links),
    table("myst_substitutions", |c| &mut c.myst.substitutions),
    list("myst_sub_delimiters", |c| &mut c.myst.sub_delimiters),
    list("myst_url_schemes", |c| &mut c.myst.url_schemes),
    flag("myst_footnote_transition", |c| &mut c.myst.footnote_transition),
    table("myst_html_meta", |c| &mut c.myst.html_meta),
    flag("myst_commonmark_only", |c| &mut c.myst.commonmark_only),
    list("myst_disable_syntax", |c| &mut c.myst.disable_syntax),
    int("myst_words_per_minute", |c| &mut c.myst.words_per_minute),
    flag("myst_dmath_enable", |c| &mut c.myst.dmath_enable),
    flag("myst_amsmath_enable", |c| &mut c.myst.amsmath_enable),
    list("locale_dirs", |c| &mut c.intl.locale_dirs),
    either("gettext_compact", |c| &mut c.intl.gettext_compact),
    flag("gettext_uuid", |c| &mut c.intl.gettext_uuid),
    flag("gettext_location", |c| &mut c.intl.gettext_location),
    flag("gettext_auto_build", |c| &mut c.intl.gettext_auto_build),
    list("gettext_additional_targets", |c| &mut c.intl.gettext_additional_targets),
    list("gettext_exclude_patterns", |c| &mut c.intl.gettext_exclude_patterns),
    flag("gettext_allow_fuzzy_translations", |c| &mut c.intl.gettext_allow_fuzzy_translations),
    text("gettext_last_translator", |c| &mut c.intl.gettext_last_translator),
    text("gettext_language_team", |c| &mut c.intl.gettext_language_team),
    text("figure_language_filename", |c| &mut c.intl.figure_language_filename),
    either("translation_progress_classes", |c| &mut c.intl.translation_progress_classes),
];

/// Sphinx settings for its other builders and Python extensions: accepted, warned about,
/// and ignored, so a migrated configuration still builds.
pub const FOREIGN_PREFIXES: &[&str] = &[
    "latex_",
    "epub_",
    "man_",
    "texinfo_",
    "autodoc_",
    "autosummary_",
    "intersphinx_",
    "napoleon_",
    "htmlhelp_",
    "applehelp_",
    "qthelp_",
    "devhelp_",
    "linkcheck_",
    "mathjax",
    "html_sidebars",
    "html_search_",
    "needs_",
    "html_codeblock_linenos_style",
    "html_scaled_image_link",
    "html_output_encoding",
    "myst_heading_slug_func",
    "myst_update_mathjax",
    "myst_mathjax_classes",
    "doctest_",
    "myst_title_to_header",
];

/// Extensions Guidedog implements natively; anything else needs the plugin system.
pub const NATIVE_EXTENSIONS: &[&str] = &[
    "sphinx.ext.mathjax",
    "sphinx.ext.todo",
    "sphinx.ext.githubpages",
    "sphinx.ext.ifconfig",
    "sphinx.ext.extlinks",
    "sphinx.ext.imgmath",
    "sphinx.ext.autosectionlabel",
    "sphinx.ext.graphviz",
    "sphinx.ext.doctest",
    "sphinx.ext.intersphinx",
    "sphinx.ext.autodoc",
    "myst_parser",
];

/// Themes Sphinx projects name; each builds with the guidedog theme.
pub const KNOWN_THEMES: &[&str] = &[
    "guidedog",
    "alabaster",
    "classic",
    "sphinxdoc",
    "scrolls",
    "agogo",
    "traditional",
    "nature",
    "haiku",
    "pyramid",
    "bizstyle",
    "sphinx_rtd_theme",
    "furo",
    "pydata_sphinx_theme",
    "sphinx_book_theme",
    "press",
];

/// The optional syntaxes `myst_enable_extensions` may list.
pub const MYST_EXTENSIONS: &[&str] = &[
    "amsmath",
    "colon_fence",
    "deflist",
    "dollarmath",
    "fieldlist",
    "html_admonition",
    "html_image",
    "linkify",
    "replacements",
    "smartquotes",
    "substitution",
    "tasklist",
];

/// The setting named `key`, or `None` when Guidedog has no such setting.
pub fn find_setting(key: &str) -> Option<&'static Setting> {
    SETTINGS.iter().find(|s| s.name == key)
}

/// Whether `key` configures a Sphinx builder or extension Guidedog does not have
/// (`latex_elements`, `napoleon_*`, ...): accepted with a warning, and ignored.
pub fn is_foreign_setting(key: &str) -> bool {
    FOREIGN_PREFIXES.iter().any(|p| key.starts_with(p))
}

/// The hint for an unknown setting: "Did you mean X?" naming the closest known setting
/// when it is close enough to be a typo, or where to look otherwise.
pub fn suggestion(key: &str) -> String {
    let mut best = ("", usize::MAX);
    for s in SETTINGS {
        let d = gd_report::edit_distance(key, s.name);
        if d < best.1 {
            best = (s.name, d);
        }
    }
    if best.1 <= (key.len() / 4).max(2) {
        return format!("Did you mean {}?", best.0);
    }
    "Check the setting's name against the Sphinx configuration documentation.".to_string()
}
