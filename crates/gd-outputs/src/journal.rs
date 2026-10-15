//! The commit journal (`<doctrees>/<builder>.commit.json`): what a decided commit
//! publishes, saved before its first step so an interrupted commit can be finished or
//! undone by the next build.

use std::path::{Path, PathBuf};

use gd_env::{ObjectBuilder, ObjectReader};
use gd_host::Identity;
use gd_json::Value;

use crate::commit::JOURNAL_VERSION;

/// How many records a commit publishes: the manifest and the environment.
pub(crate) const MAX_RECORDS: usize = 2;

/// A decided commit.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Journal {
    /// The published output folder.
    pub out_dir: PathBuf,
    /// The next output, complete; `None` when the output does not change.
    pub next: Option<PathBuf>,
    /// Where the previous output goes once replaced.
    pub old: PathBuf,
    /// The next folder's identity; `None` where the system gives none (then no
    /// exchange is tried, and the names tell where the switch stands).
    pub next_id: Option<Identity>,
    /// The manifest and the environment, when they changed.
    pub records: Vec<PendingFile>,
}

/// A record written beside its destination, to be renamed over it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct PendingFile {
    /// Where it was written.
    pub from: PathBuf,
    /// Its destination.
    pub to: PathBuf,
    /// The link to the record it replaces, to put back if the commit is undone; `None`
    /// when there was none.
    pub previous: Option<PathBuf>,
}

impl Journal {
    /// The journal as JSON; `None` when a path is not Unicode (the commit is then not
    /// decided).
    pub fn to_json(&self) -> Option<String> {
        let next_id = match self.next_id {
            None => Value::Null,
            Some(id) => ObjectBuilder::default()
                .number("device", id.device)
                .number("inode", id.inode)
                .done(),
        };
        let mut records = Vec::with_capacity(self.records.len());
        for r in &self.records {
            records.push(
                ObjectBuilder::default()
                    .text("from", text(&r.from)?)
                    .text("to", text(&r.to)?)
                    .value("previous", optional_path(r.previous.as_deref())?)
                    .done(),
            );
        }
        let value = ObjectBuilder::default()
            .number("version", u64::from(JOURNAL_VERSION))
            .text("out_dir", text(&self.out_dir)?)
            .value("next", optional_path(self.next.as_deref())?)
            .text("old", text(&self.old)?)
            .value("next_id", next_id)
            .value("records", Value::Array(records))
            .done();
        Some(gd_json::to_string(&value))
    }

    /// Reads a journal this Guidedog writes: its version, an output folder, the folders
    /// a switch needs, and at most [`MAX_RECORDS`] records that each name a file to
    /// rename and its destination. `None` for anything else, which is discarded.
    pub fn from_json(text: &str) -> Option<Journal> {
        let journal =
            ObjectReader::read_versioned(text, u64::from(JOURNAL_VERSION), read_journal)?;
        journal.is_valid().then_some(journal)
    }

    fn is_valid(&self) -> bool {
        let named = |p: &Path| !p.as_os_str().is_empty();
        named(&self.out_dir)
            && self.records.len() <= MAX_RECORDS
            && (self.next.is_none() || named(&self.old))
            && self.next.as_deref().is_none_or(named)
            && self.records.iter().all(|r| named(&r.from) && named(&r.to))
    }
}

/// A journal's fields, before [`Journal::is_valid`] checks them.
fn read_journal(r: ObjectReader<'_>) -> Option<Journal> {
    let next_id = match r.value("next_id")? {
        Value::Null => None,
        id => {
            let id = ObjectReader::of(id)?;
            Some(Identity { device: id.number("device")?, inode: id.number("inode")? })
        }
    };
    Some(Journal {
        out_dir: path_from(&r, "out_dir")?,
        next: optional_from(&r, "next")?,
        old: path_from(&r, "old")?,
        next_id,
        records: r.list("records", record_from_json)?,
    })
}

fn record_from_json(value: &Value) -> Option<PendingFile> {
    let r = ObjectReader::of(value)?;
    Some(PendingFile {
        from: path_from(&r, "from")?,
        to: path_from(&r, "to")?,
        previous: optional_from(&r, "previous")?,
    })
}

fn text(path: &Path) -> Option<&str> {
    path.to_str()
}

fn optional_path(path: Option<&Path>) -> Option<Value> {
    match path {
        None => Some(Value::Null),
        Some(p) => Some(Value::from(text(p)?)),
    }
}

fn path_from(r: &ObjectReader<'_>, key: &str) -> Option<PathBuf> {
    r.text(key).map(PathBuf::from)
}

fn optional_from(r: &ObjectReader<'_>, key: &str) -> Option<Option<PathBuf>> {
    match r.value(key)? {
        Value::Null => Some(None),
        v => Some(Some(PathBuf::from(v.as_str()?))),
    }
}
