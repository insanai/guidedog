//! reStructuredText inline markup and reference resolution for Guidedoc.
//!
//! The block reader (`guidedoc-rst`) owns lines, directives, and document structure. It
//! keeps one [`InlineState`] per read, calls [`parse_inline`] for every inline context
//! (paragraph, title, field name, ...), and calls [`resolve`] once when parsing ends:
//!
//! ```
//! use guidedoc::{LineView, NodeKind, SourceSpan, Workspace, default_request};
//! use guidedoc_rst_inline::{InlineState, NoRoleHook, Options, parse_inline, resolve};
//!
//! let text = "See *this* and `that <https://example.org>`_.";
//! let mut ws = Workspace::new();
//! let request = default_request("x.rst", text, "rst", "html");
//! let mut ctx = ws.begin_read(&request).unwrap();
//! let mut state = InlineState::new(Options::default());
//! let span = SourceSpan::new(ctx.source, 0, text.len() as u32);
//! let p = ctx.b.begin_node(NodeKind::Paragraph, span).unwrap();
//! let lines = [LineView { text, offset: 0 }];
//! parse_inline(&mut ctx, &mut state, &lines, &mut NoRoleHook).unwrap();
//! ctx.b.end_node(p).unwrap();
//! resolve(&mut ctx, &state.options).unwrap();
//! ```
//!
//! Markup follows the Docutils "Inline markup recognition rules" (start- and end-string
//! contexts, the Unicode punctuation classes, backslash escapes); resolution follows
//! Docutils' transforms (targets, footnotes, citations, substitutions, title promotion,
//! bibliographic fields, smart quotes). Problems are reported to the read's reports and
//! the text is kept; only a builder failure (a limit or capacity) is an `Err`.

#![forbid(unsafe_code)]

mod chars;
mod emit;
mod ends;
mod extension;
mod implicit;
mod interpreted;
mod languages;
mod markup;
mod messages;
mod options;
mod punctuation_table;
mod resolve;
mod roles;
mod scan;
mod uri;

#[cfg(test)]
mod tests;

use guidedoc::{Fail, LineView, ReadContext, SourceId};

pub use extension::{NoRoleHook, RoleCall, RoleHook, unescape};
pub use messages::MESSAGES;
pub use options::{Options, SmartQuotes};
pub use roles::{Role, RoleBase, standard_role};

/// One line of inline text: a view of source text and the source offset of its first
/// byte. Lines are passed with tabs unexpanded so spans stay exact; resolution expands
/// tabs in the text it keeps (to `Options::tab_width`).
pub type Line<'s> = LineView<'s>;

/// The custom attribute of a Reference whose text also defines a target: the value is
/// the normalized name of `` `text <alias_>`_ ``, an indirect target to the alias.
pub const TARGET_NAME_ATTRIBUTE: &str = "target-name";

/// The class of the paragraph that holds a section subtitle (`Options::sectsubtitle`).
pub const SECTION_SUBTITLE_CLASS: &str = "section-subtitle";

/// The class of the bibliographic field list, and the custom attribute name each
/// recognized bibliographic field carries (its English key, e.g. "author").
pub const DOCINFO_CLASS: &str = "docinfo";

/// Footnote symbols in the order the specification gives.
pub const FOOTNOTE_SYMBOLS: [&str; 10] = ["*", "†", "‡", "§", "¶", "#", "♠", "♥", "♦", "♣"];

/// Inline parsing state shared by every inline context of one read: the source the
/// current lines come from, the roles the document defined with the role directive, the
/// default role, and the settings. It also keeps the parser's reusable buffers, so
/// steady-state parsing does not allocate.
#[derive(Debug, Default)]
pub struct InlineState {
    /// The source the lines passed to `parse_inline` come from.
    pub source: SourceId,
    /// Roles the document defined (role directive), in definition order.
    pub roles: Vec<Role>,
    /// The default role's name, lowercase; "" means `title-reference`.
    pub default_role: String,
    /// The settings inline parsing and resolution need.
    pub options: Options,
    pub(crate) buffers: scan::Buffers,
}

impl InlineState {
    /// Makes a state for the main source with `options` and no custom roles.
    pub fn new(options: Options) -> InlineState {
        InlineState { options, ..InlineState::default() }
    }

    /// Returns the index in `roles` of the role the document defined as `name`
    /// (compared case-insensitively), if any.
    pub fn find_role(&self, name: &str) -> Option<usize> {
        self.roles.iter().position(|r| roles::same_name(&r.name, name))
    }

    /// Defines `role`, replacing a role of the same name, as a later role directive
    /// does. Returns its index in `roles`.
    pub fn define_role(&mut self, role: Role) -> usize {
        match self.find_role(&role.name) {
            Some(k) => {
                self.roles[k] = role;
                k
            }
            None => {
                self.roles.push(role);
                self.roles.len() - 1
            }
        }
    }
}

/// Parses `lines` as inline markup and emits the nodes as children of the builder's
/// currently open node. Markup may span lines; an unescaped line break outside
/// literals and roles becomes a SoftBreak. Markup that cannot be completed stays text
/// with a warning; unknown or misused roles are errors in the reports. Interpreted text
/// is offered to `hook` first (use [`NoRoleHook`] for none), then to the document's and
/// the standard roles. Fails only when the builder fails (limits, capacity).
pub fn parse_inline(
    ctx: &mut ReadContext<'_>,
    state: &mut InlineState,
    lines: &[Line<'_>],
    hook: &mut dyn RoleHook,
) -> Result<(), Fail> {
    markup::parse_inline_lines(ctx, state, lines, hook)
}

/// Runs role `name` on interpreted text written in another markup, such as MyST's
/// ``{name}`content` ``, as `parse_inline` would for reStructuredText. `lines` hold the
/// whole construct; the content is the byte range `content` of the lines joined by
/// "\n", taken literally (a backslash is an ordinary character). An unknown role is
/// reported and the construct stays text. Fails only when the builder fails.
pub fn apply_host_role(
    ctx: &mut ReadContext<'_>,
    state: &mut InlineState,
    hook: &mut dyn RoleHook,
    name: &str,
    lines: &[Line<'_>],
    content: std::ops::Range<usize>,
) -> Result<(), Fail> {
    interpreted::apply_host_role(ctx, state, hook, name, lines, content)
}

/// Resolves the finished document in place: specialized roles, targets (explicit,
/// implicit, indirect, anonymous), footnotes and their numbers, citations, substitution
/// references (with the cycle check), then the structure transforms the options ask
/// for: title promotion (`ctx.options.promote_title`), section subtitles, bibliographic
/// fields, smart quotes, tab expansion. Problems go to the reports; fails only when the
/// builder fails.
pub fn resolve(ctx: &mut ReadContext<'_>, options: &Options) -> Result<(), Fail> {
    resolve::resolve_document(ctx, options, None)
}

/// `resolve` with a dialect's default substitutions, such as Sphinx's `|version|`:
/// `substitute(name)` is asked for every substitution reference whose name no
/// definition has exactly (before the case-insensitive match); `Some(text)` replaces
/// the reference with that text.
pub fn resolve_with_substitutions(
    ctx: &mut ReadContext<'_>,
    options: &Options,
    substitute: &dyn Fn(&str) -> Option<String>,
) -> Result<(), Fail> {
    resolve::resolve_document(ctx, options, Some(substitute))
}

/// Appends the label of symbol footnote `n` (1-based) to `out`: after ten footnotes the
/// symbols repeat doubled, then tripled. Nothing for `n < 1`.
pub fn footnote_symbol(n: i32, out: &mut String) {
    if n < 1 {
        return;
    }
    let k = (n - 1) as usize;
    let symbol = FOOTNOTE_SYMBOLS[k % FOOTNOTE_SYMBOLS.len()];
    for _ in 0..k / FOOTNOTE_SYMBOLS.len() + 1 {
        out.push_str(symbol);
    }
}

/// Tells whether `c` is white space as Python's `str.isspace` (and the `\s` of Python
/// regular expressions) defines it, for readers that follow Python patterns.
pub fn python_space(c: char) -> bool {
    chars::is_space(c)
}
