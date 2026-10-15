//! The page layout (templates.odin and the template functions of html_site.odin and
//! intl.odin): `layout.html` in a gd-jinja environment whose loader looks first in
//! `templates_path`, then among the built-in theme's templates (gd-theme).
//!
//! gd-jinja's functions are `'static` closures, so they cannot borrow the build. What
//! they need is either owned here (the documents' page names for `pathto` and
//! `hasdoc`), or asked of the build between renders: `_()`, `gettext()`, `ngettext()`
//! and `toctree()` answer from a cache; a question the cache cannot answer yet is noted
//! and answered with a placeholder, and the page is rendered again once the builder
//! has answered it ([`Layout::render`]). Interface words stay cached for the whole
//! build, so after the first pages a page renders once.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gd_env::{BuilderKind, DocId};
use gd_host::{Confinement, Problem};
use gd_jinja::{Autoescape, Call, Environment, Error, ErrorKind, Loader, Options, Value};
use gd_report::Report;
use gd_resolve::BuildView;

use crate::mathjax::script_tag;
use crate::nav::{TocOptions, sidebar_html};

/// The layout every page is rendered with.
pub(crate) const LAYOUT: &str = "layout.html";

/// How many times a page is rendered at most while its questions are answered: a
/// template whose questions depend on earlier answers may need a third render.
const MAX_RENDERS: usize = 3;

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

/// What a template function asks of the build.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Question {
    /// `_(message)` and `gettext(message)`.
    Word(String),
    /// `ngettext(singular, plural, n)`.
    Plural(String, String, u64),
    /// `toctree(...)`, for the current page.
    Toctree(TocOptions),
}

/// The answers known so far, and the questions asked that have none yet.
#[derive(Debug, Default)]
struct Answers {
    known: HashMap<Question, String>,
    asked: Vec<Question>,
}

impl Answers {
    /// The answer to `question`, or `None` after noting it to be answered.
    fn answer(&mut self, question: Question) -> Option<String> {
        if let Some(known) = self.known.get(&question) {
            return Some(known.clone());
        }
        if !self.asked.contains(&question) {
            self.asked.push(question);
        }
        None
    }
}

/// The names `pathto` and `hasdoc` resolve, owned for the template functions.
#[derive(Debug, Default)]
struct SiteLinks {
    kind: BuilderKind,
    file_suffix: String,
    /// Per docname: its page below the output folder, and in singlehtml the fragment
    /// of its section of the single page.
    pages: HashMap<String, (String, String)>,
    /// The root document's page (the single page of singlehtml).
    root_page: String,
}

/// What the template functions share with the builder.
#[derive(Debug, Default)]
struct TemplateState {
    page: RefCell<PageState>,
    answers: RefCell<Answers>,
    links: SiteLinks,
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
    state: Rc<TemplateState>,
    sources: Rc<TemplateSources>,
    files: Vec<PathBuf>,
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
        let state = Rc::new(TemplateState { links: site_links(view), ..TemplateState::default() });
        let loader_sources = Rc::clone(&sources);
        let mut env = Environment::new(Options {
            autoescape: Autoescape::Always,
            loader: Some(Loader::custom(move |name| template_source(&loader_sources, name))),
            ..Options::default()
        });
        register_functions(&mut env, &state);
        let layout = Layout { env, state, sources, files };
        if let Err(e) = layout.env.load_template(LAYOUT) {
            return Err(layout.problem(&e));
        }
        Ok(layout)
    }

    /// The template files the build uses: every page depends on all of them.
    pub(crate) fn files(&self) -> &[PathBuf] {
        &self.files
    }

    /// Starts a page: the template functions work for it from now on.
    pub(crate) fn begin_page(&self, page: PageState) {
        *self.state.page.borrow_mut() = page;
        let mut answers = self.state.answers.borrow_mut();
        answers.known.retain(|q, _| !matches!(q, Question::Toctree(_)));
        answers.asked.clear();
    }

    /// Renders the template `name` with `ctx` for the current page, answering what its
    /// functions asked and rendering again until nothing is left unanswered. Problems
    /// reading a template it loads are added to `reports`. Fails with `template.error`.
    pub(crate) fn render(
        &self,
        view: &BuildView<'_>,
        name: &str,
        ctx: &Value,
        reports: &mut Vec<Report>,
    ) -> Result<String, Problem> {
        let mut renders = 0;
        loop {
            renders += 1;
            let rendered = self.env.load_template(name).and_then(|t| t.render(ctx));
            reports.append(&mut self.sources.errors.borrow_mut());
            let html = rendered.map_err(|e| self.problem(&e))?;
            let asked = std::mem::take(&mut self.state.answers.borrow_mut().asked);
            if asked.is_empty() || renders == MAX_RENDERS {
                return Ok(html);
            }
            let doc = self.state.page.borrow().doc;
            let mut answers = self.state.answers.borrow_mut();
            for question in asked {
                let answer = answer(view, doc, &question);
                answers.known.insert(question, answer);
            }
        }
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
        let hint = if e.hint.is_empty() {
            "Fix the template, or delete it to use Guidedog's built-in one.".to_string()
        } else {
            e.hint.clone()
        };
        Problem::new("template.error", template_title(e.kind), &location, message, hint, 1)
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

/// Answers a question with the build.
fn answer(view: &BuildView<'_>, doc: Option<DocId>, question: &Question) -> String {
    match question {
        Question::Word(message) => view.ui.gettext(message).into_owned(),
        Question::Plural(one, many, n) => view.ui.ngettext(one, many, *n).into_owned(),
        Question::Toctree(options) => sidebar_html(view, doc, *options),
    }
}

/// The page names `pathto` and `hasdoc` need.
fn site_links(view: &BuildView<'_>) -> SiteLinks {
    let pages = view
        .env
        .iter()
        .map(|(id, d)| {
            let fragment = if view.kind == BuilderKind::Singlehtml {
                view.page_anchor(id, "")
            } else {
                String::new()
            };
            (d.docname.clone(), (view.page_name(&d.docname), fragment))
        })
        .collect();
    SiteLinks {
        kind: view.kind,
        file_suffix: view.config.html.file_suffix.clone(),
        pages,
        root_page: view.page_name(&view.config.root_doc),
    }
}

/// Registers Guidedog's template functions and Sphinx's fixed names.
fn register_functions(env: &mut Environment, state: &Rc<TemplateState>) {
    let s = Rc::clone(state);
    env.add_function("_", move |c| Ok(word(&s, c)));
    let s = Rc::clone(state);
    env.add_function("gettext", move |c| Ok(word(&s, c)));
    let s = Rc::clone(state);
    env.add_function("ngettext", move |c| plural(&s, c));
    let s = Rc::clone(state);
    env.add_function("pathto", move |c| Ok(pathto(&s, c)));
    let s = Rc::clone(state);
    env.add_function("hasdoc", move |c| {
        let name = gd_jinja::to_str(&c.arg(0, "name")).into_owned();
        Ok(Value::from(s.links.pages.contains_key(&name)))
    });
    let s = Rc::clone(state);
    env.add_function("toctree", move |c| toctree(&s, c));
    env.add_function("css_tag", |c| Ok(css_tag(c)));
    env.add_function("js_tag", |c| Ok(Value::markup(&script_tag(&c.arg(0, "file"), &url_of(c)))));
    // The separators Sphinx's basic theme defines for its navigation bars.
    env.add_global("reldelim1", Value::markup(" &#187;"));
    env.add_global("reldelim2", Value::markup(" |"));
    env.add_global("embedded", Value::from(false)); // true only for help-file builders.
}

/// `_(message)` and `gettext(message)`: Guidedog's interface words in the build's
/// language.
fn word(state: &TemplateState, c: &Call<'_, '_>) -> Value {
    let message = gd_jinja::to_str(&c.arg_or(0, "message", Value::from(""))).into_owned();
    let asked = state.answers.borrow_mut().answer(Question::Word(message.clone()));
    Value::from(asked.unwrap_or(message))
}

/// `ngettext(singular, plural, n)`.
fn plural(state: &TemplateState, c: &Call<'_, '_>) -> Result<Value, Error> {
    let one = gd_jinja::to_str(&c.arg_or(0, "singular", Value::from(""))).into_owned();
    let many = gd_jinja::to_str(&c.arg_or(1, "plural", Value::from(""))).into_owned();
    let n = c.arg_int(2, "n", 1)?.unsigned_abs();
    let fallback = if n == 1 { one.clone() } else { many.clone() };
    let asked = state.answers.borrow_mut().answer(Question::Plural(one, many, n));
    Ok(Value::from(asked.unwrap_or(fallback)))
}

/// Sphinx's `toctree()` with its keyword arguments: `maxdepth` (-1 unlimited),
/// `collapse` (default true), `titles_only`, `includehidden`.
fn toctree(state: &TemplateState, c: &Call<'_, '_>) -> Result<Value, Error> {
    let mut options = TocOptions::TOCTREE;
    let depth = c.arg_int(KEYWORD_ONLY, "maxdepth", -1)?;
    if depth > 0 {
        options.maxdepth = u32::try_from(depth).unwrap_or(u32::MAX);
    }
    options.collapse = c.arg_flag(KEYWORD_ONLY, "collapse", options.collapse);
    options.titles_only = c.arg_flag(KEYWORD_ONLY, "titles_only", false);
    options.include_hidden = c.arg_flag(KEYWORD_ONLY, "includehidden", false);
    let asked = state.answers.borrow_mut().answer(Question::Toctree(options));
    Ok(Value::markup(&asked.unwrap_or_default()))
}

/// Sphinx's `pathto(otheruri, resource=False)`: a document's page, or with `resource`
/// a file below the output root, from the current page. In singlehtml a document is
/// its section of the single page. Any other name, such as genindex or a page of
/// html_additional_pages, is a page at the output root, with every builder.
fn pathto(state: &TemplateState, c: &Call<'_, '_>) -> Value {
    let target = gd_jinja::to_str(&c.arg_or(0, "otheruri", Value::from(""))).into_owned();
    if target.contains("://") || target.starts_with('#') {
        return Value::from(target);
    }
    let page = state.page.borrow();
    if c.arg_flag(1, "resource", false) {
        return Value::from(format!("{}{target}", page.root));
    }
    let links = &state.links;
    let Some((name, fragment)) = links.pages.get(&target) else {
        return Value::from(format!("{}{target}{}", page.root, links.file_suffix));
    };
    if links.kind != BuilderKind::Singlehtml {
        return Value::from(format!("{}{name}", page.root));
    }
    if page.page == links.root_page {
        return Value::from(format!("#{fragment}"));
    }
    Value::from(format!("{}#{fragment}", gd_resolve::relative_url(&page.page, &links.root_page)))
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

    #[test]
    fn a_question_is_asked_once_until_answered() {
        let mut answers = Answers::default();
        assert_eq!(answers.answer(Question::Word("Index".into())), None);
        assert_eq!(answers.answer(Question::Word("Index".into())), None);
        assert_eq!(answers.asked.len(), 1);
        answers.known.insert(Question::Word("Index".into()), "Stichwortverzeichnis".into());
        assert_eq!(
            answers.answer(Question::Word("Index".into())).as_deref(),
            Some("Stichwortverzeichnis")
        );
    }
}
