//! The conf.toml settings that change how documents read, as the readers take them: the
//! Sphinx layer's `Config` (reStructuredText) and the `myst` reader's `Settings`.

use gd_conf::{Config, ObjectTypeDecl};
use guidedoc::Labels;
use guidedoc_commonmark::{Meta, Settings as MystSettings, Substitution, SyntaxExtension};
use guidedoc_rst::SmartQuotes;
use guidedoc_sphinx::{DirectiveAlias, Extlink, ObjectType, TocParents};

/// The Sphinx layer's configuration for every document of a project, from conf.toml:
/// `labels` are the info field labels in the build's language and `today` is `|today|`.
/// Each document's read adds its docname and autodoc providers to a copy.
pub fn sphinx_config(c: &Config, labels: &Labels, today: &str) -> guidedoc_sphinx::Config {
    let mut r = guidedoc_sphinx::Config::default();
    r.default_role.clone_from(&c.default_role);
    r.primary_domain.clone_from(&c.primary_domain);
    if !c.highlight_language.is_empty() {
        r.highlight_language.clone_from(&c.highlight_language);
    }
    r.rst_prolog.clone_from(&c.rst_prolog);
    r.rst_epilog.clone_from(&c.rst_epilog);
    r.numfig = c.numfig;
    r.math_number_all = c.math_number_all;
    r.todo_include_todos = c.todo_include_todos;
    r.show_authors = c.show_authors;
    r.add_function_parentheses = c.add_function_parentheses;
    r.add_module_names = c.add_module_names;
    r.toc_object_entries = c.toc_object_entries;
    r.toc_object_entries_show_parents = match c.toc_object_entries_show_parents.as_str() {
        "hide" => TocParents::Hide,
        "all" => TocParents::All,
        _ => TocParents::Domain,
    };
    r.manpages_url.clone_from(&c.manpages_url);
    r.version.clone_from(&c.version);
    r.release.clone_from(&c.release);
    r.today = today.to_owned();
    r.intersphinx = c.has_extension("sphinx.ext.intersphinx");
    r.intersphinx_resolve_self.clone_from(&c.intersphinx.resolve_self);
    r.trim_doctest_flags = c.trim_doctest_flags;
    r.option_emphasise_placeholders = c.option_emphasise_placeholders;
    r.settings.smart_quotes = if c.smartquotes { SmartQuotes::On } else { SmartQuotes::Off };
    r.settings.trim_footnote_reference_space = c.trim_footnote_reference_space;
    r.settings.language.clone_from(&c.language);
    r.labels = labels.clone();
    // conf.toml keeps extlinks in a sorted map, so every build reads the same.
    r.extlinks = c
        .extlinks
        .iter()
        .map(|(name, (url, caption))| Extlink {
            name: name.clone(),
            url: url.clone(),
            caption: caption.clone(),
        })
        .collect();
    r.object_types = c.object_types.iter().map(object_type).collect();
    r.directive_aliases = c
        .directive_aliases
        .iter()
        .map(|(name, target)| DirectiveAlias { name: name.clone(), target: target.clone() })
        .collect();
    r
}

fn object_type(t: &ObjectTypeDecl) -> ObjectType {
    ObjectType {
        directive: t.directive.clone(),
        role: t.role.clone(),
        index: t.index.clone(),
        target: t.target,
        first_word: t.first_word,
        display: t.display.clone(),
        program: t.program,
    }
}

/// The `myst` reader's settings for every Markdown document of a project, from the
/// `myst_*` settings, starting from myst-parser 0.16.1's defaults. `today` is the
/// `{sub-ref}` today. Each document's read adds its docname and the Sphinx layer.
pub fn myst_settings(c: &Config, today: &str) -> MystSettings {
    let m = &c.myst;
    let mut s = MystSettings { commonmark_only: m.commonmark_only, ..MystSettings::default() };
    s.enable_extensions = Default::default();
    for e in m.enable_extensions.iter().filter_map(|name| SyntaxExtension::named(name)) {
        s.enable_extensions.insert(e);
    }
    if m.dmath_enable {
        s.enable_extensions.insert(SyntaxExtension::Dollarmath);
    }
    if m.amsmath_enable {
        s.enable_extensions.insert(SyntaxExtension::Amsmath);
    }
    s.disable_syntax.clone_from(&m.disable_syntax);
    s.url_schemes = (!m.url_schemes.is_empty()).then(|| m.url_schemes.clone());
    s.linkify_fuzzy_links = m.linkify_fuzzy_links;
    s.heading_anchors = usize::try_from(m.heading_anchors).unwrap_or(0);
    s.sub_delimiters = sub_delimiters(&m.sub_delimiters);
    s.footnote_transition = m.footnote_transition;
    s.dmath_allow_labels = m.dmath_allow_labels;
    s.dmath_allow_space = m.dmath_allow_space;
    s.dmath_allow_digits = m.dmath_allow_digits;
    s.dmath_double_inline = m.dmath_double_inline;
    s.words_per_minute = usize::try_from(m.words_per_minute).unwrap_or(200);
    // Sorted maps: every build reads the same.
    s.substitutions = m.substitutions.iter().map(|(k, v)| Substitution::new(k, v)).collect();
    s.html_meta =
        m.html_meta.iter().map(|(k, v)| Meta { key: k.clone(), content: v.clone() }).collect();
    s.env_config = [
        ("project", &c.project),
        ("version", &c.version),
        ("release", &c.release),
        ("language", &c.language),
        ("author", &c.author),
        ("copyright", &c.copyright),
    ]
    .iter()
    .map(|(name, value)| Substitution::new(name, value))
    .collect();
    s.today = today.to_owned();
    s
}

/// The first byte of each `myst_sub_delimiters` entry; gd-conf checks there are two
/// single characters, and the defaults stand in for anything else.
fn sub_delimiters(given: &[String]) -> [u8; 2] {
    let first =
        |i: usize, default: u8| given.get(i).and_then(|s| s.bytes().next()).unwrap_or(default);
    [first(0, b'{'), first(1, b'}')]
}
