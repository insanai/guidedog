//! Lookups and traces: what a page looked up while it was linked.
//!
//! A page's references look things up in the project's tables and folders: labels,
//! terms, objects, documents, files and images. Each lookup is noted in the page's
//! `Trace`. A later build answers each again (`gd_resolve::lookup_answer`): when every
//! answer's digest is the one stored, the page would come out the same and is kept;
//! when one differs (a label moved, an object was renamed, an image appeared) it is
//! written again. This is a verifying trace with dynamic dependencies, exact where a
//! digest of whole tables would only guess (docs/manual/internals/incremental.rst).

use std::collections::HashSet;
use std::fmt;

/// What a lookup searched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LookupKind {
    /// A label, by normalized name.
    Label,
    /// A glossary term, by lowercased name.
    Term,
    /// An object, by its table key (`py:mod.func`, `std:option:rm -r`).
    Object,
    /// An object found by the end of its name: the key is `prefix\0suffix\0role`.
    Suffix,
    /// A document, by docname.
    Doc,
    /// Whether a file exists, by path.
    File,
    /// The file an image names, by the image's URI as written in the page.
    Image,
    /// Whether the project has a module index; the key is empty.
    Modules,
    /// A C++ name, looked up in the whole symbol tree; the key is empty.
    Cpp,
    /// A template name requested by the HTML layout, answered by gd-site.
    Template,
    /// A chronological collection query, encoded by `PostlistSpec`.
    Postlist,
    /// A website-relative cross-mount file, validated before whole-site publication.
    Mount,
    /// An HTML navigation context value, native template call or body toctree.
    Navigation,
}

impl LookupKind {
    /// The letter a lookup of this kind is saved with: `L`, `T`, `O`, `S`, `D`, `F`,
    /// `I`, `M`, `C`. Saved manifests depend on these letters, so they never change.
    pub fn letter(self) -> char {
        match self {
            LookupKind::Label => 'L',
            LookupKind::Term => 'T',
            LookupKind::Object => 'O',
            LookupKind::Suffix => 'S',
            LookupKind::Doc => 'D',
            LookupKind::File => 'F',
            LookupKind::Image => 'I',
            LookupKind::Modules => 'M',
            LookupKind::Cpp => 'C',
            LookupKind::Template => 'J',
            LookupKind::Postlist => 'P',
            LookupKind::Mount => 'U',
            LookupKind::Navigation => 'N',
        }
    }

    /// The kind saved as `letter`.
    pub fn from_letter(letter: char) -> Option<LookupKind> {
        Some(match letter {
            'L' => LookupKind::Label,
            'T' => LookupKind::Term,
            'O' => LookupKind::Object,
            'S' => LookupKind::Suffix,
            'D' => LookupKind::Doc,
            'F' => LookupKind::File,
            'I' => LookupKind::Image,
            'M' => LookupKind::Modules,
            'C' => LookupKind::Cpp,
            'J' => LookupKind::Template,
            'P' => LookupKind::Postlist,
            'U' => LookupKind::Mount,
            'N' => LookupKind::Navigation,
            _ => return None,
        })
    }
}

/// One lookup a page made: what it searched, and for what.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Lookup {
    /// What it searched.
    pub kind: LookupKind,
    /// The name, path, or URI it searched for; see [`LookupKind`].
    pub key: String,
}

impl Lookup {
    /// A lookup of `kind` for `key`.
    pub fn new(kind: LookupKind, key: &str) -> Lookup {
        Lookup { kind, key: key.to_string() }
    }

    /// A suffix lookup: an object of a domain (`prefix`, such as `py:`) whose name ends
    /// with `suffix`, of a type `role` may name (`""`: any).
    pub fn suffix(prefix: &str, suffix: &str, role: &str) -> Lookup {
        Lookup { kind: LookupKind::Suffix, key: format!("{prefix}\0{suffix}\0{role}") }
    }

    /// A suffix lookup's parts: (prefix, suffix, role); `None` for another kind.
    pub fn suffix_parts(&self) -> Option<(&str, &str, &str)> {
        if self.kind != LookupKind::Suffix {
            return None;
        }
        let mut parts = self.key.splitn(3, '\0');
        Some((parts.next()?, parts.next().unwrap_or(""), parts.next().unwrap_or("")))
    }

    /// The lookup as one string, its kind's letter and its key, for the manifest.
    pub fn encode(&self) -> String {
        let mut text = String::with_capacity(1 + self.key.len());
        text.push(self.kind.letter());
        text.push_str(&self.key);
        text
    }

    /// Reads a lookup [`Lookup::encode`] wrote.
    pub fn decode(text: &str) -> Option<Lookup> {
        let mut chars = text.chars();
        let kind = LookupKind::from_letter(chars.next()?)?;
        Some(Lookup { kind, key: chars.as_str().to_string() })
    }
}

impl fmt::Display for Lookup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind.letter(), self.key.replace('\0', "|"))
    }
}

/// The lookups one page made, each once, in the order first made.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Trace {
    lookups: Vec<Lookup>,
    seen: HashSet<Lookup>,
}

impl Trace {
    /// An empty trace.
    pub fn new() -> Trace {
        Trace::default()
    }

    /// Notes a lookup; one already noted is not noted again.
    pub fn note(&mut self, lookup: Lookup) {
        if self.seen.insert(lookup.clone()) {
            self.lookups.push(lookup);
        }
    }

    /// Notes a lookup of `kind` for `key`.
    pub fn note_key(&mut self, kind: LookupKind, key: &str) {
        self.note(Lookup::new(kind, key));
    }

    /// The lookups, in the order first made.
    pub fn lookups(&self) -> &[Lookup] {
        &self.lookups
    }

    /// How many lookups were noted.
    pub fn len(&self) -> usize {
        self.lookups.len()
    }

    /// Whether nothing was looked up.
    pub fn is_empty(&self) -> bool {
        self.lookups.is_empty()
    }

    /// Empties the trace, keeping its memory for the next page.
    pub fn clear(&mut self) {
        self.lookups.clear();
        self.seen.clear();
    }

    /// The lookups, in the order first made.
    pub fn into_lookups(self) -> Vec<Lookup> {
        self.lookups
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traces_note_each_lookup_once_and_round_trip() {
        let mut trace = Trace::new();
        trace.note_key(LookupKind::Label, "intro");
        trace.note(Lookup::suffix("py:", ".open", "py:func"));
        trace.note_key(LookupKind::Label, "intro");
        assert_eq!(trace.len(), 2);
        for lookup in trace.lookups() {
            assert_eq!(Lookup::decode(&lookup.encode()).as_ref(), Some(lookup));
        }
        assert_eq!(trace.lookups()[1].suffix_parts(), Some(("py:", ".open", "py:func")));
        assert_eq!(Lookup::decode("Xabc"), None);
    }
}
