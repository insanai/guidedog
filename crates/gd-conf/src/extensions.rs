//! The settings of Sphinx's built-in extensions that read Python source or report on a
//! build: napoleon, autosummary, apidoc, viewcode, linkcode, inheritance_diagram,
//! coverage, duration and imgconverter (GDS 0011). Names and defaults are Sphinx 9.1's.

use std::collections::BTreeMap;

/// The settings of the extensions of GDS 0011, one group per extension; conf.toml keys
/// stay flat (`napoleon_use_ivar` is `config.ext.napoleon.use_ivar`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExtensionSettings {
    /// The `napoleon_*` settings.
    pub napoleon: NapoleonConfig,
    /// The `autosummary_*` settings.
    pub autosummary: AutosummaryConfig,
    /// The `apidoc_*` settings.
    pub apidoc: ApidocConfig,
    /// The `viewcode_*` settings.
    pub viewcode: ViewcodeConfig,
    /// The `linkcode_*` settings.
    pub linkcode: LinkcodeConfig,
    /// The `inheritance_*` settings.
    pub inheritance: InheritanceConfig,
    /// The `coverage_*` settings.
    pub coverage: CoverageConfig,
    /// The `duration_*` settings.
    pub duration: DurationConfig,
    /// `image_converter` and `image_converter_args`.
    pub image_converter: ImageConverterConfig,
}

/// A setting that is true, false, or a list of names, as `autosummary_generate` is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlagOrList {
    /// `true` or `false`.
    Flag(bool),
    /// A list of names, which counts as on for those names only.
    List(Vec<String>),
}

impl FlagOrList {
    /// Whether it is on for `name`: a true flag is on for every name, a list for the
    /// names it holds.
    pub fn is_on_for(&self, name: &str) -> bool {
        match self {
            FlagOrList::Flag(on) => *on,
            FlagOrList::List(names) => names.iter().any(|n| n == name),
        }
    }
}

/// One `napoleon_custom_sections` entry: a section name, and the section it behaves
/// like.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NapoleonSection {
    /// The header, as written in docstrings, such as `Side Effects`.
    pub name: String,
    /// Another section's name, `params_style` or `returns_style`; empty: a generic
    /// section, shown as a rubric and its text.
    pub like: String,
}

/// `sphinx.ext.napoleon`: Google and NumPy docstring sections, read as autodoc fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NapoleonConfig {
    /// `napoleon_google_docstring`.
    pub google_docstring: bool,
    /// `napoleon_numpy_docstring`.
    pub numpy_docstring: bool,
    /// `napoleon_include_init_with_doc`: document `__init__` when it has a docstring.
    pub include_init_with_doc: bool,
    /// `napoleon_include_private_with_doc`.
    pub include_private_with_doc: bool,
    /// `napoleon_include_special_with_doc`.
    pub include_special_with_doc: bool,
    /// `napoleon_use_admonition_for_examples`.
    pub use_admonition_for_examples: bool,
    /// `napoleon_use_admonition_for_notes`.
    pub use_admonition_for_notes: bool,
    /// `napoleon_use_admonition_for_references`.
    pub use_admonition_for_references: bool,
    /// `napoleon_use_ivar`: attributes as `:ivar:` fields.
    pub use_ivar: bool,
    /// `napoleon_use_param`: parameters as `:param:` fields.
    pub use_param: bool,
    /// `napoleon_use_rtype`: the return type as an `:rtype:` field.
    pub use_rtype: bool,
    /// `napoleon_use_keyword`: keyword arguments as `:keyword:` fields.
    pub use_keyword: bool,
    /// `napoleon_preprocess_types`: link the names in type specifications.
    pub preprocess_types: bool,
    /// `napoleon_type_aliases`: type name to the name it links to.
    pub type_aliases: BTreeMap<String, String>,
    /// `napoleon_custom_sections`, in the order written.
    pub custom_sections: Vec<NapoleonSection>,
    /// `napoleon_attr_annotations`: annotations give documented attributes their type.
    pub attr_annotations: bool,
}

/// `sphinx.ext.autosummary`: summary tables and their generated stub pages.
#[derive(Debug, Clone, PartialEq)]
pub struct AutosummaryConfig {
    /// `autosummary_context`: more variables for the stub templates, as written.
    pub context: gd_toml::Table,
    /// `autosummary_filename_map`: object name to the stub page's name.
    pub filename_map: BTreeMap<String, String>,
    /// `autosummary_generate`: generate stub pages for every document's directives, for
    /// none, or for those of the listed source files.
    pub generate: FlagOrList,
    /// `autosummary_generate_overwrite`: accepted; stub pages are generated in memory and
    /// never written over a file.
    pub generate_overwrite: bool,
    /// `autosummary_mock_imports`: accepted; empty follows `autodoc_mock_imports`.
    pub mock_imports: Vec<String>,
    /// `autosummary_imported_members`: list members a module imports.
    pub imported_members: bool,
    /// `autosummary_ignore_module_all`: list every member, not only `__all__`.
    pub ignore_module_all: bool,
}

/// One `apidoc_modules` entry: a package and where its pages go, with the general
/// `apidoc_*` settings it overrides (`None`: the general setting).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ApidocModule {
    /// `path`: the package folder, relative to conf.toml's folder.
    pub path: String,
    /// `destination`: the folder of the generated pages, inside the source folder.
    pub destination: String,
    /// `exclude_patterns`.
    pub exclude_patterns: Option<Vec<String>>,
    /// `max_depth`.
    pub max_depth: Option<i64>,
    /// `follow_links`.
    pub follow_links: Option<bool>,
    /// `separate_modules`.
    pub separate_modules: Option<bool>,
    /// `include_private`.
    pub include_private: Option<bool>,
    /// `no_headings`.
    pub no_headings: Option<bool>,
    /// `module_first`.
    pub module_first: Option<bool>,
    /// `implicit_namespaces`.
    pub implicit_namespaces: Option<bool>,
    /// `automodule_options`.
    pub automodule_options: Option<Vec<String>>,
}

/// `sphinx.ext.apidoc`: API pages generated from package folders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApidocConfig {
    /// `apidoc_modules`, in the order written.
    pub modules: Vec<ApidocModule>,
    /// `apidoc_exclude_patterns`: paths or glob patterns left out.
    pub exclude_patterns: Vec<String>,
    /// `apidoc_max_depth`: the depth of the generated table of contents.
    pub max_depth: i64,
    /// `apidoc_follow_links`: follow symbolic links (inside the readable folders).
    pub follow_links: bool,
    /// `apidoc_separate_modules`: a page per module.
    pub separate_modules: bool,
    /// `apidoc_include_private`: modules whose names start with `_` too.
    pub include_private: bool,
    /// `apidoc_no_headings`.
    pub no_headings: bool,
    /// `apidoc_module_first`: a package's own documentation before its submodules.
    pub module_first: bool,
    /// `apidoc_implicit_namespaces`: folders without `__init__.py` are packages (PEP 420).
    pub implicit_namespaces: bool,
    /// `apidoc_automodule_options`: the options of the generated `automodule` directives.
    pub automodule_options: Vec<String>,
}

/// `sphinx.ext.viewcode`: highlighted source pages, linked both ways.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewcodeConfig {
    /// `viewcode_follow_imported_members` (or `viewcode_import`): link a re-exported
    /// object to the module that defines it.
    pub follow_imported_members: bool,
    /// `viewcode_enable_epub`: accepted; Guidedog has no EPUB builder.
    pub enable_epub: bool,
    /// `viewcode_line_numbers`: line numbers in the source pages.
    pub line_numbers: bool,
}

/// `sphinx.ext.linkcode`: links from object descriptions to source code on the web.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinkcodeConfig {
    /// `linkcode_url`: the URL template, with the placeholders of [`LINKCODE_FIELDS`];
    /// empty: no links.
    pub url: String,
    /// `linkcode_branch`: the text of `{branch}`; empty: read from the repository.
    pub branch: String,
    /// `linkcode_root`: the folder `{path}` is relative to, relative to the source
    /// folder; empty: the nearest folder above the source folder holding `.git`.
    pub root: String,
}

/// The placeholders `linkcode_url` may use.
pub const LINKCODE_FIELDS: &[&str] =
    &["domain", "module", "fullname", "path", "line", "end_line", "branch"];

/// `sphinx.ext.inheritance_diagram`: class graphs drawn with Graphviz.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InheritanceConfig {
    /// `inheritance_graph_attrs`: Graphviz graph attributes.
    pub graph_attrs: BTreeMap<String, String>,
    /// `inheritance_node_attrs`: Graphviz node attributes.
    pub node_attrs: BTreeMap<String, String>,
    /// `inheritance_edge_attrs`: Graphviz edge attributes.
    pub edge_attrs: BTreeMap<String, String>,
    /// `inheritance_alias`: full class name to the name shown.
    pub alias: BTreeMap<String, String>,
}

/// `sphinx.ext.coverage`: the `coverage` builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageConfig {
    /// `coverage_modules`: the packages checked, with their submodules; empty: the
    /// modules the documents describe.
    pub modules: Vec<String>,
    /// `coverage_ignore_modules`: patterns, matched at the start of module names.
    pub ignore_modules: Vec<String>,
    /// `coverage_ignore_functions`: patterns, matched at the start of function names.
    pub ignore_functions: Vec<String>,
    /// `coverage_ignore_classes`: patterns, matched at the start of class names.
    pub ignore_classes: Vec<String>,
    /// `coverage_ignore_pyobjects`: patterns, matched anywhere in full names.
    pub ignore_pyobjects: Vec<String>,
    /// `coverage_c_path`: C header files or glob patterns, relative to the source folder.
    pub c_path: Vec<String>,
    /// `coverage_c_regexes`: C object type to the pattern that finds its names.
    pub c_regexes: BTreeMap<String, String>,
    /// `coverage_ignore_c_items`: C object type to patterns of names left out.
    pub ignore_c_items: BTreeMap<String, Vec<String>>,
    /// `coverage_write_headline`.
    pub write_headline: bool,
    /// `coverage_statistics_to_report`: the statistics table in python.txt.
    pub statistics_to_report: bool,
    /// `coverage_statistics_to_stdout`: the statistics table on the terminal.
    pub statistics_to_stdout: bool,
    /// `coverage_skip_undoc_in_source`: leave out objects without a docstring.
    pub skip_undoc_in_source: bool,
    /// `coverage_show_missing_items`: each missing object as a note too.
    pub show_missing_items: bool,
}

/// `sphinx.ext.duration`: the slowest documents to read.
#[derive(Debug, Clone, PartialEq)]
pub struct DurationConfig {
    /// `duration_print_total`: the total reading time in the summary.
    pub print_total: bool,
    /// `duration_print_slowest`: the slowest documents in the summary.
    pub print_slowest: bool,
    /// `duration_n_slowest`: how many slowest documents; 0: all.
    pub n_slowest: i64,
    /// `duration_write_json`: the JSON file in the output folder; empty: none.
    pub write_json: String,
    /// `duration_limit`: seconds a document may take to read before a warning;
    /// `None`: no limit.
    pub limit: Option<f64>,
}

/// `sphinx.ext.imgconverter`: accepted; Typst reads the images itself, so nothing is
/// converted and no program is started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageConverterConfig {
    /// `image_converter`: accepted, unused.
    pub converter: String,
    /// `image_converter_args`: accepted, unused.
    pub converter_args: Vec<String>,
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

impl Default for NapoleonConfig {
    fn default() -> NapoleonConfig {
        NapoleonConfig {
            google_docstring: true,
            numpy_docstring: true,
            include_init_with_doc: false,
            include_private_with_doc: false,
            include_special_with_doc: false,
            use_admonition_for_examples: false,
            use_admonition_for_notes: false,
            use_admonition_for_references: false,
            use_ivar: false,
            use_param: true,
            use_rtype: true,
            use_keyword: true,
            preprocess_types: false,
            type_aliases: BTreeMap::new(),
            custom_sections: Vec::new(),
            attr_annotations: true,
        }
    }
}

impl Default for AutosummaryConfig {
    fn default() -> AutosummaryConfig {
        AutosummaryConfig {
            context: gd_toml::Table::new(),
            filename_map: BTreeMap::new(),
            generate: FlagOrList::Flag(true),
            generate_overwrite: true,
            mock_imports: Vec::new(),
            imported_members: false,
            ignore_module_all: true,
        }
    }
}

impl Default for ApidocConfig {
    fn default() -> ApidocConfig {
        ApidocConfig {
            modules: Vec::new(),
            exclude_patterns: Vec::new(),
            max_depth: 4,
            follow_links: false,
            separate_modules: false,
            include_private: false,
            no_headings: false,
            module_first: false,
            implicit_namespaces: false,
            automodule_options: strings(&["members", "show-inheritance", "undoc-members"]),
        }
    }
}

impl Default for ViewcodeConfig {
    fn default() -> ViewcodeConfig {
        ViewcodeConfig { follow_imported_members: true, enable_epub: false, line_numbers: false }
    }
}

impl Default for CoverageConfig {
    fn default() -> CoverageConfig {
        CoverageConfig {
            modules: Vec::new(),
            ignore_modules: Vec::new(),
            ignore_functions: Vec::new(),
            ignore_classes: Vec::new(),
            ignore_pyobjects: Vec::new(),
            c_path: Vec::new(),
            c_regexes: BTreeMap::new(),
            ignore_c_items: BTreeMap::new(),
            write_headline: true,
            statistics_to_report: true,
            statistics_to_stdout: true,
            skip_undoc_in_source: false,
            show_missing_items: false,
        }
    }
}

impl Default for DurationConfig {
    fn default() -> DurationConfig {
        DurationConfig {
            print_total: true,
            print_slowest: true,
            n_slowest: 5,
            write_json: "sphinx-reading-durations.json".into(),
            limit: None,
        }
    }
}

impl Default for ImageConverterConfig {
    fn default() -> ImageConverterConfig {
        ImageConverterConfig { converter: "convert".into(), converter_args: Vec::new() }
    }
}
