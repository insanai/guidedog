//! Sphinx makes most ids with its own make_id: the text keeps letter case, dots, and
//! underscores ("term-API", "mod.func", "module-os.path"); every other run of
//! characters becomes one hyphen; accents are dropped; leading digits, dots,
//! underscores, and hyphens and trailing hyphens are removed. Equations use Docutils'
//! make_id instead (`docutils_id`). Ids are unique within a document: when a made id is
//! taken or empty, Sphinx uses prefix-N (or idN) from a per-prefix serial number.

use std::fmt::Write as _;

use guidedoc::{id_fold, make_id, to_lower};

use crate::state::State;

/// Streams characters into an id.
struct IdWriter<'o> {
    out: &'o mut String,
    start: usize,
    hyphen: bool,
}

impl IdWriter<'_> {
    fn char(&mut self, c: u8) {
        let word = c.is_ascii_alphanumeric() || c == b'.' || c == b'_';
        if !word {
            self.hyphen = true;
            return;
        }
        let empty = self.out.len() == self.start;
        if empty && (c.is_ascii_digit() || c == b'.' || c == b'_') {
            return;
        }
        if self.hyphen && !empty {
            self.out.push('-');
        }
        self.hyphen = false;
        self.out.push(char::from(c));
    }

    fn write(&mut self, s: &str) {
        for r in s.chars() {
            if r.is_ascii() {
                self.char(r as u8);
                continue;
            }
            if r.is_whitespace() {
                self.hyphen = true;
                continue;
            }
            let lower = to_lower(r);
            let upper = lower != r;
            // Characters without an ASCII folding are dropped, as Sphinx's ASCII does.
            for c in id_fold(lower).bytes() {
                self.char(if upper { c.to_ascii_uppercase() } else { c });
            }
        }
    }
}

/// Appends Sphinx's make_id of `prefix-term` (of `term` alone when `prefix` is empty)
/// to `out`.
pub(crate) fn write_sphinx_id(out: &mut String, prefix: &str, term: &str) {
    let start = out.len();
    let mut w = IdWriter { out, start, hyphen: false };
    if !prefix.is_empty() {
        w.write(prefix);
        w.hyphen = true;
    }
    w.write(term);
}

/// Returns Sphinx's make_id of `prefix-term`.
pub(crate) fn sphinx_id(prefix: &str, term: &str) -> String {
    let mut out = String::with_capacity(prefix.len() + term.len() + 1);
    write_sphinx_id(&mut out, prefix, term);
    out
}

/// Returns Docutils' make_id of `prefix` + `s` (lowercase, hyphens), as Sphinx makes
/// equation ids.
pub(crate) fn docutils_id(prefix: &str, s: &str) -> String {
    let joined = format!("{prefix}{s}");
    let mut out = String::with_capacity(joined.len());
    make_id(&mut out, &joined);
    out
}

impl State {
    /// Returns the next serial number of `prefix`, starting at 0.
    pub(crate) fn serial(&mut self, prefix: &str) -> u32 {
        if let Some(e) = self.serials.iter_mut().find(|e| e.0 == prefix) {
            e.1 += 1;
            return e.1 - 1;
        }
        self.serials.push((prefix.to_owned(), 1));
        0
    }

    /// Tells whether `id` was already given in this document.
    pub(crate) fn id_taken(&self, id: &str) -> bool {
        self.ids.contains(id)
    }

    /// Records `id`, so later ids avoid it.
    pub(crate) fn note_id(&mut self, id: &str) {
        self.ids.insert(id.to_owned());
    }

    /// Sphinx's make_id(env, document, prefix, term): the made id, or prefix-N (idN
    /// without a prefix) when the made id is empty, only the prefix, or taken. The id is
    /// recorded as taken.
    pub(crate) fn make_object_id(&mut self, prefix: &str, term: &str) -> String {
        let id = sphinx_id(prefix, term);
        if !id.is_empty() && id != prefix && !self.id_taken(&id) {
            self.note_id(&id);
            return id;
        }
        let mut id = String::new();
        for _ in 0..1000 {
            id.clear();
            if prefix.is_empty() {
                id.push_str("id");
            } else {
                id.push_str(prefix);
                id.push('-');
            }
            let _ = write!(id, "{}", self.serial(prefix));
            if !self.id_taken(&id) {
                self.note_id(&id);
                return id;
            }
        }
        String::new()
    }

    /// Takes the next "index-N" id; the index directive, the :index: role, and the
    /// roles that make index entries share the counter.
    pub(crate) fn index_id(&mut self) -> String {
        let id = format!("index-{}", self.index_serial);
        self.index_serial += 1;
        self.note_id(&id);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphinx_ids_keep_case_dots_and_underscores() {
        assert_eq!(sphinx_id("term", "API"), "term-API");
        assert_eq!(sphinx_id("module", "os.path"), "module-os.path");
        assert_eq!(sphinx_id("", "1 a b--"), "a-b");
        assert_eq!(sphinx_id("", "Ünïcode"), "Unicode");
    }

    #[test]
    fn taken_ids_get_serial_numbers() {
        let mut s = State::default();
        assert_eq!(s.make_object_id("term", "x"), "term-x");
        assert_eq!(s.make_object_id("term", "x"), "term-0");
        assert_eq!(s.make_object_id("", ""), "id0");
        assert_eq!(s.make_object_id("term", ""), "term-1");
    }
}
