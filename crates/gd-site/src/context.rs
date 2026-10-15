//! What `layout.html` sees for a page: Sphinx's names where Sphinx has one (pagename, docstitle,
//! rellinks, parents, metatags, script_files and the `html_context` values),
//! Guidedog's own for the rest. The functions (pathto, toctree, css_tag, ...) are
//! registered on the environment (templates.rs).

use gd_env::{BuilderKind, DocId};
use gd_jinja::Value;
use gd_resolve::BuildView;
use gd_toml::Data;

use crate::escape::push_escaped;
use crate::mathjax::mathjax_scripts;
use crate::nav::{TocOptions, outline_html, sidebar_html};
use crate::templates::PageState;

/// The accent colour of the built-in theme, unless `html_theme_options.accent` sets one.
const DEFAULT_ACCENT: &str = "#126d78";

/// The page a context is made for: a document's, or a page at the output root that is
/// no document's (`genindex.html`), named by its file.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Page<'a> {
    /// A document's page.
    Doc(DocId),
    /// A page at the output root, by its file name.
    Special(&'a str),
}

/// The context of a page whose content is `body`, and the state the template
/// functions work with while it renders. Links are made from where the page stands.
pub(crate) fn page_context(
    view: &BuildView<'_>,
    page: Page<'_>,
    body: &str,
) -> (Value, PageState) {
    let c = view.config;
    let (doc, file, title, docname) = match page {
        Page::Doc(id) => {
            let d = view.doc(id);
            (Some(id), view.page_name(&d.docname), d.title.as_str(), d.docname.as_str())
        }
        Page::Special(name) => (None, name.to_string(), "", ""),
    };
    let root = "../".repeat(file.matches('/').count());
    let ctx = Value::dict();
    for (key, value) in c.html.context.iter() {
        ctx.set(key, template_value(value));
    }
    project_names(view, &ctx);
    let text = |key: &str, value: &str| ctx.set(key, Value::from(value));
    text("title", title);
    text("docname", docname);
    // As in Sphinx, a special page's name is its file's without the suffix: genindex.
    let pagename = match page {
        Page::Doc(_) => docname,
        Page::Special(name) => name.strip_suffix(".html").unwrap_or(name),
    };
    text("pagename", pagename);
    text("pathto_root", &root);
    text("root_url", &format!("{root}{}", view.page_name(&c.root_doc)));
    text("search_url", &format!("{root}search.html"));
    text("genindex_url", &format!("{root}genindex.html"));
    text("favicon_url", &static_url(&root, &c.html.favicon));
    text("logo_url", &static_url(&root, &c.html.logo));
    text("last_updated", &view.dates.last_updated);
    file_names(view, &ctx, body, &root);
    ctx.set("body", Value::markup(body));
    ctx.set("toc", Value::markup(&sidebar_html(view, doc, TocOptions::SIDEBAR)));
    let outline = doc.map_or(String::new(), |id| outline_html(view, id));
    ctx.set("outline", Value::markup(&outline));
    ctx.set("prev", neighbour(view, doc, -1));
    ctx.set("next", neighbour(view, doc, 1));
    let html = &c.html;
    let source = match doc {
        Some(id) if html.copy_source && html.show_sourcelink => {
            let relative = &view.doc(id).relative;
            format!("{root}_sources/{relative}{}", html.sourcelink_suffix)
        }
        _ => String::new(),
    };
    text("sourcelink_url", &source);
    ctx.set("rellinks", rellinks(view, doc));
    ctx.set("parents", parents(view, doc));
    ctx.set("metatags", Value::markup(&doc.map_or(String::new(), |id| metatags(view, id))));
    (ctx, PageState { doc, page: file, root })
}

/// The names every page shares: the project's settings as Sphinx names them.
fn project_names(view: &BuildView<'_>, ctx: &Value) {
    let c = view.config;
    let html = &c.html;
    let text = |key: &str, value: &str| ctx.set(key, Value::from(value));
    text("language", &c.language);
    text("html_title", &html.title);
    text("docstitle", &html.title);
    text("html_short_title", &html.short_title);
    text("shorttitle", &html.short_title);
    text("project", &c.project);
    text("version", &c.version);
    text("release", &c.release);
    text("copyright", &c.copyright);
    text("root_doc", &c.root_doc);
    text("master_doc", &c.root_doc);
    text("builder", view.kind.name());
    text("file_suffix", &html.file_suffix);
    ctx.set("show_copyright", Value::from(html.show_copyright));
    ctx.set("show_guidedog", Value::from(html.show_sphinx));
    ctx.set("show_sphinx", Value::from(html.show_sphinx));
    ctx.set("has_source", Value::from(html.copy_source));
    ctx.set("show_source", Value::from(html.show_sourcelink));
    let accent = html.theme_options.get("accent").map_or(DEFAULT_ACCENT, String::as_str);
    text("accent", accent);
}

/// The stylesheets and scripts the page links: Guidedog's own, then the project's,
/// then MathJax's when the page has math. `script_files` is Sphinx's name for the
/// scripts.
fn file_names(view: &BuildView<'_>, ctx: &Value, body: &str, root: &str) {
    let html = &view.config.html;
    ctx.set("css_files", file_list(root, &["pygments.css", "guidedog.css"], &html.css_files));
    let js = file_list(root, &["guidedog.js"], &html.js_files);
    mathjax_scripts(view.config, &js, body, root);
    ctx.set("script_files", js.clone());
    ctx.set("js_files", js);
}

/// A value of `html_context` as templates see it, as Sphinx passes its Python values:
/// booleans, numbers, lists, and dicts with their keys in the order conf.toml defines
/// them; a date-time as its text.
fn template_value(v: &gd_toml::Value) -> Value {
    match &v.data {
        Data::String(s) => Value::from(s.as_str()),
        Data::Datetime(d) => Value::from(d.to_string()),
        Data::Integer(i) => Value::from(*i),
        Data::Float(f) => Value::from(*f),
        Data::Boolean(b) => Value::from(*b),
        Data::Array(items) => Value::list(items.iter().map(template_value)),
        Data::Table(table) => {
            let out = Value::dict();
            for (key, value) in table.iter() {
                out.set(key, template_value(value));
            }
            out
        }
    }
}

/// Guidedog's files, then the project's, as entries with a `url`.
fn file_list(root: &str, builtin: &[&str], extra: &[String]) -> Value {
    let names = builtin.iter().copied().chain(extra.iter().map(String::as_str));
    Value::list(names.map(|name| {
        let url =
            if name.contains("://") { name.to_string() } else { format!("{root}_static/{name}") };
        let entry = Value::dict();
        entry.set("url", Value::from(url));
        entry
    }))
}

/// The URL of a file `html_logo` or `html_favicon` names: copied into `_static`,
/// unless it is a URL.
fn static_url(root: &str, path: &str) -> String {
    if path.is_empty() || path.contains("://") {
        return path.to_string();
    }
    format!("{root}_static/{}", base_name(path))
}

/// The last part of a `/`-separated path.
pub(crate) fn base_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// The previous or next page in reading order, or none at either end (and on a page
/// that is no document's).
fn neighbour(view: &BuildView<'_>, doc: Option<DocId>, step: isize) -> Value {
    let order = &view.tables.order;
    let Some(i) = doc.and_then(|id| order.iter().position(|&o| o == id)) else {
        return Value::None;
    };
    let Some(&other) = i.checked_add_signed(step).and_then(|j| order.get(j)) else {
        return Value::None;
    };
    let url = view.page_url(doc, other, "");
    let n = Value::dict();
    n.set("url", Value::from(url.as_str()));
    n.set("link", Value::from(url));
    n.set("title", Value::from(view.doc(other).title.as_str()));
    n
}

/// Sphinx's related pages, (page, title, access key, link text): the general index,
/// the module index, then the next and previous pages.
fn rellinks(view: &BuildView<'_>, doc: Option<DocId>) -> Value {
    let links = Value::list([]);
    let word = |english: &str| view.ui.gettext(english).into_owned();
    let link = |page: &str, title: &str, key: &str, text: &str| {
        let parts = [page, title, key, text].map(Value::from);
        links.push(Value::tuple(parts));
    };
    if view.config.html.use_index {
        link("genindex", &word("General Index"), "I", &word("index"));
    }
    if gd_resolve::has_module_index(view) {
        link("py-modindex", &word("Python Module Index"), "", &word("modules"));
    }
    let order = &view.tables.order;
    if let Some(i) = doc.and_then(|id| order.iter().position(|&o| o == id)) {
        if let Some(&next) = order.get(i + 1) {
            let d = view.doc(next);
            link(&d.docname, &d.title, "N", &word("next"));
        }
        if let Some(&prev) = i.checked_sub(1).and_then(|j| order.get(j)) {
            let d = view.doc(prev);
            link(&d.docname, &d.title, "P", &word("previous"));
        }
    }
    links
}

/// Sphinx's toctree ancestors between the root and the page, outermost first.
fn parents(view: &BuildView<'_>, doc: Option<DocId>) -> Value {
    let root = view.doc_named(&view.config.root_doc);
    let mut chain = Vec::new();
    let mut up = doc.and_then(|id| view.tables.parent.get(id.index()).copied().flatten());
    while let Some(id) = up.filter(|&id| Some(id) != root) {
        if chain.contains(&id) {
            break; // never in a tree, but a loop would never end.
        }
        chain.push(id);
        up = view.tables.parent.get(id.index()).copied().flatten();
    }
    Value::list(chain.iter().rev().map(|&id| {
        let entry = Value::dict();
        entry.set("link", Value::from(view.page_url(doc, id, "")));
        entry.set("title", Value::from(view.doc(id).title.as_str()));
        entry
    }))
}

/// The page's meta directives as Docutils writes them: a field is a name, or
/// attributes such as `property=og:title` and `lang=en`.
fn metatags(view: &BuildView<'_>, id: DocId) -> String {
    let mut out = String::new();
    for (field, content) in &view.doc(id).meta {
        out.push_str("<meta");
        for (i, part) in field.split_whitespace().enumerate() {
            let (key, value) = match part.split_once('=') {
                Some((key, value)) => (key, value),
                None if i == 0 => ("name", part),
                None => (part, ""),
            };
            out.push_str(&format!(" {key}=\""));
            push_escaped(&mut out, value);
            out.push('"');
        }
        out.push_str(" content=\"");
        push_escaped(&mut out, content);
        out.push_str("\" />\n");
    }
    out
}

/// Whether a builder writes HTML pages that this crate lays out.
pub(crate) fn is_site_kind(kind: BuilderKind) -> bool {
    matches!(kind, BuilderKind::Html | BuilderKind::Dirhtml | BuilderKind::Singlehtml)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_files_follow_the_page_and_urls_stay() {
        let files = file_list("../", &["guidedog.css"], &["https://cdn.example/x.css".into()]);
        let urls: Vec<String> = match &files {
            Value::List(l) => {
                l.snapshot().iter().map(|e| gd_jinja::to_str(&e.get("url")).into()).collect()
            }
            _ => Vec::new(),
        };
        assert_eq!(urls, ["../_static/guidedog.css", "https://cdn.example/x.css"]);
        assert_eq!(static_url("../", "img/logo.svg"), "../_static/logo.svg");
        assert_eq!(static_url("", "https://x/logo.svg"), "https://x/logo.svg");
    }

    #[test]
    fn html_context_values_keep_their_types() {
        let table = gd_toml::parse("a = 1\nb = [true, \"x\"]\n[c]\nd = 1.5\n").unwrap_or_default();
        let value = template_value(&gd_toml::Value::new(Data::Table(table)));
        assert_eq!(value.get("a"), Value::from(1i64));
        assert_eq!(value.get("b").len(), 2);
        assert_eq!(value.get("c").get("d"), Value::from(1.5));
    }
}
