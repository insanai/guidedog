//! The configuration model: `Config` and its groups, with Sphinx's defaults.
//!
//! conf.toml keys stay flat, as in conf.py; only the struct is grouped. The settings
//! table (`settings.rs`) maps every key to its field.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::extensions::ExtensionSettings;
use crate::pattern::Pattern;

/// The name of a project's configuration, in its source directory.
pub const CONFIG_FILE: &str = "conf.toml";

/// MathJax 4 from its CDN, Sphinx's default `mathjax_path`.
pub const MATHJAX_URL: &str = "https://cdn.jsdelivr.net/npm/mathjax@4/tex-mml-chtml.js";

/// The most object and cross-reference types a project may declare.
pub const MAX_OBJECT_TYPES: usize = 64;

/// conf.toml, loaded: the project's settings with Sphinx's names and defaults.
///
/// Make one with [`Config::new`] (the defaults) or [`crate::load_config`]; a `Default`
/// config has empty folders.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Optional static publication outputs from `[site]`.
    pub site: crate::SiteConfig,
    /// The source-document folder (absolute).
    pub source_dir: PathBuf,
    /// conf.toml's folder; `-c` may put it outside `source_dir`.
    pub config_dir: PathBuf,
    /// `_build` beside the sources, or `build/` in the separated layout.
    pub build_dir: PathBuf,
    /// `project`: the project's name.
    pub project: String,
    /// `author`.
    pub author: String,
    /// `copyright`.
    pub copyright: String,
    /// `version`: the short X.Y version.
    pub version: String,
    /// `release`: the full version; `version` when unset.
    pub release: String,
    /// `today`: the date `|today|` shows; empty: the build's date in `today_fmt`.
    pub today: String,
    /// `today_fmt`: a strftime format for `|today|`.
    pub today_fmt: String,
    /// `language`: the documents' language code.
    pub language: String,
    /// `root_doc` (or `master_doc`): the document the toctrees start from.
    pub root_doc: String,
    /// `source_suffix`: the suffixes of documents.
    pub source_suffix: Vec<String>,
    /// `exclude_patterns`: Sphinx glob patterns of paths that are not documents.
    pub exclude_patterns: Vec<String>,
    /// `include_roots`: folders beyond the source folder files may come from.
    pub include_roots: Vec<String>,
    /// `include_patterns`: glob patterns of paths that may be documents.
    pub include_patterns: Vec<String>,
    /// `templates_path`: folders of HTML templates.
    pub templates_path: Vec<String>,
    /// `extensions`: the Sphinx extensions turned on (built-in names only).
    pub extensions: Vec<String>,
    /// `rst_prolog`.
    pub rst_prolog: String,
    /// `rst_epilog`.
    pub rst_epilog: String,
    /// `default_role`.
    pub default_role: String,
    /// `primary_domain`.
    pub primary_domain: String,
    /// `highlight_language`.
    pub highlight_language: String,
    /// `pygments_style`: the code style; empty is the theme's.
    pub pygments_style: String,
    /// `pygments_dark_style`: the code style of dark pages; empty is github-dark.
    pub pygments_dark_style: String,
    /// `numfig`: number figures, tables and code blocks.
    pub numfig: bool,
    /// `numfig_format`: the caption format per kind (`figure`, `table`, ...).
    pub numfig_format: BTreeMap<String, String>,
    /// `numfig_secnum_depth`.
    pub numfig_secnum_depth: i64,
    /// `math_number_all`.
    pub math_number_all: bool,
    /// `math_eqref_format`.
    pub math_eqref_format: String,
    /// `math_numfig`.
    pub math_numfig: bool,
    /// `nitpicky`: warn about every unresolved reference.
    pub nitpicky: bool,
    /// `nitpick_ignore`: (type, target) pairs never warned about.
    pub nitpick_ignore: Vec<(String, String)>,
    /// `nitpick_ignore_regex`: (type, target) patterns, each matching whole names.
    pub nitpick_ignore_regex: Vec<(Pattern, Pattern)>,
    /// `show_authors`.
    pub show_authors: bool,
    /// `add_function_parentheses`.
    pub add_function_parentheses: bool,
    /// `add_module_names`.
    pub add_module_names: bool,
    /// `toc_object_entries`.
    pub toc_object_entries: bool,
    /// `toc_object_entries_show_parents`: `domain`, `hide` or `all`.
    pub toc_object_entries_show_parents: String,
    /// `trim_footnote_reference_space`.
    pub trim_footnote_reference_space: bool,
    /// `smartquotes`.
    pub smartquotes: bool,
    /// `keep_warnings`.
    pub keep_warnings: bool,
    /// `suppress_warnings`: warning types (`type`, `type.subtype`, `type.*`) not shown.
    pub suppress_warnings: Vec<String>,
    /// `todo_include_todos`.
    pub todo_include_todos: bool,
    /// `autosectionlabel_prefix_document`: labels are `docname:Title`.
    pub autosectionlabel_prefix_document: bool,
    /// `autosectionlabel_maxdepth`: sections this deep or deeper get no label; 0: all.
    pub autosectionlabel_maxdepth: i64,
    /// `trim_doctest_flags`.
    pub trim_doctest_flags: bool,
    /// `manpages_url`.
    pub manpages_url: String,
    /// `option_emphasise_placeholders`.
    pub option_emphasise_placeholders: bool,
    /// `extlinks`: role name to (URL pattern, caption pattern).
    pub extlinks: BTreeMap<String, (String, String)>,
    /// `extlinks_detect_hardcoded_links`.
    pub extlinks_detect_hardcoded_links: bool,
    /// `[[object_types]]` and `[[crossref_types]]`, in the order declared.
    pub object_types: Vec<ObjectTypeDecl>,
    /// `[directive_aliases]`: (alias, the Sphinx directive it stands for).
    pub directive_aliases: Vec<(String, String)>,
    /// The `mathjax_*` settings.
    pub mathjax: MathjaxConfig,
    /// The `intersphinx_*` settings.
    pub intersphinx: IntersphinxConfig,
    /// The `autodoc_*` settings and their relatives.
    pub autodoc: AutodocConfig,
    /// The `odin_*` settings.
    pub odin_api: OdinApiConfig,
    /// The `rust_autoapi_*` settings.
    pub rust_api: RustApiConfig,
    /// `plugins`, `plugin_configs`, `plugin_timeout`.
    pub plugins: PluginConfig,
    /// The `html_*` settings.
    pub html: HtmlConfig,
    /// The `pdf_*` settings and `typst`.
    pub pdf: PdfConfig,
    /// The `graphviz_*` settings.
    pub graphviz: GraphvizConfig,
    /// The `myst_*` settings.
    pub myst: MystConfig,
    /// Internationalization: `locale_dirs`, `gettext_*` and their relatives.
    pub intl: IntlConfig,
    /// The settings of napoleon, autosummary, apidoc, viewcode, linkcode,
    /// inheritance_diagram, coverage, duration and imgconverter (GDS 0011).
    pub ext: ExtensionSettings,
}

/// One book of `pdf_documents`: the document it starts from, and its file, title and
/// author.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PdfDocument {
    /// The root document of the book.
    pub root: String,
    /// The PDF's name in `_build/pdf`.
    pub file: String,
    /// The title on the cover.
    pub title: String,
    /// The author on the cover.
    pub author: String,
}

/// A setting that is true, false, or a name, as `gettext_compact` is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlagOrName {
    /// `true` or `false`.
    Flag(bool),
    /// A name in quotes, which counts as on.
    Name(String),
}

impl FlagOrName {
    /// The name, when one was given.
    pub fn name(&self) -> Option<&str> {
        match self {
            FlagOrName::Flag(_) => None,
            FlagOrName::Name(name) => Some(name),
        }
    }
}

/// An object type a project declares in conf.toml, as a Sphinx extension's `setup()`
/// does with `add_object_type` (`[[object_types]]`) or `add_crossref_type`
/// (`[[crossref_types]]`, `target` set).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectTypeDecl {
    /// The directive that describes such an object.
    pub directive: String,
    /// The role that refers to one; empty: none.
    pub role: String,
    /// The index entry template, with `%s` for the name.
    pub index: String,
    /// How the signature is shown, with `%s` for it; empty: as written.
    pub display: String,
    /// The object's name is the signature's first word, not the whole signature.
    pub first_word: bool,
    /// Options that follow belong to it, as with `program`.
    pub program: bool,
    /// Declared with `[[crossref_types]]`: a target, not a described object.
    pub target: bool,
}

/// One `autodoc_default_options` entry: an option name and its value, or a flag.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AutodocDefaultOption {
    /// The option's name, such as `members`.
    pub name: String,
    /// Its value as it would follow the option; empty for a flag.
    pub value: String,
    /// The option is a flag (written `true`).
    pub flag: bool,
}

/// One `intersphinx_mapping` entry: the name references use as a prefix, the base URL
/// of the other project's pages, and where its inventory is, tried in order; an empty
/// location stands for None, the base URL's `objects.inv`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IntersphinxProject {
    /// The project's name, a reference prefix.
    pub name: String,
    /// The base URL of its pages.
    pub uri: String,
    /// Inventory locations, tried in order; `""` is `<uri>/objects.inv`.
    pub locations: Vec<String>,
}

/// `sphinx.ext.mathjax`, Sphinx's HTML math.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathjaxConfig {
    /// `mathjax_path`: the MathJax script.
    pub path: String,
    /// `mathjax_options`: the script tag's attributes.
    pub options: BTreeMap<String, String>,
    /// `mathjax_inline`: an opening and a closing delimiter.
    pub inline: Vec<String>,
    /// `mathjax_display`: an opening and a closing delimiter.
    pub display: Vec<String>,
    /// `mathjax3_config`, as the JSON `json.dumps` writes for the table; empty: unset.
    pub v3_config: String,
    /// `mathjax4_config`, as `v3_config`.
    pub v4_config: String,
    /// `mathjax_config_path`: a .js file relative to the source folder; empty: none.
    pub config_path: String,
}

/// `sphinx.ext.intersphinx`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntersphinxConfig {
    /// `intersphinx_mapping`, sorted by name.
    pub mapping: Vec<IntersphinxProject>,
    /// `intersphinx_cache_limit`: days an inventory is reused; below 0: always.
    pub cache_limit: i64,
    /// `intersphinx_timeout`: seconds; 0: no limit.
    pub timeout: i64,
    /// `intersphinx_disabled_reftypes`.
    pub disabled_reftypes: Vec<String>,
    /// `intersphinx_resolve_self`.
    pub resolve_self: String,
}

/// `sphinx.ext.autodoc`, which reads Python source without running it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutodocConfig {
    /// `autodoc_source_paths`: folders holding the packages, like `sys.path`.
    pub source_paths: Vec<String>,
    /// `autoclass_content`: `class`, `init` or `both`.
    pub autoclass_content: String,
    /// `autodoc_class_signature`: `mixed` or `separated`.
    pub class_signature: String,
    /// `autodoc_default_options`.
    pub default_options: Vec<AutodocDefaultOption>,
    /// `autodoc_docstring_signature`.
    pub docstring_signature: bool,
    /// `autodoc_inherit_docstrings`.
    pub inherit_docstrings: bool,
    /// `autodoc_member_order`: `alphabetical`, `bysource` or `groupwise`.
    pub member_order: String,
    /// `autodoc_mock_imports`: accepted; nothing is imported, so nothing is mocked.
    pub mock_imports: Vec<String>,
    /// `autodoc_preserve_defaults`.
    pub preserve_defaults: bool,
    /// `autodoc_typehints`.
    pub typehints: String,
    /// `autodoc_typehints_description_target`.
    pub typehints_description_target: String,
    /// `autodoc_typehints_format`.
    pub typehints_format: String,
    /// `autodoc_type_aliases`.
    pub type_aliases: BTreeMap<String, String>,
    /// `autodoc_use_type_comments`.
    pub use_type_comments: bool,
    /// `autodoc_warningiserror`.
    pub warningiserror: bool,
    /// `python_display_short_literal_types`.
    pub python_display_short_literal_types: bool,
    /// `strip_signature_backslash`.
    pub strip_signature_backslash: bool,
}

/// The odin domain's generating directives and API pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OdinApiConfig {
    /// `odin_autoapi_dirs`: folders whose packages get API pages.
    pub dirs: Vec<String>,
    /// `odin_collections`: collection name to folder (`core`).
    pub collections: BTreeMap<String, String>,
    /// `odin_autoapi_root`: the folder of the generated pages (`api`).
    pub root: String,
    /// `odin_autoapi_options`: `members`, `undoc-members`, `private-members`.
    pub options: Vec<String>,
    /// `odin_autoapi_member_order`: `source`, `alphabetical` or `groupwise`.
    pub member_order: String,
    /// `odin_autoapi_add_toctree_entry`: the API index joins the root's toctree.
    pub add_toctree_entry: bool,
    /// `odin_autoapi_generate_api_docs`: false: only the directives read the folders.
    pub generate_api_docs: bool,
}

/// The rust domain's API pages (gd-rustdoc).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustApiConfig {
    /// `rust_autoapi_crates`: crate folders (or root `.rs` files) that get API pages,
    /// relative to the source folder.
    pub crates: Vec<String>,
    /// `rust_autoapi_root`: the folder of the generated pages (`rustapi`).
    pub root: String,
    /// `rust_autoapi_options`: `private-members`, `hidden-members`, `noindex`.
    pub options: Vec<String>,
    /// `rust_autoapi_member_order`: `source` or `alphabetical`.
    pub member_order: String,
    /// `rust_autoapi_add_toctree_entry`: the API index joins the root's toctree.
    pub add_toctree_entry: bool,
    /// `rust_autoapi_generate_api_docs`: false: no pages are generated.
    pub generate_api_docs: bool,
}

/// Native build plugins (GDS 0010).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginConfig {
    /// `plugins`: the build-plugin IDs this project selects, in order.
    pub ids: Vec<String>,
    /// `plugin_configs`: plugin ID to its configuration file, relative to conf.toml.
    pub configs: BTreeMap<String, String>,
    /// `plugin_timeout`: seconds per installed-plugin exchange (1 to 86400).
    pub timeout: i64,
}

/// The HTML builders' settings.
#[derive(Debug, Clone, PartialEq)]
pub struct HtmlConfig {
    /// `html_theme`: one of [`crate::KNOWN_THEMES`]; all build with the guidedog theme.
    pub theme: String,
    /// `html_theme_options`.
    pub theme_options: BTreeMap<String, String>,
    /// `html_title`; derived from the project when unset (`title_derived`).
    pub title: String,
    /// `html_short_title`; `title` when unset (`short_title_derived`).
    pub short_title: String,
    /// `html_logo`: a file relative to the source folder, or a URL; empty: none.
    pub logo: String,
    /// `html_favicon`: as `logo`.
    pub favicon: String,
    /// `html_static_path`.
    pub static_path: Vec<String>,
    /// `html_extra_path`.
    pub extra_path: Vec<String>,
    /// `html_css_files`.
    pub css_files: Vec<String>,
    /// `html_js_files`.
    pub js_files: Vec<String>,
    /// `html_last_updated_fmt`: `None`: no date; `Some("")`: the default format.
    pub last_updated_fmt: Option<String>,
    /// `html_permalinks`.
    pub permalinks: bool,
    /// `html_permalinks_icon`.
    pub permalinks_icon: String,
    /// `html_use_index`.
    pub use_index: bool,
    /// `html_domain_indices`: the Python module index, py-modindex.html.
    pub domain_indices: bool,
    /// `modindex_common_prefix`: prefixes the module index leaves out.
    pub modindex_common_prefix: Vec<String>,
    /// `html_split_index`.
    pub split_index: bool,
    /// `html_copy_source`.
    pub copy_source: bool,
    /// `html_show_sourcelink`.
    pub show_sourcelink: bool,
    /// `html_sourcelink_suffix`.
    pub sourcelink_suffix: String,
    /// `html_show_copyright`.
    pub show_copyright: bool,
    /// `html_show_sphinx`.
    pub show_sphinx: bool,
    /// `html_baseurl`.
    pub baseurl: String,
    /// `html_file_suffix`.
    pub file_suffix: String,
    /// `html_link_suffix`; empty: `file_suffix`.
    pub link_suffix: String,
    /// `html_secnumber_suffix`.
    pub secnumber_suffix: String,
    /// `html_context`: the pages' variables, a table as written.
    pub context: gd_toml::Table,
    /// `html_additional_pages`: page name to the template it is made from.
    pub additional_pages: BTreeMap<String, String>,
    /// `html_title` was made from the project's name.
    pub title_derived: bool,
    /// `html_short_title` was made from `html_title`.
    pub short_title_derived: bool,
}

/// The PDF builder's settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfConfig {
    /// `pdf_documents`: the books; one from the root document when unset.
    pub documents: Vec<PdfDocument>,
    /// `pdf_paper_size`.
    pub paper_size: String,
    /// `pdf_logo`.
    pub logo: String,
    /// `pdf_toplevel_sectioning`.
    pub toplevel_sectioning: String,
    /// `pdf_show_urls`.
    pub show_urls: String,
    /// `pdf_preamble`: a Typst file the project wrote.
    pub preamble: String,
    /// `pdf_packages`: `download` or `offline`.
    pub packages: String,
    /// `pdf_fonts`: `system` or `embedded`.
    pub fonts: String,
    /// `pdf_font_paths`.
    pub font_paths: Vec<String>,
    /// `typst`: the Typst program, or `typst` for the embedded compiler.
    pub typst: String,
}

/// Graphviz, linked in through graphdog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphvizConfig {
    /// `graphviz_output_format`: `svg` (`png` is read as `svg`).
    pub output_format: String,
    /// `graphviz_dot_args`: `-Gname=value`, `-Nname=value`, `-Ename=value`.
    pub dot_args: Vec<String>,
    /// `graphviz_dot`: accepted from conf.py; Graphviz is built in.
    pub dot: String,
}

/// Markdown, with myst-parser 0.16.1's names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MystConfig {
    /// `myst_enable_extensions`: optional syntax, such as `colon_fence`.
    pub enable_extensions: Vec<String>,
    /// `myst_heading_anchors`: heading depth that gets slug anchors; 0: none.
    pub heading_anchors: i64,
    /// `myst_dmath_allow_labels`.
    pub dmath_allow_labels: bool,
    /// `myst_dmath_allow_space`.
    pub dmath_allow_space: bool,
    /// `myst_dmath_allow_digits`.
    pub dmath_allow_digits: bool,
    /// `myst_dmath_double_inline`.
    pub dmath_double_inline: bool,
    /// `myst_linkify_fuzzy_links`.
    pub linkify_fuzzy_links: bool,
    /// `myst_substitutions`.
    pub substitutions: BTreeMap<String, String>,
    /// `myst_sub_delimiters`: two characters, each doubled: `{{ name }}`.
    pub sub_delimiters: Vec<String>,
    /// `myst_url_schemes`: schemes that make external links; empty: all.
    pub url_schemes: Vec<String>,
    /// `myst_footnote_transition`.
    pub footnote_transition: bool,
    /// `myst_html_meta`.
    pub html_meta: BTreeMap<String, String>,
    /// `myst_commonmark_only`: CommonMark syntax only.
    pub commonmark_only: bool,
    /// `myst_disable_syntax`: markdown-it rule names not parsed.
    pub disable_syntax: Vec<String>,
    /// `myst_words_per_minute`.
    pub words_per_minute: i64,
    /// `myst_dmath_enable`: deprecated spelling of the dollarmath extension.
    pub dmath_enable: bool,
    /// `myst_amsmath_enable`: deprecated spelling of the amsmath extension.
    pub amsmath_enable: bool,
}

/// Internationalization, with Sphinx's names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntlConfig {
    /// `locale_dirs`: relative to the source folder.
    pub locale_dirs: Vec<String>,
    /// `gettext_compact`: true, false, or one catalog's name.
    pub gettext_compact: FlagOrName,
    /// `gettext_uuid`.
    pub gettext_uuid: bool,
    /// `gettext_location`.
    pub gettext_location: bool,
    /// `gettext_auto_build`: compile .po files to .mo beside them.
    pub gettext_auto_build: bool,
    /// `gettext_additional_targets`.
    pub gettext_additional_targets: Vec<String>,
    /// `gettext_exclude_patterns`: document paths excluded from extraction only.
    pub gettext_exclude_patterns: Vec<String>,
    /// `gettext_allow_fuzzy_translations`.
    pub gettext_allow_fuzzy_translations: bool,
    /// `gettext_last_translator`.
    pub gettext_last_translator: String,
    /// `gettext_language_team`.
    pub gettext_language_team: String,
    /// `figure_language_filename`.
    pub figure_language_filename: String,
    /// `translation_progress_classes`: true, false, `translated` or `untranslated`.
    pub translation_progress_classes: FlagOrName,
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

impl Config {
    /// The configuration before conf.toml: Sphinx's defaults, for a project whose
    /// sources are in `source_dir` (also conf.toml's folder) and whose outputs go to
    /// `build_dir`.
    pub fn new(source_dir: impl Into<PathBuf>, build_dir: impl Into<PathBuf>) -> Config {
        let source_dir = source_dir.into();
        let numfig_format = [
            ("figure", "Fig. %s"),
            ("table", "Table %s"),
            ("code-block", "Listing %s"),
            ("section", "Section %s"),
        ];
        Config {
            config_dir: source_dir.clone(),
            source_dir,
            build_dir: build_dir.into(),
            project: "Python".into(),
            language: "en".into(),
            root_doc: "index".into(),
            source_suffix: strings(&[".rst"]),
            include_patterns: strings(&["**"]),
            primary_domain: "py".into(),
            highlight_language: "default".into(),
            numfig_format: numfig_format
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            numfig_secnum_depth: 1,
            math_eqref_format: "({number})".into(),
            math_numfig: true,
            add_function_parentheses: true,
            add_module_names: true,
            toc_object_entries: true,
            toc_object_entries_show_parents: "domain".into(),
            smartquotes: true,
            trim_doctest_flags: true,
            ..Config::empty()
        }
    }

    /// Every field empty, false or zero, but the groups at their defaults.
    fn empty() -> Config {
        Config {
            source_dir: PathBuf::new(),
            site: crate::SiteConfig::default(),
            config_dir: PathBuf::new(),
            build_dir: PathBuf::new(),
            project: String::new(),
            author: String::new(),
            copyright: String::new(),
            version: String::new(),
            release: String::new(),
            today: String::new(),
            today_fmt: String::new(),
            language: String::new(),
            root_doc: String::new(),
            source_suffix: Vec::new(),
            exclude_patterns: Vec::new(),
            include_roots: Vec::new(),
            include_patterns: Vec::new(),
            templates_path: Vec::new(),
            extensions: Vec::new(),
            rst_prolog: String::new(),
            rst_epilog: String::new(),
            default_role: String::new(),
            primary_domain: String::new(),
            highlight_language: String::new(),
            pygments_style: String::new(),
            pygments_dark_style: String::new(),
            numfig: false,
            numfig_format: BTreeMap::new(),
            numfig_secnum_depth: 0,
            math_number_all: false,
            math_eqref_format: String::new(),
            math_numfig: false,
            nitpicky: false,
            nitpick_ignore: Vec::new(),
            nitpick_ignore_regex: Vec::new(),
            show_authors: false,
            add_function_parentheses: false,
            add_module_names: false,
            toc_object_entries: false,
            toc_object_entries_show_parents: String::new(),
            trim_footnote_reference_space: false,
            smartquotes: false,
            keep_warnings: false,
            suppress_warnings: Vec::new(),
            todo_include_todos: false,
            autosectionlabel_prefix_document: false,
            autosectionlabel_maxdepth: 0,
            trim_doctest_flags: false,
            manpages_url: String::new(),
            option_emphasise_placeholders: false,
            extlinks: BTreeMap::new(),
            extlinks_detect_hardcoded_links: false,
            object_types: Vec::new(),
            directive_aliases: Vec::new(),
            mathjax: MathjaxConfig::default(),
            intersphinx: IntersphinxConfig::default(),
            autodoc: AutodocConfig::default(),
            odin_api: OdinApiConfig::default(),
            rust_api: RustApiConfig::default(),
            plugins: PluginConfig::default(),
            html: HtmlConfig::default(),
            pdf: PdfConfig::default(),
            graphviz: GraphvizConfig::default(),
            myst: MystConfig::default(),
            intl: IntlConfig::default(),
            ext: ExtensionSettings::default(),
        }
    }

    /// Whether `extensions` turns on the extension `name`, such as
    /// `"sphinx.ext.autodoc"`.
    pub fn has_extension(&self, name: &str) -> bool {
        self.extensions.iter().any(|e| e == name)
    }

    /// The declared object type the role `role` (with or without `std:`) refers to.
    pub fn declared_object_type(&self, role: &str) -> Option<&ObjectTypeDecl> {
        let name = role.strip_prefix("std:").unwrap_or(role);
        self.object_types.iter().find(|t| !t.role.is_empty() && t.role == name)
    }

    /// conf.toml's path: in `config_dir`.
    pub fn config_path(&self) -> PathBuf {
        self.config_dir.join(CONFIG_FILE)
    }
}

impl Default for Config {
    /// [`Config::new`] with empty folders.
    fn default() -> Config {
        Config::new("", "")
    }
}

impl Default for MathjaxConfig {
    fn default() -> MathjaxConfig {
        MathjaxConfig {
            path: MATHJAX_URL.into(),
            options: BTreeMap::new(),
            inline: strings(&["\\(", "\\)"]),
            display: strings(&["\\[", "\\]"]),
            v3_config: String::new(),
            v4_config: String::new(),
            config_path: String::new(),
        }
    }
}

impl Default for IntersphinxConfig {
    fn default() -> IntersphinxConfig {
        IntersphinxConfig {
            mapping: Vec::new(),
            cache_limit: 5,
            timeout: 0,
            disabled_reftypes: strings(&["std:doc"]),
            resolve_self: String::new(),
        }
    }
}

impl Default for AutodocConfig {
    fn default() -> AutodocConfig {
        AutodocConfig {
            source_paths: Vec::new(),
            autoclass_content: "class".into(),
            class_signature: "mixed".into(),
            default_options: Vec::new(),
            docstring_signature: true,
            inherit_docstrings: true,
            member_order: "alphabetical".into(),
            mock_imports: Vec::new(),
            preserve_defaults: false,
            typehints: "signature".into(),
            typehints_description_target: "all".into(),
            typehints_format: "short".into(),
            type_aliases: BTreeMap::new(),
            use_type_comments: true,
            warningiserror: true,
            python_display_short_literal_types: false,
            strip_signature_backslash: false,
        }
    }
}

impl Default for OdinApiConfig {
    fn default() -> OdinApiConfig {
        OdinApiConfig {
            dirs: Vec::new(),
            collections: BTreeMap::new(),
            root: "api".into(),
            options: strings(&["members", "undoc-members"]),
            member_order: "source".into(),
            add_toctree_entry: true,
            generate_api_docs: true,
        }
    }
}

impl Default for RustApiConfig {
    fn default() -> RustApiConfig {
        RustApiConfig {
            crates: Vec::new(),
            root: "rustapi".into(),
            options: Vec::new(),
            member_order: "source".into(),
            add_toctree_entry: true,
            generate_api_docs: true,
        }
    }
}

impl Default for PluginConfig {
    fn default() -> PluginConfig {
        PluginConfig { ids: Vec::new(), configs: BTreeMap::new(), timeout: 60 }
    }
}

impl Default for HtmlConfig {
    fn default() -> HtmlConfig {
        HtmlConfig {
            theme: "guidedog".into(),
            theme_options: BTreeMap::new(),
            title: String::new(),
            short_title: String::new(),
            logo: String::new(),
            favicon: String::new(),
            static_path: Vec::new(),
            extra_path: Vec::new(),
            css_files: Vec::new(),
            js_files: Vec::new(),
            last_updated_fmt: None,
            permalinks: true,
            permalinks_icon: "\u{b6}".into(),
            use_index: true,
            domain_indices: true,
            modindex_common_prefix: Vec::new(),
            split_index: false,
            copy_source: true,
            show_sourcelink: true,
            sourcelink_suffix: ".txt".into(),
            show_copyright: true,
            show_sphinx: true,
            baseurl: String::new(),
            file_suffix: ".html".into(),
            link_suffix: String::new(),
            secnumber_suffix: ". ".into(),
            context: gd_toml::Table::new(),
            additional_pages: BTreeMap::new(),
            title_derived: false,
            short_title_derived: false,
        }
    }
}

impl Default for PdfConfig {
    fn default() -> PdfConfig {
        PdfConfig {
            documents: Vec::new(),
            paper_size: "a4".into(),
            logo: String::new(),
            toplevel_sectioning: "chapter".into(),
            show_urls: "no".into(),
            preamble: String::new(),
            packages: "download".into(),
            fonts: "system".into(),
            font_paths: Vec::new(),
            typst: "typst".into(),
        }
    }
}

impl Default for GraphvizConfig {
    fn default() -> GraphvizConfig {
        GraphvizConfig { output_format: "svg".into(), dot_args: Vec::new(), dot: String::new() }
    }
}

impl Default for MystConfig {
    fn default() -> MystConfig {
        MystConfig {
            enable_extensions: strings(&["dollarmath"]),
            heading_anchors: 0,
            dmath_allow_labels: true,
            dmath_allow_space: true,
            dmath_allow_digits: true,
            dmath_double_inline: false,
            linkify_fuzzy_links: true,
            substitutions: BTreeMap::new(),
            sub_delimiters: strings(&["{", "}"]),
            url_schemes: strings(&["http", "https", "mailto", "ftp"]),
            footnote_transition: true,
            html_meta: BTreeMap::new(),
            commonmark_only: false,
            disable_syntax: Vec::new(),
            words_per_minute: 200,
            dmath_enable: false,
            amsmath_enable: false,
        }
    }
}

impl Default for IntlConfig {
    fn default() -> IntlConfig {
        IntlConfig {
            locale_dirs: strings(&["locales"]),
            gettext_compact: FlagOrName::Flag(true),
            gettext_uuid: false,
            gettext_location: true,
            gettext_auto_build: true,
            gettext_additional_targets: Vec::new(),
            gettext_exclude_patterns: Vec::new(),
            gettext_allow_fuzzy_translations: false,
            gettext_last_translator: "FULL NAME <EMAIL@ADDRESS>".into(),
            gettext_language_team: "LANGUAGE <LL@li.org>".into(),
            figure_language_filename: "{root}.{language}{ext}".into(),
            translation_progress_classes: FlagOrName::Flag(false),
        }
    }
}
