//! gd-env: the foundation of a Guidedog build. The vocabulary every builder crate shares
//! (`BuilderKind`, `BuildOptions`, `BuildResult`, `OutputFormat`, `Asset`, `Rendered`,
//! `UiText`), the project's documents (`discover`, `SourceFile`), the environment
//! reading keeps between builds (`Environment`, `DocInfo`, `load_environment`), the
//! object store of parsed trees (`ObjectStore`), lookups and traces for verifying
//! incremental builds (`Lookup`, `Trace`), the settings digests (`reading_settings`,
//! `output_settings`), and the build plugins one build runs (`PluginRun`).
//!
//! ```no_run
//! use gd_env::{discover, load_environment, reading_settings};
//! # fn main() -> Result<(), gd_host::Problem> {
//! let loaded = gd_conf::load_config(std::path::Path::new("docs"), &Default::default())?;
//! let sources = discover(&loaded.config)?;
//! let budget = gd_host::Budget::new(gd_env::BUILD_BUDGET);
//! let path = gd_env::environment_path(&loaded.config);
//! let previous = load_environment(&path, reading_settings(&loaded.config), false, &budget)?;
//! println!("{} sources, {} known", sources.len(), previous.docs.len());
//! # Ok(())
//! # }
//! ```
//!
//! Docs: docs/manual/internals/project.rst (the crate split) and incremental.rst (every
//! digest input). Failures are `gd_host::Problem`s; nothing here prints.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod builder;
mod discover;
mod environment;
mod excerpts;
mod lookup;
mod objects;
mod page;
mod page_fields;
mod plugins;
mod posts;
mod records;
mod rendered;
mod settings;

pub use builder::{
    BUILD_BUDGET, BuildOptions, BuildResult, BuildStage, BuilderKind, Jobs, OutputFormat,
};
pub use discover::{
    Generated, MAX_DOCUMENTS, SourceFile, SourceKind, contents_root, discover, glob_match,
    is_excluded, is_included, relative_to, with_generated,
};
pub use environment::{
    CachedFile, CppDeclInfo, Dependency, DocId, DocInfo, ENVIRONMENT_VERSION, Environment,
    IndexInfo, LabelInfo, LabelKind, ModuleInfo, NumberedInfo, ObjectInfo, RefInfo, SectionInfo,
    SourceLocation, TermInfo, TocEntry, TocObjectInfo, ToctreeInfo, TodoInfo,
    dependencies_changed, digest_name, doctrees_dir, environment_path, load_environment,
    normalize_name, option_key,
};
pub use excerpts::excerpt_root;
pub use gd_conf::{Digest, Digester};
pub use lookup::{Lookup, LookupKind, Trace};
pub use objects::{OBJECTS_FOLDER, ObjectError, ObjectStore, SEARCH_FOLDER};
pub use page::{PageMeta, publication_time};
pub use page_fields::{PageMetadataError, is_publication_field, page_metadata, publication_field};
pub use plugins::{PluginRun, PluginStart, assets_url, host_problem};
pub use posts::{PostlistError, PostlistSpec};
pub use records::{ObjectBuilder, ObjectReader, read_record, report_from_json, report_to_json};
pub use rendered::{Asset, EnglishUi, Rendered, UiText};
pub use settings::{
    build_settings, executable_stamp, file_digest, output_settings, reading_settings,
};

#[cfg(test)]
mod tests;
