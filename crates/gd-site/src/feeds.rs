//! Deterministic discovery documents built only from the eligible environment.

use std::fmt::Write as _;

use gd_env::{DocId, PostlistSpec};
use gd_host::Problem;
use gd_outputs::Unit;
use gd_resolve::BuildView;

use crate::escape::push_escaped;

pub(crate) fn write_feeds(view: &BuildView<'_>, unit: &mut Unit<'_>) -> Result<(), Problem> {
    let site = &view.config.site;
    if site.sitemap {
        let mut xml = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
            <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
        );
        for (_, doc) in view.env.iter().filter(|(_, d)| !d.failed) {
            xml.push_str("<url>");
            element(&mut xml, "loc", &url(view, &doc.docname));
            if !doc.page.updated.is_empty() {
                element(&mut xml, "lastmod", &doc.page.updated.to_ascii_uppercase());
            }
            xml.push_str("</url>\n");
        }
        for file in crate::collections::files(view) {
            xml.push_str("<url>");
            element(&mut xml, "loc", &file_url(view, &file));
            xml.push_str("</url>\n");
        }
        xml.push_str("</urlset>\n");
        unit.emit_text("sitemap.xml", &xml)?;
    }
    if site.feeds {
        write_articles(view, unit)?;
    }
    Ok(())
}

fn write_articles(view: &BuildView<'_>, unit: &mut Unit<'_>) -> Result<(), Problem> {
    let posts = PostlistSpec::default().select(view.env);
    write_selected(view, unit, "", &view.config.project, &posts)
}

pub(crate) fn write_collection(
    view: &BuildView<'_>,
    unit: &mut Unit<'_>,
    c: &gd_conf::CollectionConfig,
    posts: &[DocId],
) -> Result<(), Problem> {
    write_selected(view, unit, &format!("{}/", c.output), &c.title, posts)
}

fn write_selected(
    view: &BuildView<'_>,
    unit: &mut Unit<'_>,
    prefix: &str,
    title: &str,
    posts: &[DocId],
) -> Result<(), Problem> {
    let base = &view.config.site.base_url;
    let mut rss = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
        <rss version=\"2.0\"><channel>",
    );
    element(&mut rss, "title", title);
    element(&mut rss, "link", base);
    element(&mut rss, "description", &view.config.project);
    let mut atom = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
        <feed xmlns=\"http://www.w3.org/2005/Atom\">",
    );
    element(&mut atom, "id", &format!("{base}/{prefix}"));
    element(&mut atom, "title", title);
    atom.push_str("<link rel=\"self\" href=\"");
    push_escaped(&mut atom, &format!("{base}/{prefix}atom.xml"));
    atom.push_str("\"/>");
    let updated = posts
        .iter()
        .map(|&id| {
            let p = &view.doc(id).page;
            instant(if p.updated.is_empty() { &p.date } else { &p.updated })
        })
        .max_by_key(|date| gd_env::publication_time(date).ok())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".into());
    element(&mut atom, "updated", &updated);
    for &id in posts {
        let doc = view.doc(id);
        let link = url(view, &doc.docname);
        rss.push_str("<item>");
        element(&mut rss, "title", &doc.title);
        element(&mut rss, "link", &link);
        element(&mut rss, "guid", &link);
        element(&mut rss, "description", &doc.page.summary);
        rss.push_str("</item>\n");
        atom.push_str("<entry>");
        element(&mut atom, "id", &link);
        element(&mut atom, "title", &doc.title);
        atom.push_str("<link href=\"");
        push_escaped(&mut atom, &link);
        atom.push_str("\"/>");
        element(&mut atom, "published", &instant(&doc.page.date));
        element(
            &mut atom,
            "updated",
            &instant(if doc.page.updated.is_empty() { &doc.page.date } else { &doc.page.updated }),
        );
        let authors = if doc.page.authors.is_empty() {
            std::slice::from_ref(&view.config.author)
        } else {
            &doc.page.authors
        };
        for author in authors {
            atom.push_str("<author>");
            element(
                &mut atom,
                "name",
                if author.is_empty() { &view.config.project } else { author },
            );
            atom.push_str("</author>");
        }
        element(&mut atom, "summary", &doc.page.summary);
        atom.push_str("</entry>\n");
    }
    rss.push_str("</channel></rss>\n");
    atom.push_str("</feed>\n");
    unit.emit_text(&format!("{prefix}rss.xml"), &rss)?;
    unit.emit_text(&format!("{prefix}atom.xml"), &atom)
}

fn url(view: &BuildView<'_>, docname: &str) -> String {
    let page = view.page_name(docname);
    file_url(view, &page)
}

fn file_url(view: &BuildView<'_>, page: &str) -> String {
    let mut out = format!("{}/", view.config.site.base_url);
    for byte in page.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte) {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

fn instant(date: &str) -> String {
    if date.len() == 10 { format!("{date}T00:00:00Z") } else { date.to_ascii_uppercase() }
}

fn element(out: &mut String, name: &str, value: &str) {
    let _ = write!(out, "<{name}>");
    push_escaped(out, value);
    let _ = write!(out, "</{name}>");
}
