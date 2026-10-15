//! Graphs: sphinx.ext.graphviz's directives (graphviz, graph, digraph),
//! which the reader leaves as DOT code blocks marked `graphviz`, and ```` ```dot ````
//! fences in Markdown. The link pass lays each out with Graphviz (gd-graphdog, one
//! context per worker) and shows it as an SVG image in pages and books. A graph that
//! cannot be drawn stays code, with a warning.

use std::path::PathBuf;

use gd_conf::Digest;
use gd_env::{Asset, LookupKind, OutputFormat, SourceKind};
use gd_graphdog::{Attribute, AttributeKind, Context, Engine, Format, Options};
use gd_resolve::relative_url;
use guidedoc::{AttributeKey, Document, Fail, LinkInfo, LinkKind, NodeId, NodeKind, SourceSpan};

use crate::graph_files::{GraphFiles, may_name_files};
use crate::graph_theme::{GRAPH_ADAPTIVE_CSS, adapt_graph, graph_adaptive};
use crate::image::{book_path, file_name};
use crate::page::{Page, Warning, flag};
use crate::rewrite::{Rewriter, Rule};

/// Whether a code block is a graph to draw.
pub(crate) fn is_graph(page: &Page<'_>, doc: Document<'_>, id: NodeId) -> bool {
    if flag(doc, id, "graphviz") {
        return true;
    }
    page.info.kind == SourceKind::Markdown
        && matches!(doc.attribute(id, AttributeKey::Language), Some("dot" | "graphviz"))
}

/// Writes the graph as an image, in a figure when it has a caption.
pub(crate) fn replace_graph(
    page: &mut Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    id: NodeId,
) -> Result<(), Fail> {
    let doc = rw.doc;
    let source = doc.node_text(id);
    let span = doc.node(id).span;
    let adaptive = graph_adaptive(doc, id);
    let layout = doc.custom_attribute(id, "layout").unwrap_or("");
    let drawn =
        draw_graph(page, doc, source, layout, span, adaptive && page.format == OutputFormat::Html);
    let Some(path) = drawn else { return rw.copy_node(page, id) };
    let caption = doc.custom_attribute(id, "caption");
    let outer =
        rw.open(if caption.is_some() { NodeKind::Figure } else { NodeKind::Image }, span)?;
    rw.moved_to(id, outer);
    // The code block's ids, :name: and classes move to the image.
    rw.copy_attributes(id, outer, |key, name| {
        matches!(key, AttributeKey::Id | AttributeKey::Class)
            || (key == AttributeKey::Custom && name == "name")
    })?;
    let image = match caption {
        Some(_) => {
            page.decorate(rw, id, outer)?; // its figure number.
            rw.open(NodeKind::Image, span)?
        }
        None => outer,
    };
    let alt = doc.custom_attribute(id, "alt").unwrap_or_else(|| source.trim());
    rw.b.add_attribute_string(image, AttributeKey::Alt, alt)?;
    if let Some(align) = doc.custom_attribute(id, "align") {
        rw.b.add_attribute_string(image, AttributeKey::Align, align)?;
    }
    rw.add_class(image, "graphviz")?;
    if adaptive && caption.is_some() {
        rw.add_class(image, "graphviz-adaptive")?;
    }
    let url = graph_url(page, path);
    let destination = rw.b.add_text(&url)?;
    rw.b.set_link(
        image,
        LinkInfo { destination, kind: LinkKind::External, ..LinkInfo::default() },
    )?;
    if let Some(text) = caption {
        rw.close(image)?;
        match doc.code_caption(id) {
            Some(parsed) => rw.copy_node(page, parsed)?,
            None => {
                let c = rw.open(NodeKind::Caption, span)?;
                rw.add_plain(text, span)?;
                rw.close(c)?;
            }
        }
    }
    rw.close(outer)
}

/// Lays the graph out once per distinct source, engine, and arguments, into
/// `graphs/graphviz-<digest>.svg` below the doctrees folder, which every builder
/// shares, and returns the file. A graph that may name files is drawn every time,
/// since the files are part of the drawing. `None` (with a warning) when it cannot be
/// drawn.
fn draw_graph(
    page: &mut Page<'_>,
    doc: Document<'_>,
    source: &str,
    layout: &str,
    span: SourceSpan,
    adaptive: bool,
) -> Option<PathBuf> {
    let config = page.config();
    let args = config.graphviz.dot_args.join(" ");
    let mut key = Digest::of_parts(&[source.as_bytes(), layout.as_bytes(), args.as_bytes()]);
    if adaptive {
        key = Digest::of_parts(&[&key.0, GRAPH_ADAPTIVE_CSS.as_bytes()]);
    }
    let hex = key.to_string();
    let name = format!("graphviz-{}.svg", &hex[..24]);
    let path = gd_env::doctrees_dir(config).join("graphs").join(name);
    if !may_name_files(source, &args) && path.is_file() {
        return Some(path);
    }
    let engine = if layout.is_empty() { Some(Engine::Dot) } else { Engine::from_name(layout) };
    let Some(engine) = engine else {
        let warning = Warning {
            code: "build.graphviz",
            title: "UNKNOWN GRAPH LAYOUT",
            category: "",
            message: format!("Graphviz has no layout engine named \"{layout}\"."),
            hint: "Use dot, neato, fdp, sfdp, circo, twopi, osage, or patchwork.",
        };
        page.warn(doc, span, warning);
        return None;
    };
    let context = graph_context(page)?;
    let svg = match render_svg(page, context, source, engine, adaptive) {
        Ok(svg) => svg,
        Err(failure) => {
            let warning = Warning {
                code: "build.graphviz",
                title: "GRAPH NOT DRAWN",
                category: "",
                message: failure.message,
                hint: &failure.hint,
            };
            page.warn(doc, span, warning);
            return None;
        }
    };
    if let Err(problem) = gd_host::publish(&path, &svg, true) {
        page.reports.push(problem.report());
        return None;
    }
    Some(path)
}

/// Why a graph was not drawn.
struct DrawFailure {
    message: String,
    hint: String,
}

/// Renders the graph to SVG with the files it names embedded, adapted when asked.
fn render_svg(
    page: &mut Page<'_>,
    context: &Context,
    source: &str,
    engine: Engine,
    adaptive: bool,
) -> Result<Vec<u8>, DrawFailure> {
    let config = page.config();
    let ledger = page.ledger;
    let base = config.source_dir.join(&page.info.relative);
    let mut files = GraphFiles {
        untrusted: page.view.options.untrusted,
        base: base.parent().map(PathBuf::from).unwrap_or_default(),
        roots: page.view.roots,
        named: Vec::new(),
        looked: Vec::new(),
        problem: String::new(),
    };
    let attributes = graph_attributes(&config.graphviz.dot_args);
    let rendered = {
        let mut decide = |attribute: &str, name: &str| files.decide(attribute, name);
        let mut reserve = |bytes: usize| ledger.charge(bytes);
        let options = Options {
            engine,
            format: Format::Svg,
            attributes: &attributes,
            files: Some(&mut decide),
            reserve: Some(&mut reserve),
        };
        context.render(source, options)
    };
    for path in files.looked.drain(..) {
        page.trace.note_key(LookupKind::File, &path.to_string_lossy());
    }
    let fail = |message: String, hint: &str, problem: &str| DrawFailure {
        message,
        hint: if problem.is_empty() { hint.to_string() } else { problem.to_string() },
    };
    let default_hint = "Fix the DOT source; the graph is shown as code.";
    let svg = match rendered {
        Ok(r) => r.output,
        Err(e) => {
            let at = if e.line > 0 {
                format!(" (line {} of the graph)", e.line)
            } else {
                String::new()
            };
            let hint = if e.hint.is_empty() { default_hint } else { e.hint.as_str() };
            return Err(fail(format!("{}{at}", e.message), hint, &files.problem));
        }
    };
    let svg = files.embed(svg, ledger).map_err(|m| fail(m, default_hint, &files.problem))?;
    if !adaptive {
        return Ok(svg);
    }
    adapt_graph(&svg, ledger).map_err(|e| fail(e.message.to_string(), e.hint, ""))
}

/// The worker's Graphviz context, started on first use; a worker that draws no graph
/// never starts one. When Graphviz cannot start, the first graph says so.
fn graph_context<'p>(page: &mut Page<'p>) -> Option<&'p Context> {
    let graphs = page.graphs;
    let mut fresh = false;
    let context = graphs.get_or_init(|| {
        fresh = true;
        Context::new().ok()
    });
    if context.is_none() && fresh {
        let problem = gd_host::Problem::new(
            "build.graphviz",
            "GRAPHVIZ DID NOT START",
            "",
            "Graphviz could not load its layout plugins.",
            "Run sudo dot -c to register them, or use a static build of guidedog.",
            1,
        );
        page.reports.push(problem.report());
    }
    context.as_ref()
}

/// Reads `graphviz_dot_args`: `-Gname=value`, `-Nname=value`, `-Ename=value`.
fn graph_attributes(args: &[String]) -> Vec<Attribute> {
    let mut out = Vec::new();
    for arg in args {
        let Some(rest) = arg.strip_prefix('-').filter(|r| r.len() >= 3) else { continue };
        let kind = match rest.as_bytes()[0] {
            b'G' => AttributeKind::Graph,
            b'N' => AttributeKind::Node,
            b'E' => AttributeKind::Edge,
            _ => continue,
        };
        let (name, value) = rest[1..].split_once('=').unwrap_or((&rest[1..], ""));
        out.push(Attribute { kind, name: name.to_string(), value: value.to_string() });
    }
    out
}

/// Where the page or the book finds the drawn graph; the page copies it to `_images`
/// as it copies other images.
fn graph_url(page: &mut Page<'_>, path: PathBuf) -> String {
    if page.format == OutputFormat::Pdf {
        let book = book_path(page.config(), &path);
        page.assets.push(Asset { source: path, target: book.clone() });
        return book;
    }
    let target = format!("_images/{}", file_name(&path));
    let url = relative_url(&page.view.page_name(&page.info.docname), &target);
    page.assets.push(Asset { source: path, target });
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_arguments_become_default_attributes() {
        let args: Vec<String> =
            ["-Grankdir=LR", "-Nshape=box", "-Ecolor=red", "-Tsvg", "-G"].map(String::from).into();
        let attributes = graph_attributes(&args);
        assert_eq!(attributes.len(), 3);
        assert_eq!(attributes[0].kind, AttributeKind::Graph);
        assert_eq!(attributes[1].name, "shape");
        assert_eq!(attributes[2].value, "red");
    }
}
