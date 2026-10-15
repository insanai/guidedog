//! Cross-reference resolution without the document tree (the resolution half of
//! xref.odin, odin_xref.odin, cpp_lookup.odin, intersphinx_resolve.odin, and the new
//! rust_xref), and the answers that verify a page's trace (lookups.odin).
//!
//! gd-link walks a document's tree and asks [`resolve_xref`] for each `pending_xref`;
//! every table lookup made on the way is noted in the page's `Trace`, and the next
//! build checks each with [`lookup_answer`].

use std::fmt;

use gd_env::{Asset, DocId, OutputFormat, Trace};
use gd_report::{Report, Severity};

use crate::lookups::Look;
use crate::view::BuildView;

pub use crate::lookups::{lookup_answer, role_takes};

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
    /// The reference has an explicit title (`refexplicit`).
    pub explicit: bool,
    /// The reference's content as plain text: a `:numref:` title's template, and what an
    /// inventory link shows when its entry has no display name.
    pub content: String,
    /// `:external+name:`: the one inventory to search; empty for all.
    pub inventory: String,
    /// The rest of the context the reader left on the reference, as (name, value)
    /// pairs: `js:module`, `js:object`, `c:parent`, `std:program`, `odin:package`,
    /// `odin:type`, `cpp:name`, `cpp:parent`, ... An `any` reference carries every
    /// domain's. `py:module`, `py:class` and `rust:module` may be given here or in
    /// `module` and `class`.
    pub context: Vec<(String, String)>,
}

impl XrefQuery {
    /// The context value `key` (such as `"js:module"`), borrowed; empty when the
    /// reference has none. `<domain>:module` and `<domain>:class` of the reference's own
    /// domain fall back to `module` and `class`.
    pub fn context(&self, key: &str) -> &str {
        if let Some((_, value)) = self.context.iter().find(|(k, _)| k == key) {
            return value;
        }
        let Some((domain, name)) = key.split_once(':') else { return "" };
        let own = self.reftype.split_once(':').is_some_and(|(d, _)| d == domain);
        match name {
            "module" if own => &self.module,
            "class" if own => &self.class,
            _ => "",
        }
    }

    /// Whether the context value `key` is set and is not `"0"`, as a flag.
    pub fn flag(&self, key: &str) -> bool {
        let value = self.context(key);
        !value.is_empty() && value != "0"
    }
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
    /// A download the page needs copied into the output.
    pub asset: Option<Asset>,
    /// A warning about a reference that still resolved: a `:numref:` to an item without
    /// a number, a heading anchor not found, a C++ role for another kind of
    /// declaration. gd-link reports it at the reference.
    pub warning: Option<Box<Unresolved>>,
}

/// Why a reference did not resolve, Elm style: what was not found, a hint, and the
/// closest name when one is near enough to be a typo.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Unresolved {
    /// Whether the build warns (Sphinx's `warn_dangling`, `nitpicky` and the
    /// `nitpick_ignore` lists decide).
    pub warn: bool,
    /// The warning's code, such as `build.xref.unresolved`.
    pub code: String,
    /// The category `suppress_warnings` silences, such as `ref.ref`.
    pub category: String,
    /// The report's title, such as `REFERENCE TARGET NOT FOUND`.
    pub title: String,
    /// What was not found.
    pub message: String,
    /// How to fix it.
    pub hint: String,
    /// "Did you mean ...?": the closest name of the right kind.
    pub suggestion: Option<String>,
}

impl Unresolved {
    /// The warning as a report on `path` (the document's path below the source folder),
    /// with no position yet: the caller places it at the reference.
    pub fn report(&self, path: &str) -> Report {
        let mut report = Report::error(&self.code, &self.title, &self.message).hint(&self.hint);
        if let Some(name) = &self.suggestion {
            report.hints.push(format!("Did you mean `{name}`?"));
        }
        Report {
            severity: Severity::Warning,
            path: path.to_string(),
            category: self.category.clone(),
            ..report
        }
    }

    pub(crate) fn warning(parts: [&str; 5]) -> Box<Unresolved> {
        let [code, title, category, message, hint] = parts;
        Box::new(Unresolved {
            warn: true,
            code: code.into(),
            category: category.into(),
            title: title.into(),
            message: message.into(),
            hint: hint.into(),
            suggestion: None,
        })
    }
}

impl fmt::Display for Unresolved {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} Hint: {}", self.message, self.hint)?;
        if let Some(name) = &self.suggestion {
            write!(f, " Did you mean `{name}`?")?;
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
    if query.external {
        return crate::external::resolve_external(view, query, true)
            .ok_or_else(|| crate::external::external_missing(view, query));
    }
    let mut look = Look { view, query, trace, warning: None };
    if let Some(mut found) = look.resolve_local() {
        if found.warning.is_none() {
            found.warning = look.warning.take();
        }
        return Ok(found);
    }
    if let Some(found) = crate::external::resolve_external(view, query, false) {
        return Ok(found);
    }
    Err(crate::suggest::unresolved(view, query))
}
