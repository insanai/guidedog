//! sphinx.ext.viewcode's pages: for every Python module that defines a described
//! object, `_modules/<package>/<module>.html` holds the module's source, highlighted
//! as code blocks are, with each described class and function wrapped in a block that
//! links back to its description (`[docs]`); `_modules/index.html` lists the modules.
//! Reading found the objects and their lines (`DocInfo::locations`); the link pass
//! added the `[source]` links that lead here.

use std::collections::BTreeMap;
use std::path::PathBuf;

use gd_env::{BuilderKind, DocId};
use gd_highlight::{HtmlLines, Lexer};
use gd_host::Problem;
use gd_link::{MODULES_FOLDER, module_page};
use gd_outputs::Unit;
use gd_report::Report;
use gd_resolve::{BuildView, relative_url};

use crate::escape::push_escaped;
use crate::site::Site;

/// One described object of a module: its name in the module, its lines, and the
/// description it links back to.
#[derive(Debug, Clone)]
struct Block {
    line: u32,
    end_line: u32,
    doc: DocId,
    anchor: String,
}

/// A module with described objects: its file, and its blocks by name.
#[derive(Debug, Default)]
struct SourceModule {
    path: PathBuf,
    blocks: BTreeMap<String, Block>,
}

/// Writes the source pages and their index into the site unit.
pub(crate) fn write_source_pages(
    site: &Site<'_, '_>,
    unit: &mut Unit<'_>,
    reports: &mut Vec<Report>,
) -> Result<(), Problem> {
    let view = site.view;
    let modules = source_modules(view);
    if modules.is_empty() {
        return Ok(());
    }
    let suffix = &view.config.html.file_suffix;
    let mut written = Vec::new();
    for (name, module) in &modules {
        let Some(code) = module_source(view, name, module, reports) else { continue };
        let file = module_page(name, suffix);
        let body = source_page_body(view, &file, name, &code, module);
        let html = site.special_page(&file, name, &body, reports)?;
        unit.emit_text(&file, &html)?;
        written.push(name.as_str());
    }
    let index = format!("{MODULES_FOLDER}/index{suffix}");
    let title = view.ui.gettext("Overview: module code");
    let body = index_body(view, &index, &written);
    let html = site.special_page(&index, &title, &body, reports)?;
    unit.emit_text(&index, &html)
}

/// The modules the documents' Python objects are defined in, by name. When two
/// descriptions describe one object, the first document in name order is linked.
fn source_modules(view: &BuildView<'_>) -> BTreeMap<String, SourceModule> {
    let mut modules: BTreeMap<String, SourceModule> = BTreeMap::new();
    for (id, d) in view.env.iter().filter(|(_, d)| !d.failed) {
        for l in d.locations.iter().filter(|l| l.domain == "py") {
            let module = modules.entry(l.defined_in.clone()).or_default();
            module.path.clone_from(&l.path);
            module.blocks.entry(l.fullname.clone()).or_insert_with(|| Block {
                line: l.line,
                end_line: l.end_line,
                doc: id,
                anchor: l.anchor.clone(),
            });
        }
    }
    modules
}

/// The module's source, read within the readable folders and the source size limit;
/// `None`, with a warning, when it cannot be read.
fn module_source(
    view: &BuildView<'_>,
    name: &str,
    module: &SourceModule,
    reports: &mut Vec<Report>,
) -> Option<String> {
    // The files autodoc reads, the same folders reading found the module in.
    let roots = gd_host::canonical_roots(&gd_conf::autodoc_roots(view.config));
    let read = if gd_host::confined(&roots, &module.path) {
        gd_host::read_file(&module.path, None, gd_host::MAX_SOURCE_BYTES)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
    } else {
        None
    };
    if read.is_none() {
        let message = format!(
            "The source of the module {name} ({}) could not be read, so it has no source page.",
            module.path.display()
        );
        reports.push(Report::warning("viewcode.limit", "MODULE SOURCE NOT READ", &message).hint(
            "Keep the module below autodoc_source_paths, in UTF-8, and smaller than 8 MiB.",
        ));
    }
    read
}

/// The source page's body: the heading, then the highlighted source with a block per
/// described object, as Sphinx's `collect_pages` writes it, but without the line
/// breaks around the blocks' tags, which a browser shows as empty lines.
fn source_page_body(
    view: &BuildView<'_>,
    file: &str,
    name: &str,
    code: &str,
    module: &SourceModule,
) -> String {
    let first = usize::from(view.config.ext.viewcode.line_numbers);
    let mut highlighted = String::with_capacity(code.len() * 3);
    let lines = HtmlLines { first, emphasized: &[] };
    gd_highlight::write_code_html_lines(Lexer::Python, code, lines, &mut highlighted);
    let mut lines: Vec<String> = highlighted.split('\n').map(str::to_owned).collect();
    let docs = view.ui.gettext("[docs]");
    // A line that ends a block ends where the block does: the newline after it would
    // show as an empty line, since the block itself ends the line.
    let mut closes = vec![false; lines.len()];
    for (fullname, block) in &module.blocks {
        let (start, end) = (block.line as usize, block.end_line as usize);
        if start == 0 || start > lines.len() {
            continue;
        }
        let mut open = String::from("<div class=\"viewcode-block\" id=\"");
        push_escaped(&mut open, fullname);
        open.push_str("\"><a class=\"viewcode-back\" href=\"");
        push_escaped(&mut open, &back_link(view, file, block));
        open.push_str("\">");
        push_escaped(&mut open, &docs);
        open.push_str("</a>");
        lines[start - 1].insert_str(0, &open);
        let last = end.clamp(start, lines.len());
        lines[last - 1].push_str("</div>");
        closes[last - 1] = true;
    }
    let heading = view.ui.gettext("<h1>Source code for %s</h1>");
    let mut escaped = String::new();
    push_escaped(&mut escaped, name);
    let mut body = heading.replace("%s", &escaped);
    body.push_str("<div class=\"highlight\"><pre><span></span>");
    for (i, line) in lines.iter().enumerate() {
        body.push_str(line);
        if i + 1 < lines.len() && !closes[i] {
            body.push('\n');
        }
    }
    body.push_str("</pre></div>");
    body
}

/// The URL from the source page `file` to the description of `block`.
fn back_link(view: &BuildView<'_>, file: &str, block: &Block) -> String {
    let c = view.config;
    let mut target = view.page_name(&view.doc(block.doc).docname);
    if !c.html.link_suffix.is_empty() && view.kind == BuilderKind::Html {
        let stem = target.strip_suffix(c.html.file_suffix.as_str()).map(str::to_owned);
        target = format!("{}{}", stem.unwrap_or(target), c.html.link_suffix);
    }
    let mut url = relative_url(file, &target);
    if view.kind == BuilderKind::Dirhtml {
        url = url.strip_suffix("index.html").unwrap_or(&url).to_owned();
    }
    format!("{url}#{}", block.anchor)
}

/// `_modules/index.html`'s body: the modules as nested lists, a package's submodules
/// inside it, as Sphinx writes them.
fn index_body(view: &BuildView<'_>, index: &str, modules: &[&str]) -> String {
    let mut out = view.ui.gettext("<h1>All modules for which code is available</h1>").into_owned();
    out.push('\n');
    let suffix = &view.config.html.file_suffix;
    let mut stack: Vec<String> = vec![String::new()];
    for name in modules {
        if name.starts_with(stack.last().map_or("", String::as_str)) {
            out.push_str("<ul>");
        } else {
            stack.pop();
            while !name.starts_with(stack.last().map_or("", String::as_str)) {
                stack.pop();
                out.push_str("</ul>");
            }
        }
        stack.push(format!("{name}."));
        out.push_str("<li><a href=\"");
        push_escaped(&mut out, &relative_url(index, &module_page(name, suffix)));
        out.push_str("\">");
        push_escaped(&mut out, name);
        out.push_str("</a></li>\n");
    }
    out.push_str(&"</ul>".repeat(stack.len().saturating_sub(1)));
    out
}

/// Whether this build writes source pages.
pub(crate) fn has_source_pages(view: &BuildView<'_>) -> bool {
    gd_link::viewcode_pages(view.config, view.kind)
}
