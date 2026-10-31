//! Generated images share caption, identifier, numbering and asset handling.

use std::path::PathBuf;

use gd_env::{Asset, OutputFormat};
use gd_resolve::relative_url;
use guidedoc::{AttributeKey, Fail, LinkInfo, LinkKind, NodeId, NodeKind};

use crate::image::{book_path, file_name};
use crate::page::Page;
use crate::rewrite::{Rewriter, Rule};

pub(crate) struct Drawing<'a> {
    pub path: PathBuf,
    pub source: &'a str,
    pub class: &'static str,
    pub interactive: bool,
    pub adaptive: bool,
}

pub(crate) fn replace(
    page: &mut Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    id: NodeId,
    drawing: Drawing<'_>,
) -> Result<(), Fail> {
    let doc = rw.doc;
    let span = doc.node(id).span;
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
    let alt = doc.custom_attribute(id, "alt").unwrap_or_else(|| drawing.source.trim());
    rw.b.add_attribute_string(image, AttributeKey::Alt, alt)?;
    if let Some(align) = doc.custom_attribute(id, "align") {
        rw.b.add_attribute_string(image, AttributeKey::Align, align)?;
    }
    rw.add_class(image, drawing.class)?;
    if drawing.interactive {
        // Shown so that its links work: an img element's links are not followed.
        rw.add_custom(image, "interactive", "1")?;
    }
    if drawing.adaptive && caption.is_some() {
        rw.add_class(image, "graphviz-adaptive")?;
    }
    let url = image_url(page, drawing.path);
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

/// Where the page or the book finds the drawn graph; the page copies it to `_images`
/// as it copies other images.
fn image_url(page: &mut Page<'_>, path: PathBuf) -> String {
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
