//! Sphinx object inventories (`objects.inv`), read and written as Sphinx's
//! `sphinx.util.inventory` does.
//!
//! An inventory lists the named things of a documentation project, each with its type
//! (`py:function`, `std:label`), priority, location and display name; intersphinx links
//! references to other projects through them. Version 2 files are a four-line header
//! and zlib-compressed lines; version 1 files are plain lines. Both load.
//!
//! ```
//! use gd_inventory::{Item, load_inventory, write_inventory};
//! let items = [Item {
//!     name: "len".into(),
//!     kind: "py:function".into(),
//!     priority: 1,
//!     uri: "library/functions.html#len".into(),
//!     display: "-".into(),
//! }];
//! let data = write_inventory("Python", "3.15", &items);
//! let inv = load_inventory(&data, "https://docs.python.org/3/").unwrap();
//! let len = inv.find("py:function", "len").unwrap();
//! assert_eq!(len.uri, "https://docs.python.org/3/library/functions.html#len");
//! ```

mod parse;

use gd_report::Report;
use std::collections::HashMap;
use std::fmt;

/// The most text an inventory may decompress to, against decompression bombs.
pub const MAX_BODY: usize = 256 << 20;

/// One object an inventory lists.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Item {
    /// The object's name, which may hold spaces (`"with statement"`).
    pub name: String,
    /// Its type as `domain:objtype`, such as `"py:function"`.
    pub kind: String,
    /// Sphinx's search priority: 1 important, 0 normal, -1 hidden from search.
    pub priority: i64,
    /// Its location: absolute, or relative to where the inventory came from.
    pub uri: String,
    /// The display name, or `"-"` when it is the name itself.
    pub display: String,
}

/// A loaded inventory: the project it describes and its items, indexed by type and name.
#[derive(Debug, Clone, Default)]
pub struct Inventory {
    /// The project's name, as in "(in Python v3.14)".
    pub project: String,
    /// The project's version.
    pub version: String,
    items: Vec<Item>,
    /// `by_type[kind][name]` is the index in `items` of the item that wins.
    by_type: HashMap<String, HashMap<String, usize>>,
    /// `folded[kind][lowercase name]` is the first name listed with that spelling, for the
    /// types Sphinx matches ignoring case.
    folded: HashMap<String, HashMap<String, String>>,
}

/// Types that Sphinx matches ignoring case.
const FOLDED_TYPES: [&str; 2] = ["std:label", "std:term"];

impl Inventory {
    /// An empty inventory for `project` at `version`.
    pub fn new(project: &str, version: &str) -> Inventory {
        Inventory {
            project: project.to_string(),
            version: version.to_string(),
            ..Default::default()
        }
    }

    /// Every item in the order added, borrowed; replaced duplicates are still listed.
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// The number of items added.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Reports whether the inventory lists anything of type `kind`.
    pub fn has_type(&self, kind: &str) -> bool {
        self.by_type.contains_key(kind)
    }

    /// Appends `item` and indexes it. A later item of the same type and name replaces the
    /// earlier one in lookups, as a later line overwrites in Sphinx's dictionaries.
    pub fn add(&mut self, item: Item) {
        let index = self.items.len();
        if FOLDED_TYPES.contains(&item.kind.as_str()) {
            let spellings = self.folded.entry(item.kind.clone()).or_default();
            spellings.entry(item.name.to_lowercase()).or_insert_with(|| item.name.clone());
        }
        let names = self.by_type.entry(item.kind.clone()).or_default();
        names.insert(item.name.clone(), index);
        self.items.push(item);
    }

    /// The item of type `kind` named `name`, borrowed from the inventory.
    pub fn find(&self, kind: &str, name: &str) -> Option<&Item> {
        let index = *self.by_type.get(kind)?.get(name)?;
        self.items.get(index)
    }

    /// The item of type `kind` whose name matches `name` ignoring case: the first such
    /// spelling listed, as Sphinx matches labels and terms. Other types never match here.
    pub fn find_folded(&self, kind: &str, name: &str) -> Option<&Item> {
        let first = self.folded.get(kind)?.get(&name.to_lowercase())?;
        self.find(kind, first)
    }
}

/// What is wrong with an inventory file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// The first line names no inventory version Sphinx writes.
    UnknownVersion,
    /// The header ends before its project and version lines.
    Truncated,
    /// The compressed part does not decompress.
    Compression(gd_flate::ErrorKind),
}

/// Why an inventory could not be loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error {
    /// What is wrong.
    pub kind: ErrorKind,
}

impl Error {
    /// The stable diagnostic code, such as `"inventory.version"`.
    pub fn code(&self) -> &'static str {
        match self.kind {
            ErrorKind::UnknownVersion => "inventory.version",
            ErrorKind::Truncated => "inventory.truncated",
            ErrorKind::Compression(_) => "inventory.compression",
        }
    }

    /// A sentence saying what is wrong.
    pub fn message(&self) -> &'static str {
        match self.kind {
            ErrorKind::UnknownVersion => {
                "This is not a Sphinx inventory: it does not start with \
                 `# Sphinx inventory version 1` or `version 2`."
            }
            ErrorKind::Truncated => "The inventory's header ends before its project and version.",
            ErrorKind::Compression(gd_flate::ErrorKind::Limit) => {
                "The inventory decompresses to more than 256 MiB."
            }
            ErrorKind::Compression(_) => "The inventory's compressed part is damaged.",
        }
    }

    /// A sentence saying what to do about it.
    pub fn hint(&self) -> &'static str {
        match self.kind {
            ErrorKind::UnknownVersion => {
                "Check the inventory URL: it should name the project's objects.inv, not a web \
                 page or an error page."
            }
            _ => "Download the inventory again, or rebuild the project that publishes it.",
        }
    }

    /// An Elm-style report naming the inventory's `path` or URL.
    pub fn report(&self, path: &str) -> Report {
        let report = Report::error(self.code(), "BAD INVENTORY", self.message()).hint(self.hint());
        Report { path: path.to_string(), ..report }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for Error {}

/// Reads an inventory of version 1 or 2 from `data`, which is only borrowed. `base` is
/// joined to relative locations, as Sphinx joins them with the URL or folder the
/// inventory came from; `""` leaves them as written. Lines that do not parse are
/// skipped, as Sphinx skips them. Fails on an unknown header or damaged compression.
pub fn load_inventory(data: &[u8], base: &str) -> Result<Inventory, Error> {
    let (first, rest) = parse::next_line(data);
    let version = match first.trim_ascii_end() {
        b"# Sphinx inventory version 1" => 1,
        b"# Sphinx inventory version 2" => 2,
        _ => return Err(Error { kind: ErrorKind::UnknownVersion }),
    };
    let (project, version_text, rest) = parse::header(rest)?;
    let mut inventory = Inventory::new(&project, &version_text);
    if version == 1 {
        parse::load_v1(&mut inventory, &String::from_utf8_lossy(rest), base);
        return Ok(inventory);
    }
    let (_, compressed) = parse::next_line(rest); // "# The remainder of this file is ..."
    let body = gd_flate::zlib_decompress(compressed, MAX_BODY)
        .map_err(|e| Error { kind: ErrorKind::Compression(e.kind) })?;
    parse::load_v2(&mut inventory, &String::from_utf8_lossy(&body), base);
    Ok(inventory)
}

/// Writes `items` as a version 2 inventory, in the order given (see [`sort_items`]).
/// A location ending in `#name` is shortened to `#$`, and a display equal to the name
/// (or empty) is written as `-`, as Sphinx writes them. Returns the file's bytes.
pub fn write_inventory(project: &str, version: &str, items: &[Item]) -> Vec<u8> {
    let mut body = String::with_capacity(items.len() * 64);
    for item in items {
        write_line(&mut body, item);
    }
    let mut out = Vec::with_capacity(body.len() / 3 + 128);
    out.extend_from_slice(b"# Sphinx inventory version 2\n# Project: ");
    out.extend_from_slice(project.as_bytes());
    out.extend_from_slice(b"\n# Version: ");
    out.extend_from_slice(version.as_bytes());
    out.extend_from_slice(b"\n# The remainder of this file is compressed using zlib.\n");
    gd_flate::zlib_compress_into(&mut out, body.as_bytes(), gd_flate::Level::Default);
    out
}

fn write_line(body: &mut String, item: &Item) {
    use std::fmt::Write;
    let shortened = item.uri.find('#').is_some_and(|hash| {
        !item.name.is_empty() && item.uri[hash + 1..].ends_with(item.name.as_str())
    });
    let uri = if shortened { &item.uri[..item.uri.len() - item.name.len()] } else { &item.uri };
    let display =
        if item.display.is_empty() || item.display == item.name { "-" } else { &item.display };
    let dollar = if shortened { "$" } else { "" };
    let _ =
        writeln!(body, "{} {} {} {uri}{dollar} {display}", item.name, item.kind, item.priority);
}

/// Orders items as Sphinx writes them: by domain, then name, display, type and location.
pub fn sort_items(items: &mut [Item]) {
    fn domain(item: &Item) -> &str {
        item.kind.split(':').next().unwrap_or("")
    }
    items.sort_by(|a, b| {
        domain(a)
            .cmp(domain(b))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.display.cmp(&b.display))
            .then_with(|| a.kind.cmp(&b.kind))
            .then_with(|| a.uri.cmp(&b.uri))
    });
}

#[cfg(test)]
mod tests;
