//! Cross-reference resolution without the document tree (the resolution half of
//! xref.odin, odin_xref.odin, cpp_lookup.odin, intersphinx_resolve.odin, and the new
//! rust_xref), and the answers that verify a page's trace (lookups.odin).
//!
//! gd-link walks a document's tree and asks [`resolve_xref`] for each `pending_xref`;
//! every table lookup made on the way is noted in the page's `Trace`, and the next
//! build checks each with [`lookup_answer`].

use std::fmt;

use gd_env::{Digest, DocId, Lookup, OutputFormat, Trace};

use crate::view::BuildView;

/// One reference to resolve, as the reader left it on a `pending_xref` node.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct XrefQuery {
    /// The document the reference is in.
    pub from: DocId,
    /// The role: `ref`, `doc`, `numref`, `term`, `py:func`, `rust:struct`, ...
    pub reftype: String,
    /// The target as written (`~` and a leading `.` kept: they change the search).
    pub target: String,
    /// The current module (`py:module`, `rust:module`), for relative names.
    pub module: String,
    /// The current class, for Python methods and attributes.
    pub class: String,
    /// The target started with `.`: search by suffix, as Sphinx's searchmode 1.
    pub refspecific: bool,
    /// A Markdown link's heading anchor (`page.md#slug`); empty for none.
    pub slug: String,
    /// The reference was `:external:`: only the inventories are searched.
    pub external: bool,
    /// The format the page is linked for: URLs in HTML and text, anchors in the book.
    pub format: OutputFormat,
}

/// Where a reference points, and what it shows when it has no explicit title.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Resolved {
    /// The document it points into; `None` for an external or generated target.
    pub doc: Option<DocId>,
    /// The id on that page; empty for the page itself.
    pub anchor: String,
    /// The text to show when the reference has no explicit title.
    pub title: String,
    /// A URL, set instead of `doc` for downloads, generated pages and inventories.
    pub url: String,
    /// Resolved through another project's inventory: the link's tooltip, such as
    /// "(in Python v3.14)"; empty otherwise.
    pub external: String,
    /// The output has no page to link to: the title stands alone.
    pub no_link: bool,
    /// Show the reference's own content rather than `title`.
    pub keep_content: bool,
}

/// Why a reference did not resolve, Elm style: what was not found, a hint, and the
/// closest name when one is near enough to be a typo.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Unresolved {
    /// Whether the build warns (Sphinx's `warn_dangling`, `nitpicky` and the
    /// `nitpick_ignore` lists decide).
    pub warn: bool,
    /// The warning's code, such as `build.xref.missing`.
    pub code: String,
    /// The category `suppress_warnings` silences, such as `ref.ref`.
    pub category: String,
    /// What was not found.
    pub message: String,
    /// How to fix it.
    pub hint: String,
    /// "Did you mean ...?": the closest name of the right kind.
    pub suggestion: Option<String>,
}

impl fmt::Display for Unresolved {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} Hint: {}", self.message, self.hint)?;
        if let Some(name) = &self.suggestion {
            write!(f, " Did you mean {name}?")?;
        }
        Ok(())
    }
}

impl std::error::Error for Unresolved {}

/// Resolves `query` against the project tables, then the inventories, noting every
/// lookup in `trace`. Roles: `doc`, `ref`, `numref`, `eq`/`math:numref`, `term`,
/// `download`, `option`, `keyword`, `token`, `any`, declared object types, and the
/// domains (`py`, `c`, `cpp`, `js`, `std`, `odin`, `rust`). Rust references
/// (`:rust:struct:`, `:rust:fn:`, ...) look up the full `::` path, the current module
/// first; `~` shows the last component and a leading `.` searches by suffix, as Python
/// references do; `rust:any` falls back to the inventories. The failure is boxed, as
/// it is the rare path and carries several texts.
pub fn resolve_xref(
    view: &BuildView<'_>,
    query: &XrefQuery,
    trace: &mut Trace,
) -> Result<Resolved, Box<Unresolved>> {
    let _ = (view, query, trace);
    todo!("wave 1: port xref.odin resolution, odin_xref, cpp_lookup, intersphinx_resolve, rust")
}

/// Answers a lookup the page of `page` (a docname) made, against this build's tables
/// and folders, as the digest of the answer: the target's document, anchor, title,
/// kind, type, number and key for a name; the title for a document; whether a file
/// exists; the file an image resolves to; whether there is a module index; the C++
/// tree's digest. A missing target has its own answer. `Generation::reuse` compares it
/// with the digest stored when the page was written.
pub fn lookup_answer(view: &BuildView<'_>, page: &str, lookup: &Lookup) -> Digest {
    let _ = (view, page, lookup);
    todo!("wave 1: port lookup_value as digests")
}

/// The roles a domain's object type may be named by (Sphinx's `object_types`), for
/// suffix searches and `any`: `role_takes("py:meth", "method")` is true. Roles of other
/// domains, and an empty role, take any type.
pub fn role_takes(role: &str, objtype: &str) -> bool {
    let _ = (role, objtype);
    todo!("wave 1: port role_takes, odin_role_takes, cpp_role_takes and the rust roles")
}
