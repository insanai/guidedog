//! The Rust API pages of the last build, kept in `<doctrees>/rust-api.json` so that a
//! build whose crates did not change does not parse them again: parsing every crate
//! with tree-sitter was most of the time of a build with nothing to do.
//!
//! A crate's pages are reused only when everything they were made from is as it was:
//! the same Guidedog executable, crate folder and options (the key); the same files
//! below the crate's folder (the listing, so a module file created where a `mod`
//! declaration looked for one counts); and the same bytes in every file the load read
//! or tried to read (`gd_rustdoc::Crate::inputs`). The record is only a memo: one that
//! is missing, damaged or of another version is ignored, and the pages are made again.

use std::fs;
use std::path::{Path, PathBuf};

use gd_conf::{Config, Digest, Digester};
use gd_env::{ObjectBuilder, ObjectReader, report_from_json, report_to_json};
use gd_json::Value;
use gd_report::Report;

/// The record's format; a record of another one is ignored.
const VERSION: u64 = 1;

/// Folders the listing does not look into: build output and hidden folders (`.git`),
/// whose churn would otherwise make every build parse the crates again. A file in them
/// that a crate reads is still checked byte for byte through its inputs.
fn skipped_folder(name: &str) -> bool {
    name == "target" || name.starts_with('.')
}

/// One crate's pages and what they were made from.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CratePages {
    /// The crate's name as code writes it.
    pub(crate) name: String,
    /// The index page's text.
    pub(crate) index: String,
    /// Each module page: its path below the crate (`my_crate/parser.rst`) and its text.
    pub(crate) modules: Vec<(String, String)>,
    /// What loading and documenting the crate reported.
    pub(crate) reports: Vec<Report>,
    /// Every file the pages were made from, with the digest of its bytes then (`None`
    /// when it could not be read).
    pub(crate) inputs: Vec<(PathBuf, Option<Digest>)>,
    /// Which files existed below the crate's folder ([`listing_digest`]).
    pub(crate) listing: Digest,
}

impl CratePages {
    /// Whether the pages still hold for a crate folder whose listing is now `listing`:
    /// every input has the bytes it had.
    pub(crate) fn still_holds(&self, listing: &Digest) -> bool {
        self.listing == *listing
            && self.inputs.iter().all(|(path, digest)| gd_env::file_digest(path) == *digest)
    }
}

/// The key of a crate's pages: the executable that made them (a new Guidedog may write
/// other pages), the crate's path and the options.
pub(crate) fn crate_key(path: &Path, options: &gd_rustdoc::Options) -> Digest {
    let mut d = Digester::new();
    d.text(gd_env::executable_stamp())
        .text(&path.display().to_string())
        .text(&format!("{options:?}"));
    d.finish()
}

/// The folder a crate's files are read from, as gd-rustdoc finds it: the folder given,
/// or the folder of a root file given.
pub(crate) fn crate_folder(path: &Path) -> PathBuf {
    if path.is_file() {
        return path.parent().map(Path::to_path_buf).unwrap_or_default();
    }
    path.to_path_buf()
}

/// The digest of which files and folders exist below `folder` (their relative paths,
/// sorted), without the folders [`skipped_folder`] names. Contents are not read.
pub(crate) fn listing_digest(folder: &Path) -> Digest {
    let mut d = Digester::new();
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        let Ok(entries) = fs::read_dir(folder.join(&relative)) else { continue };
        let mut names: Vec<(String, bool)> = entries
            .flatten()
            .map(|e| {
                let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
                (e.file_name().to_string_lossy().into_owned(), is_dir)
            })
            .collect();
        names.sort();
        for (name, is_dir) in names {
            let path = relative.join(&name);
            d.text(&path.to_string_lossy()).flag(is_dir);
            if is_dir && !skipped_folder(&name) {
                pending.push(path);
            }
        }
    }
    d.finish()
}

/// The crates' pages the last build recorded, by key.
#[derive(Debug, Default)]
pub(crate) struct RustCache {
    path: PathBuf,
    entries: Vec<(Digest, CratePages)>,
}

impl RustCache {
    /// Reads the record of `c`'s build; empty when there is none or it cannot be read.
    pub(crate) fn load(c: &Config) -> RustCache {
        let path = gd_env::doctrees_dir(c).join("rust-api.json");
        let entries = fs::read_to_string(&path).ok().and_then(|t| entries_from_json(&t));
        RustCache { path, entries: entries.unwrap_or_default() }
    }

    /// Takes the pages recorded under `key`; check them with
    /// [`CratePages::still_holds`] before using them.
    pub(crate) fn take(&mut self, key: &Digest) -> Option<CratePages> {
        let i = self.entries.iter().position(|(k, _)| k == key)?;
        Some(self.entries.swap_remove(i).1)
    }

    /// Records `crates` for the next build, unless they are what the record holds
    /// already: `reused` of them were taken from it, and it holds no other. A record
    /// that cannot be written is left out: the next build parses the crates again.
    pub(crate) fn save(&self, crates: &[(Digest, CratePages)], reused: usize) {
        if reused == crates.len() && self.entries.is_empty() {
            return;
        }
        let value = ObjectBuilder::default()
            .number("version", VERSION)
            .list("crates", crates, entry_to_json)
            .done();
        let _ = gd_host::write_unsynced(&self.path, gd_json::to_string(&value).as_bytes());
    }
}

fn entries_from_json(text: &str) -> Option<Vec<(Digest, CratePages)>> {
    ObjectReader::read_versioned(text, VERSION, |r| r.list("crates", entry_from_json))
}

fn entry_to_json(entry: &(Digest, CratePages)) -> Value {
    let (key, p) = entry;
    let module = |m: &(String, String)| {
        ObjectBuilder::default().text("path", &m.0).text("text", &m.1).done()
    };
    let input = |i: &(PathBuf, Option<Digest>)| {
        let digest = i.1.map_or(Value::Null, |d| Value::from(d.to_string()));
        ObjectBuilder::default()
            .text("path", &i.0.to_string_lossy())
            .value("digest", digest)
            .done()
    };
    ObjectBuilder::default()
        .digest("key", key)
        .digest("listing", &p.listing)
        .text("name", &p.name)
        .text("index", &p.index)
        .list("modules", &p.modules, module)
        .list("inputs", &p.inputs, input)
        .list("reports", &p.reports, report_to_json)
        .done()
}

fn entry_from_json(v: &Value) -> Option<(Digest, CratePages)> {
    let r = ObjectReader::of(v)?;
    let module = |v: &Value| {
        let m = ObjectReader::of(v)?;
        Some((m.text("path")?, m.text("text")?))
    };
    let input = |v: &Value| {
        let i = ObjectReader::of(v)?;
        let digest = match i.value("digest")? {
            Value::Null => None,
            d => Some(Digest::from_hex(d.as_str()?)?),
        };
        Some((PathBuf::from(i.text("path")?), digest))
    };
    let pages = CratePages {
        name: r.text("name")?,
        index: r.text("index")?,
        modules: r.list("modules", module)?,
        reports: r.list("reports", report_from_json)?,
        inputs: r.list("inputs", input)?,
        listing: r.digest("listing")?,
    };
    Some((r.digest("key")?, pages))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pages() -> CratePages {
        CratePages {
            name: "shapes".into(),
            index: "Index\n=====\n".into(),
            modules: vec![("shapes.rst".into(), "shapes\n======\n".into())],
            reports: vec![Report::error("rust.autodoc.parse", "T", "m").hint("h")],
            inputs: vec![
                ("/nowhere/src/lib.rs".into(), None),
                ("/x/README.md".into(), Some(Digest::of_text("x"))),
            ],
            listing: Digest::of_text("listing"),
        }
    }

    #[test]
    fn records_read_back_whole() {
        let entry = (Digest::of_text("key"), pages());
        let text = gd_json::to_string(&entry_to_json(&entry));
        assert_eq!(entry_from_json(&gd_json::parse(&text).unwrap()), Some(entry));
    }

    #[test]
    fn pages_hold_while_their_files_are_as_they_were() {
        let mut p = pages();
        p.inputs.truncate(1);
        assert!(p.still_holds(&Digest::of_text("listing")));
        assert!(!p.still_holds(&Digest::of_text("another listing")));
        p.inputs[0].1 = Some(Digest::of_text("now readable"));
        assert!(!p.still_holds(&Digest::of_text("listing")));
    }

    #[test]
    fn the_listing_sees_new_files_but_not_build_output() {
        let dir = std::env::temp_dir().join(format!("gd-read-listing-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("target")).unwrap();
        let before = listing_digest(&dir);
        fs::write(dir.join("target/out.rlib"), "x").unwrap();
        assert_eq!(listing_digest(&dir), before);
        fs::write(dir.join("src/new.rs"), "").unwrap();
        assert_ne!(listing_digest(&dir), before);
        let _ = fs::remove_dir_all(&dir);
    }
}
