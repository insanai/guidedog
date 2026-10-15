//! A toy reader and two toy renderers registered under the ids the host route uses
//! (`rst`, `commonmark`; `html`, `typst`), since the real adapters live in other crates.
//! Every line becomes a paragraph; `include NAME` includes a file and reads its lines;
//! `deep` reports nesting too deep (an error); `!` reports a warning. The Typst renderer
//! writes `#bad()` for a line `bad`, which Typst rejects.

#![allow(dead_code)] // Each test binary uses part of this module.

use guidedoc::{
    Arg, CORE_DEPTH, Conformance, Fail, Message, NodeKind, ROOT, ReadContext, Reader, Registry,
    RenderContext, Renderer, Severity, SourceId, SourceSpan, SourceUse, Status, arg_int,
};

pub const TOY_WARNING: Message = Message {
    code: "toy.warning",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "TOY WARNING",
    message: "A warning line {0}.",
    hint: "Remove it.",
};

fn read_lines(ctx: &mut ReadContext<'_>, source: SourceId) -> Result<(), Fail> {
    let text = ctx.source_of(source);
    let mut offset = 0u32;
    for line in text.split('\n') {
        let span = SourceSpan::new(source, offset, offset + line.len() as u32);
        offset += line.len() as u32 + 1;
        ctx.b.spend(line.len() as u64 + 1)?; // polls cancellation, as real readers do.
        if let Some(name) = line.strip_prefix("include ") {
            if let Some(id) = ctx.include_source(source, name, span, SourceUse::Parsed) {
                read_lines(ctx, id)?;
            }
            continue;
        }
        match line {
            "" => {}
            "deep" => ctx.report(CORE_DEPTH, span, arg_int(3), Arg::None),
            "!" => ctx.report(TOY_WARNING, span, guidedoc::arg_span(span), Arg::None),
            _ => {
                ctx.b.with_node(NodeKind::Paragraph, span, |b, _| {
                    b.add_text_leaf(NodeKind::Text, span, line).map(|_| ())
                })?;
            }
        }
    }
    Ok(())
}

fn toy_read(ctx: &mut ReadContext<'_>) -> Result<(), Fail> {
    read_lines(ctx, ctx.source)
}

fn toy_html(ctx: &mut RenderContext<'_>) -> Result<(), Fail> {
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

fn toy_typst(ctx: &mut RenderContext<'_>) -> Result<(), Fail> {
    let doc = ctx.doc;
    for paragraph in doc.children(ROOT) {
        ctx.out.emit_mark(doc.node(paragraph).span);
        for child in doc.children(paragraph) {
            let text = doc.node_text(child);
            if text == "bad" {
                ctx.out.emit("#bad()");
            } else {
                ctx.out.emit(text);
            }
        }
        ctx.out.emit("\n\n");
    }
    Ok(())
}

const fn reader(id: &'static str) -> Reader {
    Reader {
        id,
        name: "Toy",
        version: "1",
        conformance: Conformance::Partial,
        read: toy_read,
        read_inline: None,
    }
}

const fn renderer(
    id: &'static str,
    media_type: &'static str,
    render: guidedoc::RenderFn,
) -> Renderer {
    Renderer {
        id,
        name: "Toy",
        version: "1",
        media_type,
        conformance: Conformance::Partial,
        render,
    }
}

pub fn toy_registry() -> Registry {
    Registry {
        readers: vec![reader("rst"), reader("commonmark")],
        renderers: vec![
            renderer("html", "text/html", toy_html),
            renderer("typst", "text/x-typst", toy_typst),
        ],
    }
}

/// A fresh, resolved temporary folder that removes itself when dropped.
pub struct TempDir(pub std::path::PathBuf);

impl TempDir {
    pub fn new(tag: &str) -> TempDir {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("gd-host-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temporary folder");
        TempDir(dir.canonicalize().expect("resolved temporary folder"))
    }

    pub fn join(&self, part: &str) -> std::path::PathBuf {
        self.0.join(part)
    }

    pub fn write(&self, part: &str, text: &str) -> std::path::PathBuf {
        let path = self.join(part);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("folder");
        }
        std::fs::write(&path, text).expect("written");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
