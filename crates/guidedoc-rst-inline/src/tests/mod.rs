//! Tests; `Fx` is the fixture a test body receives: a read context and an inline state.

mod extra;
mod inline;
mod limits;
mod promote;
mod punctuation;
mod resolution;
mod roles;

use guidedoc::{
    LinkInfo, LinkKind, NodeFlags, NodeId, NodeKind, ReadContext, SourceSpan, Workspace,
    default_request, dump_to_string, trusted_policy, validate,
};

use crate::{InlineState, Line, NoRoleHook, Role, parse_inline, resolve};

/// A read in progress over one source, as `read_document` prepares it for a reader.
pub(crate) struct Fx<'w> {
    pub ctx: ReadContext<'w>,
    pub state: InlineState,
    pub source: &'w str,
}

/// Runs `body` with a fixture over `source`; `promote` sets `promote_title`.
pub(crate) fn fixture<R>(source: &str, promote: bool, body: impl FnOnce(&mut Fx<'_>) -> R) -> R {
    let mut ws = Workspace::new();
    let mut request = default_request("test.rst", source, "rst", "html");
    request.read.promote_title = promote;
    request.policy = trusted_policy();
    let ctx = ws.begin_read(&request).expect("the fixture starts a read");
    let mut fx = Fx { ctx, state: InlineState::default(), source };
    body(&mut fx)
}

impl<'w> Fx<'w> {
    /// Splits `text`, found at its first occurrence in the source, into lines.
    pub fn lines_of(&self, text: &str) -> Vec<Line<'w>> {
        let base = self.source.find(text).expect("fragment is not in the source");
        let text = &self.source[base..base + text.len()];
        let mut out = Vec::new();
        let mut start = 0;
        for (i, c) in text.char_indices().chain([(text.len(), '\n')]) {
            if c == '\n' {
                out.push(Line { text: &text[start..i], offset: (base + start) as u32 });
                start = i + 1;
            }
        }
        out
    }

    pub fn inlines(&mut self, text: &str) {
        let lines = self.lines_of(text);
        let _ = parse_inline(&mut self.ctx, &mut self.state, &lines, &mut NoRoleHook);
    }

    pub fn open(&mut self, kind: NodeKind, text: &str) -> NodeId {
        let at = if text.is_empty() { 0 } else { self.source.find(text).unwrap_or(0) as u32 };
        let span = SourceSpan::new(self.ctx.source, at, at + text.len() as u32);
        self.ctx.b.begin_node(kind, span).expect("begin")
    }

    pub fn close(&mut self, id: NodeId) {
        self.ctx.b.end_node(id).expect("end");
    }

    pub fn para(&mut self, text: &str) {
        let p = self.open(NodeKind::Paragraph, text);
        self.inlines(text);
        self.close(p);
    }

    /// Adds a Target: external with `dest`, indirect with `refname`, else internal.
    pub fn target_with(
        &mut self,
        name: &str,
        dest: &str,
        refname: &str,
        anonymous: bool,
    ) -> NodeId {
        let id = self.open(NodeKind::Target, "");
        let b = &mut self.ctx.b;
        let t = b.add_text(name).expect("text");
        b.set_text(id, t);
        if anonymous {
            b.add_flags(id, NodeFlags::ANONYMOUS);
        }
        let mut info = LinkInfo::UNRESOLVED;
        if !dest.is_empty() {
            info.destination = b.add_text(dest).expect("text");
            info.kind = LinkKind::External;
        }
        if !refname.is_empty() {
            info.name = b.add_text(refname).expect("text");
        }
        b.set_link(id, info).expect("link");
        self.close(id);
        id
    }

    pub fn target(&mut self, name: &str) -> NodeId {
        self.target_with(name, "", "", false)
    }

    pub fn section(&mut self, level: i32, title: &str) -> NodeId {
        let s = self.open(NodeKind::Section, title);
        self.ctx.b.set_value(s, level);
        let h = self.open(NodeKind::Heading, title);
        self.ctx.b.set_value(h, level);
        self.inlines(title);
        self.close(h);
        s
    }

    pub fn footnote(&mut self, label: &str, body: &str) -> NodeId {
        let id = self.open(NodeKind::Footnote, "");
        let t = self.ctx.b.add_text(label).expect("text");
        self.ctx.b.set_text(id, t);
        if label == "*" {
            self.ctx.b.add_flags(id, NodeFlags::SYMBOL);
        } else if label.starts_with('#') {
            self.ctx.b.add_flags(id, NodeFlags::AUTO);
        }
        self.para(body);
        self.close(id);
        id
    }

    pub fn citation(&mut self, label: &str, body: &str) {
        let id = self.open(NodeKind::Citation, "");
        let t = self.ctx.b.add_text(label).expect("text");
        self.ctx.b.set_text(id, t);
        self.para(body);
        self.close(id);
    }

    pub fn definition(&mut self, name: &str, body: &str) {
        let id = self.open(NodeKind::SubstitutionDefinition, "");
        let t = self.ctx.b.add_text(name).expect("text");
        self.ctx.b.set_text(id, t);
        self.inlines(body);
        self.close(id);
    }

    pub fn comment(&mut self) {
        let t = self.ctx.b.add_text("note").expect("text");
        self.ctx.b.add_leaf(NodeKind::Comment, SourceSpan::default(), t).expect("leaf");
    }

    pub fn add_attribute(&mut self, id: NodeId, key: guidedoc::AttributeKey, value: &str) {
        self.ctx.b.add_attribute_string(id, key, value).expect("attribute");
    }

    pub fn add_custom(&mut self, id: NodeId, name: &str, value: &str) {
        self.ctx.b.add_custom_attribute(id, name, value).expect("attribute");
    }

    /// The tree outline of the snapshot.
    pub fn dump(&self) -> String {
        dump_to_string(self.ctx.document())
    }

    /// The codes of the reports, joined by ",".
    pub fn codes(&mut self) -> String {
        let codes: Vec<&str> = self.ctx.reports().items().iter().map(|d| d.code).collect();
        codes.join(",")
    }

    pub fn node(&self, id: NodeId) -> guidedoc::Node {
        self.ctx.b.snap.nodes[id]
    }

    /// Resolves, closes the root, validates, and dumps.
    pub fn finish(&mut self) -> String {
        let options = self.state.options.clone();
        if resolve(&mut self.ctx, &options).is_err() || self.ctx.b.finish().is_err() {
            return "builder failed".to_owned();
        }
        match validate(self.ctx.b.snap, &[self.source.len()], &self.ctx.limits) {
            Ok(()) => self.dump(),
            Err(fail) => fail.code().to_owned(),
        }
    }
}

/// Parses `source` as one paragraph and returns the paragraph's children, dedented,
/// followed by the report codes after "|" when there are any.
pub(crate) fn inline_dump(source: &str, default_role: &str, roles: &[Role]) -> String {
    fixture(source, false, |f| {
        f.state.roles = roles.to_vec();
        f.state.default_role = default_role.to_owned();
        f.para(source);
        if f.ctx.b.finish().is_err() {
            return "builder failed".to_owned();
        }
        if let Err(fail) = validate(f.ctx.b.snap, &[source.len()], &f.ctx.limits) {
            return fail.code().to_owned();
        }
        let mut out = String::new();
        for line in f.dump().lines().skip(2).filter(|l| !l.is_empty()) {
            out.push_str(&line[4..]);
            out.push('\n');
        }
        let codes = f.codes();
        if !codes.is_empty() {
            out.push_str("| ");
            out.push_str(&codes);
        }
        out
    })
}

#[track_caller]
pub(crate) fn expect_inline(source: &str, want: &str) {
    assert_eq!(inline_dump(source, "", &[]), want, "source: {source:?}");
}
