//! The manifest (`.doctrees/<builder>.outputs.json`): every output unit of the last
//! published generation, with what it was made from.

use std::collections::BTreeMap;

use gd_env::{Asset, Digest, Lookup};
use gd_report::Report;

/// The manifest's format version. 2 is the Rust port's first: lookups carry their
/// answers' digests (the Odin build wrote 1, with the answers folded into one digest).
pub const MANIFEST_VERSION: u32 = 2;

/// The key of the unit of files every page shares: static files, indexes, assets.
pub const SITE_UNIT: &str = "";

/// One output unit: a document's page (keyed by docname), a book (`book:<file>`), the
/// search index (`search`), or the site's shared files ([`SITE_UNIT`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutputRecord {
    /// The digest of the unit's inputs, lookups excluded (docs/manual/internals/
    /// incremental.rst lists them per builder).
    pub inputs: Digest,
    /// The files it made, below the output folder, with `/` separators.
    pub files: Vec<String>,
    /// The images and downloads the site copies for it.
    pub assets: Vec<Asset>,
    /// What its references looked up, each with the digest of the answer it got: the
    /// verifying trace `Generation::reuse` checks.
    pub lookups: Vec<(Lookup, Digest)>,
    /// What writing it reported, given again when it is reused.
    pub reports: Vec<Report>,
}

/// Every unit of one generation, by key.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Manifest {
    /// The units, by key (sorted, so the JSON is stable).
    pub units: BTreeMap<String, OutputRecord>,
}

impl Manifest {
    /// The manifest as the JSON the commit saves (gd-env's `ObjectBuilder`).
    pub fn to_json(&self) -> String {
        todo!("wave 1: write the manifest with gd_env::ObjectBuilder")
    }

    /// Reads a manifest; `None` for another version or anything that is not one (the
    /// build then writes every unit).
    pub fn from_json(text: &str) -> Option<Manifest> {
        let _ = text;
        todo!("wave 1: read the manifest with gd_env::ObjectReader")
    }
}
