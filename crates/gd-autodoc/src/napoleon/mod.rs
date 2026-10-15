//! `sphinx.ext.napoleon`: Google-style and NumPy-style docstring sections become the
//! reStructuredText fields autodoc renders.
//!
//! ```text
//! Args:                          :param width: The width.
//!     width (float): The width.  :type width: float
//! Returns:                  =>   :returns: The product.
//!     float: The product.        :rtype: float
//! ```
//!
//! The conversion is a pure function of the docstring's lines and the settings. It
//! works line by line and records, for each line it writes, the docstring line it came
//! from. What it cannot read is a [`NapoleonNote`]; from the line of the first note on,
//! the docstring is kept as written.

mod docstring;
mod numpy;
mod patterns;
mod sections;
mod types;

#[cfg(test)]
mod tests;

use docstring::Parser;
use sections::Section;

/// `sphinx.ext.napoleon`'s settings, with Sphinx's names less `napoleon_`; [`Default`]
/// gives Sphinx 9.1's defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Napoleon {
    /// `napoleon_google_docstring`: convert Google-style sections.
    pub google_docstring: bool,
    /// `napoleon_numpy_docstring`: convert NumPy-style sections.
    pub numpy_docstring: bool,
    /// `napoleon_include_init_with_doc`: document `__init__` when it has a docstring.
    pub include_init_with_doc: bool,
    /// `napoleon_include_private_with_doc`: document `_private` members with docstrings.
    pub include_private_with_doc: bool,
    /// `napoleon_include_special_with_doc`: document `__special__` members with
    /// docstrings.
    pub include_special_with_doc: bool,
    /// `napoleon_use_admonition_for_examples`.
    pub use_admonition_for_examples: bool,
    /// `napoleon_use_admonition_for_notes`.
    pub use_admonition_for_notes: bool,
    /// `napoleon_use_admonition_for_references`.
    pub use_admonition_for_references: bool,
    /// `napoleon_use_ivar`: attributes as `:ivar:` fields.
    pub use_ivar: bool,
    /// `napoleon_use_param`: parameters as `:param:` fields.
    pub use_param: bool,
    /// `napoleon_use_rtype`: the return type as an `:rtype:` field.
    pub use_rtype: bool,
    /// `napoleon_use_keyword`: keyword arguments as `:keyword:` fields.
    pub use_keyword: bool,
    /// `napoleon_preprocess_types`: link the names in type specifications.
    pub preprocess_types: bool,
    /// `napoleon_type_aliases`: a type's name and what it links to.
    pub type_aliases: Vec<(String, String)>,
    /// `napoleon_custom_sections`, in the order written.
    pub custom_sections: Vec<NapoleonSection>,
    /// `napoleon_attr_annotations`: annotations give documented attributes their type.
    pub attr_annotations: bool,
}

impl Default for Napoleon {
    fn default() -> Napoleon {
        Napoleon {
            google_docstring: true,
            numpy_docstring: true,
            include_init_with_doc: false,
            include_private_with_doc: false,
            include_special_with_doc: false,
            use_admonition_for_examples: false,
            use_admonition_for_notes: false,
            use_admonition_for_references: false,
            use_ivar: false,
            use_param: true,
            use_rtype: true,
            use_keyword: true,
            preprocess_types: false,
            type_aliases: Vec::new(),
            custom_sections: Vec::new(),
            attr_annotations: true,
        }
    }
}

/// One `napoleon_custom_sections` entry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NapoleonSection {
    /// The header as docstrings write it, such as `Side Effects`.
    pub name: String,
    /// The section it behaves like: another section's name, `params_style` or
    /// `returns_style`; empty for a rubric and its text.
    pub like: String,
}

/// The two docstring styles napoleon reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DocstringStyle {
    /// `Args:` headers with indented bodies.
    #[default]
    Google,
    /// `Parameters` headers underlined with `----------`.
    Numpy,
}

/// The object a docstring belongs to, as napoleon sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NapoleonContext<'a> {
    /// autodoc's object type: `module`, `class`, `method`, `attribute`, ...; `""` or
    /// `object` for none in particular.
    pub what: &'a str,
    /// The object's full name; an attribute's docstring with a name may start with
    /// its type (`int: The count.`).
    pub name: &'a str,
    /// The annotations of a module's or class's attributes, by name, when the object
    /// is known; they give an `Attributes` entry without a type its type.
    pub annotations: Option<&'a [(String, String)]>,
    /// The directive has `:no-index:`, which the attributes and methods it writes
    /// get too.
    pub no_index: bool,
    /// Sphinx's `strip_signature_backslash`: a name ending in `_` is escaped.
    pub strip_signature_backslash: bool,
}

/// A part of a docstring napoleon could not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NapoleonNote {
    /// `napoleon.section`, `napoleon.field` or `napoleon.type`.
    pub code: &'static str,
    /// The docstring line it is about, counted from 0.
    pub line: usize,
    /// What is wrong, as a sentence.
    pub text: String,
}

/// A converted docstring.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NapoleonOutput {
    /// The reStructuredText lines.
    pub lines: Vec<String>,
    /// For each line, the docstring line (counted from 0) it came from.
    pub origins: Vec<usize>,
    /// What could not be read; from the first note's line on, the docstring is kept
    /// as written.
    pub notes: Vec<NapoleonNote>,
}

/// What every parser of one conversion shares.
pub(crate) struct Shared<'a> {
    pub(crate) settings: &'a Napoleon,
    pub(crate) sections: Vec<(String, Section)>,
    pub(crate) strip_backslash: bool,
}

/// The custom sections of `settings` whose `like` names no section napoleon knows; a
/// docstring reads them as rubrics.
pub fn unknown_custom_sections(settings: &Napoleon) -> Vec<String> {
    sections::section_table(&settings.custom_sections).1
}

/// Converts `lines` in one style, as `GoogleDocstring(lines).lines()` or
/// `NumpyDocstring(lines).lines()` does, for the object `ctx` describes. When a line
/// cannot be read, the lines before it are converted and the rest kept as written.
pub fn convert_docstring(
    style: DocstringStyle,
    lines: &[String],
    settings: &Napoleon,
    ctx: &NapoleonContext<'_>,
) -> NapoleonOutput {
    let (sections, _) = sections::section_table(&settings.custom_sections);
    let shared = Shared { settings, sections, strip_backslash: ctx.strip_signature_backslash };
    let (out, origins, notes) = run(style, lines, &shared, ctx);
    let Some(cut) = notes.iter().map(|n| n.line).min().filter(|&c| c < lines.len()) else {
        return NapoleonOutput { lines: out, origins, notes };
    };
    let notes = notes.into_iter().filter(|n| n.line == cut).collect();
    let (mut head, mut head_origins, _) = run(style, &lines[..cut], &shared, ctx);
    let tail = &lines[cut..];
    if head.last().is_some_and(|l| !l.is_empty()) && tail.first().is_some_and(|l| !l.is_empty()) {
        head.push(String::new());
        head_origins.push(cut);
    }
    head.extend(tail.iter().map(|l| l.trim_end().to_owned()));
    head_origins.extend(cut..lines.len());
    NapoleonOutput { lines: head, origins: head_origins, notes }
}

/// Converts with one parser and returns its lines, origins and notes.
fn run(
    style: DocstringStyle,
    lines: &[String],
    shared: &Shared<'_>,
    ctx: &NapoleonContext<'_>,
) -> (Vec<String>, Vec<usize>, Vec<NapoleonNote>) {
    let mut p = Parser::new(style, shared, ctx, lines);
    p.parse();
    p.finish()
}

/// `_process_docstring`: converts NumPy sections, then Google sections, as the
/// settings ask, for the object `ctx` describes.
pub fn napoleon_docstring(
    lines: &[String],
    settings: &Napoleon,
    ctx: &NapoleonContext<'_>,
) -> NapoleonOutput {
    let identity = NapoleonOutput {
        lines: lines.to_vec(),
        origins: (0..lines.len()).collect(),
        notes: Vec::new(),
    };
    let mut out = identity;
    for (on, style) in [
        (settings.numpy_docstring, DocstringStyle::Numpy),
        (settings.google_docstring, DocstringStyle::Google),
    ] {
        if !on {
            continue;
        }
        let pass = convert_docstring(style, &out.lines, settings, ctx);
        let origins = pass.origins.iter().map(|&o| out.origins.get(o).copied().unwrap_or(o));
        let notes = pass.notes.into_iter().map(|n| NapoleonNote {
            line: out.origins.get(n.line).copied().unwrap_or(n.line),
            ..n
        });
        out.notes.extend(notes);
        out.origins = origins.collect();
        out.lines = pass.lines;
    }
    out
}

/// Whether napoleon's member filter documents a member autodoc would skip: an
/// `__init__`, `__special__` or `_private` member of a module or class, that has a
/// docstring, when the matching `napoleon_include_*_with_doc` setting is on.
/// `parent_what` is the parent's autodoc object type.
pub fn includes_member(settings: &Napoleon, parent_what: &str, name: &str, has_doc: bool) -> bool {
    let is_member = matches!(parent_what, "class" | "exception" | "module");
    if name == "__weakref__" || !has_doc || !is_member {
        return false;
    }
    let is_init = name == "__init__";
    let is_special = !is_init && name.len() > 4 && name.starts_with("__") && name.ends_with("__");
    let is_private = !is_init && !is_special && name.starts_with('_');
    (is_special && settings.include_special_with_doc)
        || (is_private && settings.include_private_with_doc)
        || (is_init && settings.include_init_with_doc)
}
