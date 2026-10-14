//! A toy reader and renderer that prove the pipeline end to end: every line becomes a
//! paragraph of text; "!" lines report a warning, "?" lines a content error, and "%"
//! lines a capacity error. The renderer writes `<p>text</p>` per paragraph.

#![allow(dead_code)] // Each test binary uses part of this module.

use guidedoc::{
    Arg, Builder, CORE_CAPACITY, Conformance, Document, Fail, Message, NodeKind, PassOutcome,
    ROOT, ReadContext, Reader, Registry, RenderContext, Renderer, Request, Severity, SourceSpan,
    Status, arg_int, arg_static,
};

pub const TOY_WARNING: Message = Message {
    code: "toy.warning",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "TOY",
    message: "A warning line.",
    hint: "Remove it.",
};

pub const TOY_ERROR: Message = Message {
    code: "toy.error",
    severity: Severity::Error,
    status: Status::Unsupported,
    title: "TOY",
    message: "An unknown construct.",
    hint: "Remove it.",
};

pub fn toy_read(ctx: &mut ReadContext<'_>) -> Result<(), Fail> {
    let source = ctx.source_of(ctx.source);
    let mut offset = 0u32;
    for line in source.split('\n') {
        let span = SourceSpan::new(ctx.source, offset, offset + line.len() as u32);
        offset += line.len() as u32 + 1;
        match line {
            "" => {}
            "!" => ctx.report(TOY_WARNING, span, Arg::None, Arg::None),
            "?" => ctx.report(TOY_ERROR, span, Arg::None, Arg::None),
            "%" => ctx.report(CORE_CAPACITY, span, arg_static("toy"), arg_int(1)),
            _ => {
                ctx.b.with_node(NodeKind::Paragraph, span, |b, _| {
                    b.add_text_leaf(NodeKind::Text, span, line).map(|_| ())
                })?;
            }
        }
    }
    Ok(())
}

pub fn toy_render(ctx: &mut RenderContext<'_>) -> Result<(), Fail> {
    let doc = ctx.doc;
    for paragraph in doc.children(ROOT) {
        ctx.out.emit("<p>");
        for child in doc.children(paragraph) {
            ctx.out.emit(doc.node_text(child));
        }
        ctx.out.emit("</p>");
    }
    Ok(())
}

pub const TOY_READER: Reader = Reader {
    id: "toy",
    name: "Toy",
    version: "1",
    conformance: Conformance::Partial,
    read: toy_read,
    read_inline: None,
};

pub const TOY_RENDERER: Renderer = Renderer {
    id: "toy",
    name: "Toy",
    version: "1",
    media_type: "text/html",
    conformance: Conformance::Partial,
    render: toy_render,
};

pub fn toy_registry() -> Registry {
    Registry { readers: vec![TOY_READER], renderers: vec![TOY_RENDERER] }
}

pub fn toy_request(text: &str) -> Request<'_> {
    guidedoc::default_request("toy.txt", text, "toy", "toy")
}

/// A pass that copies every child of the root: a rebuild that changes nothing.
pub fn copy_all(doc: Document<'_>, b: &mut Builder<'_>) -> PassOutcome {
    for child in doc.children(ROOT) {
        if b.copy_subtree(doc, child).is_err() {
            return PassOutcome::Failed;
        }
    }
    PassOutcome::Rebuilt
}
