//! A catalog: its records, the numbers its registry reserves, its labels and process.

use crate::config::Process;
use crate::definition::{definition, definition_mismatch};
use crate::metadata::{Record, is_number_shape, is_valid_slug, parse_metadata};
use crate::problem::{io_problem, shown};
use crate::scan::{SCAN_CHUNK, scan_record};
use crate::transaction::{journal_path, recovery_needed};
use crate::{DEFAULT_PREFIX, LABELS, MAX_INDEX_BYTES, MAX_JOURNAL_BYTES, MAX_NUMBER, MAX_RECORDS};
use crate::{Prefix, Problem, Result, exists, is_valid_prefix, read_text};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A catalog's records in number order, drafts last, with what governs them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Catalog {
    /// The catalog's folder; record paths are inside it.
    pub dir: PathBuf,
    /// The catalog's prefix.
    pub prefix: Prefix,
    /// The records, sorted by number (10000 after 9999), drafts last.
    pub records: Vec<Record>,
    /// The highest number the registry reserves, beyond those records hold.
    pub reserved: u32,
    /// The labels records may carry; empty for the prefix's default vocabulary.
    pub labels: Vec<String>,
    /// The process records follow.
    pub process: Process,
}

impl Catalog {
    /// An empty catalog with `prefix` and the default process, not tied to a folder:
    /// what [`crate::read_record`] needs to check a record on its own.
    pub fn new(prefix: Prefix) -> Catalog {
        Catalog { prefix, ..Catalog::default() }
    }

    /// The labels records may carry, borrowed, or `None` when any well-formed label is
    /// allowed: the catalog's own list, or GDS 0001's for a `gds` catalog without one.
    pub fn vocabulary(&self) -> Option<Vec<&str>> {
        if !self.labels.is_empty() {
            return Some(self.labels.iter().map(String::as_str).collect());
        }
        (self.prefix.as_str() == DEFAULT_PREFIX).then(|| LABELS.to_vec())
    }

    /// Tells whether a record of this catalog may carry `label`.
    pub fn allows_label(&self, label: &str) -> bool {
        is_valid_slug(label) && self.vocabulary().is_none_or(|v| v.contains(&label))
    }
}

/// The file format of a catalog's new records and index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    /// reStructuredText: `registry.rst`, `bundle.rst`, `records/*.rst`.
    #[default]
    Rst,
    /// The older Typst catalogs: `registry.typ`, `bundle.typ`, `records/*.typ`.
    Typst,
}

impl Format {
    /// The file extension with its dot: `".rst"` or `".typ"`.
    pub fn suffix(self) -> &'static str {
        match self {
            Format::Rst => ".rst",
            Format::Typst => ".typ",
        }
    }

    /// The generated index files: registry and reading order.
    pub(crate) fn index_files(self) -> [&'static str; 2] {
        match self {
            Format::Rst => ["registry.rst", "bundle.rst"],
            Format::Typst => ["registry.typ", "bundle.typ"],
        }
    }
}

/// The format of the catalog in `dir`, chosen by its record template; RST without one.
pub fn catalog_format(dir: &Path) -> Format {
    if exists(&dir.join("template/rfc-template.rst")) {
        Format::Rst
    } else if exists(&dir.join("template/rfc-template.typ")) {
        Format::Typst
    } else {
        Format::Rst
    }
}

/// Loads the catalog in `dir`: its records (`records/*.rst`, or `*.typ` in an older
/// catalog), the numbers its registry reserves, and its configuration when it has one,
/// whose prefix must be `prefix`. Each record's metadata is kept, never its text. An
/// update awaiting recovery, an unreadable, too large, or malformed record, or two
/// records with one number or slug is a problem.
pub fn load_catalog(dir: &Path, prefix: &Prefix) -> Result<Catalog> {
    let mut c = Catalog { dir: dir.to_path_buf(), ..Catalog::new(prefix.clone()) };
    if let Some(d) = definition(dir)? {
        if d.prefix != *prefix {
            return Err(definition_mismatch(dir, &d.prefix, prefix));
        }
        (c.labels, c.process) = (d.labels, d.process);
    }
    if exists(&journal_path(dir, prefix)) {
        return Err(recovery_needed(prefix, dir));
    }
    let mut records = Vec::new();
    let mut metadata = 0;
    for path in record_files(dir, prefix)? {
        let (header, sections) = scan_record(prefix, &path, SCAN_CHUNK)?;
        metadata += header.len();
        if metadata as u64 > MAX_JOURNAL_BYTES {
            return Err(Problem::new(prefix.code("count"), "The catalog's metadata is too large",
                shown(dir), "The metadata of the records together is more than 256 MiB.",
                "Split the catalog into several catalogs, each with its own prefix.")
            .exit(3));
        }
        if is_inherited(&header, prefix) {
            records.push(historical_record(path));
            continue;
        }
        let mut r = parse_metadata(&c, &path, &header)?;
        r.sections = sections;
        records.push(r);
    }
    sort_by_number(&mut records);
    c.records = records;
    c.reserved = read_reservations(dir, prefix)?;
    check_duplicates(&c)?;
    Ok(c)
}

/// The record files of the catalog in `dir`, in no particular order.
fn record_files(dir: &Path, prefix: &Prefix) -> Result<Vec<PathBuf>> {
    let folder = dir.join("records");
    let ns = prefix.as_str();
    let entries = std::fs::read_dir(&folder).map_err(|e| io_problem(ns, &folder, &e))?;
    let mut files = Vec::new();
    for (count, entry) in entries.enumerate() {
        if count >= MAX_RECORDS {
            return Err(Problem::new(prefix.code("count"), "Too many files", shown(dir),
                "The records folder has more than 100,000 entries.",
                "Split the catalog into several catalogs, each with its own prefix.")
            .exit(3));
        }
        let path = entry.map_err(|e| io_problem(ns, &folder, &e))?.path();
        if path.extension().is_some_and(|e| e == "rst" || e == "typ") {
            files.push(path);
        }
    }
    Ok(files)
}

/// Tells whether a record is an older Typst record of another process, such as the ZDS
/// records GDS began from, kept as provenance and never changed.
fn is_inherited(header: &str, prefix: &Prefix) -> bool {
    let Some(rest) = header.strip_prefix("#let ") else { return false };
    let name = rest.split("-number").next().unwrap_or("");
    is_valid_prefix(name) && name != prefix.as_str()
}

fn historical_record(path: PathBuf) -> Record {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
    let (number, slug) = stem.split_once('-').unwrap_or((&stem, ""));
    Record {
        number: number.to_string(),
        slug: slug.to_string(),
        title: slug.to_string(),
        state: "historical".to_string(),
        historical: true,
        path,
        ..Record::default()
    }
}

/// Orders records (owned or borrowed) by number, numerically (10000 after 9999), drafts
/// last, and records of one number by slug.
pub(crate) fn sort_by_number<R: Borrow<Record>>(records: &mut [R]) {
    let order = |r: &Record| -> u32 {
        if is_number_shape(&r.number) { r.number.parse().unwrap_or(u32::MAX) } else { u32::MAX }
    };
    records.sort_by(|a, b| {
        let (a, b) = (a.borrow(), b.borrow());
        order(a).cmp(&order(b)).then_with(|| a.slug.cmp(&b.slug))
    });
}

/// The number the registry reserves through, or 0.
fn read_reservations(dir: &Path, prefix: &Prefix) -> Result<u32> {
    let path = dir.join(format!("registry{}", catalog_format(dir).suffix()));
    if !exists(&path) {
        return Ok(0);
    }
    let text = read_text(prefix.as_str(), &path, MAX_INDEX_BYTES)?;
    let rst = format!(":{prefix}-reserved-through:");
    let typst = format!("#let {prefix}-reserved-through = ");
    for line in text.lines() {
        let Some(raw) = line.strip_prefix(&rst).or_else(|| line.strip_prefix(&typst)) else {
            continue;
        };
        return match raw.trim().parse::<u32>() {
            Ok(n) if n <= MAX_NUMBER => Ok(n),
            _ => Err(crate::metadata::metadata_problem(prefix, &path, "reserved-through")),
        };
    }
    Ok(0)
}

/// Finds two records with one slug or one number, in time linear in the catalog's size.
fn check_duplicates(c: &Catalog) -> Result<()> {
    let mut slugs: HashMap<&str, usize> = HashMap::with_capacity(c.records.len());
    let mut numbers: HashMap<&str, usize> = HashMap::with_capacity(c.records.len());
    for (i, r) in c.records.iter().enumerate() {
        let mut taken = slugs.get(r.slug.as_str()).copied();
        if taken.is_none() && !r.is_draft() {
            taken = numbers.get(r.number.as_str()).copied();
        }
        if let Some(j) = taken {
            return Err(Problem::new(c.prefix.code("duplicate"),
                "Discussion identity is already used", shown(&r.path),
                format!("{} has the same number or slug.", c.records[j].file_name()),
                "Rename one record so that its number and slug are unique."));
        }
        slugs.insert(&r.slug, i);
        if !r.is_draft() {
            numbers.insert(&r.number, i);
        }
    }
    Ok(())
}

/// The one record `id` names, borrowed from the catalog: its number (`7` or `0007`),
/// slug, title, or file name. No match, or more than one, is a problem that lists the
/// candidates or suggests a close slug.
pub fn resolve<'c>(c: &'c Catalog, id: &str) -> Result<&'c Record> {
    let normalized = match id.parse::<u32>() {
        Ok(n) if (1..=MAX_NUMBER).contains(&n) => c.process.format_number(n),
        _ => id.to_string(),
    };
    let mut matches = c.records.iter().filter(|r| {
        r.number == normalized || r.slug == id || r.title == id || r.file_name() == id
    });
    let first = matches.next();
    let second = matches.next();
    if let (Some(r), None) = (first, second) {
        return Ok(r);
    }
    let mut candidates = String::new();
    let all = c.records.iter().filter(|r| {
        r.number == normalized || r.slug == id || r.title == id || r.file_name() == id
    });
    for r in all {
        candidates.push(' ');
        candidates.push_str(r.file_name());
    }
    let close = gd_report::suggest(id, c.records.iter().map(|r| r.slug.as_str()));
    Err(Problem::new(c.prefix.code("record"), "Select one discussion", id,
        format!("The identifier is missing or ambiguous.{candidates}"),
        "Run the list command; use a number, exact slug, full title, or filename.")
    .exit(2)
    .suggest(close.filter(|_| first.is_none())))
}

/// Checks that every record other than a historical one has the sections the record
/// template requires, Abstract and References; the first missing one is a problem. A
/// record is prose: the source limits of GDS 0001 do not apply to it.
pub fn check_sources(c: &Catalog) -> Result<()> {
    for r in c.records.iter().filter(|r| !r.historical) {
        let missing = if !r.sections.has_abstract {
            "Abstract"
        } else if !r.sections.has_references {
            "References"
        } else {
            continue;
        };
        return Err(Problem::new(c.prefix.code("section"), "A required section is missing",
            shown(&r.path), format!("{missing} is missing."),
            format!("Add the {missing} section using the discussion template.")));
    }
    Ok(())
}

/// Tells whether `path` (relative, `/`-separated) may be changed by a plan: an index
/// file, or a record file named by a number or draft number and a slug.
pub(crate) fn is_safe_change_path(path: &str) -> bool {
    let index = Format::Rst.index_files().into_iter().chain(Format::Typst.index_files());
    if index.into_iter().any(|f| f == path) {
        return true;
    }
    let Some(name) = path.strip_prefix("records/") else { return false };
    let Some(name) = name.strip_suffix(".rst").or_else(|| name.strip_suffix(".typ")) else {
        return false;
    };
    let Some((number, slug)) = name.split_once('-') else { return false };
    (is_number_shape(number) || number == crate::DRAFT_NUMBER) && is_valid_slug(slug)
}

