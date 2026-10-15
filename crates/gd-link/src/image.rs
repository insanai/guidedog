//! Images a page shows (`BuildView::image_file` finds the file), and the paths the
//! book reads files by (`book_root` and `book_path`).

use std::path::{Path, PathBuf};

use gd_conf::Config;
use gd_env::{Asset, LookupKind, OutputFormat};
use gd_resolve::relative_url;
use guidedoc::{Content, Document, Fail, LinkInfo, NodeId, NodeKind, SourceSpan};

use crate::page::{Page, Warning};
use crate::rewrite::Rewriter;

/// The last part of a path, as text.
pub(crate) fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Copies an image with its destination set where the output finds it: below
/// `_images` in pages (the file joins the page's assets), a book path in the book.
pub(crate) fn replace_image(
    page: &mut Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    id: NodeId,
) -> Result<(), Fail> {
    let doc = rw.doc;
    let link = doc.link_info(id).unwrap_or(LinkInfo::UNRESOLVED);
    let uri = doc.text(link.destination);
    if uri.is_empty() || uri.contains("://") || uri.starts_with("data:") {
        return rewrite_image(page, rw, id, uri);
    }
    page.trace.note_key(LookupKind::Image, uri);
    let view = page.view;
    let found = view.image_file(&page.info.docname, uri);
    let present = found.as_ref().is_some_and(|f| f.path.is_file());
    if !present {
        report_missing_image(page, doc, uri, found.is_some(), doc.node(id).span);
    }
    let pdf = page.format == OutputFormat::Pdf;
    let Some(file) = found else {
        // A page keeps the image, where a browser shows the alternative text; Typst
        // stops at a missing file, so the book leaves it out, as Sphinx does.
        return if pdf { Ok(()) } else { rewrite_image(page, rw, id, uri) };
    };
    if pdf && !present {
        return Ok(());
    }
    if pdf && !book_reads(&file.path) {
        return replace_unreadable(page, rw, id, uri);
    }
    let url = if pdf {
        let book = book_path(view.config, &file.path);
        page.assets.push(Asset { source: file.path, target: book.clone() });
        book
    } else {
        let url = relative_url(&view.page_name(&page.info.docname), &file.target);
        page.assets.push(Asset { source: file.path, target: file.target });
        url
    };
    rewrite_image(page, rw, id, &url)
}

/// Copies an image node with its destination set to `url`.
fn rewrite_image(
    page: &mut Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    id: NodeId,
    url: &str,
) -> Result<(), Fail> {
    let doc = rw.doc;
    let copy = rw.open(NodeKind::Image, doc.node(id).span)?;
    rw.moved_to(id, copy);
    rw.copy_attributes(id, copy, |_, _| true)?;
    let mut link = doc.link_info(id).unwrap_or(LinkInfo::UNRESOLVED);
    link.destination = rw.b.add_text(url)?;
    link.title = rw.b.add_text(doc.text(link.title))?;
    link.name = rw.b.add_text(doc.text(link.name))?;
    rw.b.set_link(copy, link)?;
    rw.rewrite_children(page, id)?;
    rw.close(copy)
}

/// The image formats Typst reads, by file suffix: the book shows these as they are,
/// so sphinx.ext.imgconverter has nothing to convert.
const BOOK_IMAGE_FORMATS: [&str; 7] = ["png", "jpg", "jpeg", "gif", "svg", "webp", "pdf"];

/// Whether the book can show the image file at `path`.
fn book_reads(path: &Path) -> bool {
    let suffix = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase());
    suffix.is_some_and(|s| BOOK_IMAGE_FORMATS.contains(&s.as_str()))
}

/// Shows an image the book cannot read as its alternative text, with the warning
/// `imgconverter.format`. In a figure the image is left out, and the caption stays.
fn replace_unreadable(
    page: &mut Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    id: NodeId,
    uri: &str,
) -> Result<(), Fail> {
    let doc = rw.doc;
    let span = doc.node(id).span;
    let format = Path::new(uri).extension().map_or_else(
        || "an unknown format".to_owned(),
        |e| format!("the {} format", e.to_string_lossy().to_uppercase()),
    );
    let warning = Warning {
        code: "imgconverter.format",
        title: "IMAGE FORMAT NOT READ BY THE BOOK",
        category: "imgconverter",
        message: format!(
            "The image \"{uri}\" is in {format}, which the PDF cannot show, so the book \
             shows its alternative text instead."
        ),
        hint: "Provide a PNG, JPEG, GIF or SVG version, and name it picture.* so each \
               output picks the version it reads.",
    };
    page.warn(doc, span, warning);
    let alt = doc.attribute(id, guidedoc::AttributeKey::Alt).unwrap_or(uri);
    let parent = doc.parent_of(id).map_or(NodeKind::Document, |p| doc.kind(p));
    let paragraph = match guidedoc::content_model(parent) {
        Content::Blocks => Some(rw.open(NodeKind::Paragraph, span)?),
        Content::Inlines => None,
        _ => return Ok(()),
    };
    let emphasis = rw.open(NodeKind::Emphasis, span)?;
    rw.moved_to(id, emphasis);
    rw.add_plain(alt, span)?;
    rw.close(emphasis)?;
    match paragraph {
        Some(p) => rw.close(p),
        None => Ok(()),
    }
}

/// Warns, as Sphinx does, that an image the document shows is absent or refused.
fn report_missing_image(
    page: &mut Page<'_>,
    doc: Document<'_>,
    uri: &str,
    readable: bool,
    span: SourceSpan,
) {
    let warning = if readable {
        Warning {
            code: "build.image.missing",
            title: "IMAGE NOT FOUND",
            category: "image.not_readable",
            message: format!("This document shows the image \"{uri}\", which does not exist."),
            hint: "Fix the path: it is relative to the document, or to the source folder \
                   when it starts with /.",
        }
    } else {
        Warning {
            code: "build.image.refused",
            title: "IMAGE OUTSIDE THE PROJECT",
            category: "image.not_readable",
            message: format!(
                "This document shows the image \"{uri}\", which is outside the source \
                 folder and include_roots."
            ),
            hint: "Add its folder to include_roots in conf.toml, such as \"..\".",
        }
    };
    page.warn(doc, span, warning);
}

/// The folder the book's paths are relative to: the source folder, or the nearest
/// folder that holds it, every include root, and the graphs folder (where graphs are
/// drawn). Typst runs in a folder that holds only those (gd-book's typst root).
pub fn book_root(config: &Config) -> PathBuf {
    let graphs = gd_env::doctrees_dir(config).join("graphs");
    let roots = gd_conf::readable_roots(config);
    let mut root = config.source_dir.clone();
    while !graphs.starts_with(&root) || !roots.iter().all(|r| r.starts_with(&root)) {
        match root.parent() {
            Some(parent) => root = parent.to_path_buf(),
            None => break,
        }
    }
    root
}

/// Names a file for the book: absolute below [`book_root`], as Typst reads it.
pub fn book_path(config: &Config, path: &Path) -> String {
    format!("/{}", gd_env::relative_to(&book_root(config), path))
}
