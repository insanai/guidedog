//! Applying translations to a node's copy: its attributes, its inline content (parsed
//! from the translation), and its progress class.

use guidedoc::{
    Arg, Attribute, AttributeKey, AttributeRange, Builder, Diagnostic, Fail, NodeId, NodeKind,
    Request, Severity, Snapshot, Source, SourceId, SourceSpan, Status, arg_text,
};

use crate::messages::I18N_MARKUP;
use crate::translate::{PassState, Walk};
use crate::units::{Unit, index_parts, inline_span, write_message};

/// The table lengths of a builder's snapshot, so a translation that cannot be used is
/// taken back without a trace.
#[derive(Debug, Clone, Copy)]
struct Mark {
    nodes: usize,
    attributes: usize,
    lists: usize,
    links: usize,
    cells: usize,
    text: usize,
}

impl Mark {
    fn of(snap: &Snapshot) -> Mark {
        Mark {
            nodes: snap.nodes.len(),
            attributes: snap.attributes.len(),
            lists: snap.lists.len(),
            links: snap.links.len(),
            cells: snap.cells.len(),
            text: snap.text.len(),
        }
    }

    /// Truncates every table back to the mark. Only closed nodes after the mark are
    /// removed, so the builder's open stack is untouched.
    fn rollback(self, snap: &mut Snapshot) {
        snap.nodes.truncate(self.nodes);
        snap.attributes.truncate(self.attributes);
        snap.lists.truncate(self.lists);
        snap.links.truncate(self.links);
        snap.cells.truncate(self.cells);
        snap.text.truncate(self.text);
    }
}

/// The outcome of parsing one translation.
enum Parsed {
    /// Its nodes were copied into the pass's builder.
    Copied,
    /// The reader refused it, saying why.
    Refused(&'static str),
}

impl<'a> Walk<'_, 'a, '_, '_, '_> {
    /// Copies node `id`'s attributes to `copy`, translating those that carry messages:
    /// alt text, toctree captions and titles, meta values, and index entries.
    pub(crate) fn copy_attributes(
        &mut self,
        id: NodeId,
        copy: NodeId,
        unit: Unit,
    ) -> Result<(), Fail> {
        let doc = self.doc;
        for a in doc.attributes(id) {
            let original = doc.text(a.value);
            let name = doc.text(a.name);
            let custom = a.key == AttributeKey::Custom;
            let translatable = match unit {
                Unit::Alt => a.key == AttributeKey::Alt,
                Unit::Graphviz => custom && name == "alt",
                Unit::Entry | Unit::Contents => a.key == AttributeKey::Title,
                Unit::Caption => custom && name == "caption",
                Unit::Meta => custom,
                Unit::Index if custom && name == "entries" => {
                    let value = self.translate_entries(original)?;
                    let name = self.b.add_text(name)?;
                    self.b.add_attribute_named(copy, a.key, value, name)?;
                    continue;
                }
                Unit::Index | Unit::None | Unit::Inline | Unit::Literal | Unit::Parsed => false,
            };
            let mut value = original;
            if translatable && let Some(s) = self.lookup(original) {
                value = s;
                self.counts.translated += 1;
            }
            let value = self.b.add_text(value)?;
            let name = self.b.add_text(name)?;
            self.b.add_attribute_named(copy, a.key, value, name)?;
        }
        Ok(())
    }

    /// Rewrites index entry lines with each part translated, keeping the entry types,
    /// separators, and "!" markers.
    fn translate_entries(&mut self, entries: &str) -> Result<guidedoc::Text, Fail> {
        let mut out = std::mem::take(&mut self.st.scratch);
        out.clear();
        for (k, line) in entries.split('\n').enumerate() {
            if k > 0 {
                out.push('\n');
            }
            let mut at = 0;
            for part in index_parts(line) {
                out.push_str(&line[at..part.start]);
                match self.lookup(part.text) {
                    Some(s) => {
                        out.push_str(s);
                        self.counts.translated += 1;
                    }
                    None => out.push_str(part.text),
                }
                at = part.start + part.text.len();
            }
            out.push_str(&line[at..]);
        }
        let text = self.b.add_text(&out);
        self.st.scratch = out;
        text
    }

    /// Parses node `id`'s translation as the new children of its copy. Returns false,
    /// leaving the builder as it was, when there is no translation or it cannot be
    /// used: markup the reader refuses, or references the original does not have.
    pub(crate) fn translate_inline(&mut self, id: NodeId) -> Result<bool, Fail> {
        let Some(read_inline) = self.t.read_inline else {
            return Ok(false);
        };
        let Some(msgstr) = self.inline_translation(id) else {
            return Ok(false);
        };
        let doc = self.doc;
        let original = doc.node(id).span;
        let mark = Mark::of(self.b.snap);
        let parsed = parse_translation(self.st, self.t, read_inline, msgstr, self.b, original)?;
        if let Parsed::Refused(why) = parsed {
            self.b.report(I18N_MARKUP, original, arg_text(msgstr), arg_text(why));
            self.counts.kept += 1;
            return Ok(false);
        }
        // The new nodes stand where the original's inline content does.
        let span = inline_span(doc, id).unwrap_or(original);
        for n in &mut self.b.snap.nodes[mark.nodes..] {
            n.span = span;
        }
        if !self.match_references(id, NodeId::from_index(mark.nodes), msgstr)? {
            mark.rollback(self.b.snap);
            self.counts.kept += 1;
            return Ok(false);
        }
        self.counts.translated += 1;
        Ok(true)
    }

    /// Gives a translatable node's copy its progress class. Its children are built by
    /// now, so its attributes are copied to the end of the table with the class added;
    /// a node's attributes need only be one range.
    pub(crate) fn mark_progress(
        &mut self,
        id: NodeId,
        copy: NodeId,
        translated: bool,
    ) -> Result<(), Fail> {
        let mut message = std::mem::take(&mut self.st.message);
        let has_message = write_message(self.doc, id, &mut message);
        self.st.message = message;
        let wanted =
            if translated { self.t.progress.translated } else { self.t.progress.untranslated };
        if !has_message || !wanted {
            return Ok(());
        }
        let class = if translated { "translated" } else { "untranslated" };
        let class = self.b.add_text(class)?;
        let b = &mut *self.b;
        let old = b.snap.nodes[copy.index()].attributes;
        let first = b.snap.attributes.len();
        b.snap.attributes.extend_from_within(old.range());
        b.snap.attributes.push(Attribute {
            key: AttributeKey::Class,
            name: guidedoc::Text::default(),
            value: class,
        });
        let count = b.snap.attributes.len() - first;
        b.snap.nodes[copy.index()].attributes = attribute_range(first, count);
        Ok(())
    }
}

/// Returns the attribute rows `first..first + count`.
pub(crate) fn attribute_range(first: usize, count: usize) -> AttributeRange {
    AttributeRange {
        first: u32::try_from(first).unwrap_or(u32::MAX),
        count: u32::try_from(count).unwrap_or(0),
    }
}

/// Parses `msgstr` as inline content with the pass's reader workspace and copies the
/// result, as children of the open node, into `b`. The reader's warnings move to `b`'s
/// reports at `at`, the original's span. A builder failure (limit, capacity) fails the
/// pass; anything else the reader rejects refuses the translation.
fn parse_translation(
    st: &mut PassState,
    t: &crate::translate::Translation<'_>,
    read_inline: guidedoc::ReadInlineFn,
    msgstr: &str,
    b: &mut Builder<'_>,
    at: SourceSpan,
) -> Result<Parsed, Fail> {
    let name = if t.source_name.is_empty() { "<translations>" } else { t.source_name };
    let request = Request {
        source: Source { name, text: msgstr },
        read: t.read,
        policy: t.policy,
        limits: t.limits,
        ..Request::default()
    };
    let mut ctx = match st.reader.begin_read(&request) {
        Ok(ctx) => ctx,
        Err(fail) => return Ok(Parsed::Refused(fail.diagnostic.title)),
    };
    let whole = SourceSpan::new(SourceId::MAIN, 0, u32::try_from(msgstr.len()).unwrap_or(0));
    let read = ctx.b.begin_node(NodeKind::Paragraph, whole).and_then(|wrapper| {
        read_inline(&mut ctx, whole)?;
        ctx.b.end_node(wrapper).map(|()| wrapper)
    });
    let wrapper = match read {
        Ok(wrapper) => wrapper,
        Err(fail) if matches!(ctx.b.status(), Status::Limit | Status::Capacity) => {
            return Err(b.fail(*fail.diagnostic));
        }
        Err(fail) => return Ok(Parsed::Refused(refusal(&fail.diagnostic))),
    };
    if ctx.reports().error_count() > 0 {
        let why = ctx.reports().primary().map_or("the reader refused it", refusal);
        return Ok(Parsed::Refused(why));
    }
    let warnings: Vec<Diagnostic> = ctx.reports().items().to_vec();
    for d in warnings.iter().filter(|d| d.severity != Severity::Error) {
        b.reports().report(moved_diagnostic(d, at, |span| ctx.source_text(span)));
    }
    let parsed = ctx.document();
    for child in parsed.children(wrapper) {
        b.copy_subtree(parsed, child)?;
    }
    Ok(Parsed::Copied)
}

/// Returns the title of the reader's complaint.
fn refusal(d: &Diagnostic) -> &'static str {
    if d.title.is_empty() { "the reader refused it" } else { d.title }
}

/// Returns a diagnostic of the translation moved to span `at` of the document, its
/// span arguments quoted from the translation.
fn moved_diagnostic<'s>(
    d: &Diagnostic,
    at: SourceSpan,
    quote: impl Fn(SourceSpan) -> &'s str,
) -> Diagnostic {
    let mut out = d.clone();
    out.span = at;
    for arg in &mut out.args {
        if let Arg::Span(span) = *arg {
            *arg = arg_text(quote(span));
        }
    }
    out
}
