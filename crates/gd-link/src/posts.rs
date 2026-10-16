//! Collection directives become ordinary lists, so every renderer can render them.

use gd_env::{LookupKind, PostlistSpec};
use gd_resolve::Resolved;
use guidedoc::{Fail, ListInfo, ListStyle, NodeId, NodeKind, SourceSpan};

use crate::page::Page;
use crate::rewrite::Rewriter;
use crate::xref::link_destination;

pub(crate) fn replace_postlist(
    page: &mut Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    id: NodeId,
) -> Result<(), Fail> {
    let spec = match PostlistSpec::from_node(rw.doc, id) {
        Ok(spec) => spec,
        Err(_) => return Ok(()), // Reading diagnoses invalid queries and fails the build.
    };
    page.trace.note_key(LookupKind::Postlist, &spec.encode());
    let span = rw.doc.node(id).span;
    let container = rw.open(NodeKind::Container, span)?;
    let style = rw.doc.custom_attribute(id, "style").unwrap_or("list");
    rw.add_class(container, &format!("postlist postlist-{style}"))?;
    let list = rw.open(NodeKind::List, span)?;
    rw.b.set_list(list, ListInfo { style: ListStyle::Bullet, delimiter: b'*', start: 1 })?;
    for other in spec.select(page.view.env) {
        let doc = page.view.doc(other);
        let item = rw.open(NodeKind::ListItem, span)?;
        let para = rw.open(NodeKind::Paragraph, span)?;
        rw.add_plain(&format!("{} ", doc.page.date), span)?;
        let target = Resolved { doc: Some(other), ..Resolved::default() };
        let (url, kind) = link_destination(page, &target);
        let link = rw.open_link(&url, "", kind, span)?;
        rw.add_plain(&doc.title, span)?;
        rw.close(link)?;
        rw.close(para)?;
        if spec.excerpts && !doc.page.summary.is_empty() {
            summary(rw, &doc.page.summary, span)?;
        }
        rw.close(item)?;
    }
    rw.close(list)?;
    rw.close(container)
}

fn summary(rw: &mut Rewriter<'_, '_, '_>, text: &str, span: SourceSpan) -> Result<(), Fail> {
    let para = rw.open(NodeKind::Paragraph, span)?;
    rw.add_plain(text, span)?;
    rw.close(para)
}
