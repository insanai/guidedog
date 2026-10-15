//! Renderers write Guidedog's own words in the language of the UI translator.

use gd_i18n::{fill_labels, ui_translator};
use guidedoc::{
    Conformance, Fail, Labels, NodeKind, ReadContext, Reader, RenderOptions, Renderer, Request,
    SourceSpan, Workspace, default_request, read_document, render_document, trusted_policy,
};

const TEXT: &str = ".. note:: Careful.\n\n.. warning:: Stop.\n";

/// Reads the admonitions with the reStructuredText reader, then adds the version note the
/// Sphinx layer would write for `.. versionadded:: 2.1`.
fn read_notes(ctx: &mut ReadContext<'_>) -> Result<(), Fail> {
    guidedoc_rst::read(ctx)?;
    let at = SourceSpan::default();
    let v = ctx.b.begin_node(NodeKind::VersionModified, at)?;
    ctx.b.add_custom_attribute(v, "type", "versionadded")?;
    ctx.b.add_custom_attribute(v, "version", "2.1")?;
    let p = ctx.b.begin_node(NodeKind::Paragraph, at)?;
    ctx.b.add_text_leaf(NodeKind::Text, at, "New.")?;
    ctx.b.end_node(p)?;
    ctx.b.end_node(v)
}

fn render(renderer: &Renderer, labels: Option<&Labels>) -> String {
    let mut ws = Workspace::new();
    let reader = Reader {
        id: "notes",
        name: "Notes",
        version: "1",
        conformance: Conformance::Partial,
        read: read_notes,
        read_inline: None,
    };
    let request = Request {
        policy: trusted_policy(),
        render: RenderOptions { labels, ..RenderOptions::default() },
        ..default_request("x.rst", TEXT, "notes", renderer.id)
    };
    if let Err(fail) = read_document(&mut ws, &reader, &request) {
        panic!("read: {}", fail.diagnostic.formatted_message());
    }
    match render_document(&mut ws, renderer, &request) {
        Ok(out) => out.to_owned(),
        Err(fail) => panic!("{}: {}", renderer.id, fail.diagnostic.formatted_message()),
    }
}

#[test]
fn renderers_write_translated_labels() {
    let (tr, _) = ui_translator("fr", &[], false);
    let labels = fill_labels(&tr);
    let renderers =
        [guidedoc_html::renderer(), guidedoc_text::renderer(), guidedoc_typst::renderer()];
    for renderer in &renderers {
        let out = render(renderer, Some(&labels));
        assert!(out.contains("Note"), "{}:\n{out}", renderer.id);
        let added = if renderer.id == "typst" {
            "Ajouté dans la version 2\\.1"
        } else {
            "Ajouté dans la version 2.1"
        };
        assert!(out.contains(added), "{}:\n{out}", renderer.id);
        assert!(out.contains("Avertissement"), "{}:\n{out}", renderer.id);
    }
    let english = render(&guidedoc_html::renderer(), None);
    assert!(english.contains("Added in version 2.1"), "{english}");
}
