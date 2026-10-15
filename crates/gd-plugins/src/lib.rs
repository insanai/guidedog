//! Typed build plugins (GDS 0010) and publishers (GDS 0009), linked into an application
//! or run as installed programs behind a bounded JSON Lines protocol.
//!
//! A [`Plugin`] is either build hooks ([`BuildPlugin`]: source text, complete HTML
//! pages, generated static assets) or a [`Publisher`] (sends a finished site to a
//! destination), never both. A normal build never invokes a publisher.
//!
//! ```text
//! BuildPlugin    load_plugin  -> PluginSession     process_source / process_html /
//!                                                  collect_assets; Drop unloads
//! Publisher      load_publisher -> PublisherSession   plan / publish / status / recover
//! Snapshot       snapshot(dir)  a frozen, checked copy of a built site; Drop removes it
//! ```
//!
//! Plugin authors implement a trait and, for an installed program, serve it with one
//! call, [`serve`] (build hooks) or [`serve_publisher`]:
//!
//! ```no_run
//! use std::borrow::Cow;
//! use gd_plugins::{BuildPlugin, Hooks, PluginInfo, Problem, SourceInput};
//!
//! #[derive(Clone, Default)]
//! struct Shout;
//!
//! impl BuildPlugin for Shout {
//!     fn info(&self) -> PluginInfo {
//!         PluginInfo::new("shout", "Shout", "1.0.0", Hooks { source: true, ..Hooks::NONE })
//!     }
//!     fn process_source<'a>(&mut self, input: &SourceInput<'a>)
//!         -> Result<Cow<'a, str>, Problem> {
//!         Ok(Cow::Owned(input.text.to_uppercase()))
//!     }
//! }
//!
//! fn main() -> std::process::ExitCode {
//!     gd_plugins::serve(Shout)
//! }
//! ```
//!
//! Installed programs are recorded in the machine registry ([`read_machine`],
//! [`register`], [`unregister`]), pinned by SHA-256, and reached through
//! [`process_plugin`] and [`process_publisher`]. Nothing here discovers, downloads or
//! starts a program by itself. Failures are [`Problem`]s with hints, never panics.

#![forbid(unsafe_code)]
// In unit tests, a test failing on unexpected input is the point.
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod assets;
mod build;
mod build_session;
mod cancel;
mod check;
mod child;
pub mod files;
mod machine;
mod paths;
mod problem;
mod process_build;
mod process_publish;
mod publisher;
mod registry;
mod select;
mod serve_build;
mod serve_publish;
mod session;
mod snapshot;
#[cfg(test)]
mod testutil;
mod wire;

pub use build::{
    Asset, BuildPlugin, Hooks, HtmlInput, LoadContext, PluginInfo, PluginOptions, SourceFormat,
    SourceInput,
};
pub use build_session::{PluginSession, load_plugin};
pub use cancel::Cancel;
pub use check::check_plugin;
pub use machine::{
    CONFIG_VARIABLE, MACHINE_FILE, Machine, MachineEntry, config_dir, inspect_entry, machine_path,
    read_machine, register, unregister, verify_entry,
};
pub use problem::{EXIT_ACCEPTED, EXIT_UNKNOWN, Problem, ProblemData};
pub use process_build::process_plugin;
pub use process_publish::{
    DEFAULT_REQUEST_TIMEOUT, HANDSHAKE_TIMEOUT, inspect_program, process_publisher,
};
pub use publisher::{
    Capabilities, Change, ChangeKind, LoadOptions, Loaded, Outcome, Plan, Publisher,
    PublisherInfo, Source, State,
};
pub use registry::{Plugin, Registry};
pub use select::{MAX_BUILD_PLUGINS, select_build_plugin};
pub use serve_build::{serve, serve_plugin, serve_streams};
pub use serve_publish::{serve_publisher, serve_publisher_streams};
pub use session::{PublisherSession, load_publisher};
pub use snapshot::{Entry, Snapshot, SnapshotMode, SnapshotOptions, digest_hex, snapshot};
pub use wire::{Handshake, select_version};

/// The version of the publisher interface ([`Publisher`]).
pub const API_VERSION: Version = Version { major: 1, minor: 1 };
/// The version of the build-plugin interface ([`BuildPlugin`]).
pub const PLUGIN_API_VERSION: Version = Version { major: 1, minor: 0 };
/// The publisher process protocol's version.
pub const PROTOCOL_VERSION: Version = Version { major: 1, minor: 0 };
/// The build-plugin process protocol's version.
pub const BUILD_PROTOCOL_VERSION: Version = Version { major: 1, minor: 0 };
/// The argument that starts a plugin program in protocol mode.
pub const PROTOCOL_FLAG: &str = "--guidedog-protocol";
/// The publisher protocol's name in a handshake.
pub const PROTOCOL_NAME: &str = "guidedog-publisher";
/// The build-plugin protocol's name in a handshake.
pub const BUILD_PROTOCOL_NAME: &str = "guidedog-build-plugin";
/// The version of the snapshot manifest and its digest.
pub const SNAPSHOT_SCHEMA: i64 = 1;
/// The largest request line a publisher program reads (1 MiB).
pub const MAX_REQUEST: usize = 1024 * 1024;
/// The largest response line or handshake a host reads (64 MiB).
pub const MAX_RESPONSE: usize = 64 * 1024 * 1024;
/// The largest source or HTML text a hook receives or returns (8 MiB).
pub const MAX_PLUGIN_TEXT: usize = 8 * 1024 * 1024;
/// The most generated assets one plugin may return.
pub const MAX_PLUGIN_ASSETS: usize = 256;
/// The most generated asset bytes one plugin may return (16 MiB).
pub const MAX_PLUGIN_ASSET_BYTES: usize = 16 * 1024 * 1024;
/// The longest path a plugin input, dependency or published file may have, in bytes.
pub const MAX_PATH: usize = 1024;

/// A major and minor version. A minor version adds only optional things to its major
/// version; a major version changes or removes something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Version {
    /// Incompatible changes.
    pub major: u16,
    /// Optional additions.
    pub minor: u16,
}

impl Version {
    /// Makes a version from its two numbers.
    pub const fn new(major: u16, minor: u16) -> Version {
        Version { major, minor }
    }

    /// Whether code built against `self` can serve a host speaking `host`: the same
    /// major version and a minor version the host knows.
    pub fn is_compatible_with(self, host: Version) -> bool {
        self.major == host.major && self.minor <= host.minor
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Whether `id` can name a plugin: 2 to 32 lowercase ASCII letters, digits and
/// hyphens, starting with a letter.
pub fn is_valid_id(id: &str) -> bool {
    let b = id.as_bytes();
    (2..=32).contains(&b.len())
        && b[0].is_ascii_lowercase()
        && b.iter().all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_short_lowercase_names() {
        for good in ["pages", "product-badge", "a1"] {
            assert!(is_valid_id(good), "{good}");
        }
        for bad in ["", "a", "Pages!", "1abc", "pages_x", &"a".repeat(33)] {
            assert!(!is_valid_id(bad), "{bad}");
        }
    }

    #[test]
    fn compatible_versions_share_a_major_and_a_known_minor() {
        assert!(Version::new(1, 0).is_compatible_with(API_VERSION));
        assert!(!Version::new(2, 0).is_compatible_with(API_VERSION));
        assert!(!Version::new(1, 2).is_compatible_with(API_VERSION));
    }
}
