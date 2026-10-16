//! `Site`: what one HTML build's writing shares (the view, the layout, the labels), and
//! laying a page out: its context, the layout, then the build plugins' HTML hooks.

use gd_env::{DocId, SourceFile};
use gd_host::Problem;
use gd_jinja::Value;
use gd_report::Report;
use gd_resolve::BuildView;
use guidedoc::Labels;

use crate::context::{Page, page_context};
use crate::templates::{LAYOUT, Layout};

/// One HTML build's writing state.
pub(crate) struct Site<'v, 'b> {
    pub view: &'v BuildView<'b>,
    pub layout: Layout,
    pub labels: Option<&'v Labels>,
    pub sources: &'v [SourceFile],
    pub articles: crate::article_context::ArticleIndex,
}

impl Site<'_, '_> {
    /// A document's page: its body in the layout, through the plugins.
    pub(crate) fn document_page(
        &self,
        id: DocId,
        body: &str,
        reports: &mut Vec<Report>,
    ) -> Result<String, Problem> {
        let (ctx, state) = page_context(self.view, Page::Doc(id), body, &self.articles);
        let template = self.view.doc(id).page.html_template();
        let html = self.layout.render(self.view, template, &ctx, &state, reports)?;
        let file = state.page;
        self.plugin_html(&file, &self.view.doc(id).docname, html)
    }

    /// The single page of singlehtml: the root document's page holding every document,
    /// without previous and next links.
    pub(crate) fn single_page(
        &self,
        root: DocId,
        body: &str,
        reports: &mut Vec<Report>,
    ) -> Result<String, Problem> {
        let (ctx, state) = page_context(self.view, Page::Doc(root), body, &self.articles);
        ctx.set("prev", Value::None);
        ctx.set("next", Value::None);
        let html = self.layout.render(self.view, LAYOUT, &ctx, &state, reports)?;
        let file = state.page;
        self.plugin_html(&file, &self.view.config.root_doc, html)
    }

    /// A page at the output root that is no document's (`genindex.html`): its links,
    /// in the sidebar, the relation bar and `pathto`, are made from that file.
    pub(crate) fn special_page(
        &self,
        name: &str,
        title: &str,
        body: &str,
        reports: &mut Vec<Report>,
    ) -> Result<String, Problem> {
        let (ctx, state) = page_context(self.view, Page::Special(name), body, &self.articles);
        ctx.set("title", Value::from(title));
        let html = self.layout.render(self.view, LAYOUT, &ctx, &state, reports)?;
        self.plugin_html(name, name, html)
    }

    /// A page of `html_additional_pages`: the template alone, with the context of a
    /// page at the output root whose pagename is `name`.
    pub(crate) fn additional_page(
        &self,
        name: &str,
        file: &str,
        template: &str,
        reports: &mut Vec<Report>,
    ) -> Result<String, Problem> {
        let (ctx, state) = page_context(self.view, Page::Special(file), "", &self.articles);
        ctx.set("pagename", Value::from(name));
        let html = self.layout.render(self.view, template, &ctx, &state, reports)?;
        self.plugin_html(file, name, html)
    }

    /// Passes a laid-out page through the build plugins' HTML hooks.
    fn plugin_html(&self, page: &str, docname: &str, html: String) -> Result<String, Problem> {
        if self.view.plugins.is_empty() {
            return Ok(html);
        }
        let language = &self.view.config.language;
        let processed = self.view.plugins.process_html(page, docname, language, &html)?;
        Ok(processed.into_owned())
    }
}
