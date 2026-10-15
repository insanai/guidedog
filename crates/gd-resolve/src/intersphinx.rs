//! `sphinx.ext.intersphinx`: other projects' inventories (intersphinx.odin).
//!
//! Inventories are fetched when a build starts and kept in
//! `<build>/.doctrees/__intersphinx_cache__/<name>_objects.inv`, reused for
//! `intersphinx_cache_limit` days; local inventory files are always read again. An
//! `--untrusted` build fetches nothing.

use std::collections::BTreeMap;

use gd_conf::Config;
use gd_env::Digest;
use gd_host::{Budget, Problem};
use gd_inventory::Inventory;
use gd_report::Report;

/// The loaded inventories: each by its `intersphinx_mapping` name, and all merged, a
/// later name replacing an earlier one's entries, as Sphinx merges them.
#[derive(Debug, Clone, Default)]
pub struct Intersphinx {
    /// Each project's inventory, by name.
    pub named: BTreeMap<String, Inventory>,
    /// Every inventory merged.
    pub main: Inventory,
    /// The digest of every inventory loaded (name, URI, base and bytes), for the page
    /// digests.
    pub digest: Digest,
}

/// How inventories are loaded.
#[derive(Debug, Clone, Copy)]
pub struct IntersphinxOptions<'a> {
    /// Fetch remote inventories (false in an `--untrusted` build: only cached and local
    /// ones are read).
    pub network: bool,
    /// The budget the inventories are held within for the build.
    pub budget: &'a Budget,
}

/// Loads every project's inventory as Sphinx's `load_mappings` does: each location in
/// turn, a fresh cached copy of a remote inventory first. A project none of whose
/// locations works gets a warning in `reports` and is left out. Nothing is loaded
/// unless `sphinx.ext.intersphinx` is on. Fails only when the budget cannot hold an
/// inventory.
pub fn load_intersphinx(
    config: &Config,
    options: IntersphinxOptions<'_>,
    reports: &mut Vec<Report>,
) -> Result<Intersphinx, Problem> {
    let _ = (config, options, reports);
    todo!("wave 1: port load_intersphinx and load_inventory with gd-fetch")
}
