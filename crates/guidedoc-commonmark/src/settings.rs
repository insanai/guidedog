//! The "myst" reader's settings: myst-parser 0.16.1's `myst_*` configuration values,
//! the optional syntax they enable, and the markdown-it rule names that
//! `disable_syntax` turns off.

use std::fmt;
use std::sync::Arc;

use crate::extension::Extension;

/// The "myst" reader's settings, named as myst-parser's `myst_*` configuration values
/// without the prefix. They reach the reader through `ReadOptions::extension`, made with
/// `guidedoc::read_extension(&settings)`; no extension means `Settings::default()`. The
/// "commonmark" reader (pure CommonMark) takes none. Start from `Settings::default()`,
/// since several defaults are not zero:
///
/// ```
/// use guidedoc_commonmark::{Settings, SyntaxExtension};
/// let mut settings = Settings::default();
/// settings.enable_extensions.insert(SyntaxExtension::Deflist);
/// settings.heading_anchors = 2;
/// ```
#[derive(Clone)]
pub struct Settings {
    /// Directive and role handlers (the Sphinx layer provides them); `None` means none,
    /// and directives are then reported as needing them.
    pub extension: Option<Arc<dyn Extension>>,
    /// myst_commonmark_only: CommonMark syntax only (no directives, roles, or other
    /// MyST syntax), but a document's tree as for MyST: headings make sections.
    pub commonmark_only: bool,
    /// myst_enable_extensions: the optional syntax. Default: dollarmath.
    pub enable_extensions: SyntaxExtensions,
    /// myst_disable_syntax: markdown-it rule names whose syntax is not parsed, such as
    /// "emphasis", "table", "html_block" (see `Rule::named`). Unknown names warn.
    pub disable_syntax: Vec<String>,
    /// myst_url_schemes: schemes of `[text](scheme:...)` links that are external; other
    /// destinations are cross-references. `None` recognizes every scheme. Default
    /// http, https, mailto, and ftp.
    pub url_schemes: Option<Vec<String>>,
    /// myst_linkify_fuzzy_links: linkify recognizes URLs without a scheme. Default true.
    pub linkify_fuzzy_links: bool,
    /// myst_heading_anchors: headings up to this level get a GitHub-style slug anchor
    /// that `[](#slug)` reaches. 0 means none.
    pub heading_anchors: usize,
    /// myst_heading_slug_func names a Python callable and cannot be supported; a
    /// non-empty value is reported and the built-in slugs are used.
    pub heading_slug_func: String,
    /// myst_substitutions: global substitutions; front matter overrides them.
    pub substitutions: Vec<Substitution>,
    /// myst_sub_delimiters: the characters doubled around substitution references.
    /// Default `{` and `}`, giving `{{ name }}`.
    pub sub_delimiters: [u8; 2],
    /// myst_html_meta: HTML meta tags for every document; front matter html_meta
    /// overrides entries with the same key.
    pub html_meta: Vec<Meta>,
    /// myst_footnote_transition: a transition of class "footnotes" precedes the
    /// footnotes at the end of the document. Default true.
    pub footnote_transition: bool,
    /// myst_dmath_allow_labels: `$$...$$ (label)`. Default true.
    pub dmath_allow_labels: bool,
    /// myst_dmath_allow_space: inline math may start or end with a space. Default true.
    pub dmath_allow_space: bool,
    /// myst_dmath_allow_digits: inline math may follow or precede a digit. Default true.
    pub dmath_allow_digits: bool,
    /// myst_dmath_double_inline: `$$...$$` inside a paragraph. Default false.
    pub dmath_double_inline: bool,
    /// myst_words_per_minute: for the wordcount-minutes substitution. Default 200.
    pub words_per_minute: usize,
    /// The document's name ("usage/intro"), which substitutions reach as env.docname.
    pub docname: String,
    /// Configuration values substitutions reach as env.config.<name>.
    pub env_config: Vec<Substitution>,
    /// The `{sub-ref}` today: the project's formatted date; empty means
    /// `ReadOptions::today`.
    pub today: String,
}

impl Default for Settings {
    /// myst-parser 0.16.1's defaults.
    fn default() -> Settings {
        Settings {
            extension: None,
            commonmark_only: false,
            enable_extensions: SyntaxExtensions::of(&[SyntaxExtension::Dollarmath]),
            disable_syntax: Vec::new(),
            url_schemes: Some(DEFAULT_URL_SCHEMES.iter().map(|s| (*s).to_owned()).collect()),
            linkify_fuzzy_links: true,
            heading_anchors: 0,
            heading_slug_func: String::new(),
            substitutions: Vec::new(),
            sub_delimiters: *b"{}",
            html_meta: Vec::new(),
            footnote_transition: true,
            dmath_allow_labels: true,
            dmath_allow_space: true,
            dmath_allow_digits: true,
            dmath_double_inline: false,
            words_per_minute: 200,
            docname: String::new(),
            env_config: Vec::new(),
            today: String::new(),
        }
    }
}

impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Settings")
            .field("extension", &self.extension.is_some())
            .field("commonmark_only", &self.commonmark_only)
            .field("enable_extensions", &self.enable_extensions)
            .field("disable_syntax", &self.disable_syntax)
            .field("heading_anchors", &self.heading_anchors)
            .finish_non_exhaustive()
    }
}

/// The URL schemes myst-parser 0.16.1 links by default.
pub const DEFAULT_URL_SCHEMES: [&str; 4] = ["http", "https", "mailto", "ftp"];

/// A substitution: a name and its Markdown value. Front matter mappings become dotted
/// names ("key.sub").
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Substitution {
    /// The name, as `{{ name }}` writes it.
    pub name: String,
    /// The value, Markdown.
    pub value: String,
}

impl Substitution {
    /// Makes a substitution from borrowed text.
    pub fn new(name: &str, value: &str) -> Substitution {
        Substitution { name: name.to_owned(), value: value.to_owned() }
    }
}

/// One HTML meta tag: the key as MyST writes it ("description lang=en",
/// "http-equiv=Content-Type", "keywords") and its content.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Meta {
    /// The key.
    pub key: String,
    /// The content.
    pub content: String,
}

/// An entry of myst_enable_extensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum SyntaxExtension {
    Amsmath,
    ColonFence,
    Deflist,
    Dollarmath,
    Fieldlist,
    HtmlAdmonition,
    HtmlImage,
    Linkify,
    Replacements,
    Smartquotes,
    Substitution,
    Tasklist,
}

impl SyntaxExtension {
    /// Every extension, in declaration order.
    pub const ALL: [SyntaxExtension; 12] = [
        SyntaxExtension::Amsmath,
        SyntaxExtension::ColonFence,
        SyntaxExtension::Deflist,
        SyntaxExtension::Dollarmath,
        SyntaxExtension::Fieldlist,
        SyntaxExtension::HtmlAdmonition,
        SyntaxExtension::HtmlImage,
        SyntaxExtension::Linkify,
        SyntaxExtension::Replacements,
        SyntaxExtension::Smartquotes,
        SyntaxExtension::Substitution,
        SyntaxExtension::Tasklist,
    ];

    /// Returns its name in myst_enable_extensions ("colon_fence").
    pub fn name(self) -> &'static str {
        match self {
            SyntaxExtension::Amsmath => "amsmath",
            SyntaxExtension::ColonFence => "colon_fence",
            SyntaxExtension::Deflist => "deflist",
            SyntaxExtension::Dollarmath => "dollarmath",
            SyntaxExtension::Fieldlist => "fieldlist",
            SyntaxExtension::HtmlAdmonition => "html_admonition",
            SyntaxExtension::HtmlImage => "html_image",
            SyntaxExtension::Linkify => "linkify",
            SyntaxExtension::Replacements => "replacements",
            SyntaxExtension::Smartquotes => "smartquotes",
            SyntaxExtension::Substitution => "substitution",
            SyntaxExtension::Tasklist => "tasklist",
        }
    }

    /// Finds an extension by its myst_enable_extensions name.
    pub fn named(name: &str) -> Option<SyntaxExtension> {
        SyntaxExtension::ALL.into_iter().find(|e| e.name() == name)
    }
}

/// A set of optional syntax extensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SyntaxExtensions(u16);

impl SyntaxExtensions {
    /// No extension.
    pub const NONE: SyntaxExtensions = SyntaxExtensions(0);
    /// Every extension.
    pub const ALL: SyntaxExtensions = SyntaxExtensions((1 << 12) - 1);

    /// Returns the set holding `list`.
    pub const fn of(list: &[SyntaxExtension]) -> SyntaxExtensions {
        let mut bits = 0u16;
        let mut i = 0;
        while i < list.len() {
            bits |= 1 << list[i] as u16;
            i += 1;
        }
        SyntaxExtensions(bits)
    }

    /// Tells whether `e` is in the set.
    pub const fn has(self, e: SyntaxExtension) -> bool {
        self.0 & (1 << e as u16) != 0
    }

    /// Adds `e`.
    pub fn insert(&mut self, e: SyntaxExtension) {
        self.0 |= 1 << e as u16;
    }

    /// Removes `e`.
    pub fn remove(&mut self, e: SyntaxExtension) {
        self.0 &= !(1 << e as u16);
    }

    /// Tells whether the set is empty.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// A syntax element `disable_syntax` can turn off, by markdown-it's rule name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum Rule {
    Table,
    Code,
    Fence,
    Blockquote,
    Hr,
    List,
    Reference,
    HtmlBlock,
    Heading,
    Lheading,
    Emphasis,
    Backticks,
    Link,
    Image,
    Autolink,
    HtmlInline,
    Entity,
    Escape,
    Newline,
    FrontMatter,
    MystTarget,
    MystLineComment,
    MystBlockBreak,
    MystRole,
    MathBlock,
    MathInline,
    FootnoteDef,
    FootnoteRef,
    /// Not a markdown-it rule: `{name}` fences (off with `commonmark_only`).
    MystDirective,
}

impl Rule {
    /// Every rule, in declaration order.
    pub const ALL: [Rule; 29] = [
        Rule::Table,
        Rule::Code,
        Rule::Fence,
        Rule::Blockquote,
        Rule::Hr,
        Rule::List,
        Rule::Reference,
        Rule::HtmlBlock,
        Rule::Heading,
        Rule::Lheading,
        Rule::Emphasis,
        Rule::Backticks,
        Rule::Link,
        Rule::Image,
        Rule::Autolink,
        Rule::HtmlInline,
        Rule::Entity,
        Rule::Escape,
        Rule::Newline,
        Rule::FrontMatter,
        Rule::MystTarget,
        Rule::MystLineComment,
        Rule::MystBlockBreak,
        Rule::MystRole,
        Rule::MathBlock,
        Rule::MathInline,
        Rule::FootnoteDef,
        Rule::FootnoteRef,
        Rule::MystDirective,
    ];

    /// Returns markdown-it's name of the rule; "" for `MystDirective`.
    pub fn name(self) -> &'static str {
        const NAMES: [&str; 29] = [
            "table",
            "code",
            "fence",
            "blockquote",
            "hr",
            "list",
            "reference",
            "html_block",
            "heading",
            "lheading",
            "emphasis",
            "backticks",
            "link",
            "image",
            "autolink",
            "html_inline",
            "entity",
            "escape",
            "newline",
            "front_matter",
            "myst_target",
            "myst_line_comment",
            "myst_block_break",
            "myst_role",
            "math_block",
            "math_inline",
            "footnote_def",
            "footnote_ref",
            "",
        ];
        NAMES[self as usize]
    }

    /// Finds a rule by its markdown-it name.
    pub fn named(name: &str) -> Option<Rule> {
        if name.is_empty() {
            return None;
        }
        Rule::ALL.into_iter().find(|r| r.name() == name)
    }
}

/// A set of rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct Rules(u32);

impl Rules {
    pub(crate) const fn of(list: &[Rule]) -> Rules {
        let mut bits = 0u32;
        let mut i = 0;
        while i < list.len() {
            bits |= 1 << list[i] as u32;
            i += 1;
        }
        Rules(bits)
    }

    pub(crate) const fn has(self, r: Rule) -> bool {
        self.0 & (1 << r as u32) != 0
    }

    pub(crate) fn insert(&mut self, r: Rule) {
        self.0 |= 1 << r as u32;
    }
}

/// The syntax MyST adds to CommonMark, off with `commonmark_only`.
pub(crate) const MYST_RULES: Rules = Rules::of(&[
    Rule::Table,
    Rule::FrontMatter,
    Rule::MystTarget,
    Rule::MystLineComment,
    Rule::MystBlockBreak,
    Rule::MystRole,
    Rule::MystDirective,
    Rule::MathBlock,
    Rule::MathInline,
    Rule::FootnoteDef,
    Rule::FootnoteRef,
]);
