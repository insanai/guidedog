//! Every built-in renderer writes every node kind. The renderers route kinds with
//! exhaustive matches, so a new kind does not compile until it is routed; this test
//! renders one document holding every kind of `NodeKind::ALL` with each renderer of the
//! built-in registry and fails when a renderer reports a kind its route does not write
//! (`core.render.kind`), fails otherwise, or drops text a reader sees. A kind added to
//! the AST fails the "every kind is present" check until this document holds it.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

use guidedoc::{
    AdmonitionKind, AttributeKey as Key, Builder, CellInfo, Conformance, Fail, INVALID_NODE,
    LinkInfo, LinkKind, ListInfo, ListStyle, NodeId, NodeKind as K, ReadContext, Reader, Registry,
    Request, Source, SourceSpan, Text, Workspace, convert, read_document, trusted_policy,
};

type R = Result<(), Fail>;

/// Builder shorthands; every call passes on the builder's failure.
struct Tb<'t, 'b> {
    b: &'t mut Builder<'b>,
}

impl Tb<'_, '_> {
    fn open(&mut self, kind: K, value: i32) -> Result<NodeId, Fail> {
        let id = self.b.begin_node(kind, SourceSpan::default())?;
        self.b.set_value(id, value);
        Ok(id)
    }

    fn close(&mut self, id: NodeId) -> R {
        self.b.end_node(id)
    }

    fn text(&mut self, s: &str) -> R {
        self.b.add_text_leaf(K::Text, SourceSpan::default(), s).map(|_| ())
    }

    /// A node of `kind` whose own text is `s`, without children.
    fn leaf(&mut self, kind: K, s: &str) -> Result<NodeId, Fail> {
        let id = self.b.begin_node(kind, SourceSpan::default())?;
        let t = self.b.add_text(s)?;
        self.b.set_text(id, t);
        self.b.end_node(id)?;
        Ok(id)
    }

    /// `kind` holding the inline text `s`.
    fn wrap(&mut self, kind: K, s: &str) -> Result<NodeId, Fail> {
        let id = self.open(kind, 0)?;
        self.text(s)?;
        self.close(id)?;
        Ok(id)
    }

    fn para(&mut self, s: &str) -> R {
        self.wrap(K::Paragraph, s).map(|_| ())
    }

    /// `kind` holding one paragraph.
    fn block(&mut self, kind: K, s: &str, value: i32) -> Result<NodeId, Fail> {
        let id = self.open(kind, value)?;
        self.para(s)?;
        self.close(id)?;
        Ok(id)
    }

    fn link(&mut self, id: NodeId, kind: LinkKind, destination: &str, name: &str) -> R {
        let destination = self.b.add_text(destination)?;
        let name = self.b.add_text(name)?;
        let info =
            LinkInfo { destination, name, kind, resolved: INVALID_NODE, title: Text::default() };
        self.b.set_link(id, info).map(|_| ())
    }

    fn set_text(&mut self, id: NodeId, s: &str) -> R {
        let t = self.b.add_text(s)?;
        self.b.set_text(id, t);
        Ok(())
    }

    fn image(&mut self) -> R {
        let id = self.open(K::Image, 0)?;
        self.b.add_attribute_string(id, Key::Alt, "picture")?;
        self.link(id, LinkKind::External, "picture.png", "")?;
        self.close(id)
    }

    fn list(&mut self) -> R {
        let list = self.open(K::List, 0)?;
        self.b.set_list(list, ListInfo { style: ListStyle::Bullet, delimiter: b'*', start: 1 })?;
        self.block(K::ListItem, "item", 0)?;
        self.close(list)
    }
}

fn read_every_kind(ctx: &mut ReadContext<'_>) -> R {
    let t = &mut Tb { b: &mut ctx.b };
    t.wrap(K::Title, "title")?;
    t.wrap(K::Subtitle, "subtitle")?;
    let section = t.open(K::Section, 1)?;
    let heading = t.wrap(K::Heading, "section")?;
    t.b.set_value(heading, 1);
    every_inline(t)?;
    every_structure(t)?;
    every_literal(t)?;
    every_box(t)?;
    every_sphinx(t)?;
    t.close(section)
}

fn every_inline(t: &mut Tb<'_, '_>) -> R {
    let p = t.open(K::Paragraph, 0)?;
    t.text("text ")?;
    for kind in [K::Emphasis, K::Strong, K::Subscript, K::Superscript, K::TitleReference] {
        t.wrap(kind, "x")?;
    }
    for kind in [K::Abbreviation, K::Role, K::Xref] {
        t.wrap(kind, "x")?;
    }
    let target = t.open(K::InlineTarget, 0)?;
    t.set_text(target, "inline target")?;
    t.text("inline target")?;
    t.close(target)?;
    t.leaf(K::Code, "code")?;
    t.leaf(K::Math, "\\alpha")?;
    let raw = t.leaf(K::RawInline, "<b>raw</b>")?;
    t.b.add_attribute_string(raw, Key::Format, "html")?;
    t.leaf(K::SoftBreak, "")?;
    t.leaf(K::HardBreak, "")?;
    for (kind, destination) in [(K::Link, "https://example.org"), (K::Reference, "https://x.org")]
    {
        let id = t.open(kind, 0)?;
        t.link(id, LinkKind::External, destination, "")?;
        t.text("link")?;
        t.close(id)?;
    }
    t.image()?;
    for (kind, name) in [
        (K::FootnoteReference, "1"),
        (K::CitationReference, "CIT"),
        (K::SubstitutionReference, "s"),
    ] {
        let id = t.leaf(kind, name)?;
        t.link(id, LinkKind::Unresolved, "", name)?;
    }
    t.leaf(K::Index, "")?;
    t.close(p)
}

fn every_structure(t: &mut Tb<'_, '_>) -> R {
    let quote = t.open(K::BlockQuote, 0)?;
    t.para("quoted")?;
    t.wrap(K::Attribution, "someone")?;
    t.close(quote)?;
    t.list()?;
    let dl = t.open(K::DefinitionList, 0)?;
    let item = t.open(K::DefinitionItem, 0)?;
    t.wrap(K::Term, "term")?;
    t.wrap(K::Classifier, "classifier")?;
    t.block(K::Definition, "definition", 0)?;
    t.close(item)?;
    t.close(dl)?;
    let fl = t.open(K::FieldList, 0)?;
    let field = t.open(K::Field, 0)?;
    t.wrap(K::FieldName, "name")?;
    t.block(K::FieldBody, "body", 0)?;
    t.close(field)?;
    t.close(fl)?;
    let ol = t.open(K::OptionList, 0)?;
    let item = t.open(K::OptionItem, 0)?;
    let group = t.open(K::OptionGroup, 0)?;
    let option = t.open(K::Option, 0)?;
    t.set_text(option, "--all")?;
    t.leaf(K::OptionArgument, "N")?;
    t.close(option)?;
    t.close(group)?;
    t.block(K::OptionDescription, "option", 0)?;
    t.close(item)?;
    t.close(ol)?;
    let lines = t.open(K::LineBlock, 0)?;
    t.wrap(K::Line, "line")?;
    t.close(lines)
}

fn every_literal(t: &mut Tb<'_, '_>) -> R {
    t.leaf(K::CodeBlock, "code\n")?;
    t.wrap(K::Preformatted, "preformatted")?;
    t.leaf(K::DoctestBlock, ">>> doctest")?;
    t.leaf(K::MathBlock, "\\beta")?;
    let raw = t.leaf(K::RawBlock, "<hr>")?;
    t.b.add_attribute_string(raw, Key::Format, "html")?;
    t.leaf(K::ThematicBreak, "")?;
    let table = t.open(K::Table, 1)?;
    t.wrap(K::Title, "table")?;
    for part in [K::TableHead, K::TableBody] {
        let part = t.open(part, 0)?;
        let row = t.open(K::Row, 0)?;
        let cell = t.open(K::Cell, 0)?;
        t.b.set_cell(cell, CellInfo::default())?;
        t.para("cell")?;
        t.close(cell)?;
        t.close(row)?;
        t.close(part)?;
    }
    t.close(table)
}

fn every_box(t: &mut Tb<'_, '_>) -> R {
    let figure = t.open(K::Figure, 0)?;
    t.image()?;
    t.wrap(K::Caption, "caption")?;
    t.block(K::Legend, "legend", 0)?;
    t.close(figure)?;
    t.block(K::Admonition, "note", AdmonitionKind::Note as i32)?;
    for (kind, body) in [(K::Topic, "topic body"), (K::Sidebar, "sidebar body")] {
        let id = t.open(kind, 0)?;
        t.wrap(K::Title, "boxed")?;
        t.para(body)?;
        t.close(id)?;
    }
    t.wrap(K::Rubric, "rubric")?;
    t.block(K::Container, "container", 0)?;
    t.block(K::Compound, "compound", 0)?;
    let note = t.block(K::Footnote, "footnote", 1)?;
    t.set_text(note, "1")?;
    let citation = t.block(K::Citation, "citation", 0)?;
    t.set_text(citation, "CIT")?;
    t.leaf(K::Target, "")?;
    t.leaf(K::Comment, "comment")?;
    t.wrap(K::SubstitutionDefinition, "substituted")?;
    t.leaf(K::Contents, "")?;
    let directive = t.open(K::Directive, 0)?;
    t.b.add_custom_attribute(directive, "name", "custom")?;
    t.para("directive")?;
    t.close(directive)
}

fn every_sphinx(t: &mut Tb<'_, '_>) -> R {
    let toctree = t.open(K::Toctree, 0)?;
    t.leaf(K::ToctreeEntry, "page")?;
    t.close(toctree)?;
    t.block(K::Only, "only", 0)?;
    let glossary = t.open(K::Glossary, 0)?;
    let dl = t.open(K::DefinitionList, 0)?;
    let item = t.open(K::DefinitionItem, 0)?;
    t.wrap(K::Term, "glossary term")?;
    t.block(K::Definition, "meaning", 0)?;
    t.close(item)?;
    t.close(dl)?;
    t.close(glossary)?;
    let version = t.open(K::VersionModified, 0)?;
    t.b.add_custom_attribute(version, "type", "versionadded")?;
    t.b.add_custom_attribute(version, "version", "1.0")?;
    t.para("added")?;
    t.close(version)?;
    every_description(t)?;
    let hlist = t.open(K::Hlist, 0)?;
    t.list()?;
    t.close(hlist)?;
    let productions = t.open(K::ProductionList, 0)?;
    t.wrap(K::Production, "rule")?;
    t.close(productions)
}

fn every_description(t: &mut Tb<'_, '_>) -> R {
    let desc = t.open(K::Desc, 0)?;
    t.leaf(K::Index, "")?;
    let signature = t.open(K::DescSignature, 0)?;
    t.wrap(K::DescAnnotation, "def ")?;
    t.wrap(K::DescAddname, "modname.")?;
    t.wrap(K::DescName, "f")?;
    let parameters = t.open(K::DescParameterlist, 0)?;
    t.wrap(K::DescParameter, "a")?;
    let optional = t.open(K::DescOptional, 0)?;
    t.wrap(K::DescParameter, "b")?;
    t.close(optional)?;
    t.close(parameters)?;
    t.wrap(K::DescReturns, "int")?;
    t.close(signature)?;
    t.block(K::DescContent, "described", 0)?;
    t.close(desc)
}

const KINDS_READER: Reader = Reader {
    id: "kinds",
    name: "Every node kind",
    version: "1",
    conformance: Conformance::Partial,
    read: read_every_kind,
    read_inline: None,
};

/// Text of the document that every renderer writes, one piece from each kind that holds
/// text a reader sees.
const RENDERED_TEXT: &[&str] = &[
    "title",
    "subtitle",
    "section",
    "text",
    "code",
    "alpha",
    "link",
    "quoted",
    "someone",
    "item",
    "term",
    "classifier",
    "definition",
    "name",
    "body",
    "--all",
    "option",
    "line",
    "preformatted",
    "doctest",
    "beta",
    "table",
    "cell",
    "caption",
    "legend",
    "note",
    "topic body",
    "sidebar body",
    "rubric",
    "container",
    "compound",
    "footnote",
    "citation",
    "directive",
    "only",
    "glossary term",
    "meaning",
    "added",
    "described",
    "modname",
    "int",
    "rule",
];

fn request(renderer: &str) -> Request<'_> {
    let mut policy = trusted_policy();
    policy.keep_documents_with_errors = true;
    Request {
        source: Source { name: "kinds.txt", text: "every node kind" },
        reader: "kinds",
        renderer,
        policy,
        ..Request::default()
    }
}

#[test]
fn the_document_holds_every_node_kind() {
    let mut ws = Workspace::new();
    let doc = read_document(&mut ws, &KINDS_READER, &request("html")).unwrap();
    let mut seen = [false; K::ALL.len()];
    for node in doc.nodes() {
        seen[node.kind as usize] = true;
    }
    let missing: Vec<K> = K::ALL.iter().copied().filter(|k| !seen[*k as usize]).collect();
    assert!(missing.is_empty(), "the test document lacks {missing:?}; add them above");
}

#[test]
fn every_renderer_writes_every_node_kind() {
    let defaults = guidedoc_defaults::registry();
    let registry = Registry { readers: vec![KINDS_READER], renderers: defaults.renderers };
    let mut ws = Workspace::new();
    for renderer in &registry.renderers {
        let result = convert(&mut ws, &registry, &request(renderer.id)).map(str::to_owned);
        let codes: Vec<&str> = ws.reports().items().iter().map(|d| d.code).collect();
        let out = result.unwrap_or_else(|f| panic!("{}: {f:?} {codes:?}", renderer.id));
        assert!(!codes.contains(&"core.render.kind"), "{}: {codes:?}", renderer.id);
        for marker in RENDERED_TEXT {
            assert!(out.contains(marker), "{} dropped {marker:?}:\n{out}", renderer.id);
        }
    }
}
