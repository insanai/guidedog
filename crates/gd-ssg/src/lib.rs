//! Static publication authoring: source edits and linked `guidedog ssg` commands.
//! Builds remain the normal Guidedog pipeline; this crate owns no renderer.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod assemble;
mod command;
mod lifecycle;
mod metadata;
mod mounts;
mod presets;
mod theme;
mod transaction;

pub use command::plugin;
pub use lifecycle::{Publication, create_post, read_metadata, transition};
pub use mounts::{SiteBuildOptions, build_site};
pub use presets::{Preset, create_site};
pub use theme::install_theme;
