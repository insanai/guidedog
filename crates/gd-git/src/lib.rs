//! gd-git writes Git objects and the messages of Git's push protocol, without a git
//! program: object IDs, trees built from a list of files, commits, pack files, and the
//! receive-pack advertisement, update request and report.
//!
//! It does no networking: a caller sends the request it builds over smart HTTP (the Pages
//! publisher uses `gd-fetch`) or to any receive-pack, and parses the reply. Only SHA-1
//! repositories, Git's default, are supported. Packs and requests are written to files a
//! chunk at a time, so a large site never sits in memory whole. Failure is an [`Error`],
//! never a panic.
//!
//! ```
//! let page = b"<p>Hello</p>\n";
//! let blob = gd_git::object_id(gd_git::Kind::Blob, page);
//! let files = [gd_git::File { path: "index.html".into(), mode: gd_git::Mode::File, id: blob }];
//! let trees = gd_git::build_trees(&files).expect("a valid tree");
//! let root = &trees[trees.len() - 1]; // the root tree comes last
//! let who = gd_git::Person { name: "Ada".into(), email: "ada@example.org".into(),
//!     time: 1_760_000_000, offset: 0 };
//! let commit = gd_git::commit_bytes(&gd_git::Commit { tree: root.id, parents: vec![],
//!     author: who.clone(), committer: who, message: "Publish the site.\n".into() });
//! println!("{}", gd_git::object_id(gd_git::Kind::Commit, &commit));
//! ```

mod error;
mod pack;
mod protocol;
mod tree;

#[cfg(test)]
mod tests;

use std::fmt;
use std::io::{BufReader, Write};
use std::path::Path;

use gd_sha::Sha1;

pub use error::{Error, ErrorKind};
pub use pack::PackWriter;
pub use protocol::{
    Advertisement, MAX_ADVERTISEMENT, Ref, RefStatus, Report, Update, parse_advertisement,
    parse_report, write_request,
};
pub use tree::{File, Tree, build_trees, is_valid_path};

/// An object's ID: the SHA-1 of its type, size and contents. `Display` writes it in 40
/// lowercase hexadecimal digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct Id(pub [u8; 20]);

impl Id {
    /// The ID Git's protocol uses for a reference that does not exist.
    pub const ZERO: Id = Id([0; 20]);

    /// Returns the ID in 40 lowercase hexadecimal digits.
    pub fn to_hex(&self) -> String {
        let mut text = [0u8; 40];
        String::from(hex_str(&mut text, self))
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut text = [0u8; 40];
        f.write_str(hex_str(&mut text, self))
    }
}

/// Writes `id` into `out` and borrows it as text, without allocating.
fn hex_str<'a>(out: &'a mut [u8; 40], id: &Id) -> &'a str {
    hex_into(out, id);
    // Every byte is an ASCII hexadecimal digit, so this cannot fail.
    std::str::from_utf8(out).unwrap_or_default()
}

/// The type of an object; the value is its type number in a pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A commit.
    Commit = 1,
    /// A tree (a folder listing).
    Tree = 2,
    /// A blob (a file's contents).
    Blob = 3,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Commit => "commit",
            Kind::Tree => "tree",
            Kind::Blob => "blob",
        }
    }
}

/// How a tree lists an entry: a file, an executable file, or a tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Mode {
    /// A regular file (`100644`).
    #[default]
    File,
    /// An executable file (`100755`).
    Executable,
    /// A folder (`40000`).
    Tree,
}

/// An author or committer: a name, an email address, the time in seconds since 1970
/// (UTC), and the time zone's offset from UTC in minutes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Person {
    /// The person's name; line breaks, angle brackets and NUL are dropped when written.
    pub name: String,
    /// The email address, cleaned like `name`.
    pub email: String,
    /// Seconds since 1970-01-01 UTC.
    pub time: i64,
    /// The time zone's offset from UTC in minutes.
    pub offset: i32,
}

/// A commit's contents. Its message ends with a line break when it has text.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Commit {
    /// The root tree.
    pub tree: Id,
    /// The parent commits; empty for the first commit of a branch.
    pub parents: Vec<Id>,
    /// Who wrote the change.
    pub author: Person,
    /// Who made the commit.
    pub committer: Person,
    /// The message, as written.
    pub message: String,
}

fn hex_into(out: &mut [u8], id: &Id) {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    for (i, &c) in id.0.iter().enumerate() {
        out[2 * i] = DIGITS[usize::from(c >> 4)];
        out[2 * i + 1] = DIGITS[usize::from(c & 15)];
    }
}

/// Parses an ID written in 40 hexadecimal digits (either case); `None` for anything else.
pub fn parse_id(text: &str) -> Option<Id> {
    let bytes = text.as_bytes();
    if bytes.len() != 40 {
        return None;
    }
    let mut id = Id::ZERO;
    for (i, slot) in id.0.iter_mut().enumerate() {
        *slot = digit(bytes[2 * i])? << 4 | digit(bytes[2 * i + 1])?;
    }
    Some(id)
}

pub(crate) fn digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Returns the ID of an object of `kind` with contents `data`.
pub fn object_id(kind: Kind, data: &[u8]) -> Id {
    let mut hash = start(kind, data.len() as u64);
    hash.update(data);
    Id(hash.finish())
}

/// Returns the ID of the blob holding the file at `path`, which is read a chunk at a
/// time, and the file's size.
///
/// Fails with [`ErrorKind::Io`] when the file cannot be read and [`ErrorKind::Changed`]
/// when its size changes while it is read.
pub fn file_id(path: &Path) -> Result<(Id, u64), Error> {
    let file = std::fs::File::open(path).map_err(|e| Error::unreadable(path, &e))?;
    let size = file.metadata().map_err(|e| Error::unreadable(path, &e))?.len();
    let mut hash = start(Kind::Blob, size);
    // The hasher is an `io::Write` sink: the file streams through it a chunk at a time.
    let mut reader = BufReader::with_capacity(pack::CHUNK, file);
    let read = std::io::copy(&mut reader, &mut hash).map_err(|e| Error::unreadable(path, &e))?;
    if read != size {
        return Err(Error::changed(path, "changed while it was read"));
    }
    Ok((Id(hash.finish()), read))
}

/// A SHA-1 hasher that has seen the header of an object of `kind` and `size` bytes.
pub(crate) fn start(kind: Kind, size: u64) -> Sha1 {
    let mut hash = Sha1::new();
    // Writing into the hasher cannot fail.
    let _ = write!(hash, "{} {size}\0", kind.name());
    hash
}

/// Returns the contents of the commit `c`, as Git writes it.
pub fn commit_bytes(c: &Commit) -> Vec<u8> {
    let mut out = Vec::new();
    write_commit(&mut out, c);
    out
}

/// Appends the contents of the commit `c` to `out`, as Git writes it; clear and reuse
/// `out` to write many commits without allocating.
pub fn write_commit(out: &mut Vec<u8>, c: &Commit) {
    let people = c.author.name.len() + c.author.email.len();
    let people = people + c.committer.name.len() + c.committer.email.len();
    out.reserve(46 + 48 * c.parents.len() + 2 * 40 + people + 1 + c.message.len());
    // Writing into a Vec cannot fail.
    let _ = writeln!(out, "tree {}", c.tree);
    for p in &c.parents {
        let _ = writeln!(out, "parent {p}");
    }
    person(out, "author", &c.author);
    person(out, "committer", &c.committer);
    out.push(b'\n');
    out.extend_from_slice(c.message.as_bytes());
}

fn person(out: &mut Vec<u8>, role: &str, p: &Person) {
    let sign = if p.offset >= 0 { '+' } else { '-' };
    let offset = p.offset.unsigned_abs();
    out.extend_from_slice(role.as_bytes());
    out.push(b' ');
    one_line(out, &p.name);
    out.extend_from_slice(b" <");
    one_line(out, &p.email);
    let _ = writeln!(out, "> {} {sign}{:02}{:02}", p.time, offset / 60, offset % 60);
}

/// Appends `s` without the characters that would break a commit's header: line breaks,
/// angle brackets and NUL.
fn one_line(out: &mut Vec<u8>, s: &str) {
    let bad = |c: &u8| matches!(c, b'\n' | b'\r' | b'<' | b'>' | b'\0');
    s.as_bytes().split(bad).for_each(|part| out.extend_from_slice(part));
}
