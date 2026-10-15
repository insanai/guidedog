//! The extension interface: how a dialect runs the directives and roles MyST Markdown
//! writes, as the RST reader's host interface does for reStructuredText. The Sphinx
//! layer implements `Extension` so one set of handlers serves both markups. The reader
//! parses a directive's parts; the handler checks and runs them, and asks the reader to
//! parse nested content with `Host::parse_body` and `Host::parse_inline`, so content
//! written in Markdown stays Markdown.

use std::any::Any;
use std::ops::Range;

use guidedoc::{Builder, Fail, LineView, NodeId, ReadContext, SourceId, SourceSpan, Text};

use crate::blocks::{list_item_lines, parse_lines};
use crate::host::Host;
use crate::myst::{inline_lines, skip_front_matter};
use crate::settings::Settings;

/// A view of one input line: its text without the line break, and the source offset of
/// its first byte. Handlers receive content as lines and pass nested content back so.
pub type Line<'s> = LineView<'s>;

/// What became of a directive a handler was offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectiveResult {
    /// It ran.
    Ran,
    /// The handler does not know the name; the reader reports it.
    Unknown,
    /// The handler refused the directive and reported why.
    Failed,
}

/// How a directive takes its first line: as an argument, or (without arguments) as the
/// first line of content; and whether it takes options at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectiveShape {
    /// It takes an argument.
    pub argument: bool,
    /// It has options; without, an option block stays content.
    pub options: bool,
}

impl Default for DirectiveShape {
    /// An argument and options, the shape assumed when the extension does not say.
    fn default() -> Self {
        DirectiveShape { argument: true, options: true }
    }
}

/// Directive and role handlers for the "myst" reader (`Settings::extension`). Every
/// method borrows the host for the call; per-read state goes in
/// `Host::set_extension_state`, typically from `begin`. Names arrive as written; compare
/// them case-insensitively, as myst-parser does.
pub trait Extension: Send + Sync {
    /// Describes directive `name`; `None` for a directive the extension does not know,
    /// which the reader reports.
    fn shape(&self, h: &mut Host<'_, '_>, name: &str) -> Option<DirectiveShape> {
        let _ = (h, name);
        Some(DirectiveShape::default())
    }

    /// Runs a parsed directive. Its lines borrow the reader's buffers for the call.
    fn directive(&self, h: &mut Host<'_, '_>, d: &Directive<'_>) -> Result<DirectiveResult, Fail>;

    /// Runs a role; `Ok(false)` for a role it does not know, which the reader reports
    /// and keeps as text.
    fn role(&self, h: &mut Host<'_, '_>, r: &Role<'_>) -> Result<bool, Fail>;

    /// Runs after the reader opens a Table node (`table`), before its rows, so a table
    /// directive around it can add its title and attributes; `Ok(true)` when it did,
    /// and the reader then adds no attributes of its own.
    fn table(
        &self,
        h: &mut Host<'_, '_>,
        table: NodeId,
        span: SourceSpan,
        columns: usize,
    ) -> Result<bool, Fail> {
        let _ = (h, table, span, columns);
        Ok(false)
    }

    /// Runs before the document's blocks (and before inline content read alone).
    fn begin(&self, h: &mut Host<'_, '_>) -> Result<(), Fail> {
        let _ = h;
        Ok(())
    }

    /// Runs after the document's blocks and footnotes.
    fn end(&self, h: &mut Host<'_, '_>) -> Result<(), Fail> {
        let _ = h;
        Ok(())
    }

    /// Links references, targets, and footnotes instead of the reader's own resolution;
    /// `Ok(false)` (the default) lets the reader resolve them. The Sphinx layer resolves
    /// as for reStructuredText.
    fn resolve(&self, h: &mut Host<'_, '_>) -> Result<bool, Fail> {
        let _ = h;
        Ok(false)
    }
}

/// One directive option: its name and value lines. A value written as a YAML scalar has
/// its quoting and folding processed, so its lines may not be views of the source;
/// `span` is where the value is written.
#[derive(Debug, Clone)]
pub struct DirectiveOption<'d> {
    /// The option's name.
    pub name: &'d str,
    /// Its value; empty for a flag.
    pub value: Vec<Line<'d>>,
    /// Where it is written.
    pub span: SourceSpan,
}

impl DirectiveOption<'_> {
    /// Returns the value's lines joined by "\n".
    pub fn text(&self) -> String {
        join_lines(&self.value)
    }
}

/// A directive the reader has split into its parts. Lines are views of `source` (or of
/// the reader's copies of it); they borrow the reader for the call.
#[derive(Debug, Clone)]
pub struct Directive<'d> {
    /// The name, as written.
    pub name: &'d str,
    /// Where the name is.
    pub name_span: SourceSpan,
    /// The whole directive.
    pub span: SourceSpan,
    /// The source the lines belong to.
    pub source: SourceId,
    /// The argument; empty when there is none.
    pub argument: Vec<Line<'d>>,
    /// The options, in order.
    pub options: Vec<DirectiveOption<'d>>,
    /// The content after the options.
    pub content: Vec<Line<'d>>,
    /// The directive stands where sections are allowed.
    pub sections: bool,
}

impl Directive<'_> {
    /// Returns the option named `name`.
    pub fn option(&self, name: &str) -> Option<&DirectiveOption<'_>> {
        self.options.iter().find(|o| o.name == name)
    }

    /// Returns the argument's lines joined by "\n".
    pub fn argument_text(&self) -> String {
        join_lines(&self.argument)
    }
}

/// A role, `{name}`content``. `lines` hold the whole construct split at line breaks;
/// `content` is the range of the content in the lines joined by "\n".
#[derive(Debug, Clone)]
pub struct Role<'r> {
    /// The role's name, as written.
    pub name: &'r str,
    /// The whole construct, as lines.
    pub lines: Vec<Line<'r>>,
    /// The content's range in the joined lines.
    pub content: Range<usize>,
    /// The whole construct.
    pub span: SourceSpan,
    /// The source the lines belong to.
    pub source: SourceId,
}

impl Role<'_> {
    /// Returns the content: the lines joined by "\n", cut to `content`.
    pub fn content_text(&self) -> String {
        let joined = join_lines(&self.lines);
        joined.get(self.content.clone()).unwrap_or("").to_owned()
    }
}

/// The items of a bullet list, as `Host::list_items` splits it: each item's content
/// lines, and the index of its first line in the given lines.
#[derive(Debug, Clone, Default)]
pub struct ListItems<'l> {
    /// Each item's content lines, cut at its content column.
    pub items: Vec<Vec<Line<'l>>>,
    /// Where each item starts in the given lines.
    pub starts: Vec<usize>,
}

fn join_lines(lines: &[Line<'_>]) -> String {
    let mut out = String::new();
    for (k, l) in lines.iter().enumerate() {
        if k > 0 {
            out.push('\n');
        }
        out.push_str(l.text);
    }
    out
}

impl<'h, 'a> Host<'h, 'a> {
    /// Parses `lines` of `source` as Markdown body content of the builder's open node.
    /// `sections` lets headings open sections, as at the top of a document; `classes`,
    /// when set, go to every top-level element. Included sources lose their front
    /// matter, as MyST ignores it.
    pub fn parse_body(
        &mut self,
        lines: &[Line<'_>],
        source: SourceId,
        sections: bool,
        classes: Option<Text>,
    ) -> Result<(), Fail> {
        let saved = std::mem::replace(&mut self.ms.each, classes);
        let mut body = lines;
        if self.myst && source != self.ctx.source && lines.first().is_some_and(|l| l.offset <= 3) {
            body = skip_front_matter(lines);
        }
        let result = parse_lines(self, body, source, sections, false);
        self.ms.each = saved;
        result
    }

    /// Parses `lines` of `source` as Markdown inline content of the builder's open node.
    pub fn parse_inline(&mut self, lines: &[Line<'_>], source: SourceId) -> Result<(), Fail> {
        inline_lines(self, lines, source)
    }

    /// Returns the read context, for handlers that build nodes or include sources.
    pub fn context(&mut self) -> &mut ReadContext<'a> {
        self.ctx
    }

    /// Returns the builder of the read.
    pub fn builder(&mut self) -> &mut Builder<'a> {
        &mut self.ctx.b
    }

    /// Returns the reader's settings.
    pub fn settings(&self) -> &'h Settings {
        self.settings
    }

    /// Tells whether the MyST syntax is on (the "myst" reader).
    pub fn is_myst(&self) -> bool {
        self.myst
    }

    /// Gives `classes` (pool text) to the next block the reader emits, as Docutils'
    /// class directive without content does.
    pub fn set_classes_for_next(&mut self, classes: Text) {
        self.ms.pending = Some(classes);
    }

    /// Sets MyST's include options `:relative-images:` and `:relative-docs:` for the
    /// content of the include directive being run; returns the previous values, which
    /// the handler restores afterwards.
    pub fn include_paths(&mut self, images: bool, docs: &str) -> (bool, String) {
        let old_images = std::mem::replace(&mut self.rel_images, images);
        let old_docs = std::mem::replace(&mut self.rel_docs, docs.to_owned());
        (old_images, old_docs)
    }

    /// Splits `lines` that hold one bullet list into the content lines of its items
    /// (for list-table). `Ok(None)` when the lines are anything but one bullet list.
    pub fn list_items<'l>(
        &mut self,
        lines: &[Line<'l>],
        source: SourceId,
    ) -> Result<Option<ListItems<'l>>, Fail> {
        list_item_lines(self, lines, source)
    }

    /// Stores the extension's per-read state, replacing any earlier state.
    pub fn set_extension_state<T: Any>(&mut self, state: T) {
        self.ext_state = Some(Box::new(state));
    }

    /// Returns the extension's per-read state, when it is a `T`.
    pub fn extension_state<T: Any>(&mut self) -> Option<&mut T> {
        self.ext_state.as_mut().and_then(|s| s.downcast_mut::<T>())
    }

    /// Writes a reference name into the text pool, case-folded with white space
    /// collapsed, as targets and names are stored.
    pub fn name_text(&mut self, s: &str) -> Result<Text, Fail> {
        self.normalized_name(s)
    }
}
