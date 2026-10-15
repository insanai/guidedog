#![allow(clippy::unwrap_used)]

//! The extension and host interfaces, as the Sphinx layer and the MyST reader use them.

use std::cell::Cell;

use guidedoc::{
    Fail, NodeKind, ReadContext, Text, Workspace, default_request, dump_to_string, read_document,
};
use guidedoc::{Reader, SourceSpan};
use guidedoc_rst::{
    ArgRule, ContentRule, Directive, DirectiveHandler, Extension, Host, HostDirective, HostItem,
    HostedResult, Line, OptType, Parser, RoleCall, Settings, embed_begin, opt, read_with,
};

const SHOUT_OPTIONS: &[guidedoc_rst::OptSpec] = &[opt("level", OptType::PosInt)];

/// A dialect with one directive, one role, a prologue, and a default substitution.
#[derive(Default)]
struct Dialect {
    runs: Cell<usize>,
}

impl Extension for Dialect {
    fn directive(&self, name: &str) -> Option<DirectiveHandler<'_>> {
        guidedoc_rst::equal_fold(name, "shout").then_some(DirectiveHandler {
            name: "shout",
            arg: ArgRule::Optional,
            final_ws: true,
            content: ContentRule::Optional,
            options: SHOUT_OPTIONS,
            tag: 7,
        })
    }

    fn run_directive(&self, p: &mut Parser<'_, '_>, d: &Directive<'_>) -> Result<(), Fail> {
        assert_eq!(d.handler().map(|h| h.tag), Some(7));
        self.runs.set(self.runs.get() + 1);
        let id = p.directive_node(d, "shout")?;
        if let Some(level) = d.option_value("level") {
            p.custom(id, "level", &level)?;
        }
        p.parse_body(&d.content, false, None)?;
        p.close(id)
    }

    fn role(&self, ctx: &mut ReadContext<'_>, call: &RoleCall<'_>) -> bool {
        if call.name != "loud" {
            return false;
        }
        let upper = call.text.to_uppercase();
        ctx.b.add_text_leaf(NodeKind::Text, call.span, &upper).is_ok()
    }

    fn has_prologue(&self) -> bool {
        true
    }

    fn prologue(&self, p: &mut Parser<'_, '_>) -> Result<(), Fail> {
        let Some(id) = p.ctx.add_source("<prolog>", "Prologue.\n") else {
            return Ok(());
        };
        p.parse_source(id, true)
    }

    fn substitution(&self, name: &str) -> Option<String> {
        (name == "version").then(|| "1.2".to_owned())
    }
}

fn read_dialect(ctx: &mut ReadContext<'_>) -> Result<(), Fail> {
    let dialect = Dialect::default();
    read_with(ctx, &Settings::default(), Some(&dialect))
}

const DIALECT: Reader =
    Reader { id: "dialect", read: read_dialect, read_inline: None, ..guidedoc_rst::reader() };

#[test]
fn a_dialect_adds_directives_roles_hooks_and_substitutions() {
    let text = ":field: f\n\n.. shout:: arg\n   :level: 3\n\n   Body :loud:`hey` |version|.\n";
    let mut ws = Workspace::new();
    let mut request = default_request("x.rst", text, "dialect", "html");
    request.read.promote_title = false;
    let doc = read_document(&mut ws, &DIALECT, &request).unwrap();
    let dump = dump_to_string(doc);
    let paragraphs = dump.find("text \"Prologue.\"").unwrap();
    assert!(dump.find("field-list").unwrap() < paragraphs, "{dump}");
    assert!(dump.contains("directive @name=\"shout\" @level=\"3\""), "{dump}");
    assert!(dump.contains("text \"HEY\""), "{dump}");
    assert!(dump.contains("1.2"), "{dump}");
}

#[test]
fn unknown_options_of_a_handler_are_reported() {
    let mut ws = Workspace::new();
    let text = ".. shout::\n   :volume: 3\n\n   x\n";
    let request = default_request("x.rst", text, "dialect", "html");
    let _ = read_document(&mut ws, &DIALECT, &request);
    let codes: Vec<_> = ws.reports().items().iter().map(|d| d.code).collect();
    assert!(codes.contains(&"rst.directive.option.unknown"), "{codes:?}");
}

/// A host markup whose body is one paragraph per non-blank line, prefixed "host:".
struct LineHost;

impl Host for LineHost {
    fn body(
        &self,
        p: &mut Parser<'_, '_>,
        lines: &[Line<'_>],
        _: bool,
        _: Option<Text>,
    ) -> Result<(), Fail> {
        for l in lines.iter().filter(|l| !l.is_blank()) {
            let id = p.open(NodeKind::Paragraph, p.line_span(l))?;
            p.ctx.b.add_text_leaf(NodeKind::Text, p.line_span(l), "host:")?;
            p.inline(std::slice::from_ref(l))?;
            p.close(id)?;
        }
        Ok(())
    }

    fn inline(&self, p: &mut Parser<'_, '_>, lines: &[Line<'_>]) -> Result<(), Fail> {
        let views = p.host_views(lines);
        p.inline_views(&views)
    }

    fn items<'s>(&self, _: &mut Parser<'_, '_>, _: &[Line<'s>]) -> Option<Vec<HostItem<'s>>> {
        None
    }
}

fn read_hosted(ctx: &mut ReadContext<'_>) -> Result<(), Fail> {
    let text = ctx.source_of(ctx.source);
    let mut buf = String::new();
    let mut st = embed_begin(ctx, &Settings::default(), None)?;
    let mut p = st.parser(ctx, None, Some(&LineHost));
    let views: Vec<_> = text
        .split_inclusive('\n')
        .scan(0u32, |offset, l| {
            let v = guidedoc::LineView { text: l.trim_end_matches('\n'), offset: *offset };
            *offset += l.len() as u32;
            Some(v)
        })
        .collect();
    let lines = p.host_lines(&views[1..], &mut buf);
    let span = SourceSpan::new(p.src(), 0, text.len() as u32);
    let h = HostDirective {
        name: "note",
        name_span: span,
        span,
        content: lines,
        ..HostDirective::default()
    };
    assert_eq!(p.run_hosted(&h)?, HostedResult::Ran);
    let unknown = HostDirective { name: "nothing", ..h.clone() };
    assert_eq!(p.run_hosted(&unknown)?, HostedResult::Unknown);
    p.embed_end()
}

const HOSTED: Reader =
    Reader { id: "hosted", read: read_hosted, read_inline: None, ..guidedoc_rst::reader() };

#[test]
fn a_host_runs_directives_whose_content_it_parses() {
    let text = "note\none *two*\nthree\n";
    let mut ws = Workspace::new();
    let mut request = default_request("x.md", text, "hosted", "html");
    request.read.promote_title = false;
    let doc = read_document(&mut ws, &HOSTED, &request).unwrap();
    let want = concat!(
        "document\n",
        "  admonition value=7\n",
        "    paragraph\n",
        "      text \"host:\"\n",
        "      text \"one \"\n",
        "      emphasis\n",
        "        text \"two\"\n",
        "    paragraph\n",
        "      text \"host:\"\n",
        "      text \"three\"\n",
    );
    assert_eq!(dump_to_string(doc), want);
}
