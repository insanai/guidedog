//! Hosts: another markup whose documents use this reader's directives and roles, such
//! as MyST Markdown. The host reads the main source with its own reader, keeps a
//! `ReaderState` made by `embed_begin`, and runs each directive it finds through
//! `run_hosted` (and each role through `guidedoc_rst_inline::apply_host_role`). A
//! handler asks for nested content with `Parser::parse_body` and `Parser::inline`, which
//! parse it in the markup the directive was written in: the host's for a hosted
//! directive, reStructuredText otherwise. Handlers that take their content literally
//! (code, math, raw) read the lines themselves. reStructuredText inside a hosted
//! document (MyST's eval-rst) is parsed with `Parser::parse_blocks`, and directives
//! found there are reStructuredText again.

use guidedoc::{Fail, LineView, ReadContext, SourceSpan, Text};
use guidedoc_rst_inline::resolve_with_substitutions;

use crate::directives::{DirOption, Directive};
use crate::extension::Extension;
use crate::lines::{Line, expand_views, trim_blank_end, trim_blank_start};
use crate::parser::{Parser, ReaderState};
use crate::settings::Settings;
use crate::specs::{ArgRule, ContentRule, OptSpec, find_directive};

/// The host markup's side of hosted directives. Lines are views of the current source
/// (`Parser::src`), as the block parser's are, so every node keeps exact spans. Methods
/// take `&self`: a host keeps its parser state in cells, as they run re-entrantly.
pub trait Host {
    /// Parses nested body content of the open node; `sections` allows headings to make
    /// sections, and `each`, when set, gives these classes to every top-level element
    /// (the class directive).
    fn body(
        &self,
        p: &mut Parser<'_, '_>,
        lines: &[Line<'_>],
        sections: bool,
        each: Option<Text>,
    ) -> Result<(), Fail>;

    /// Parses inline content of the open node.
    fn inline(&self, p: &mut Parser<'_, '_>, lines: &[Line<'_>]) -> Result<(), Fail>;

    /// Splits lines that hold one bullet list, in the host's list syntax, into its
    /// items (list-table); `None` when the lines are not one bullet list.
    fn items<'s>(&self, p: &mut Parser<'_, '_>, lines: &[Line<'s>]) -> Option<Vec<HostItem<'s>>>;
}

/// One item of a host's bullet list: its content lines and the index in the given
/// lines of the line it starts on.
#[derive(Debug, Clone, Default)]
pub struct HostItem<'s> {
    /// The item's content.
    pub lines: Vec<Line<'s>>,
    /// Where the item starts in the lines `Host::items` was given.
    pub start: usize,
}

/// A directive option as the host wrote it: its name and value lines.
#[derive(Debug, Clone)]
pub struct HostOption<'s> {
    /// The option name.
    pub name: &'s str,
    /// The value lines.
    pub value: Vec<Line<'s>>,
    /// Where it was written.
    pub span: SourceSpan,
}

/// A directive the host has split into its parts.
#[derive(Debug, Clone, Default)]
pub struct HostDirective<'s> {
    /// The directive name.
    pub name: &'s str,
    /// Where the name was written.
    pub name_span: SourceSpan,
    /// The whole directive.
    pub span: SourceSpan,
    /// The argument lines; empty when there is none.
    pub arg: Vec<Line<'s>>,
    /// The options, in order.
    pub options: Vec<HostOption<'s>>,
    /// The content lines.
    pub content: Vec<Line<'s>>,
    /// The directive stands where sections are allowed.
    pub sections: bool,
}

/// The outcome of `run_hosted`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostedResult {
    /// The directive ran.
    Ran,
    /// No directive has that name; nothing was reported.
    Unknown,
    /// The arguments, options, or content were refused and reported.
    Failed,
}

/// How a directive takes its parts, for a host that checks them itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectiveShape<'h> {
    /// Whether it takes an argument.
    pub arg: ArgRule,
    /// The last argument may contain white space.
    pub final_ws: bool,
    /// Whether it takes content.
    pub content: ContentRule,
    /// Its options.
    pub options: &'h [OptSpec],
}

impl<'p, 'a> Parser<'p, 'a> {
    /// Looks directive `name` up as the reader would, the dialect's handlers first.
    pub fn directive_shape(&self, name: &str) -> Option<DirectiveShape<'p>> {
        if let Some(h) = self.ext.and_then(|e| e.directive(name)) {
            let options = h.options;
            return Some(DirectiveShape {
                arg: h.arg,
                final_ws: h.final_ws,
                content: h.content,
                options,
            });
        }
        let s = find_directive(name)?;
        Some(DirectiveShape {
            arg: s.arg,
            final_ws: s.final_ws,
            content: s.content,
            options: s.options,
        })
    }

    /// Checks and runs a directive written in the host markup. Options are checked as
    /// in reStructuredText; a host with other rules checks them first
    /// (`directive_shape`, `option_problem`) and passes only those it keeps. Fails only
    /// when the builder fails.
    pub fn run_hosted(&mut self, h: &HostDirective<'_>) -> Result<HostedResult, Fail> {
        let Some(runner) = self.runner_for(h.name) else {
            return Ok(HostedResult::Unknown);
        };
        let mut d = Directive::new(h.name, runner, h.span, h.name_span, h.sections);
        let specs = d.shape().3;
        let spec_name = d.spec_name();
        let mut opts = Vec::with_capacity(h.options.len());
        for o in &h.options {
            let value =
                DirOption { spec: OptSpec::default(), lines: o.value.clone(), span: o.span };
            if !self.check_option(spec_name, specs, o.name, value, &mut opts) {
                return Ok(HostedResult::Failed);
            }
        }
        d.options = opts;
        d.content = trim_blank_end(trim_blank_start(&h.content)).to_vec();
        if !self.directive_arguments(&mut d, &h.arg) {
            return Ok(HostedResult::Failed);
        }
        let hosted = std::mem::replace(&mut self.st.hosted, true);
        let r = self.run(&d);
        self.st.hosted = hosted;
        r.map(|()| HostedResult::Ran)
    }

    /// Makes lines of a host's views of source text: tabs are expanded to the parser's
    /// tab stops into `buf`, as the reader's own lines are.
    pub fn host_lines<'s>(&self, views: &[LineView<'s>], buf: &'s mut String) -> Vec<Line<'s>> {
        expand_views(views, self.st.tab, buf)
    }

    /// Gives the host the source text of lines, tabs unexpanded.
    pub fn host_views<'s>(&self, lines: &[Line<'s>]) -> Vec<LineView<'s>> {
        lines.iter().map(|l| l.view(self.st.tab)).collect()
    }

    /// Lends the read context and the reader's state to `f`. A host that parses nested
    /// content with a reader of its own, whose directives run through this reader again,
    /// uses it to move the state where those handlers find it (and back) while the
    /// context is with its reader.
    pub fn lend<R>(&mut self, f: impl FnOnce(&mut ReadContext<'a>, &mut ReaderState) -> R) -> R {
        f(self.ctx, self.st)
    }

    /// Ends an embedded read: runs the dialect's end hook, closes sections, reports
    /// what was left dangling, and resolves the document as `read` does. The state's
    /// storage then goes back to the workspace (`ReaderState::end`).
    pub fn embed_end(&mut self) -> Result<(), Fail> {
        if let Some(ext) = self.ext {
            ext.end(self)?;
        }
        self.finish_document()?;
        let options = self.st.inl.options.clone();
        let result = match self.ext {
            Some(ext) => resolve_with_substitutions(self.ctx, &options, &|n| ext.substitution(n)),
            None => guidedoc_rst_inline::resolve(self.ctx, &options),
        };
        self.st.end(self.ctx);
        result
    }
}

/// Prepares the reader for a host that reads the main source itself: the state for
/// `ctx` with `settings`, after the dialect's begin hook ran. The host views the state
/// as a parser (`ReaderState::parser`) for every call and ends with
/// `Parser::embed_end`. Fails only when the builder fails.
pub fn embed_begin(
    ctx: &mut ReadContext<'_>,
    settings: &Settings,
    ext: Option<&dyn Extension>,
) -> Result<ReaderState, Fail> {
    let mut st = ReaderState::new(ctx, settings);
    if let Some(e) = ext {
        e.begin(&mut st.parser(ctx, ext, None))?;
    }
    Ok(st)
}
