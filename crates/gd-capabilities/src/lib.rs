//! What the linked Guidedog supports, read from the registries it is built from rather
//! than kept in tables of its own: readers and renderers (`guidedoc_defaults`), document
//! formats with their source maps and edit operations (`guidedoc_edit`), builders
//! (`gd_env`), directives and roles (the Sphinx layer's catalog), Markdown and Sphinx
//! extensions (`gd_conf`), and the plugins linked in (`gd_plugins`). Editors and tools
//! ask [`load_capabilities`] instead of guessing, and `guidedog capabilities --json`
//! writes the same report ([`write_json`]).
//!
//! ```
//! let report = gd_capabilities::load_capabilities(&gd_plugins::Registry::default(), None);
//! assert!(report.directives.iter().any(|d| d.name == "note"));
//! let mut json = String::new();
//! gd_capabilities::write_json(&report, &mut json);
//! assert!(json.starts_with("{\"schema\":1,"));
//! ```

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod catalog;
mod json;

pub use json::write_json;

/// The version of the report's structure and JSON form; it changes when a field changes
/// meaning or disappears. New fields may appear without a change.
pub const SCHEMA_VERSION: u32 = 1;

/// How a directive, role or extension becomes available to a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnabledBy {
    /// Always, in every project.
    Default,
    /// When the project's configuration enables it (`extension` names the setting).
    Config,
    /// When a plugin provides it. The plugin interface of this version adds builds,
    /// publishers and commands, not markup, so no entry has this value yet.
    Plugin,
}

impl EnabledBy {
    /// The stable name: `default`, `config` or `plugin`.
    pub fn name(self) -> &'static str {
        match self {
            EnabledBy::Default => "default",
            EnabledBy::Config => "config",
            EnabledBy::Plugin => "plugin",
        }
    }
}

/// A reader or renderer of the engine registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adapter {
    /// Its id, such as `sphinx` or `html`.
    pub id: &'static str,
    /// The name people see.
    pub name: &'static str,
    /// Its version.
    pub version: &'static str,
    /// `planned`, `partial` or `conformant`.
    pub conformance: &'static str,
}

/// A document format a project reads, with what editors may do with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatCapability {
    /// `rst` or `markdown` (and `typst` for native Typst, which `convert` compiles).
    pub name: &'static str,
    /// The reader id; empty for native Typst.
    pub reader: &'static str,
    /// The file suffixes, with their dots.
    pub suffixes: Vec<&'static str>,
    /// `guidedoc_edit::load_source_map` maps documents of this format.
    pub source_map: bool,
    /// The block kinds a map may hold.
    pub block_kinds: Vec<&'static str>,
    /// The visual operations `guidedoc_edit::apply_operation` supports.
    pub operations: Vec<&'static str>,
}

/// A builder and what its output offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuilderCapability {
    /// The builder's name, such as `html` or `pdf`.
    pub name: &'static str,
    /// The format its documents render in: `html`, `pdf` or `text`.
    pub format: &'static str,
    /// It writes output a person views (HTML pages or PDF books), so a preview build
    /// (drafts included) is useful.
    pub preview: bool,
    /// `guidedog serve` can rebuild and serve it live.
    pub serve: bool,
    /// Its output carries source positions: `data-gd-source` attributes on HTML blocks,
    /// or a PDF's page map.
    pub output_source_map: bool,
    /// Its output folder holds a dependency manifest (`gd-dependencies.json`).
    pub dependency_manifest: bool,
}

/// One option of a directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionCapability {
    /// Its name.
    pub name: &'static str,
    /// Its value type, such as `flag`, `text` or `choice`.
    pub kind: &'static str,
    /// The choices of a `choice` option.
    pub choices: Vec<&'static str>,
}

/// A directive a project can use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectiveCapability {
    /// Its canonical name, such as `note` or `py:function`.
    pub name: String,
    /// `docutils`, `sphinx`, `myst`, `project`, or `domain:<name>`.
    pub origin: String,
    /// How it becomes available.
    pub enabled_by: EnabledBy,
    /// The setting that enables it, such as `sphinx.ext.autodoc`; empty when none.
    pub extension: &'static str,
    /// Available with the configuration the report was made for.
    pub enabled: bool,
    /// The formats it is written in: `rst`, `markdown`.
    pub formats: Vec<&'static str>,
    /// Whether it takes an argument: `none`, `optional` or `required`.
    pub argument: &'static str,
    /// Whether it takes content: `none`, `optional` or `required`.
    pub content: &'static str,
    /// Its options.
    pub options: Vec<OptionCapability>,
}

/// An interpreted text role a project can use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleCapability {
    /// Its name, such as `ref` or `py:func`.
    pub name: String,
    /// `docutils`, `sphinx`, `project`, or `domain:<name>`.
    pub origin: String,
    /// How it becomes available.
    pub enabled_by: EnabledBy,
    /// The setting that enables it; empty when none.
    pub extension: &'static str,
    /// Available with the configuration the report was made for.
    pub enabled: bool,
}

/// An extension a project's configuration may enable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionCapability {
    /// Its name: `colon_fence` for Markdown, `sphinx.ext.autodoc` for Sphinx.
    pub name: &'static str,
    /// Enabled in a project without configuration.
    pub default: bool,
    /// Enabled with the configuration the report was made for.
    pub enabled: bool,
}

/// A plugin linked into the program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginCapability {
    /// Its id, such as `pages`.
    pub id: String,
    /// `build` or `publisher`.
    pub kind: &'static str,
    /// Its version.
    pub version: String,
    /// The commands it adds, such as `pages workflow`.
    pub commands: Vec<String>,
}

/// The whole report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    /// [`SCHEMA_VERSION`].
    pub schema: u32,
    /// The Guidedog version.
    pub guidedog: &'static str,
    /// The version of the source map structure ([`guidedoc_edit::SOURCE_MAP_VERSION`]).
    pub source_map_version: u32,
    /// The engine's readers.
    pub readers: Vec<Adapter>,
    /// The engine's renderers.
    pub renderers: Vec<Adapter>,
    /// The document formats.
    pub formats: Vec<FormatCapability>,
    /// The builders.
    pub builders: Vec<BuilderCapability>,
    /// The directives.
    pub directives: Vec<DirectiveCapability>,
    /// The roles.
    pub roles: Vec<RoleCapability>,
    /// The optional MyST syntax extensions.
    pub markdown_extensions: Vec<ExtensionCapability>,
    /// The Sphinx extensions Guidedog implements.
    pub sphinx_extensions: Vec<ExtensionCapability>,
    /// The plugins linked in.
    pub plugins: Vec<PluginCapability>,
}

/// The report for a program that links `plugins`, with `enabled` fields for the project
/// configuration `config` (`None`: a project without configuration). Never fails: every
/// fact comes from the linked code.
pub fn load_capabilities(
    plugins: &gd_plugins::Registry,
    config: Option<&gd_conf::Config>,
) -> Capabilities {
    let default_config = gd_conf::Config::default();
    let config = config.unwrap_or(&default_config);
    let registry = guidedoc_defaults::registry();
    let adapters =
        |list: Vec<(&'static str, &'static str, &'static str, guidedoc::Conformance)>| {
            list.into_iter()
                .map(|(id, name, version, c)| Adapter {
                    id,
                    name,
                    version,
                    conformance: conformance(c),
                })
                .collect()
        };
    let readers = registry.readers.iter().map(|r| (r.id, r.name, r.version, r.conformance));
    let renderers = registry.renderers.iter().map(|r| (r.id, r.name, r.version, r.conformance));
    Capabilities {
        schema: SCHEMA_VERSION,
        guidedog: guidedoc::VERSION,
        source_map_version: guidedoc_edit::SOURCE_MAP_VERSION,
        readers: adapters(readers.collect()),
        renderers: adapters(renderers.collect()),
        formats: formats(),
        builders: builders(),
        directives: catalog::directives(config),
        roles: catalog::roles(config),
        markdown_extensions: catalog::markdown_extensions(config, &default_config),
        sphinx_extensions: catalog::sphinx_extensions(config),
        plugins: plugin_capabilities(plugins),
    }
}

fn conformance(c: guidedoc::Conformance) -> &'static str {
    match c {
        guidedoc::Conformance::Planned => "planned",
        guidedoc::Conformance::Partial => "partial",
        guidedoc::Conformance::Conformant => "conformant",
    }
}

fn formats() -> Vec<FormatCapability> {
    let kinds: Vec<&'static str> =
        guidedoc_edit::BlockKind::ALL.iter().map(|k| k.name()).collect();
    let mut out: Vec<FormatCapability> = guidedoc_edit::Format::ALL
        .iter()
        .map(|&f| FormatCapability {
            name: f.name(),
            reader: f.reader(),
            suffixes: f.suffixes().to_vec(),
            source_map: true,
            block_kinds: kinds.clone(),
            operations: guidedoc_edit::operations(f).iter().map(|o| o.name()).collect(),
        })
        .collect();
    out.push(FormatCapability {
        name: "typst",
        reader: "",
        suffixes: vec![".typ"],
        source_map: false,
        block_kinds: Vec::new(),
        operations: Vec::new(),
    });
    out
}

fn builders() -> Vec<BuilderCapability> {
    gd_env::BuilderKind::ALL
        .iter()
        .map(|&b| {
            let format = match b.format() {
                gd_env::OutputFormat::Html => "html",
                gd_env::OutputFormat::Pdf => "pdf",
                gd_env::OutputFormat::Text => "text",
            };
            let viewed = format != "text";
            BuilderCapability {
                name: b.name(),
                format,
                preview: viewed,
                serve: b == gd_env::BuilderKind::Html,
                output_source_map: false,
                dependency_manifest: false,
            }
        })
        .collect()
}

fn plugin_capabilities(plugins: &gd_plugins::Registry) -> Vec<PluginCapability> {
    plugins
        .plugins()
        .iter()
        .map(|p| {
            let version = match (p.build_info(), p.publisher_info()) {
                (Some(info), _) => info.version.clone(),
                (None, Some(info)) => info.version.clone(),
                (None, None) => String::new(),
            };
            PluginCapability {
                id: p.id().to_string(),
                kind: if p.is_publisher() { "publisher" } else { "build" },
                version,
                commands: p.commands().iter().map(|c| c.name()).collect(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find<'a>(report: &'a Capabilities, name: &str) -> &'a DirectiveCapability {
        report.directives.iter().find(|d| d.name == name).unwrap()
    }

    #[test]
    fn the_report_follows_the_registries() {
        let report = load_capabilities(&gd_plugins::Registry::default(), None);
        assert!(report.readers.iter().any(|r| r.id == "sphinx"));
        assert!(report.renderers.iter().any(|r| r.id == "typst"));
        let rst = &report.formats[0];
        assert_eq!((rst.name, rst.reader), ("rst", "sphinx"));
        assert!(rst.operations.contains(&"move-block") && rst.block_kinds.contains(&"admonition"));
        assert!(report.builders.iter().any(|b| b.name == "pdf" && b.preview && !b.serve));
        let note = find(&report, "note");
        assert_eq!(
            (note.origin.as_str(), note.enabled_by, note.enabled),
            ("docutils", EnabledBy::Default, true)
        );
        let auto = find(&report, "automodule");
        assert_eq!(
            (auto.enabled_by, auto.extension, auto.enabled),
            (EnabledBy::Config, "sphinx.ext.autodoc", false)
        );
        assert_eq!(find(&report, "eval-rst").formats, vec!["markdown"]);
        let colon = report.markdown_extensions.iter().find(|e| e.name == "colon_fence").unwrap();
        assert!(!colon.default && !colon.enabled);
        let dollar = report.markdown_extensions.iter().find(|e| e.name == "dollarmath").unwrap();
        assert!(dollar.default && dollar.enabled);
    }

    #[test]
    fn the_configuration_decides_what_is_enabled() {
        let extensions = vec!["sphinx.ext.autodoc".into(), "sphinx.ext.intersphinx".into()];
        let mut config = gd_conf::Config { extensions, ..Default::default() };
        config.myst.enable_extensions.push("colon_fence".into());
        let report = load_capabilities(&gd_plugins::Registry::default(), Some(&config));
        assert!(find(&report, "automodule").enabled);
        let external = report.roles.iter().find(|r| r.name == "external").unwrap();
        assert!(external.enabled);
        let colon = report.markdown_extensions.iter().find(|e| e.name == "colon_fence").unwrap();
        assert!(colon.enabled && !colon.default);
        let autodoc =
            report.sphinx_extensions.iter().find(|e| e.name == "sphinx.ext.autodoc").unwrap();
        assert!(autodoc.enabled);
    }
}
