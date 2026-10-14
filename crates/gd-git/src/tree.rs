//! Tree objects built from a list of files.

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::{Error, ErrorKind, Id, Kind, Mode, object_id};

/// A file a tree holds: its path from the tree's root with `/` between folders, its mode
/// ([`Mode::File`] or [`Mode::Executable`]) and its blob's ID.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct File {
    /// The path from the root, such as `_static/site.css`.
    pub path: String,
    /// `File` or `Executable`; `Tree` is refused.
    pub mode: Mode,
    /// The blob's ID, from [`crate::object_id`] or [`crate::file_id`].
    pub id: Id,
}

/// A tree object: the folder it is (`""` for the root), its ID and its contents.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tree {
    /// The folder's path from the root; `""` for the root.
    pub path: String,
    /// The tree's object ID.
    pub id: Id,
    /// The tree's contents, as a pack stores it.
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
struct Entry<'a> {
    name: &'a str,
    mode: Mode,
    id: Id,
}

/// Builds the tree objects that hold `files`: one for each folder, every folder before
/// the folder that holds it, the root last.
///
/// Fails with [`ErrorKind::InvalidPath`] for an empty list, a path Git does not accept
/// (see [`is_valid_path`]), two files with one path, or a path that is both a file and a
/// folder.
pub fn build_trees(files: &[File]) -> Result<Vec<Tree>, Error> {
    if files.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidPath,
            "A tree needs at least one file.",
            "Build the site before publishing it; an empty site cannot be committed.",
        ));
    }
    // Keys and names borrow the files' paths: nothing is copied until the output.
    let mut folders: HashMap<&str, Vec<Entry<'_>>> = HashMap::with_capacity(files.len());
    let mut names: HashMap<&str, Mode> = HashMap::with_capacity(files.len() * 2);
    for f in files {
        add_file(&mut folders, &mut names, f)?;
    }
    let mut order: Vec<(&str, Vec<Entry<'_>>)> = folders.into_iter().collect();
    // The deepest folders first, so that a folder's trees are made before it is.
    order.sort_by(|(a, _), (b, _)| depth(b).cmp(&depth(a)).then_with(|| a.cmp(b)));
    let mut ids: HashMap<&str, Id> = HashMap::with_capacity(order.len());
    let mut out = Vec::with_capacity(order.len());
    let mut path = String::new();
    for (folder, mut entries) in order {
        for e in entries.iter_mut().filter(|e| e.mode == Mode::Tree) {
            path.clear();
            if !folder.is_empty() {
                path.push_str(folder);
                path.push('/');
            }
            path.push_str(e.name);
            e.id = ids.get(path.as_str()).copied().unwrap_or(Id::ZERO);
        }
        let data = tree_bytes(&mut entries);
        let id = object_id(Kind::Tree, &data);
        ids.insert(folder, id);
        out.push(Tree { path: folder.to_string(), id, data });
    }
    Ok(out)
}

fn add_file<'a>(
    folders: &mut HashMap<&'a str, Vec<Entry<'a>>>,
    names: &mut HashMap<&'a str, Mode>,
    f: &'a File,
) -> Result<(), Error> {
    if !is_valid_path(&f.path) || f.mode == Mode::Tree {
        return Err(invalid(format!("{:?} is not a path Git can hold.", f.path)));
    }
    if names.contains_key(f.path.as_str()) {
        return Err(invalid(format!("{:?} is listed twice, or is also a folder.", f.path)));
    }
    names.insert(&f.path, f.mode);
    let (mut folder, name) = split(&f.path);
    folders.entry(folder).or_default().push(Entry { name, mode: f.mode, id: f.id });
    while !folder.is_empty() {
        match names.get(folder) {
            Some(Mode::Tree) => break,
            Some(_) => {
                return Err(invalid(format!("{folder:?} is both a file and a folder.")));
            }
            None => {}
        }
        names.insert(folder, Mode::Tree);
        let (parent, base) = split(folder);
        let entry = Entry { name: base, mode: Mode::Tree, id: Id::ZERO };
        folders.entry(parent).or_default().push(entry);
        folder = parent;
    }
    Ok(())
}

fn invalid(message: String) -> Error {
    Error::new(
        ErrorKind::InvalidPath,
        message,
        "Rename the file: Git refuses empty, `.`, `..` and `.git` parts, and NUL bytes.",
    )
}

fn depth(folder: &str) -> usize {
    folder.matches('/').count() + usize::from(!folder.is_empty())
}

/// The contents of a tree listing `entries`, in Git's order: by name, with a folder's
/// name compared as if it ended with `/`.
fn tree_bytes(entries: &mut [Entry]) -> Vec<u8> {
    entries.sort_by(compare);
    let size: usize =
        entries.iter().map(|e| mode_text(e.mode).len() + 1 + e.name.len() + 1 + 20).sum();
    let mut bytes = Vec::with_capacity(size);
    for e in entries.iter() {
        bytes.extend_from_slice(mode_text(e.mode).as_bytes());
        bytes.push(b' ');
        bytes.extend_from_slice(e.name.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&e.id.0);
    }
    bytes
}

fn compare(a: &Entry, b: &Entry) -> Ordering {
    // Git compares names as if a folder's ended with `/`; the end of any name is decisive
    // because names hold no NUL and no `/`.
    sort_key(a).cmp(sort_key(b))
}

fn sort_key<'a>(e: &Entry<'a>) -> impl Iterator<Item = u8> + 'a {
    let end = if e.mode == Mode::Tree { b'/' } else { 0 };
    e.name.bytes().chain(std::iter::once(end))
}

fn mode_text(mode: Mode) -> &'static str {
    match mode {
        Mode::File => "100644",
        Mode::Executable => "100755",
        Mode::Tree => "40000",
    }
}

fn split(path: &str) -> (&str, &str) {
    match path.rfind('/') {
        Some(i) => (&path[..i], &path[i + 1..]),
        None => ("", path),
    }
}

/// Tells whether Git accepts `path` as a file's path in a tree: not empty, no NUL, and no
/// empty, `.`, `..` or `.git` (any case) part.
pub fn is_valid_path(path: &str) -> bool {
    if path.is_empty() || path.contains('\0') {
        return false;
    }
    path.split('/').all(|part| {
        !(part.is_empty() || part == "." || part == ".." || part.eq_ignore_ascii_case(".git"))
    })
}
