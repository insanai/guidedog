//! Extraction and translation of the node shapes the Sphinx layer writes (diagrams,
//! toctrees, index entries, meta values, info fields, cross-references), built with
//! guidedoc's Builder, since the Sphinx reader is not ported yet. A toy inline reader
//! parses translations: `@name` is a cross-reference to `name`, the rest is text.

use gd_gettext::parse_po;
use gd_i18n::{ExtractOptions, Target, Targets, Template, Translation, translate_document};
use guidedoc::{
    AttributeKey, Conformance, Fail, LinkInfo, LinkKind, NodeFlags, NodeId, NodeKind, ReadContext,
    ReadFn, Reader, SourceId, SourceSpan, Workspace, default_request, dump_to_string,
    read_document,
};

fn span(text: &str, part: &str) -> SourceSpan {
    let start = text.find(part).unwrap_or_else(|| panic!("{part:?} is not in the source"));
    SourceSpan::new(SourceId::MAIN, start as u32, (start + part.len()) as u32)
}

/// Reads `text` with a toy reader whose `read` builds the document.
fn read_toy<'w>(ws: &'w mut Workspace, read: ReadFn, text: &str) -> guidedoc::Document<'w> {
    let reader = Reader {
        id: "toy",
        name: "Toy",
        version: "1",
        conformance: Conformance::Partial,
        read,
        read_inline: Some(toy_inline),
    };
    let request = default_request("index.rst", text, "toy", "html");
    match read_document(ws, &reader, &request) {
        Ok(doc) => doc,
        Err(fail) => panic!("read failed: {}", fail.diagnostic.formatted_message()),
    }
}

/// Parses `@name` as a cross-reference (role `ref`, explicit title) and the rest as
/// text.
fn toy_inline(ctx: &mut ReadContext<'_>, at: SourceSpan) -> Result<(), Fail> {
    let source = ctx.source_of(at.source);
    let text = &source[at.start as usize..at.end as usize];
    let mut words = text.split('@');
    if let Some(first) = words.next().filter(|s| !s.is_empty()) {
        ctx.b.add_text_leaf(NodeKind::Text, at, first)?;
    }
    for piece in words {
        let name_len = piece.find([' ', '.']).unwrap_or(piece.len());
        let (name, rest) = piece.split_at(name_len);
        let x = ctx.b.begin_node(NodeKind::Xref, at)?;
        ctx.b.add_custom_attribute(x, "reftype", "ref")?;
        ctx.b.add_custom_attribute(x, "refexplicit", "1")?;
        let name_text = ctx.b.add_text(name)?;
        ctx.b.set_link(x, LinkInfo { name: name_text, ..LinkInfo::UNRESOLVED })?;
        ctx.b.add_text_leaf(NodeKind::Text, at, &name.to_uppercase())?;
        ctx.b.end_node(x)?;
        if !rest.is_empty() {
            ctx.b.add_text_leaf(NodeKind::Text, at, rest)?;
        }
    }
    Ok(())
}

fn extract(ws: &mut Workspace, read: ReadFn, text: &str, targets: Targets) -> Vec<String> {
    let doc = read_toy(ws, read, text);
    let mut pot = Template::new("x");
    pot.extract(doc, &ExtractOptions { targets, ..ExtractOptions::default() });
    pot.message_ids().map(str::to_owned).collect()
}

/// Reads with `read`, translates with `po`, and returns the tree and diagnostic codes.
fn translate(read: ReadFn, text: &str, po: &str, targets: Targets) -> (String, Vec<&'static str>) {
    let mut ws = Workspace::new();
    read_toy(&mut ws, read, text);
    let catalog = parse_po(po, "de.po").unwrap_or_else(|e| panic!("catalog: {e}"));
    let before = ws.reports().items().len();
    let translation = Translation {
        catalog: Some(&catalog),
        read_inline: Some(toy_inline),
        targets,
        ..Translation::default()
    };
    let (doc, _) = match translate_document(&mut ws, translation) {
        Ok(result) => result,
        Err(fail) => panic!("pass failed: {}", fail.diagnostic.formatted_message()),
    };
    let tree = dump_to_string(doc);
    (tree, ws.reports().items()[before..].iter().map(|d| d.code).collect())
}

#[track_caller]
fn has(haystack: &str, needle: &str) {
    assert!(haystack.contains(needle), "missing {needle:?} in:\n{haystack}");
}

#[track_caller]
fn lacks(haystack: &str, needle: &str) {
    assert!(!haystack.contains(needle), "unexpected {needle:?} in:\n{haystack}");
}

const GRAPHVIZ: &str = ".. graphviz::\n   :alt: A flow\n\n   digraph { a -> b; }\n";

/// A graphviz directive: a code block with the diagram's source and alt text.
fn read_graphviz(ctx: &mut ReadContext<'_>) -> Result<(), Fail> {
    let at = span(GRAPHVIZ, GRAPHVIZ);
    let block = ctx.b.begin_node(NodeKind::CodeBlock, at)?;
    ctx.b.add_custom_attribute(block, "graphviz", "")?;
    ctx.b.add_custom_attribute(block, "alt", "A flow")?;
    let code = ctx.b.add_text("digraph { a -> b; }\n")?;
    ctx.b.set_text(block, code);
    ctx.b.end_node(block)
}

#[test]
fn extracts_diagram_alt_without_its_source() {
    let mut ws = Workspace::new();
    let image = Targets::of(&[Target::Image]);
    assert_eq!(extract(&mut ws, read_graphviz, GRAPHVIZ, image), ["A flow"]);
    let both = Targets::of(&[Target::Image, Target::LiteralBlock]);
    assert_eq!(extract(&mut ws, read_graphviz, GRAPHVIZ, both), ["A flow", "digraph { a -> b; }"]);
    assert!(extract(&mut ws, read_graphviz, GRAPHVIZ, Targets::default()).is_empty());
}

#[test]
fn diagram_alt_and_literal_targets_are_independent() {
    let po = "msgid \"A flow\"\nmsgstr \"Ein Ablauf\"\n\n\
              msgid \"digraph { a -> b; }\"\nmsgstr \"digraph { x -> y; }\"\n";
    let (alt, _) = translate(read_graphviz, GRAPHVIZ, po, Targets::of(&[Target::Image]));
    has(&alt, "@alt=\"Ein Ablauf\"");
    has(&alt, "digraph { a -> b; }");
    lacks(&alt, "digraph { x -> y; }");
    let targets = Targets::of(&[Target::Image, Target::LiteralBlock]);
    let (both, _) = translate(read_graphviz, GRAPHVIZ, po, targets);
    has(&both, "@alt=\"Ein Ablauf\"");
    has(&both, "digraph { x -> y; }\\n");
    let (literal, _) =
        translate(read_graphviz, GRAPHVIZ, po, Targets::of(&[Target::LiteralBlock]));
    has(&literal, "@alt=\"A flow\"");
    has(&literal, "digraph { x -> y; }");
}

const ATTRIBUTES: &str = ".. toctree::\n   :caption: Contents\n\n   intro\n\
                          \x20  Other page <other>\n\n.. index:: single: Alpha; !Beta\n\n\
                          .. meta::\n   :description: About it\n\nText.\n";

/// A toctree with a caption and an explicit title, an index, a meta directive, and a
/// paragraph.
fn read_attributes(ctx: &mut ReadContext<'_>) -> Result<(), Fail> {
    let t = ATTRIBUTES;
    let toctree = ctx.b.begin_node(NodeKind::Toctree, span(t, ".. toctree::"))?;
    ctx.b.add_custom_attribute(toctree, "caption", "Contents")?;
    ctx.b.add_text_leaf(NodeKind::ToctreeEntry, span(t, "intro"), "intro")?;
    let entry = ctx.b.begin_node(NodeKind::ToctreeEntry, span(t, "Other page <other>"))?;
    ctx.b.add_attribute_string(entry, AttributeKey::Title, "Other page")?;
    ctx.b.end_node(entry)?;
    ctx.b.end_node(toctree)?;
    let index = ctx.b.begin_node(NodeKind::Index, span(t, ".. index::"))?;
    ctx.b.add_custom_attribute(index, "entries", "single: Alpha; !Beta")?;
    ctx.b.add_attribute_string(index, AttributeKey::Id, "index-0")?;
    ctx.b.end_node(index)?;
    let meta = ctx.b.begin_node(NodeKind::Directive, span(t, ".. meta::"))?;
    ctx.b.add_attribute_string(meta, AttributeKey::Name, "meta")?;
    ctx.b.add_custom_attribute(meta, "description", "About it")?;
    ctx.b.end_node(meta)?;
    let p = ctx.b.begin_node(NodeKind::Paragraph, span(t, "Text."))?;
    ctx.b.add_text_leaf(NodeKind::Text, span(t, "Text."), "Text.")?;
    ctx.b.end_node(p)
}

#[test]
fn extracts_attribute_messages() {
    let mut ws = Workspace::new();
    let got = extract(&mut ws, read_attributes, ATTRIBUTES, Targets::of(&[Target::Index]));
    assert_eq!(got, ["Contents", "Other page", "Alpha", "Beta", "About it", "Text."]);
    let plain = extract(&mut ws, read_attributes, ATTRIBUTES, Targets::default());
    assert_eq!(plain, ["Contents", "Other page", "About it", "Text."]);
}

#[test]
fn attributes_carry_messages() {
    let po = "msgid \"Contents\"\nmsgstr \"Inhalt\"\n\nmsgid \"Other page\"\n\
              msgstr \"Andere Seite\"\n\nmsgid \"Beta\"\nmsgstr \"Bäta\"\n\n\
              msgid \"About it\"\nmsgstr \"Darüber\"\n";
    let (tree, codes) = translate(read_attributes, ATTRIBUTES, po, Targets::of(&[Target::Index]));
    assert!(codes.is_empty(), "{codes:?}");
    has(&tree, "@caption=\"Inhalt\"");
    has(&tree, "@title=\"Andere Seite\"");
    has(&tree, "@entries=\"single: Alpha; !Bäta\"");
    has(&tree, "@description=\"Darüber\"");
    let (plain, _) = translate(read_attributes, ATTRIBUTES, po, Targets::default());
    has(&plain, "@entries=\"single: Alpha; !Beta\"");
}

const FIELD: &str = ".. py:function:: f(a)\n\n   :param int a: The @a.\n";

/// An info field's body: the argument and type a transform wrote (Generated), then
/// the written content.
fn read_field(ctx: &mut ReadContext<'_>) -> Result<(), Fail> {
    let t = FIELD;
    let p = ctx.b.begin_node(NodeKind::Paragraph, span(t, ":param int a: The @a."))?;
    let name = ctx.b.add_text_leaf(NodeKind::Strong, span(t, "a:"), "")?;
    ctx.b.add_flags(name, NodeFlags::GENERATED);
    let arg = ctx.b.add_text_leaf(NodeKind::Text, span(t, "int"), "a (int) – ")?;
    ctx.b.add_flags(arg, NodeFlags::GENERATED);
    ctx.b.add_text_leaf(NodeKind::Text, span(t, "The "), "The ")?;
    let x = ctx.b.begin_node(NodeKind::Xref, span(t, "@a"))?;
    ctx.b.add_custom_attribute(x, "reftype", "ref")?;
    ctx.b.add_custom_attribute(x, "py:module", "pkg")?;
    let name = ctx.b.add_text("a")?;
    ctx.b.set_link(x, LinkInfo { name, ..LinkInfo::UNRESOLVED })?;
    ctx.b.add_text_leaf(NodeKind::Text, span(t, "@a"), "a")?;
    ctx.b.end_node(x)?;
    ctx.b.add_text_leaf(NodeKind::Text, span(t, "a."), ".")?;
    ctx.b.end_node(p)
}

// As in Sphinx, an info field's message is its content; the argument and type the
// field puts before it stay, in front of the translation.
#[test]
fn info_fields_translate_their_content() {
    let mut ws = Workspace::new();
    assert_eq!(extract(&mut ws, read_field, FIELD, Targets::default()), ["The @a."]);
    let (tree, codes) = translate(
        read_field,
        FIELD,
        "msgid \"The @a.\"\nmsgstr \"Das @a.\"\n",
        Targets::default(),
    );
    assert!(codes.is_empty(), "{codes:?}");
    let generated = tree.find("\"a (int) – \"").unwrap_or(usize::MAX);
    let translated = tree.find("\"Das \"").unwrap_or(0);
    assert!(generated < translated, "{tree}");
    lacks(&tree, "\"The \"");
}

// A matched cross-reference keeps the context the original recorded (its module) and
// the translation's own explicit title.
#[test]
fn cross_references_keep_their_context() {
    let po = "msgid \"The @a.\"\nmsgstr \"Das @a hier.\"\n";
    let (tree, codes) = translate(read_field, FIELD, po, Targets::default());
    assert!(codes.is_empty(), "{codes:?}");
    has(&tree, "@py:module=\"pkg\"");
    has(&tree, "@refexplicit=\"1\"");
    has(&tree, "text \"A\"");
    let unknown = "msgid \"The @a.\"\nmsgstr \"Das @b.\"\n";
    let (tree, codes) = translate(read_field, FIELD, unknown, Targets::default());
    assert_eq!(codes, ["i18n.references.unknown"]);
    has(&tree, "\"The \"");
    let dropped = "msgid \"The @a.\"\nmsgstr \"Das.\"\n";
    let (tree, codes) = translate(read_field, FIELD, dropped, Targets::default());
    assert_eq!(codes, ["i18n.references.inconsistent"]);
    has(&tree, "\"Das.\"");
}

const LINKED: &str = "Intro\n=====\n\nSee @intro now.\n";

/// A section and a paragraph whose cross-reference resolves to it.
fn read_linked(ctx: &mut ReadContext<'_>) -> Result<(), Fail> {
    let t = LINKED;
    let section = ctx.b.begin_node(NodeKind::Section, span(t, t))?;
    ctx.b.set_value(section, 1);
    let h = ctx.b.begin_node(NodeKind::Heading, span(t, "Intro"))?;
    ctx.b.set_value(h, 1);
    ctx.b.add_text_leaf(NodeKind::Text, span(t, "Intro"), "Intro")?;
    ctx.b.end_node(h)?;
    let p = ctx.b.begin_node(NodeKind::Paragraph, span(t, "See @intro now."))?;
    ctx.b.add_text_leaf(NodeKind::Text, span(t, "See "), "See ")?;
    let x = ctx.b.begin_node(NodeKind::Xref, span(t, "@intro"))?;
    ctx.b.add_custom_attribute(x, "reftype", "ref")?;
    let name = ctx.b.add_text("intro")?;
    let link =
        LinkInfo { name, kind: LinkKind::Internal, resolved: section, ..LinkInfo::UNRESOLVED };
    ctx.b.set_link(x, link)?;
    ctx.b.add_text_leaf(NodeKind::Text, span(t, "intro"), "intro")?;
    ctx.b.end_node(x)?;
    ctx.b.add_text_leaf(NodeKind::Text, span(t, " now."), " now.")?;
    ctx.b.end_node(p)?;
    ctx.b.end_node(section)
}

#[test]
fn links_reach_the_copies_of_their_targets() {
    let po = "msgid \"Intro\"\nmsgstr \"Einleitung\"\n\nmsgid \"See @intro now.\"\n\
              msgstr \"Siehe @intro jetzt.\"\n";
    let mut ws = Workspace::new();
    read_toy(&mut ws, read_linked, LINKED);
    let catalog = parse_po(po, "de.po").unwrap_or_else(|e| panic!("catalog: {e}"));
    let translation = Translation {
        catalog: Some(&catalog),
        read_inline: Some(toy_inline),
        ..Translation::default()
    };
    let (doc, counts) =
        translate_document(&mut ws, translation).unwrap_or_else(|f| panic!("{f:?}"));
    assert_eq!(counts.translated, 2);
    let tree = dump_to_string(doc);
    has(&tree, "@id=\"intro\"");
    has(&tree, "\"Siehe \"");
    let xref = (0..doc.node_count())
        .map(NodeId::from_index)
        .find(|&id| doc.kind(id) == NodeKind::Xref)
        .unwrap_or_else(|| panic!("no xref in:\n{tree}"));
    let link = doc.link_info(xref).unwrap_or(LinkInfo::UNRESOLVED);
    assert_eq!(link.kind, LinkKind::Internal);
    assert_eq!(doc.kind(link.resolved), NodeKind::Section);
    let mut title = String::new();
    doc.plain_text(link.resolved, &mut title);
    has(&title, "Einleitung");
}
