//! Where `load_template`, `extends`, `include` and `import` read templates from.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;

/// A procedure returning the source of a template by name, or None.
pub type LoadFn = Rc<dyn Fn(&str) -> Option<String>>;

/// A source of templates by name.
#[derive(Clone)]
pub enum Loader {
    /// Templates held in memory, by name.
    Memory(HashMap<String, String>),
    /// The first of these directories that holds the name; names use "/" and may not
    /// climb out of a directory with "..".
    Filesystem(Vec<PathBuf>),
    /// Your own: returns the source of a name, or None when there is none.
    Custom(LoadFn),
}

impl Loader {
    /// A loader serving the given (name, source) pairs.
    pub fn memory<N: Into<String>, S: Into<String>>(
        sources: impl IntoIterator<Item = (N, S)>,
    ) -> Loader {
        Loader::Memory(sources.into_iter().map(|(n, s)| (n.into(), s.into())).collect())
    }

    /// A loader reading from directories, searched in order.
    pub fn filesystem<P: Into<PathBuf>>(directories: impl IntoIterator<Item = P>) -> Loader {
        Loader::Filesystem(directories.into_iter().map(Into::into).collect())
    }

    /// A loader calling f with each name.
    pub fn custom(f: impl Fn(&str) -> Option<String> + 'static) -> Loader {
        Loader::Custom(Rc::new(f))
    }

    /// The source of the template called name, or None when the loader has none.
    pub fn load(&self, name: &str) -> Option<String> {
        match self {
            Loader::Memory(sources) => sources.get(name).cloned(),
            Loader::Filesystem(directories) => load_file(directories, name),
            Loader::Custom(f) => f(name),
        }
    }
}

impl fmt::Debug for Loader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Loader::Memory(sources) => {
                let mut names: Vec<&String> = sources.keys().collect();
                names.sort();
                f.debug_tuple("Memory").field(&names).finish()
            }
            Loader::Filesystem(directories) => {
                f.debug_tuple("Filesystem").field(directories).finish()
            }
            Loader::Custom(_) => f.write_str("Custom(..)"),
        }
    }
}

fn load_file(directories: &[PathBuf], name: &str) -> Option<String> {
    let pieces = split_template_path(name)?;
    directories.iter().find_map(|dir| {
        let path = pieces.iter().fold(dir.clone(), |path, piece| path.join(piece));
        if path.is_file() { std::fs::read_to_string(path).ok() } else { None }
    })
}

/// Splits "a/b.html" into pieces, refusing ".." and backslashes; None for a name that
/// names nothing inside the directory.
fn split_template_path(name: &str) -> Option<Vec<&str>> {
    let mut pieces = Vec::new();
    for piece in name.split('/') {
        if piece == ".." || piece.contains('\\') {
            return None;
        }
        if !piece.is_empty() && piece != "." {
            pieces.push(piece);
        }
    }
    (!pieces.is_empty()).then_some(pieces)
}
