//! What a caller controls: limits, policy, read and render options, the request, and the
//! caller-side interfaces (resource provider, highlighter, passes, cancellation).

use std::any::Any;
use std::fmt;
use std::sync::Arc;

use crate::builder::Builder;
use crate::document::Document;
use crate::emit::Emitter;
use crate::stack::DEFAULT_STACK_BYTES;
use crate::types::{AdmonitionKind, SourceSpan};

/// Bounds on the work and memory of one conversion. Reaching one is a `Limit` or
/// `Capacity` result, never a panic. `Default` gives `default_limits()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Nested blocks and inline markup.
    pub max_depth: usize,
    /// Nodes in one document.
    pub max_nodes: usize,
    /// Reader and resolution steps (`Builder::spend`).
    pub max_work: u64,
    /// Nested includes.
    pub max_include_depth: usize,
    /// Bytes of included sources in one conversion.
    pub max_included_bytes: usize,
    /// Substitution nesting.
    pub max_expansion: usize,
    /// Machine stack a recursive reader or renderer may use (`StackGuard`).
    pub stack_bytes: usize,
    /// Bytes in a snapshot's text pool.
    pub max_text_bytes: usize,
    /// Diagnostics stored; later ones are counted as suppressed.
    pub max_reports: usize,
    /// Bytes of rendered output.
    pub max_output_bytes: usize,
}

/// Returns the documented limits: depth 256, a million nodes, 2^32 work steps, includes
/// 8 deep and 32 MiB in total, expansion 16, 512 KiB of stack, 256 MiB of text, 256
/// reports, and 1 GiB of output.
pub fn default_limits() -> Limits {
    Limits {
        max_depth: 256,
        max_nodes: 1_000_000,
        max_work: 1 << 32,
        max_include_depth: 8,
        max_included_bytes: 32 << 20,
        max_expansion: 16,
        stack_bytes: DEFAULT_STACK_BYTES,
        max_text_bytes: 256 << 20,
        max_reports: 256,
        max_output_bytes: 1 << 30,
    }
}

impl Default for Limits {
    fn default() -> Limits {
        default_limits()
    }
}

/// What to do with content a policy governs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ContentPolicy {
    /// An error that names the construct and the policy.
    #[default]
    Refuse,
    /// A warning; the content is left out.
    Omit,
    /// Trusted content is emitted.
    Allow,
}

/// Trust decisions of one conversion. `Default` gives `default_policy()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    /// Raw HTML, raw Typst, raw LaTeX.
    pub raw: ContentPolicy,
    /// `javascript:`, `vbscript:`, and non-image `data:` URLs.
    pub unsafe_urls: ContentPolicy,
    /// Math the target cannot translate.
    pub math: ContentPolicy,
    /// Whether include directives may request resources.
    pub includes: bool,
    /// Warnings fail the conversion.
    pub strict: bool,
    /// Keep a document whose errors are all content errors (`content_error`), as
    /// Docutils does below its halt level; renderers then report them as warnings.
    pub keep_documents_with_errors: bool,
}

/// Returns the safe policy: raw content, unsafe URLs, and untranslatable math refused;
/// includes may be requested.
pub fn default_policy() -> Policy {
    Policy {
        raw: ContentPolicy::Refuse,
        unsafe_urls: ContentPolicy::Refuse,
        math: ContentPolicy::Refuse,
        includes: true,
        strict: false,
        keep_documents_with_errors: false,
    }
}

/// Returns the policy for conformance tests and content the caller wrote: everything
/// allowed.
pub fn trusted_policy() -> Policy {
    Policy {
        raw: ContentPolicy::Allow,
        unsafe_urls: ContentPolicy::Allow,
        math: ContentPolicy::Allow,
        ..default_policy()
    }
}

impl Default for Policy {
    fn default() -> Policy {
        default_policy()
    }
}

/// A reader's own settings (an `rst::Settings`, a `commonmark::Settings`, ...): a
/// borrowed value and its type name. `read_extension` makes one; a reader takes it back
/// with `ReadContext::reader_settings`, which refuses settings of another type.
#[derive(Clone, Copy)]
pub struct ReadExtension<'a> {
    /// The settings.
    pub data: &'a dyn Any,
    /// Their type's name, for diagnostics.
    pub type_name: &'static str,
}

impl fmt::Debug for ReadExtension<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReadExtension").field("type_name", &self.type_name).finish()
    }
}

/// Wraps borrowed reader settings with their type; they must outlive the read.
pub fn read_extension<T: Any>(settings: &T) -> ReadExtension<'_> {
    ReadExtension { data: settings, type_name: std::any::type_name::<T>() }
}

impl<'a> ReadExtension<'a> {
    /// Returns the settings as `&T`, or `None` when they are of another type.
    pub fn downcast<T: Any>(&self) -> Option<&'a T> {
        self.data.downcast_ref::<T>()
    }
}

/// Options every reader understands. `Default` gives `default_read_options()`.
#[derive(Debug, Clone, Copy)]
pub struct ReadOptions<'a> {
    /// RST: a lone top-level section becomes the title.
    pub promote_title: bool,
    /// RST tab stops; zero means 8.
    pub tab_width: usize,
    /// RST date substitutions, e.g. "2026-09-29".
    pub today: &'a str,
    /// The reader's own settings; `None` means its defaults.
    pub extension: Option<ReadExtension<'a>>,
}

/// Returns the read options: title promotion on, tab width 8, no reader settings.
pub fn default_read_options() -> ReadOptions<'static> {
    ReadOptions { promote_title: true, tab_width: 8, today: "", extension: None }
}

impl Default for ReadOptions<'_> {
    fn default() -> Self {
        default_read_options()
    }
}

/// How HTML writes math for a script (MathJax, KaTeX) to typeset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MathStyle {
    /// Class "math", the text in `\( \)` or `\[ \]`, as Docutils writes it.
    #[default]
    Plain,
    /// As Sphinx's mathjax extension writes it.
    Sphinx,
}

/// A word or phrase a renderer writes by itself; hosts translate them (`Labels`).
/// Labels holding "%s" take a value (a version number); see `split_label`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)] // ENGLISH_LABELS gives each label's text.
pub enum Label {
    Attention,
    Caution,
    Danger,
    Error,
    Hint,
    Important,
    Note,
    Tip,
    Warning,
    SeeAlso,
    Todo,
    Contents,
    VersionAdded,
    VersionChanged,
    VersionDeprecated,
    VersionRemoved,
    SectionAuthor,
    ModuleAuthor,
    CodeAuthor,
    LinkHeading,
    LinkDefinition,
    LinkImage,
    LinkTable,
    LinkCode,
    LinkEquation,
    LinkTerm,
    Parameters,
    Variables,
    Raises,
    Returns,
    ReturnType,
    ReturnValues,
    TemplateParameters,
    Throws,
    Arguments,
    Availability,
}

/// The number of labels.
pub const LABEL_COUNT: usize = 36;

/// Translated labels; a label without a translation falls back to English.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Labels {
    words: [String; LABEL_COUNT],
}

impl Default for Labels {
    /// Every label empty: English throughout.
    fn default() -> Labels {
        Labels { words: std::array::from_fn(|_| String::new()) }
    }
}

impl Labels {
    /// Sets the translation of `l`.
    pub fn set(&mut self, l: Label, text: impl Into<String>) {
        self.words[l as usize] = text.into();
    }

    /// Returns the translation of `l`, or "" when it has none.
    pub fn get(&self, l: Label) -> &str {
        &self.words[l as usize]
    }
}

/// The labels in English (Sphinx's msgids), indexed by `Label as usize`.
pub const ENGLISH_LABELS: [&str; LABEL_COUNT] = [
    "Attention",
    "Caution",
    "Danger",
    "Error",
    "Hint",
    "Important",
    "Note",
    "Tip",
    "Warning",
    "See also",
    "Todo",
    "Contents",
    "Added in version %s",
    "Changed in version %s",
    "Deprecated since version %s",
    "Removed in version %s",
    "Section author: ",
    "Module author: ",
    "Code author: ",
    "Link to this heading",
    "Link to this definition",
    "Link to this image",
    "Link to this table",
    "Link to this code",
    "Link to this equation",
    "Link to this term",
    "Parameters",
    "Variables",
    "Raises",
    "Returns",
    "Return type",
    "Return values",
    "Template Parameters",
    "Throws",
    "Arguments",
    "Availability",
];

/// Writes highlighted HTML for a code block. It belongs to the caller's trust boundary.
/// It returns false, having written nothing, to let the renderer write plain escaped
/// code instead.
pub trait Highlighter {
    /// Writes `code` in `language` as highlighted HTML into `out`.
    fn highlight(&self, language: &str, code: &str, out: &mut Emitter<'_>) -> bool;
}

/// Options every renderer understands. `Default` is a standalone document in English.
#[derive(Clone, Copy)]
pub struct RenderOptions<'a> {
    /// A complete document rather than a body fragment.
    pub standalone: bool,
    /// CommonMark reference HTML, for conformance tests.
    pub reference_profile: bool,
    /// Overrides the document's own title.
    pub title: &'a str,
    /// BCP 47 tag; "en" when empty.
    pub language: &'a str,
    /// Standalone HTML: CSS emitted in a style element.
    pub stylesheet: &'a str,
    /// Number sections.
    pub section_numbers: bool,
    /// Prepended to every anchor, e.g. a document ID in a book (`build_anchors`).
    pub anchor_prefix: &'a str,
    /// HTML: a link to each heading, caption, and signature.
    pub permalinks: bool,
    /// "¶" when empty.
    pub permalink_icon: &'a str,
    /// After section numbers; ". " when empty.
    pub secnumber_suffix: &'a str,
    /// Highlights code blocks; `None` writes escaped code.
    pub highlighter: Option<&'a dyn Highlighter>,
    /// The words renderers write, translated; `None` is English.
    pub labels: Option<&'a Labels>,
    /// How HTML writes math.
    pub math: MathStyle,
    /// HTML inline math delimiters with `MathStyle::Sphinx`; empty: `\(` `\)`.
    pub math_inline: [&'a str; 2],
    /// HTML displayed math delimiters with `MathStyle::Sphinx`; empty: `\[` `\]`.
    pub math_display: [&'a str; 2],
}

impl Default for RenderOptions<'_> {
    fn default() -> Self {
        RenderOptions {
            standalone: true,
            reference_profile: false,
            title: "",
            language: "",
            stylesheet: "",
            section_numbers: false,
            anchor_prefix: "",
            permalinks: false,
            permalink_icon: "",
            secnumber_suffix: "",
            highlighter: None,
            labels: None,
            math: MathStyle::Plain,
            math_inline: ["", ""],
            math_display: ["", ""],
        }
    }
}

impl fmt::Debug for RenderOptions<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RenderOptions")
            .field("standalone", &self.standalone)
            .field("reference_profile", &self.reference_profile)
            .field("title", &self.title)
            .field("language", &self.language)
            .field("anchor_prefix", &self.anchor_prefix)
            .field("highlighter", &self.highlighter.is_some())
            .field("labels", &self.labels.is_some())
            .field("math", &self.math)
            .finish_non_exhaustive()
    }
}

/// Returns a label in the render's language, or in English when it has none.
pub fn label<'a>(options: &RenderOptions<'a>, l: Label) -> &'a str {
    match options.labels {
        Some(words) if !words.get(l).is_empty() => words.get(l),
        _ => ENGLISH_LABELS[l as usize],
    }
}

/// Returns the title of an admonition kind; "" for a generic admonition.
pub fn admonition_label<'a>(options: &RenderOptions<'a>, k: AdmonitionKind) -> &'a str {
    let l = match k {
        AdmonitionKind::Generic => return "",
        AdmonitionKind::Attention => Label::Attention,
        AdmonitionKind::Caution => Label::Caution,
        AdmonitionKind::Danger => Label::Danger,
        AdmonitionKind::Error => Label::Error,
        AdmonitionKind::Hint => Label::Hint,
        AdmonitionKind::Important => Label::Important,
        AdmonitionKind::Note => Label::Note,
        AdmonitionKind::Tip => Label::Tip,
        AdmonitionKind::Warning => Label::Warning,
        AdmonitionKind::Seealso => Label::SeeAlso,
        AdmonitionKind::Todo => Label::Todo,
    };
    label(options, l)
}

/// Splits a label at its "%s": the text before and after the value.
pub fn split_label(s: &str) -> (&str, &str) {
    match s.find("%s") {
        Some(i) => (&s[..i], &s[i + 2..]),
        None => (s, ""),
    }
}

/// Why a resource provider did not provide a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceStatus {
    /// It does not exist.
    Missing,
    /// The host refuses to provide it.
    Refused,
    /// The host's memory budget cannot hold it.
    Limit,
}

/// A provided resource: its text, shared so a host can cache it, and a canonical name
/// for cycle checks (usually an absolute path).
#[derive(Debug, Clone)]
pub struct Resource {
    /// UTF-8 text.
    pub text: Arc<str>,
    /// The name cycle checks compare.
    pub canonical: String,
}

/// Provides included files. It belongs to the caller's trust boundary and may allocate.
/// The core passes the name of the requesting source and the name as written.
pub trait ResourceProvider {
    /// Returns the resource `name` that source `from` asks for.
    fn provide(&self, from: &str, name: &str) -> Result<Resource, ResourceStatus>;
}

/// What a pass did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassOutcome {
    /// It left the document as it is (costs nothing).
    Unchanged,
    /// It wrote a new document into the builder.
    Rebuilt,
    /// It failed; the previous document stays.
    Failed,
}

/// The function of a pass: it reads an immutable document and may rebuild it.
pub type PassFn<'a> = dyn Fn(Document<'_>, &mut Builder<'_>) -> PassOutcome + 'a;

/// A named transform run between reading and rendering (`transform_document`).
#[derive(Clone, Copy)]
pub struct Pass<'a> {
    /// Its name, for diagnostics.
    pub name: &'a str,
    /// What it does.
    pub run: &'a PassFn<'a>,
}

impl fmt::Debug for Pass<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pass").field("name", &self.name).finish_non_exhaustive()
    }
}

/// Records that rendered output from `offset` on came from `span`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MapEntry {
    /// Byte offset in the output.
    pub offset: u32,
    /// Where it came from.
    pub span: SourceSpan,
}

/// A named source text, borrowed for the conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Source<'a> {
    /// Its name ("intro.rst").
    pub name: &'a str,
    /// Its text.
    pub text: &'a str,
}

/// A complete conversion request. `Default` (and `default_request`) give the documented
/// defaults; change fields with struct-update syntax.
#[derive(Clone, Copy, Default)]
pub struct Request<'a> {
    /// The main source.
    pub source: Source<'a>,
    /// The reader's id ("rst").
    pub reader: &'a str,
    /// The renderer's id ("html").
    pub renderer: &'a str,
    /// Read options.
    pub read: ReadOptions<'a>,
    /// Render options.
    pub render: RenderOptions<'a>,
    /// Trust decisions.
    pub policy: Policy,
    /// Bounds.
    pub limits: Limits,
    /// Transforms between reading and rendering.
    pub passes: &'a [Pass<'a>],
    /// Provides included files; `None` refuses includes.
    pub resources: Option<&'a dyn ResourceProvider>,
    /// Polled during long work; true cancels.
    pub cancel: Option<&'a dyn Fn() -> bool>,
    /// Whether renderers fill the workspace's source map.
    pub source_map: bool,
}

impl fmt::Debug for Request<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Request")
            .field("source", &self.source.name)
            .field("reader", &self.reader)
            .field("renderer", &self.renderer)
            .field("policy", &self.policy)
            .field("passes", &self.passes.len())
            .finish_non_exhaustive()
    }
}

/// Returns a complete request with the documented defaults: default read options, a
/// standalone render, `default_policy()`, `default_limits()`, no passes, no resource
/// provider, no cancellation. It borrows its arguments.
pub fn default_request<'a>(
    name: &'a str,
    text: &'a str,
    reader: &'a str,
    renderer: &'a str,
) -> Request<'a> {
    Request { source: Source { name, text }, reader, renderer, ..Request::default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_fall_back_to_english() {
        let mut words = Labels::default();
        words.set(Label::Note, "Remarque");
        let options = RenderOptions { labels: Some(&words), ..RenderOptions::default() };
        assert_eq!(label(&options, Label::Note), "Remarque");
        assert_eq!(label(&options, Label::Tip), "Tip");
        assert_eq!(admonition_label(&options, AdmonitionKind::Seealso), "See also");
        assert_eq!(split_label("Added in version %s"), ("Added in version ", ""));
        assert_eq!(ENGLISH_LABELS[Label::Availability as usize], "Availability");
    }
}
