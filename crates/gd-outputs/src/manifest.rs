//! The manifest (`.doctrees/<builder>.outputs.json`): every output unit of the last
//! published generation, with what it was made from.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gd_env::{Asset, Digest, Lookup, ObjectBuilder, ObjectReader, read_record};
use gd_host::{Budget, Problem};
use gd_json::{Object, Value};
use gd_report::Report;

/// The manifest's format version. In version 2 lookups carry their answers' digests,
/// so a changed answer is known exactly; a manifest of another version is not read.
pub const MANIFEST_VERSION: u32 = 2;

/// The key of the unit of files every page shares: static files, indexes, assets.
pub const SITE_UNIT: &str = "";

/// One output unit: a document's page (keyed by docname), a book (`book:<file>`), the
/// search index (`search`), or the site's shared files ([`SITE_UNIT`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutputRecord {
    /// The digest of the unit's inputs, lookups excluded (docs/manual/internals/
    /// incremental.rst lists them per builder). The zero digest marks a unit whose
    /// output is not trusted (an unreadable journal was discarded): it is never reused.
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
        let mut units = Object::new();
        for (key, unit) in &self.units {
            units.insert(key.clone(), record_to_json(unit));
        }
        let value = ObjectBuilder::default()
            .number("version", u64::from(MANIFEST_VERSION))
            .value("units", Value::Object(units))
            .done();
        gd_json::to_string(&value)
    }

    /// Reads a manifest; `None` for another version or anything that is not one (the
    /// build then writes every unit).
    pub fn from_json(text: &str) -> Option<Manifest> {
        ObjectReader::read_versioned(text, u64::from(MANIFEST_VERSION), |r| {
            let mut units = BTreeMap::new();
            for (key, unit) in r.value("units")?.as_object()? {
                units.insert(key.clone(), record_from_json(unit)?);
            }
            Some(Manifest { units })
        })
    }

    /// The manifest saved at `path`, read within `budget`: empty when there is none, it
    /// cannot be read, or it is not a manifest of this version. Fails only when the
    /// budget cannot hold the file (`host.budget`).
    pub(crate) fn load(path: &Path, budget: &Budget) -> Result<Manifest, Problem> {
        Ok(read_record(path, budget)?.and_then(|t| Manifest::from_json(&t)).unwrap_or_default())
    }
}

fn record_to_json(unit: &OutputRecord) -> Value {
    ObjectBuilder::default()
        .digest("inputs", &unit.inputs)
        .texts("files", &unit.files)
        .list("assets", &unit.assets, asset_to_json)
        .list("lookups", &unit.lookups, lookup_to_json)
        .list("reports", &unit.reports, gd_env::report_to_json)
        .done()
}

fn record_from_json(value: &Value) -> Option<OutputRecord> {
    let r = ObjectReader::of(value)?;
    Some(OutputRecord {
        inputs: r.digest("inputs")?,
        files: r.texts("files")?,
        assets: r.list("assets", asset_from_json)?,
        lookups: r.list("lookups", lookup_from_json)?,
        reports: r.list("reports", gd_env::report_from_json)?,
    })
}

fn asset_to_json(asset: &Asset) -> Value {
    ObjectBuilder::default()
        .text("source", &asset.source.to_string_lossy())
        .text("target", &asset.target)
        .done()
}

fn asset_from_json(value: &Value) -> Option<Asset> {
    let r = ObjectReader::of(value)?;
    Some(Asset { source: PathBuf::from(r.text("source")?), target: r.text("target")? })
}

fn lookup_to_json(entry: &(Lookup, Digest)) -> Value {
    ObjectBuilder::default().text("lookup", &entry.0.encode()).digest("answer", &entry.1).done()
}

fn lookup_from_json(value: &Value) -> Option<(Lookup, Digest)> {
    let r = ObjectReader::of(value)?;
    Some((Lookup::decode(&r.text("lookup")?)?, r.digest("answer")?))
}
