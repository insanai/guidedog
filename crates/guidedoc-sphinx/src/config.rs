//! The configuration a Sphinx read needs: the conf.toml settings with Sphinx's names.

use std::fmt;
use std::sync::Arc;

use guidedoc::{ENGLISH_LABELS, Label, Labels};
use guidedoc_rst::{Settings, SmartQuotes};

use crate::autodoc::AutodocProvider;

/// One sphinx.ext.extlinks entry: `:name:`part`` links to `url` with `%s` replaced by
/// the part; `caption`, when set, is the link text with `%s` replaced likewise.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Extlink {
    /// The role name.
    pub name: String,
    /// The URL pattern.
    pub url: String,
    /// The caption pattern; empty shows the URL.
    pub caption: String,
}

/// toc_object_entries_show_parents: how much of an object's hierarchy its table of
/// contents entry shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TocParents {
    /// As the domain names it: Python's `Class.method`, C++'s enclosing objects.
    #[default]
    Domain,
    /// The object's own name.
    Hide,
    /// Every part: `module.Class.method`, `ns::Class::f`.
    All,
}

/// An object type a project declares, as a Sphinx extension declares one with
/// `app.add_object_type` (a directive that describes an object and a role that refers to
/// it) or `app.add_crossref_type` (a directive that only makes a target). Both live in
/// the standard domain.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectType {
    /// The directive name ("setting").
    pub directive: String,
    /// The role name ("setting"); empty for none.
    pub role: String,
    /// Sphinx's indextemplate, "pair: %s; setting"; empty for none.
    pub index: String,
    /// add_crossref_type: a target and an index entry, no description.
    pub target: bool,
    /// The object's name is the signature up to its first space.
    pub first_word: bool,
    /// How the signature is shown, "%s" standing for it; empty shows it as written.
    pub display: String,
    /// The name becomes the program the options that follow belong to.
    pub program: bool,
}

/// Another name for a directive: `name` runs `target`, which may name its domain
/// ("py:function").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirectiveAlias {
    /// The new name ("django-admin-option").
    pub name: String,
    /// The directive it names ("option").
    pub target: String,
}

/// The autodoc provider of one domain: what generates the text its auto directives
/// stand for ("py" for automodule and the others, "odin" for odin:autopackage, ...).
#[derive(Clone)]
pub struct Autodoc {
    /// The domain the provider serves.
    pub domain: String,
    /// The host's generator.
    pub provider: Arc<dyn AutodocProvider>,
}

impl fmt::Debug for Autodoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Autodoc").field("domain", &self.domain).finish_non_exhaustive()
    }
}

/// The conf.toml settings reading needs, with Sphinx's names. It reaches the reader
/// through `ReadOptions::extension` (`guidedoc::read_extension(&config)`). Start from
/// `Config::default()`: several defaults are true. Settings the base reader owns (smart
/// quotes, PEP and RFC base URLs, footnote reference space, language) live in
/// `settings`.
#[derive(Debug, Clone)]
pub struct Config {
    /// The reStructuredText reader's settings.
    pub settings: Settings,
    /// The document's name, e.g. "usage/intro".
    pub docname: String,
    /// The default role; empty means Docutils' title-reference.
    pub default_role: String,
    /// The default domain ("py"); empty means none.
    pub primary_domain: String,
    /// The default highlighting language ("default").
    pub highlight_language: String,
    /// sphinx.ext.extlinks.
    pub extlinks: Vec<Extlink>,
    /// Parsed before every reStructuredText document.
    pub rst_prolog: String,
    /// Parsed after every reStructuredText document.
    pub rst_epilog: String,
    /// Figures, tables, and code blocks are numbered.
    pub numfig: bool,
    /// Every display equation is numbered.
    pub math_number_all: bool,
    /// sphinx.ext.todo shows its todos.
    pub todo_include_todos: bool,
    /// sectionauthor and the other author directives show their authors.
    pub show_authors: bool,
    /// "()" after the names of callables (true).
    pub add_function_parentheses: bool,
    /// Module names before object names (true).
    pub add_module_names: bool,
    /// Objects have table of contents entries (true).
    pub toc_object_entries: bool,
    /// How much of an object's hierarchy its table of contents entry shows.
    pub toc_object_entries_show_parents: TocParents,
    /// The manpage role's URL, with "{page}", "{section}", and "{path}" substituted.
    pub manpages_url: String,
    /// option_emphasise_placeholders.
    pub option_emphasise_placeholders: bool,
    /// Doctest flags and `<BLANKLINE>` are hidden (true).
    pub trim_doctest_flags: bool,
    /// The default substitution |version|: the short version.
    pub version: String,
    /// |release|: the full release.
    pub release: String,
    /// |today|: today's date, already formatted.
    pub today: String,
    /// sphinx.ext.intersphinx: the :external: roles are read.
    pub intersphinx: bool,
    /// An external role naming this inventory is an ordinary reference.
    pub intersphinx_resolve_self: String,
    /// The info field labels ("Parameters", "Returns") in the document's language;
    /// empty ones are English.
    pub labels: Labels,
    /// The object types the project declares.
    pub object_types: Vec<ObjectType>,
    /// Other names for directives.
    pub directive_aliases: Vec<DirectiveAlias>,
    /// The autodoc providers, by domain; a domain without one leaves its auto
    /// directives unknown, as Sphinx without the extension.
    pub autodoc: Vec<Autodoc>,
}

impl Default for Config {
    /// Sphinx's defaults, with the base reader's default settings and smart quotes on.
    fn default() -> Config {
        let settings = Settings { smart_quotes: SmartQuotes::On, ..Settings::default() };
        Config {
            settings,
            docname: String::new(),
            default_role: String::new(),
            primary_domain: "py".to_owned(),
            highlight_language: "default".to_owned(),
            extlinks: Vec::new(),
            rst_prolog: String::new(),
            rst_epilog: String::new(),
            numfig: false,
            math_number_all: false,
            todo_include_todos: false,
            show_authors: false,
            add_function_parentheses: true,
            add_module_names: true,
            toc_object_entries: true,
            toc_object_entries_show_parents: TocParents::Domain,
            manpages_url: String::new(),
            option_emphasise_placeholders: false,
            trim_doctest_flags: true,
            version: String::new(),
            release: String::new(),
            today: String::new(),
            intersphinx: false,
            intersphinx_resolve_self: String::new(),
            labels: Labels::default(),
            object_types: Vec::new(),
            directive_aliases: Vec::new(),
            autodoc: Vec::new(),
        }
    }
}

impl Config {
    /// Returns label `l` in the document's language, English when it has none.
    pub fn label(&self, l: Label) -> &str {
        match self.labels.get(l) {
            "" => ENGLISH_LABELS[l as usize],
            word => word,
        }
    }

    /// Returns the autodoc provider of `domain`.
    pub fn autodoc_for(&self, domain: &str) -> Option<&Arc<dyn AutodocProvider>> {
        self.autodoc.iter().find(|a| a.domain == domain).map(|a| &a.provider)
    }
}
