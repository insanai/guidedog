//! Generated collections and taxonomy archives share pagination and output ownership.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use gd_conf::CollectionConfig;
use gd_env::{Digester, DocId, Lookup, LookupKind, PostlistSpec};
use gd_host::Problem;
use gd_jinja::Value;
use gd_outputs::{Generation, UnitRecord};
use gd_report::Report;
use gd_resolve::{BuildView, relative_url};

use crate::context::{Page, page_context};
use crate::escape::push_escaped;
use crate::site::Site;

pub(crate) fn query(c: &CollectionConfig) -> PostlistSpec {
    PostlistSpec {
        path: c.path.clone(),
        category: c.category.clone(),
        tags: c.tags.clone(),
        author: c.author.clone(),
        series: c.series.clone(),
        year: c.year.clone(),
        excerpts: true,
        ..Default::default()
    }
}

/// Stable, case-portable labels. Escapes preserve distinctions such as Rust/rust.
pub(crate) fn slug(term: &str) -> String {
    if term.len() > 180 {
        return format!("~h-{}", gd_env::Digest::of_text(term));
    }
    let mut slug = String::new();
    for b in term.bytes() {
        if b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' {
            slug.push(char::from(b));
        } else {
            let _ = write!(slug, "~{b:02x}");
        }
    }
    if slug.len() > 180 {
        return format!("~h-{}", gd_env::Digest::of_text(term));
    }
    if !gd_conf::valid_output(&slug) && !slug.contains('~') {
        slug.insert_str(0, "~r-");
    }
    slug
}

pub(crate) fn definitions(view: &BuildView<'_>) -> Vec<CollectionConfig> {
    let mut out = view.config.site.collections.clone();
    for kind in &view.config.site.taxonomies {
        let terms: BTreeSet<_> = view
            .env
            .docs
            .iter()
            .filter(|d| !d.page.date.is_empty())
            .flat_map(|d| terms(kind, &d.page))
            .collect();
        for term in terms {
            let mut c = CollectionConfig {
                output: format!("{kind}/{}", slug(&term)),
                title: format!("{kind}: {term}"),
                page_size: view.config.site.page_size,
                ..Default::default()
            };
            match kind.as_str() {
                "tags" => c.tags.push(term),
                "category" => c.category = term,
                "year" => c.year = term,
                "authors" => c.author = term,
                "series" => c.series = term,
                _ => continue,
            }
            out.push(c);
        }
    }
    out
}

pub(crate) fn terms(kind: &str, page: &gd_env::PageMeta) -> Vec<String> {
    match kind {
        "tags" => page.tags.clone(),
        "authors" => page.authors.iter().filter(|s| !s.is_empty()).cloned().collect(),
        "category" if !page.category.is_empty() => vec![page.category.clone()],
        "series" if !page.series.is_empty() => vec![page.series.clone()],
        "year" => page.date.get(..4).map(|s| vec![s.into()]).unwrap_or_default(),
        _ => Vec::new(),
    }
}

pub(crate) fn page_file(output: &str, number: usize) -> String {
    if number == 1 {
        format!("{output}/index.html")
    } else {
        format!("{output}/page/{number}/index.html")
    }
}

pub(crate) fn files(view: &BuildView<'_>) -> Vec<String> {
    definitions(view)
        .iter()
        .flat_map(|c| {
            let count = query(c).select(view.env).len();
            (1..=page_count(count, c.page_size))
                .map(|n| page_file(&c.output, n))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn page_count(count: usize, size: usize) -> usize {
    if size == 0 { 1 } else { count.div_ceil(size).max(1) }
}

pub(crate) fn write(
    site: &Site<'_, '_>,
    generation: &mut Generation,
    reports: &mut Vec<Report>,
) -> Result<(), Problem> {
    for c in definitions(site.view) {
        write_collection(site, &c, generation, reports)?;
    }
    Ok(())
}

fn write_collection(
    site: &Site<'_, '_>,
    c: &CollectionConfig,
    generation: &mut Generation,
    reports: &mut Vec<Report>,
) -> Result<(), Problem> {
    let spec = query(c);
    let posts = spec.select(site.view.env);
    let count = page_count(posts.len(), c.page_size);
    let mut paths: Vec<_> = (1..=count).map(|n| page_file(&c.output, n)).collect();
    if c.feeds {
        paths.extend([format!("{}/rss.xml", c.output), format!("{}/atom.xml", c.output)]);
    }
    for file in &paths {
        generation.check_available(file)?;
    }
    for number in 1..=count {
        let file = page_file(&c.output, number);
        let first = if c.page_size == 0 { 0 } else { (number - 1) * c.page_size };
        let last = if c.page_size == 0 {
            posts.len()
        } else {
            first.saturating_add(c.page_size).min(posts.len())
        };
        write_page(site, c, &posts[first..last], number, count, generation, reports)?;
        // Ownership is per page, so shrinking a collection removes its old tail.
        debug_assert!(paths.contains(&file));
    }
    if c.feeds {
        let mut unit = generation.unit(&format!("collection-feed:{}", c.output));
        crate::feeds::write_collection(site.view, &mut unit, c, &posts)?;
        unit.finish(UnitRecord::default());
    }
    Ok(())
}

fn write_page(
    site: &Site<'_, '_>,
    c: &CollectionConfig,
    posts: &[DocId],
    number: usize,
    count: usize,
    generation: &mut Generation,
    reports: &mut Vec<Report>,
) -> Result<(), Problem> {
    let view = site.view;
    let file = page_file(&c.output, number);
    let body = body(view, c, posts, number, count);
    let mut digest = Digester::new();
    digest.digest(&crate::digests::html_settings(view));
    digest.text(&format!("{c:?}")).text(&body);
    for &id in posts {
        digest.digest(&view.doc(id).page.digest());
    }
    let inputs = digest.finish();
    let key = format!("collection:{file}");
    if let Some(old) = generation.reuse(&key, inputs, |l| {
        if l.kind == LookupKind::Navigation {
            crate::navigation_trace::answer(view, &file, l, &site.articles)
        } else {
            site.layout.template_digest(&l.key)
        }
    }) {
        reports.extend(old.reports.iter().cloned());
        return Ok(());
    }
    let (ctx, state) = page_context(view, Page::Special(&file), &body, &site.articles);
    let title = if c.title.is_empty() { c.output.as_str() } else { c.title.as_str() };
    ctx.set("title", Value::from(title));
    let collection = Value::dict();
    collection.set("page", Value::from(number as i64));
    collection.set("pages", Value::from(count as i64));
    collection.set("title", Value::from(title));
    collection.set("output", Value::from(c.output.as_str()));
    let adjacent = |n| Value::from(relative_url(&file, &page_file(&c.output, n)));
    collection.set("previous_url", if number > 1 { adjacent(number - 1) } else { Value::None });
    collection.set("next_url", if number < count { adjacent(number + 1) } else { Value::None });
    collection.set("posts", Value::list(posts.iter().map(|&id| post_value(view, &file, id))));
    ctx.set("collection", collection);
    let template = if c.template.is_empty() { "collection.html" } else { &c.template };
    let mut own = Vec::new();
    let html = site.layout.render(view, template, &ctx, &state, &mut own)?;
    let mut lookups: Vec<_> = site
        .layout
        .take_template_loads()
        .into_iter()
        .map(|name| {
            let digest = site.layout.template_digest(&name);
            (Lookup::new(LookupKind::Template, &name), digest)
        })
        .collect();
    for lookup in site.layout.take_navigation_lookups() {
        let digest = crate::navigation_trace::answer(view, &file, &lookup, &site.articles);
        lookups.push((lookup, digest));
    }
    let mut unit = generation.unit(&key);
    unit.emit_text(&file, &html)?;
    reports.extend(own.iter().cloned());
    unit.finish(UnitRecord { inputs, lookups, reports: own, ..Default::default() });
    Ok(())
}

fn post_value(view: &BuildView<'_>, file: &str, id: DocId) -> Value {
    let doc = view.doc(id);
    let value = Value::dict();
    value.set("title", Value::from(doc.title.as_str()));
    value.set("docname", Value::from(doc.docname.as_str()));
    value.set("date", Value::from(doc.page.date.as_str()));
    value.set("url", Value::from(relative_url(file, &view.page_name(&doc.docname))));
    value.set("summary", Value::from(doc.page.summary.as_str()));
    let meta = crate::context::metadata(&doc.page);
    if doc.page.pdf == "article" {
        let html = view.page_name(&doc.docname);
        let stem = html.strip_suffix(&view.config.html.file_suffix).unwrap_or(&html);
        meta.set("pdf_url", Value::from(relative_url(file, &format!("{stem}.pdf"))));
    } else {
        meta.set("pdf_url", Value::None);
    }
    value.set("meta", meta);
    value
}

fn body(
    view: &BuildView<'_>,
    c: &CollectionConfig,
    posts: &[DocId],
    number: usize,
    count: usize,
) -> String {
    let file = page_file(&c.output, number);
    let mut out = String::from("<h1>");
    push_escaped(&mut out, if c.title.is_empty() { &c.output } else { &c.title });
    out.push_str("</h1><div class=\"postlist postlist-cards\"><ul>");
    for &id in posts {
        let d = view.doc(id);
        out.push_str("<li><p>");
        push_escaped(&mut out, &d.page.date);
        out.push(' ');
        link(&mut out, &relative_url(&file, &view.page_name(&d.docname)), &d.title);
        out.push_str("</p><p>");
        push_escaped(&mut out, &d.page.summary);
        out.push_str("</p></li>");
    }
    out.push_str("</ul></div><nav aria-label=\"Pagination\">");
    if number > 1 {
        link(&mut out, &relative_url(&file, &page_file(&c.output, number - 1)), "Previous");
    }
    let _ = write!(out, " <span>Page {number} of {count}</span> ");
    if number < count {
        link(&mut out, &relative_url(&file, &page_file(&c.output, number + 1)), "Next");
    }
    out.push_str("</nav>");
    out
}

fn link(out: &mut String, url: &str, title: &str) {
    out.push_str("<a href=\"");
    push_escaped(out, url);
    out.push_str("\">");
    push_escaped(out, title);
    out.push_str("</a>");
}
