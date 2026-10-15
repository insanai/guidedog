//! MyST Markdown with the Sphinx layer: the "myst" reader parses Markdown and hands
//! each directive and role here (`guidedoc_commonmark::Extension`); they run on an
//! embedded reStructuredText parser whose host is the Markdown reader
//! (`guidedoc_rst::Host`), so their nested content is parsed as Markdown again.
//! `eval-rst` parses its content as reStructuredText. The document is resolved as
//! reStructuredText is (references, footnotes, targets, bibliographic fields, smart
//! quotes).
//!
//! The two readers never hold the read context at once: a directive detaches the
//! Markdown reader's state from the context (`Host::detach`) and lends the context to
//! the embedded parser; nested Markdown attaches it again (`Detached::attach`), with the
//! parser's state moved where nested directives find it (`Parser::lend`).

use std::cell::RefCell;
use std::sync::Arc;

use guidedoc::{Fail, NodeId, ReadContext, SourceSpan};
use guidedoc_commonmark::{
    Detached, Directive as MdDirective, DirectiveResult, DirectiveShape, Extension,
    Host as MdHost, Role as MdRole, Settings as MystSettings,
};
use guidedoc_rst::{
    ArgRule, HostDirective, HostItem, HostOption, HostedResult, Line, Parser, ReaderState,
    embed_begin,
};

use crate::config::Config;
use crate::extension::Sphinx;
use crate::state::State;
use crate::text::equal_fold;

/// The Sphinx layer as the "myst" reader's extension. Make the reader's settings with
/// `myst_settings`.
#[derive(Debug)]
pub struct MystExtension {
    config: Config,
}

/// Returns the "myst" reader's settings (`myst`) with the Sphinx layer's directives and
/// roles configured by `config`. Sphinx applies rst_prolog and rst_epilog to
/// reStructuredText only, so they are left out; tabs are four columns, as in
/// myst-parser. A `myst.docname` left empty takes `config.docname`.
pub fn myst_settings(config: Config, myst: MystSettings) -> MystSettings {
    let mut config = config;
    config.rst_prolog.clear();
    config.rst_epilog.clear();
    config.settings.tab_width = 4;
    let mut settings = myst;
    if settings.docname.is_empty() {
        settings.docname.clone_from(&config.docname);
    }
    settings.extension = Some(Arc::new(MystExtension { config }));
    settings
}

/// The extension's per-read state, kept in the Markdown reader's host: the embedded
/// parser's state and the Sphinx layer's. Either is out of it while a directive runs.
#[derive(Debug, Default)]
struct MdState {
    reader: Option<ReaderState>,
    sphinx: State,
}

impl MystExtension {
    /// Runs `f` with the embedded parser over the host's context; `None` before `begin`.
    fn run<R>(
        &self,
        h: &mut MdHost<'_, '_>,
        f: impl FnOnce(&Sphinx<'_>, &mut Parser<'_, '_>) -> R,
    ) -> Option<R> {
        let md = h.extension_state::<MdState>()?;
        let mut reader = md.reader.take()?;
        let sx = Sphinx::with_state(&self.config, std::mem::take(&mut md.sphinx));
        let r = h.detach(|ctx, det| {
            let host = Embedded { det: RefCell::new(det), sx: &sx, config: &self.config };
            let mut p = reader.parser(ctx, Some(&sx), Some(&host));
            f(&sx, &mut p)
        });
        if let Some(md) = h.extension_state::<MdState>() {
            md.reader = Some(reader);
            md.sphinx = sx.into_state();
        }
        Some(r)
    }
}

impl Extension for MystExtension {
    fn begin(&self, h: &mut MdHost<'_, '_>) -> Result<(), Fail> {
        let sx = Sphinx::new(&self.config);
        let reader = h.detach(|ctx, _| embed_begin(ctx, &self.config.settings, Some(&sx)))?;
        h.set_extension_state(MdState { reader: Some(reader), sphinx: sx.into_state() });
        Ok(())
    }

    fn resolve(&self, h: &mut MdHost<'_, '_>) -> Result<bool, Fail> {
        self.run(h, |_, p| p.embed_end()).transpose()?;
        Ok(true)
    }

    fn shape(&self, h: &mut MdHost<'_, '_>, name: &str) -> Option<DirectiveShape> {
        if equal_fold(name, "eval-rst") {
            return Some(DirectiveShape { argument: false, options: false });
        }
        let shape = self.run(h, |_, p| {
            p.directive_shape(name).map(|s| DirectiveShape {
                argument: s.arg != ArgRule::None,
                options: !s.options.is_empty(),
            })
        });
        shape.flatten()
    }

    fn directive(
        &self,
        h: &mut MdHost<'_, '_>,
        d: &MdDirective<'_>,
    ) -> Result<DirectiveResult, Fail> {
        if equal_fold(d.name, "eval-rst") {
            let r = self.run(h, |_, p| eval_rst(p, d));
            return r.transpose().map(|_| DirectiveResult::Ran);
        }
        // MyST's include options; the reader rewrites the paths of included content.
        let include = equal_fold(d.name, "include");
        let images = include && d.option("relative-images").is_some();
        let docs = d.option("relative-docs").filter(|_| include).and_then(|o| o.value.first());
        let saved = h.include_paths(images, docs.map_or("", |l| l.text));
        let ran = self.run(h, |_, p| hosted(p, d, include));
        h.include_paths(saved.0, &saved.1);
        let Some((result, pending)) = ran.transpose()? else { return Ok(DirectiveResult::Failed) };
        if let Some(classes) = pending {
            h.set_classes_for_next(classes);
        }
        Ok(match result {
            HostedResult::Ran => DirectiveResult::Ran,
            HostedResult::Unknown => DirectiveResult::Unknown,
            HostedResult::Failed => DirectiveResult::Failed,
        })
    }

    fn role(&self, h: &mut MdHost<'_, '_>, r: &MdRole<'_>) -> Result<bool, Fail> {
        let ran = self.run(h, |_, p| {
            let tab = p.tab();
            let frame = p.enter_source(r.source, tab);
            let result = p.apply_host_role(r.name, &r.lines, r.content.clone());
            p.leave_source(frame);
            result
        });
        ran.transpose().map(|r| r.is_some())
    }

    /// Gives a table directive's title and options to the table the Markdown reader
    /// emits in its content. MyST tables are sized automatically.
    fn table(
        &self,
        h: &mut MdHost<'_, '_>,
        table: NodeId,
        span: SourceSpan,
        columns: usize,
    ) -> Result<bool, Fail> {
        let ran = self.run(h, |_, p| {
            let Some(mut extra) = p.take_table_extra() else { return Ok(false) };
            if extra.width_class.is_empty() {
                extra.width_class = "colwidths-auto";
            }
            p.table_decorate(table, span, columns, &[], Some(&mut extra))?;
            Ok(true)
        });
        Ok(ran.transpose()?.unwrap_or(false))
    }
}

/// eval-rst: the content as reStructuredText. Section titles inside it would interleave
/// the two readers' sections, so they are not sections.
fn eval_rst(p: &mut Parser<'_, '_>, d: &MdDirective<'_>) -> Result<(), Fail> {
    let tab = p.tab();
    let frame = p.enter_source(d.source, tab);
    let mut buf = String::new();
    let content = p.host_lines(&d.content, &mut buf);
    let r = p.parse_blocks(&content, false, None);
    p.leave_source(frame);
    r
}

/// Runs a directive of the Markdown source through the embedded parser; returns its
/// outcome and the classes a class directive left for the next element.
fn hosted(
    p: &mut Parser<'_, '_>,
    d: &MdDirective<'_>,
    include: bool,
) -> Result<(HostedResult, Option<guidedoc::Text>), Fail> {
    let tab = p.tab();
    let frame = p.enter_source(d.source, tab);
    let (mut arg_buf, mut content_buf) = (String::new(), String::new());
    let arg = p.host_lines(&d.argument, &mut arg_buf);
    let content = p.host_lines(&d.content, &mut content_buf);
    let kept: Vec<_> = d
        .options
        .iter()
        .filter(|o| !(include && matches!(o.name, "relative-images" | "relative-docs")))
        .collect();
    let mut bufs: Vec<String> = vec![String::new(); kept.len()];
    let mut options = Vec::with_capacity(kept.len());
    for (o, buf) in kept.iter().zip(bufs.iter_mut()) {
        let value = p.host_lines(&o.value, buf);
        options.push(HostOption { name: o.name, value, span: o.span });
    }
    let directive = HostDirective {
        name: d.name,
        name_span: d.name_span,
        span: d.span,
        arg,
        options,
        content,
        sections: d.sections,
    };
    let r = p.run_hosted(&directive);
    let pending = p.take_pending_classes();
    p.leave_source(frame);
    Ok((r?, pending))
}

/// The embedded parser's host: nested content of a Markdown directive is Markdown.
struct Embedded<'x, 'h, 'c> {
    det: RefCell<&'x mut Detached<'h>>,
    sx: &'x Sphinx<'c>,
    config: &'c Config,
}

impl Embedded<'_, '_, '_> {
    /// Parses with the Markdown reader over the parser's context, the parser's state
    /// and the Sphinx layer's moved where nested directives find them.
    fn with_markdown<R>(
        &self,
        p: &mut Parser<'_, '_>,
        f: impl FnOnce(&mut MdHost<'_, '_>) -> R,
    ) -> Option<R> {
        let mut det = self.det.borrow_mut();
        p.lend(|ctx: &mut ReadContext<'_>, st: &mut ReaderState| {
            let fresh = ReaderState::new(ctx, &self.config.settings);
            let real = std::mem::replace(st, fresh);
            let md = det.extension_state::<MdState>()?;
            md.reader = Some(real);
            md.sphinx = std::mem::take(&mut *self.sx.state());
            let r = det.attach(ctx, f);
            let md = det.extension_state::<MdState>()?;
            if let Some(real) = md.reader.take() {
                // The stand-in state gives its storage back to the workspace.
                std::mem::replace(st, real).end(ctx);
            }
            *self.sx.state() = std::mem::take(&mut md.sphinx);
            Some(r)
        })
    }
}

impl guidedoc_rst::Host for Embedded<'_, '_, '_> {
    fn body(
        &self,
        p: &mut Parser<'_, '_>,
        lines: &[Line<'_>],
        sections: bool,
        each: Option<guidedoc::Text>,
    ) -> Result<(), Fail> {
        let views = p.host_views(lines);
        let src = p.src();
        self.with_markdown(p, |h| h.parse_body(&views, src, sections, each)).unwrap_or(Ok(()))
    }

    fn inline(&self, p: &mut Parser<'_, '_>, lines: &[Line<'_>]) -> Result<(), Fail> {
        let views = p.host_views(lines);
        let src = p.src();
        self.with_markdown(p, |h| h.parse_inline(&views, src)).unwrap_or(Ok(()))
    }

    fn items<'s>(&self, p: &mut Parser<'_, '_>, lines: &[Line<'s>]) -> Option<Vec<HostItem<'s>>> {
        let views = p.host_views(lines);
        let src = p.src();
        let found = self.with_markdown(p, |h| h.list_items(&views, src))?.ok()??;
        let items = found
            .items
            .into_iter()
            .zip(found.starts)
            .map(|(item, start)| HostItem {
                lines: item.into_iter().map(Line::from_view).collect(),
                start,
            })
            .collect();
        Some(items)
    }
}
