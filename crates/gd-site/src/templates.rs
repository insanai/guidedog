//! The page layout and its template functions: `layout.html` in a gd-jinja environment
//! whose loader looks first in `templates_path`, then among the built-in theme's
//! templates (gd-theme).
//!
//! Functions that need nothing of the build (`css_tag`, `js_tag`) and Sphinx's fixed
//! names are the environment's. Those that read the build or the page (`pathto`,
//! `hasdoc`, `toctree`, `_`, `gettext`, `ngettext`) are gd-jinja `Locals` of each
//! render, borrowing the view and the page, so a page is rendered exactly once.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gd_env::{BuilderKind, DocId, Lookup};
use gd_host::{Confinement, Problem};
use gd_jinja::{Autoescape, Call, Environment, Error, ErrorKind, Loader, Locals, Options, Value};
use gd_report::Report;
use gd_resolve::BuildView;

use crate::mathjax::script_tag;
use crate::nav::{TocOptions, sidebar_html};

/// The layout every page is rendered with.
pub(crate) const LAYOUT: &str = "layout.html";

/// A keyword-only argument of a template function (Sphinx's `toctree(maxdepth=..)`).
const KEYWORD_ONLY: usize = usize::MAX;

/// The page a template is being rendered for.
#[derive(Debug, Clone, Default)]
pub(crate) struct PageState {
    /// The page's document; `None` for a page that is no document's (genindex.html).
    pub doc: Option<DocId>,
    /// The page's file below the output folder.
    pub page: String,
    /// From the page to the output root, such as `"../"`.
    pub root: String,
}

/// Where templates come from: the project's template files (listed once, before the
/// first page) and the built-in theme.
#[derive(Debug, Default)]
struct TemplateSources {
    source_dir: PathBuf,
    dirs: Vec<String>,
    files: HashSet<PathBuf>,
    /// Problems reading a template, reported with the page that loaded it.
    errors: RefCell<Vec<Report>>,
}

/// The page layout: a template environment with Guidedog's template functions.
pub(crate) struct Layout {
    env: Environment,
    sources: Rc<TemplateSources>,
    files: Vec<PathBuf>,
    /// The build's memory budget in bytes, which bounds what one render may build.
    budget: usize,
    navigation: RefCell<Vec<Lookup>>,
    /// How many templates were rendered, for tests that a page renders once.
    #[cfg(test)]
    pub(crate) renders: std::cell::Cell<usize>,
}

impl std::fmt::Debug for Layout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Layout").field("files", &self.files).finish_non_exhaustive()
    }
}

impl Layout {
    /// Lists the templates the build uses (warning in `reports` about any that may not be
    /// read), makes the environment, and parses `layout.html` once. Fails with
    /// `template.error` when the layout does not parse or cannot be found.
    pub(crate) fn load(
        view: &BuildView<'_>,
        reports: &mut Vec<Report>,
    ) -> Result<Layout, Problem> {
        let files = template_files(view, reports);
        let sources = Rc::new(TemplateSources {
            source_dir: view.config.source_dir.clone(),
            dirs: view.config.templates_path.clone(),
            files: files.iter().cloned().collect(),
            errors: RefCell::default(),
        });
        let loader_sources = Rc::clone(&sources);
        // A render may build at most the whole budget: a value or page larger is
        // refused before it is allocated, as the budget's problem.
        let budget = view.budget.limit();
        let mut env = Environment::new(Options {
            autoescape: Autoescape::Always,
            max_output: budget,
            loader: Some(Loader::custom(move |name| template_source(&loader_sources, name))),
            ..Options::default()
        });
        register_functions(&mut env);
        let layout = Layout {
            env,
            sources,
            files,
            budget,
            navigation: RefCell::default(),
            #[cfg(test)]
            renders: std::cell::Cell::new(0),
        };
        if let Err(e) = layout.env.load_template(LAYOUT) {
            return Err(layout.problem(&e));
        }
        Ok(layout)
    }

    /// The effective source, so overrides and missing optional includes invalidate pages.
    pub(crate) fn template_digest(&self, name: &str) -> gd_env::Digest {
        let mut digest = gd_env::Digester::new();
        digest.text(name);
        match template_source(&self.sources, name) {
            Some(text) => {
                digest.flag(true).text(&text);
            }
            None => {
                digest.flag(false);
            }
        }
        digest.finish()
    }

    pub(crate) fn take_template_loads(&self) -> Vec<String> {
        self.env.take_template_loads()
    }

    pub(crate) fn take_navigation_lookups(&self) -> Vec<Lookup> {
        let mut lookups = std::mem::take(&mut *self.navigation.borrow_mut());
        lookups.sort();
        lookups.dedup();
        lookups
    }

    /// Renders the template `name` with `ctx` for `page`, once, with the functions that
    /// read the build. Problems reading a template it loads are added to `reports`.
    /// Fails with `template.error`.
    pub(crate) fn render(
        &self,
        view: &BuildView<'_>,
        name: &str,
        ctx: &Value,
        page: &PageState,
        reports: &mut Vec<Report>,
    ) -> Result<String, Problem> {
        #[cfg(test)]
        self.renders.set(self.renders.get() + 1);
        self.navigation.borrow_mut().clear();
        let article = ctx.get("page");
        if let Value::Dict(dict) = &article {
            dict.begin_key_trace();
        }
        if let Value::Dict(dict) = ctx {
            dict.begin_key_trace();
        }
        let locals = page_functions(view, page, &self.navigation);
        self.env.take_template_loads();
        let rendered = self.env.load_template(name).and_then(|t| t.render_with(ctx, &locals));
        if let Value::Dict(dict) = &article {
            for key in dict
                .end_key_trace()
                .into_iter()
                .filter(|k| matches!(k.as_str(), "older" | "newer"))
            {
                self.navigation
                    .borrow_mut()
                    .push(crate::navigation_trace::lookup(&format!("page\0{key}")));
            }
        }
        if let Value::Dict(dict) = ctx {
            for key in dict
                .end_key_trace()
                .into_iter()
                .filter(|k| crate::navigation_trace::context_key(k))
            {
                self.navigation
                    .borrow_mut()
                    .push(crate::navigation_trace::lookup(&format!("ctx\0{key}")));
            }
        }
        reports.append(&mut self.sources.errors.borrow_mut());
        rendered.map_err(|e| self.problem(&e))
    }

    /// A template error as the problem that stops the build, at the template's file
    /// and line.
    fn problem(&self, e: &Error) -> Problem {
        let file = match template_path(&self.sources, &e.template) {
            Some(path) => gd_env::relative_to(&self.sources.source_dir, &path),
            None => format!("{} (built in)", e.template),
        };
        let location = if e.line == 0 { file } else { format!("{file}:{}", e.line) };
        let mut message = e.message.clone();
        if !e.excerpt.is_empty() {
            message = format!("{message}\n\n    {}", e.excerpt);
        }
        if e.kind == ErrorKind::Limit && e.hint.contains("Options::max_output") {
            return self.budget_problem(&location, message);
        }
        let hint = if e.hint.is_empty() {
            "Fix the template, or delete it to use Guidedog's built-in one.".to_string()
        } else {
            e.hint.clone()
        };
        Problem::new("template.error", template_title(e.kind), &location, message, hint, 1)
    }
}

impl Layout {
    /// A render that would build more than the budget: the budget's problem (exit 3),
    /// at the template line, with the flag that raises it.
    fn budget_problem(&self, location: &str, message: String) -> Problem {
        let hint = format!(
            "Build a smaller value in the template, or run again with a larger --budget \
             (this build's is {} MiB).",
            self.budget >> 20
        );
        Problem::new("host.budget", "MEMORY BUDGET REACHED", location, message, hint, 3)
    }
}

fn template_title(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::Syntax => "TEMPLATE SYNTAX ERROR",
        ErrorKind::Undefined => "UNDEFINED TEMPLATE VALUE",
        ErrorKind::Type => "TEMPLATE TYPE ERROR",
        ErrorKind::NotFound => "TEMPLATE NOT FOUND",
        ErrorKind::Limit => "TEMPLATE LIMIT REACHED",
        ErrorKind::Memory => "TEMPLATE OUT OF MEMORY",
        ErrorKind::Runtime => "TEMPLATE ERROR",
    }
}

/// Registers the functions that need nothing of the build, and Sphinx's fixed names.
fn register_functions(env: &mut Environment) {
    env.add_function("css_tag", |c| Ok(css_tag(c)));
    env.add_function("js_tag", |c| Ok(Value::markup(&script_tag(&c.arg(0, "file"), &url_of(c)))));
    // The separators Sphinx's basic theme defines for its navigation bars.
    env.add_global("reldelim1", Value::markup(" &#187;"));
    env.add_global("reldelim2", Value::markup(" |"));
    env.add_global("embedded", Value::from(false)); // true only for help-file builders.
}

/// The functions of one page's render, borrowing the view and the page.
fn page_functions<'a>(
    view: &'a BuildView<'a>,
    page: &'a PageState,
    trace: &'a RefCell<Vec<Lookup>>,
) -> Locals<'a> {
    let mut locals = Locals::new();
    locals.add_function("_", move |c| Ok(word(view, c)));
    locals.add_function("gettext", move |c| Ok(word(view, c)));
    locals.add_function("ngettext", move |c| plural(view, c));
    locals.add_function("pathto", move |c| {
        let target = gd_jinja::to_str(&c.arg_or(0, "otheruri", Value::from(""))).into_owned();
        let resource = c.arg_flag(1, "resource", false);
        trace
            .borrow_mut()
            .push(crate::navigation_trace::lookup(&format!("pathto\0{resource}\0{target}")));
        Ok(Value::from(path_to(view, page, &target, resource)))
    });
    locals.add_function("hasdoc", move |c| {
        let name = gd_jinja::to_str(&c.arg(0, "name")).into_owned();
        trace.borrow_mut().push(crate::navigation_trace::lookup(&format!("hasdoc\0{name}")));
        Ok(Value::from(view.doc_named(&name).is_some()))
    });
    locals.add_function("toctree", move |c| toctree(view, page, trace, c));
    locals
}

/// `_(message)` and `gettext(message)`: Guidedog's interface words in the build's
/// language.
fn word(view: &BuildView<'_>, c: &Call<'_, '_>) -> Value {
    let message = gd_jinja::to_str(&c.arg_or(0, "message", Value::from(""))).into_owned();
    Value::from(view.ui.gettext(&message).into_owned())
}

/// `ngettext(singular, plural, n)`.
fn plural(view: &BuildView<'_>, c: &Call<'_, '_>) -> Result<Value, Error> {
    let one = gd_jinja::to_str(&c.arg_or(0, "singular", Value::from(""))).into_owned();
    let many = gd_jinja::to_str(&c.arg_or(1, "plural", Value::from(""))).into_owned();
    let n = c.arg_int(2, "n", 1)?.unsigned_abs();
    Ok(Value::from(view.ui.ngettext(&one, &many, n).into_owned()))
}

/// Sphinx's `toctree()` with its keyword arguments: `maxdepth` (-1 unlimited),
/// `collapse` (default true), `titles_only`, `includehidden`.
fn toctree(
    view: &BuildView<'_>,
    page: &PageState,
    trace: &RefCell<Vec<Lookup>>,
    c: &Call<'_, '_>,
) -> Result<Value, Error> {
    let mut options = TocOptions::TOCTREE;
    let depth = c.arg_int(KEYWORD_ONLY, "maxdepth", -1)?;
    if depth > 0 {
        options.maxdepth = u32::try_from(depth).unwrap_or(u32::MAX);
    }
    options.collapse = c.arg_flag(KEYWORD_ONLY, "collapse", options.collapse);
    options.titles_only = c.arg_flag(KEYWORD_ONLY, "titles_only", false);
    options.include_hidden = c.arg_flag(KEYWORD_ONLY, "includehidden", false);
    let key = format!(
        "toctree\0{}/{}/{}/{}",
        options.maxdepth, options.collapse, options.titles_only, options.include_hidden
    );
    trace.borrow_mut().push(crate::navigation_trace::lookup(&key));
    Ok(Value::markup(&sidebar_html(view, page.doc, options)))
}

/// Sphinx's `pathto(otheruri, resource=False)`: a document's page, or with `resource`
/// a file below the output root, from the current page. In singlehtml a document is
/// its section of the single page. Any other name, such as genindex or a page of
/// html_additional_pages, is a page at the output root, with every builder.
pub(crate) fn path_to(
    view: &BuildView<'_>,
    page: &PageState,
    target: &str,
    resource: bool,
) -> String {
    if target.contains("://") || target.starts_with('#') {
        return target.into();
    }
    if resource {
        return format!("{}{target}", page.root);
    }
    let Some(id) = view.doc_named(target) else {
        return format!("{}{target}{}", page.root, view.config.html.file_suffix);
    };
    if view.kind == BuilderKind::Singlehtml {
        return view.page_url(page.doc, id, "");
    }
    format!("{}{}", page.root, view.page_name(target))
}

/// A `css_files` or `js_files` entry's URL: a dict's `url` (none for an inline script),
/// or the entry itself as a string.
fn url_of(c: &Call<'_, '_>) -> String {
    let entry = c.arg(0, "file");
    let url = entry.get("url");
    if !url.is_undefined() {
        return gd_jinja::to_str(&url).into_owned();
    }
    if matches!(entry, Value::Dict(_)) {
        return String::new();
    }
    gd_jinja::to_str(&entry).into_owned()
}

fn css_tag(c: &Call<'_, '_>) -> Value {
    let mut out = String::from("<link rel=\"stylesheet\" href=\"");
    crate::escape::push_escaped(&mut out, &url_of(c));
    out.push_str("\" type=\"text/css\" />");
    Value::markup(&out)
}

/// The source of a template: the project's (in `templates_path`) unless the name asks
/// for the theme's own with `!`, then the built-in theme's.
fn template_source(sources: &TemplateSources, name: &str) -> Option<String> {
    if !name.starts_with('!')
        && let Some(path) = template_path(sources, name)
    {
        match gd_host::read_file(&path, None, gd_host::MAX_SOURCE_BYTES) {
            Ok(data) => return Some(String::from_utf8_lossy(&data).into_owned()),
            Err(p) => sources.errors.borrow_mut().push(p.report()),
        }
        return None;
    }
    match builtin_name(name) {
        "layout.html" => Some(gd_theme::LAYOUT_HTML.to_string()),
        "page.html" => Some("{% extends \"layout.html\" %}".into()),
        "article.html" => Some(gd_theme::ARTICLE_HTML.to_string()),
        "landing.html" => Some(gd_theme::LANDING_HTML.to_string()),
        "collection.html" => Some("{% extends \"layout.html\" %}".into()),
        _ => None,
    }
}

/// The theme template a Sphinx name refers to. Sphinx writes `!layout.html` for the
/// theme's own layout and `<theme>/layout.html` for a named theme's. Guidedog's theme
/// stands in for every theme it accepts (`KNOWN_THEMES`) and for `basic`, the theme
/// Sphinx's themes derive from, so all of these give `layout.html`.
fn builtin_name(name: &str) -> &str {
    let n = name.strip_prefix('!').unwrap_or(name);
    let Some((theme, file)) = n.split_once('/') else { return n };
    if theme == "basic" || gd_conf::KNOWN_THEMES.contains(&theme) { file } else { n }
}

/// Finds a template among the project's template files. Names cannot leave
/// `templates_path`.
fn template_path(sources: &TemplateSources, name: &str) -> Option<PathBuf> {
    if name.contains("..") || name.starts_with('/') || name.is_empty() {
        return None;
    }
    sources
        .dirs
        .iter()
        .map(|dir| gd_conf::clean_path(&sources.source_dir.join(dir).join(name)))
        .find(|path| sources.files.contains(path))
}

/// Every file below `templates_path`, in its subfolders too, since the loader finds
/// included templates by relative paths such as `partials/nav.html`. A folder or file
/// outside the readable folders, as named or through a link, is left out with a
/// warning, so it is neither loaded nor a page's input.
fn template_files(view: &BuildView<'_>, reports: &mut Vec<Report>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in &view.config.templates_path {
        let path = gd_conf::clean_path(&view.config.source_dir.join(dir));
        if !path.is_dir() {
            continue;
        }
        let why = gd_host::confinement(view.roots, &path);
        let what = "templates_path folder";
        if let Some(w) =
            gd_conf::unread_warning("build.template", "TEMPLATES NOT USED", &path, what, why)
        {
            reports.push(w);
            continue;
        }
        list_templates(view.roots, &path, &mut out, reports);
    }
    out
}

/// Lists the files below `dir` in name order, depth first, leaving out hidden names.
fn list_templates(
    roots: &[PathBuf],
    dir: &Path,
    out: &mut Vec<PathBuf>,
    reports: &mut Vec<Report>,
) {
    let mut pending = vec![dir.to_path_buf()];
    while let Some(folder) = pending.pop() {
        let Ok(mut entries) = gd_host::list_folder(&folder) else { continue };
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        let mut folders = Vec::new();
        for e in entries {
            if e.name.to_string_lossy().starts_with('.') {
                continue;
            }
            let path = folder.join(&e.name);
            let why = gd_host::confinement(roots, &path);
            if why != Confinement::Inside {
                let w = gd_conf::unread_warning(
                    "build.template",
                    "TEMPLATE NOT USED",
                    &path,
                    "template",
                    why,
                );
                reports.extend(w);
                continue;
            }
            if path.is_dir() {
                folders.push(path);
            } else if path.is_file() {
                out.push(path);
            }
        }
        pending.extend(folders.into_iter().rev());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_names_lead_to_the_built_in_layout() {
        assert_eq!(builtin_name("!layout.html"), "layout.html");
        assert_eq!(builtin_name("basic/layout.html"), "layout.html");
        assert_eq!(builtin_name("alabaster/layout.html"), "layout.html");
        assert_eq!(builtin_name("partials/nav.html"), "partials/nav.html");
    }
}
