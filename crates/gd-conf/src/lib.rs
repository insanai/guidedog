//! gd-conf: conf.toml, a Guidedog project's configuration. The model (`Config`, grouped
//! by area, with Sphinx's names and defaults), loading (`load_config`, `find_project`),
//! `-D` overrides (`apply_override`), the checks Sphinx makes when it starts
//! (`check_config`), the settings table gd-migrate converts conf.py with (`SETTINGS`,
//! `find_setting`, `suggestion`), and trust (`readable_roots`, `confine_untrusted`).
//!
//! ```no_run
//! use std::path::Path;
//! let loaded = gd_conf::load_config(Path::new("docs"), &gd_conf::LoadOptions {
//!     overrides: vec!["numfig=true".into()],
//!     ..Default::default()
//! });
//! match loaded {
//!     Ok(l) => println!("{} in {}", l.config.project, l.config.source_dir.display()),
//!     Err(p) => eprintln!("{p}"),
//! }
//! ```
//!
//! Failures are `gd_host::Problem`s (`config.invalid`, `cli.define`, `project.missing`,
//! exit 2) with a hint; warnings are `gd_report::Report`s. Nothing here prints.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod check;
mod check_ext;
mod config;
mod custom;
mod custom_ext;
mod digest;
mod extensions;
mod json;
mod linkcode;
mod load;
mod paths;
mod pattern;
mod settings;
mod store;
mod trust;

pub use check::{
    FIGURE_FIELDS, GETTEXT_TARGETS, autodoc_roots, check_config, check_figure_format,
};
pub use config::{
    AutodocConfig, AutodocDefaultOption, CONFIG_FILE, Config, FlagOrName, GraphvizConfig,
    HtmlConfig, IntersphinxConfig, IntersphinxProject, IntlConfig, MATHJAX_URL, MAX_OBJECT_TYPES,
    MathjaxConfig, MystConfig, ObjectTypeDecl, OdinApiConfig, PdfConfig, PdfDocument,
    PluginConfig, RustApiConfig,
};
pub use custom_ext::APIDOC_MODULE_KEYS;
pub use digest::{Digest, Digester};
pub use extensions::{
    ApidocConfig, ApidocModule, AutosummaryConfig, CoverageConfig, DurationConfig,
    ExtensionSettings, FlagOrList, ImageConverterConfig, InheritanceConfig, LINKCODE_FIELDS,
    LinkcodeConfig, NapoleonConfig, NapoleonSection, ViewcodeConfig,
};
pub use json::{json_dumps, json_dumps_table};
pub use linkcode::{LinkcodeObject, LinkcodeSite};
pub use load::{
    LoadOptions, Loaded, MAX_CONFIG_BYTES, apply_override, find_project, finish_config,
    load_config, merge_table,
};
pub use paths::{absolute, clean_path, readable_roots, source_path};
pub use pattern::{Pattern, PatternError};
pub use settings::{
    FOREIGN_PREFIXES, KNOWN_THEMES, MYST_EXTENSIONS, NATIVE_EXTENSIONS, SETTINGS, Setting,
    SettingKind, find_setting, is_foreign_setting, suggestion,
};
pub use store::apply_table;
pub use trust::{confine_untrusted, rust_crate_roots, unread_warning};

#[cfg(test)]
mod tests;
