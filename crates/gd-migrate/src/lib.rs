//! gd-migrate: `guidedog migrate` and `guidedog quickstart`.
//!
//! [`migrate`] turns a Sphinx conf.py into conf.toml **without running Python**: literal
//! assignments (strings, numbers, lists, tuples, dictionaries, names assigned earlier,
//! `+`, `%`, f-strings, a few `str` methods, the current year) become settings, checked
//! against gd-conf's settings table; everything Python would have computed is reported
//! with the setting it affects and kept as a comment. A project's own extension modules
//! are read for the object types they declare. The result is loaded before it is
//! written, so a migrated conf.toml always builds.
//!
//! [`quickstart`] creates a new project: conf.toml, the root document, and the default
//! templates and styles (from the `gd-theme` crate) copied in to edit.
//!
//! ```no_run
//! let done = gd_migrate::migrate(&gd_migrate::MigrateOptions {
//!     input: "docs/conf.py".into(),
//!     output: gd_migrate::Destination::Nowhere,
//!     ..Default::default()
//! });
//! match done {
//!     Ok(m) => print!("{}", m.conversion.toml),
//!     Err(f) => eprintln!("{f}"),
//! }
//! ```
//!
//! Failures are `gd_host::Problem`s with a hint; notes are `gd_report::Report`s with
//! source excerpts. Nothing here prints.

#![forbid(unsafe_code)]

mod clock;
mod convert;
mod extensions;
mod legacy;
mod migrate;
mod pyconf;
mod quickstart;
mod read;

pub use convert::check_conf_toml;
pub use legacy::convert_guidedog_toml;
pub use migrate::{
    Conversion, Destination, MAX_INPUT_BYTES, MigrateFailure, MigrateOptions, Migrated,
    convert_conf_py, migrate,
};
pub use quickstart::{QuickstartOptions, quickstart};
