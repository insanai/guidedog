//! Versioned theme archives and offline verification of content-addressed installations.
//! Archives are JSON with base64 file values; they contain data, never executable hooks.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use gd_json::{Object, Value};
use gd_sha::Sha256;

mod files;
pub use files::{read_tree, verify};

/// Maximum total unencoded theme bytes (64 MiB).
pub const MAX_BYTES: usize = 64 << 20;
/// Maximum file count, bounding decoding and offline directory walks.
pub const MAX_FILES: usize = 4096;
/// The project's lock file. Builds read this file without making network requests.
pub const LOCK_FILE: &str = "guidedog-theme.lock";

/// A theme archive or installation failed validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

/// The canonical archive contents, sorted by portable relative filename.
pub type Files = BTreeMap<String, Vec<u8>>;

/// The immutable archive pin and its canonical unencoded file tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lock {
    /// Original archive URL or local path, for explicit future reinstalls.
    pub source: String,
    /// SHA-256 of the exact downloaded archive bytes.
    pub archive_sha256: String,
    /// SHA-256 of the canonical file tree; also names the installed directory.
    pub tree_sha256: String,
}

impl Lock {
    /// Serializes the supplied source and pins without adding timestamps.
    pub fn encode(&self) -> String {
        let mut object = Object::new();
        object.insert("format".into(), Value::from(1i64));
        object.insert("source".into(), Value::from(self.source.as_str()));
        object.insert("archive_sha256".into(), Value::from(self.archive_sha256.as_str()));
        object.insert("tree_sha256".into(), Value::from(self.tree_sha256.as_str()));
        gd_json::to_string(&Value::Object(object))
    }

    /// Reads a version-one lock and validates both checksum representations.
    pub fn decode(text: &str) -> Result<Self, Error> {
        let value = gd_json::parse(text).map_err(|e| Error(e.to_string()))?;
        if value.get("format").and_then(Value::as_i64) != Some(1) {
            return Err(error("Unsupported theme lock format."));
        }
        let text = |key| {
            value
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| error("Incomplete theme lock."))
        };
        let lock = Self {
            source: text("source")?,
            archive_sha256: text("archive_sha256")?,
            tree_sha256: text("tree_sha256")?,
        };
        if !valid_digest(&lock.archive_sha256) || !valid_digest(&lock.tree_sha256) {
            return Err(error("Theme checksums must be 64 lowercase hexadecimal digits."));
        }
        Ok(lock)
    }

    /// The installed directory beneath the project root; its name is a validated digest.
    pub fn directory(&self, project: &Path) -> PathBuf {
        project.join("_themes").join(&self.tree_sha256)
    }
}

/// SHA-256 of raw archive bytes, matching sha256sum and the installation pin.
pub fn archive_digest(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(bytes);
    gd_sha::to_hex(&hash.finish())
}

/// Hashes sorted names and file bytes with length prefixes and a versioned domain tag.
pub fn tree_digest(files: &Files) -> String {
    let mut hash = Sha256::new();
    hash.update(b"guidedog-theme-v1\0");
    for (name, data) in files {
        hash.update(&(name.len() as u64).to_be_bytes());
        hash.update(name.as_bytes());
        hash.update(&(data.len() as u64).to_be_bytes());
        hash.update(data);
    }
    gd_sha::to_hex(&hash.finish())
}

/// Writes the bounded version-one archive. Names and contents are validated first.
pub fn encode(files: &Files) -> Result<Vec<u8>, Error> {
    validate(files)?;
    let mut entries = Object::new();
    for (name, data) in files {
        entries.insert(name.clone(), Value::from(gd_json::base64::encode(data)));
    }
    let mut object = Object::new();
    object.insert("format".into(), Value::from(1i64));
    object.insert("files".into(), Value::Object(entries));
    Ok(gd_json::to_string(&Value::Object(object)).into_bytes())
}

/// Decodes a bounded theme archive, rejecting traversal, aliases and case collisions.
pub fn decode(bytes: &[u8]) -> Result<Files, Error> {
    if bytes.len() > 2 * MAX_BYTES {
        return Err(error("Theme archive exceeds its byte limit."));
    }
    let text =
        std::str::from_utf8(bytes).map_err(|_| error("Theme archive must be UTF-8 JSON."))?;
    let value = gd_json::parse(text).map_err(|e| Error(e.to_string()))?;
    if value.get("format").and_then(Value::as_i64) != Some(1) {
        return Err(error("Unsupported theme archive format."));
    }
    let entries = value
        .get("files")
        .and_then(Value::as_object)
        .ok_or_else(|| error("Theme archive has no files object."))?;
    if entries.len() > MAX_FILES {
        return Err(error("Theme archive exceeds its file limit."));
    }
    let mut files = Files::new();
    let mut bytes = 0usize;
    for (name, value) in entries {
        let text =
            value.as_str().ok_or_else(|| error("Theme file values must be base64 strings."))?;
        bytes = bytes
            .checked_add(gd_json::base64::decoded_len(text))
            .ok_or_else(|| error("Theme size overflow."))?;
        if bytes > MAX_BYTES {
            return Err(error("Theme archive exceeds its expanded byte limit."));
        }
        let data =
            gd_json::base64::decode(text).ok_or_else(|| error("Invalid base64 theme file."))?;
        files.insert(name.clone(), data);
    }
    validate(&files)?;
    Ok(files)
}

pub(crate) fn validate(files: &Files) -> Result<(), Error> {
    if files.is_empty() || files.len() > MAX_FILES {
        return Err(error("Theme must contain 1 to 4096 files."));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut bytes = 0usize;
    for (name, data) in files {
        if !valid_path(name) {
            return Err(Error(format!("Unsafe theme path: {name}.")));
        }
        let lower = name.to_ascii_lowercase();
        if !seen.insert(lower.clone()) {
            return Err(Error(format!("Case-colliding theme path: {name}.")));
        }
        bytes = bytes.checked_add(data.len()).ok_or_else(|| error("Theme size overflow."))?;
        if bytes > MAX_BYTES {
            return Err(error("Theme exceeds its expanded byte limit."));
        }
    }
    for name in &seen {
        for (index, _) in name.match_indices('/') {
            if seen.contains(&name[..index]) {
                return Err(error("A theme file overlaps another file's directory."));
            }
        }
    }
    Ok(())
}

pub(crate) fn valid_path(path: &str) -> bool {
    (path.starts_with("_templates/") || path.starts_with("_static/"))
        && path.len() <= 512
        && path.split('/').all(|p| {
            !p.is_empty()
                && p != "."
                && p != ".."
                && p.len() <= 100
                && !p.ends_with('.')
                && p.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                && !reserved(p.split('.').next().unwrap_or(""))
        })
}

fn reserved(name: &str) -> bool {
    let s = name.to_ascii_lowercase();
    matches!(s.as_str(), "con" | "prn" | "aux" | "nul")
        || s.strip_prefix("com")
            .or_else(|| s.strip_prefix("lpt"))
            .is_some_and(|n| n.len() == 1 && matches!(n.as_bytes()[0], b'1'..=b'9'))
}

pub(crate) fn valid_digest(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
}
pub(crate) fn error(message: &str) -> Error {
    Error(message.into())
}
