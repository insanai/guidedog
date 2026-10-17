//! Render once per listing occurrence, shared by its default body and template data.

use gd_env::{Asset, Digest, DocId, Lookup, OutputFormat};
use gd_host::Problem;
use gd_jinja::Value;
use gd_link::{Excerpt, PageRequest, Worker, render_excerpt, render_options};
use gd_report::Report;
use guidedoc_defaults::HtmlHighlighter;

use crate::site::Site;

#[derive(Default)]
pub(crate) struct Posts {
    pub html: Vec<String>,
    pub values: Vec<Value>,
    pub assets: Vec<Asset>,
    pub lookups: Vec<(Lookup, Digest)>,
    pub reports: Vec<Report>,
}

pub(crate) fn render(site: &Site<'_, '_>, file: &str, posts: &[DocId]) -> Result<Posts, Problem> {
    let view = site.view;
    let mut out = Posts::default();
    let mut worker = Worker::new();
    let highlighter = HtmlHighlighter::new();
    for &id in posts {
        let doc = view.doc(id);
        let html = if doc.excerpt {
            let prefix = format!("excerpt-{}-", view.doc_key(id));
            let excerpt = Excerpt { destination: file, prefix: &prefix };
            let mut options = render_options(view, id, OutputFormat::Html);
            options.permalinks = false;
            options.highlighter = Some(&highlighter);
            options.labels = site.labels;
            let request = PageRequest { doc: id, renderer: "html", options, ..Default::default() };
            let rendered = render_excerpt(view, &mut worker, &request, &excerpt);
            if rendered.failed {
                return Err(Problem::new(
                    "site.excerpt",
                    "CANNOT RENDER EXCERPT",
                    &doc.relative,
                    "The article introduction could not be rendered.",
                    "Fix the article's reported errors and build again.",
                    5,
                ));
            }
            out.lookups.extend(rendered.trace.into_lookups().into_iter().map(|l| {
                let digest = gd_resolve::lookup_answer(view, file, &l);
                (l, digest)
            }));
            out.assets.extend(rendered.assets);
            out.reports.extend(rendered.reports);
            rendered.body
        } else if doc.page.summary.is_empty() {
            String::new()
        } else {
            let mut html = String::from("<p>");
            crate::escape::push_escaped(&mut html, &doc.page.summary);
            html.push_str("</p>");
            html
        };
        let value = crate::collections::post_value(view, file, id);
        value.set("excerpt_html", Value::markup(&html));
        out.values.push(value);
        out.html.push(html);
    }
    Ok(out)
}
