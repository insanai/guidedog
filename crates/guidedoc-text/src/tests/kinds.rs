use guidedoc::{
    AdmonitionKind, AttributeKey as A, Builder, Fail, LinkInfo, LinkKind, ListInfo, ListStyle,
    NodeFlags, NodeKind as K, Status,
};

use super::basics::{LONG, heading};
use super::*;

fn external(b: &mut Builder<'_>, id: guidedoc::NodeId, dest: &str) -> Result<(), Fail> {
    let destination = b.add_text(dest)?;
    b.set_link(id, LinkInfo { destination, kind: LinkKind::External, ..LinkInfo::UNRESOLVED })?;
    Ok(())
}

fn frames(b: &mut Builder<'_>) -> Result<(), Fail> {
    open_v(b, K::Admonition, AdmonitionKind::Note as i32)?;
    para(b, &format!("Careful {LONG}"))?;
    shut(b)?;
    open_v(b, K::Admonition, AdmonitionKind::Warning as i32)?;
    para(b, "One.")?;
    para(b, "Two.")?;
    shut(b)?;
    open(b, K::Admonition)?;
    inline(b, K::Title, "Custom")?;
    para(b, "Generic.")?;
    shut(b)?;
    open(b, K::BlockQuote)?;
    para(b, "Quoted.")?;
    inline(b, K::Attribution, "Someone")?;
    shut(b)?;
    let code = leaf(b, K::CodeBlock, "def f():\n\n    return 1\n")?;
    b.add_custom_attribute(code, "caption", "Example")?;
    b.add_custom_attribute(code, "number-label", "Listing 1")?;
    leaf(b, K::MathBlock, "e = mc^2")?;
    let note = open_v(b, K::Footnote, 1)?;
    set_text(b, note, "1")?;
    para(b, "A note.")?;
    shut(b)?;
    let cite = open(b, K::Citation)?;
    set_text(b, cite, "CIT")?;
    para(b, "A book.")?;
    shut(b)?;
    frames_more(b)
}

fn frames_more(b: &mut Builder<'_>) -> Result<(), Fail> {
    inline(b, K::Rubric, "Aside")?;
    open(b, K::Topic)?;
    inline(b, K::Title, "Topic")?;
    para(b, "Body.")?;
    shut(b)?;
    open(b, K::LineBlock)?;
    inline(b, K::Line, "first")?;
    open(b, K::LineBlock)?;
    inline(b, K::Line, "nested")?;
    shut(b)?;
    shut(b)?;
    open(b, K::Figure)?;
    custom(b, "number-label", "Fig. 2")?;
    let img = open(b, K::Image)?;
    attr(b, A::Alt, "a cat")?;
    external(b, img, "cat.png")?;
    shut(b)?;
    inline(b, K::Caption, "The cat.")?;
    shut(b)?;
    leaf(b, K::Comment, "hidden").map(|_| ())
}

#[test]
fn frames_and_literal_blocks() {
    let r = render(frames, frag());
    assert_eq!(r.status, Status::Ok);
    expect_text(
        &r,
        "Note: Careful Guidedoc converts documents without calling an\n\
         \x20     allocator, so embedders hand it memory once and it never asks\n\
         \x20     for more.\n\nWarning:\n\n   One.\n\n   Two.\n\nCustom: Generic.\n\n\
         \x20  Quoted.\n\n\
         \x20  -- Someone\n\nListing 1 Example\n\n   def f():\n\n       return 1\n\n\
         \x20  e = mc^2\n\n[1] A note.\n\n[CIT] A book.\n\n-[ Aside ]-\n\nTopic\n\nBody.\n\n\
         first\n   nested\n\n[image: a cat]\n\nFig. 2 The cat.\n",
    );
}

fn tables(b: &mut Builder<'_>) -> Result<(), Fail> {
    open_v(b, K::Table, 3)?;
    custom(b, "number-label", "Table 1")?;
    inline(b, K::Title, "Sizes")?;
    open(b, K::TableHead)?;
    open(b, K::Row)?;
    cell(b, "Name", 1, 2)?;
    cell(b, "Size", 1, 1)?;
    shut(b)?;
    shut(b)?;
    open(b, K::TableBody)?;
    open(b, K::Row)?;
    cell(b, "tall", 2, 1)?;
    cell(b, "b", 1, 1)?;
    cell(b, "1", 1, 1)?;
    shut(b)?;
    open(b, K::Row)?;
    cell(b, "c", 1, 1)?;
    cell(b, "", 1, 1)?;
    shut(b)?;
    shut(b)?;
    shut(b)
}

#[test]
fn tables_are_drawn_with_spans() {
    let r = render(tables, frag());
    assert_eq!(r.status, Status::Ok);
    expect_text(
        &r,
        "Table 1 Sizes\n\n+------+---+------+\n| Name     | Size |\n\
         +======+===+======+\n| tall | b | 1    |\n+      +---+------+\n|      | c |      |\n\
         +------+---+------+\n",
    );
    let wide = render(
        |b| {
            open_v(b, K::Table, 2)?;
            open(b, K::TableBody)?;
            open(b, K::Row)?;
            cell(b, LONG, 1, 1)?;
            cell(b, "short", 1, 1)?;
            shut(b)?;
            shut(b)?;
            shut(b)
        },
        frag(),
    );
    let dashes = "-".repeat(60);
    expect_text(
        &wide,
        &format!(
            "+{dashes}+-------+\n\
             | Guidedoc converts documents without calling an allocator,  | short |\n\
             | so embedders hand it memory once and it never asks for     |       |\n\
             | more.                                                      |       |\n\
             +{dashes}+-------+\n"
        ),
    );
}

fn inlines(b: &mut Builder<'_>) -> Result<(), Fail> {
    let def = open(b, K::SubstitutionDefinition)?;
    set_text(b, def, "name")?;
    text(b, "Guidedoc")?;
    shut(b)?;
    open(b, K::Paragraph)?;
    inline(b, K::Emphasis, "em")?;
    text(b, " ")?;
    inline(b, K::Strong, "strong")?;
    text(b, " ")?;
    leaf(b, K::Code, "x = 1")?;
    text(b, " H")?;
    inline(b, K::Subscript, "2")?;
    text(b, "O x")?;
    inline(b, K::Superscript, "2")?;
    text(b, " ")?;
    let reference = open_v(b, K::FootnoteReference, 1)?;
    set_text(b, reference, "1")?;
    shut(b)?;
    text(b, " ")?;
    open(b, K::Abbreviation)?;
    attr(b, A::Title, "HyperText Markup Language")?;
    text(b, "HTML")?;
    shut(b)?;
    text(b, " ")?;
    let sub = open(b, K::SubstitutionReference)?;
    set_text(b, sub, "name")?;
    set_link(b, sub, LinkKind::Internal, def)?;
    shut(b)?;
    text(b, " ")?;
    let link = open(b, K::Link)?;
    external(b, link, "https://x.org")?;
    text(b, "site")?;
    shut(b)?;
    for name in ["kbd", "command", "dfn", "file", "guilabel", "other"] {
        text(b, " ")?;
        let r = open(b, K::Role)?;
        set_text(b, r, name)?;
        text(b, name)?;
        shut(b)?;
    }
    text(b, " ")?;
    inline(b, K::TitleReference, "Dune")?;
    shut(b)
}

#[test]
fn inline_markup_follows_sphinx() {
    let r = render(inlines, frag());
    assert_eq!(r.status, Status::Ok);
    expect_text(
        &r,
        "*em* **strong** \"x = 1\" H_2O x^2 [1] HTML (HyperText Markup Language)\n\
         Guidedoc site kbd **command** *dfn* \"file\" guilabel other *Dune*\n",
    );
}

fn sphinx(b: &mut Builder<'_>) -> Result<(), Fail> {
    open(b, K::Toctree)?;
    leaf(b, K::ToctreeEntry, "intro")?;
    shut(b)?;
    open(b, K::Only)?;
    para(b, "Only text.")?;
    shut(b)?;
    let index = open(b, K::Index)?;
    b.add_attribute_string(index, A::Id, "index-0")?;
    shut(b)?;
    open(b, K::VersionModified)?;
    custom(b, "type", "versionchanged")?;
    custom(b, "version", "2.0")?;
    para(b, "Now faster.")?;
    shut(b)?;
    open(b, K::VersionModified)?;
    custom(b, "type", "versionremoved")?;
    custom(b, "version", "3.0")?;
    shut(b)?;
    open(b, K::Desc)?;
    custom(b, "domain", "py")?;
    open(b, K::DescSignature)?;
    inline(b, K::DescAddname, "mod.")?;
    inline(b, K::DescName, "func")?;
    open(b, K::DescParameterlist)?;
    inline(b, K::DescParameter, "a")?;
    open(b, K::DescOptional)?;
    inline(b, K::DescParameter, "b=1")?;
    shut(b)?;
    shut(b)?;
    inline(b, K::DescReturns, "int")?;
    shut(b)?;
    open(b, K::DescContent)?;
    para(b, "Does things.")?;
    shut(b)?;
    shut(b)?;
    sphinx_more(b)
}

fn sphinx_more(b: &mut Builder<'_>) -> Result<(), Fail> {
    open(b, K::Glossary)?;
    open(b, K::DefinitionList)?;
    open(b, K::DefinitionItem)?;
    inline(b, K::Term, "API")?;
    open(b, K::Definition)?;
    para(b, "Interface.")?;
    shut(b)?;
    shut(b)?;
    shut(b)?;
    shut(b)?;
    open_v(b, K::Hlist, 2)?;
    let bullets = ListInfo { style: ListStyle::Bullet, delimiter: b'*', start: 1 };
    list(b, bullets, true, &["a", "b", "c"])?;
    shut(b)?;
    open(b, K::ProductionList)?;
    let p = open(b, K::Production)?;
    set_text(b, p, "expr")?;
    text(b, "term \"+\" term")?;
    shut(b)?;
    inline(b, K::Production, "| letter")?;
    shut(b)?;
    open(b, K::Directive)?;
    attr(b, A::Name, "codeauthor")?;
    custom(b, "author", "Ann")?;
    custom(b, "show", "1")?;
    shut(b)?;
    open(b, K::Paragraph)?;
    let x = open(b, K::Xref)?;
    let name = b.add_text("t")?;
    b.set_link(x, LinkInfo { name, ..LinkInfo::UNRESOLVED })?;
    text(b, "see this")?;
    shut(b)?;
    shut(b)?;
    open_v(b, K::Admonition, AdmonitionKind::Todo as i32)?;
    para(b, "Later.")?;
    shut(b)
}

#[test]
fn sphinx_kinds_render_as_text() {
    let r = render(sphinx, frag());
    assert_eq!(r.status, Status::Ok);
    expect_text(
        &r,
        "Only text.\n\nChanged in version 2.0: Now faster.\n\nRemoved in version 3.0.\n\n\
         mod.func(a[, b=1]) -> int\n\n   Does things.\n\nAPI\n   Interface.\n\n* a\n* b\n* c\n\n\
         \x20  expr ::= term \"+\" term\n            | letter\n\nCode author: Ann\n\nsee this\n\n\
         Todo: Later.\n",
    );
}

fn contents(b: &mut Builder<'_>) -> Result<(), Fail> {
    open(b, K::Directive)?;
    attr(b, A::Name, "sectnum")?;
    shut(b)?;
    open(b, K::Contents)?;
    shut(b)?;
    open_v(b, K::Section, 1)?;
    heading(b, 1, "Alpha")?;
    open_v(b, K::Section, 2)?;
    heading(b, 2, "Beta")?;
    shut(b)?;
    shut(b)
}

#[test]
fn contents_and_section_numbers() {
    let r = render(contents, frag());
    assert_eq!(r.status, Status::Ok);
    expect_text(
        &r,
        "Contents\n\n* 1 Alpha\n  * 1.1 Beta\n\n1 Alpha\n*******\n\n1.1 Beta\n========\n",
    );
}

fn everything(b: &mut Builder<'_>) -> Result<(), Fail> {
    frames(b)?;
    tables(b)?;
    inlines(b)?;
    sphinx(b)?;
    contents(b)
}

#[test]
fn rendering_is_deterministic() {
    let a = render(everything, frag());
    let b = render(everything, frag());
    assert_eq!(a.status, Status::Ok);
    assert_eq!(a.text, b.text);
    // The standalone option changes nothing in plain text.
    let standalone = render(everything, RenderOptions::default());
    assert_eq!(standalone.text, a.text);
}

fn remaining(b: &mut Builder<'_>) -> Result<(), Fail> {
    inline(b, K::Title, "Doc")?;
    inline(b, K::Subtitle, "Sub")?;
    open(b, K::Preformatted)?;
    text(b, "keep  ")?;
    inline(b, K::Emphasis, "this")?;
    shut(b)?;
    leaf(b, K::DoctestBlock, ">>> 1 + 1\n2\n")?;
    open(b, K::Sidebar)?;
    inline(b, K::Title, "Side")?;
    inline(b, K::Subtitle, "note")?;
    para(b, "Aside.")?;
    shut(b)?;
    open(b, K::Container)?;
    open(b, K::Compound)?;
    para(b, "Inside.")?;
    shut(b)?;
    shut(b)?;
    let target = open(b, K::Target)?;
    set_text(b, target, "python")?;
    external(b, target, "https://python.org")?;
    shut(b)?;
    remaining_inlines(b)?;
    let math = leaf(b, K::MathBlock, "x = 1")?;
    b.add_custom_attribute(math, "number", "2")?;
    for show in [false, true] {
        open(b, K::Directive)?;
        attr(b, A::Name, "sectionauthor")?;
        custom(b, "author", "Bo")?;
        if show {
            custom(b, "show", "1")?;
        }
        shut(b)?;
    }
    open(b, K::Directive)?;
    attr(b, A::Name, "target-notes")?;
    shut(b)?;
    open(b, K::Directive)?;
    attr(b, A::Name, "custom")?;
    inline(b, K::Title, "Heading")?;
    para(b, "Body.")?;
    shut(b)
}

fn remaining_inlines(b: &mut Builder<'_>) -> Result<(), Fail> {
    open(b, K::Paragraph)?;
    let spot = open(b, K::InlineTarget)?;
    set_text(b, spot, "spot")?;
    text(b, "spot")?;
    shut(b)?;
    text(b, " ")?;
    leaf(b, K::CitationReference, "CIT")?;
    text(b, " ")?;
    let star = open_v(b, K::FootnoteReference, 2)?;
    set_text(b, star, "*")?;
    b.add_flags(star, NodeFlags::SYMBOL);
    shut(b)?;
    text(b, " ")?;
    leaf(b, K::Math, "a^2")?;
    text(b, " ")?;
    let img = open(b, K::Image)?;
    attr(b, A::Alt, "icon")?;
    external(b, img, "i.png")?;
    shut(b)?;
    shut(b)
}

#[test]
fn remaining_kinds_render_as_text() {
    let r = render(remaining, frag());
    assert_eq!(r.status, Status::Ok);
    expect_text(
        &r,
        "Doc\n***\n\nSub\n===\n\n   keep  *this*\n\n   >>> 1 + 1\n   2\n\n\
         Side -- note\n\nAside.\n\nInside.\n\nspot [CIT] [†] a^2 [image: icon]\n\n   x = 1\n\
         \x20  (2)\n\nSection author: Bo\n\n* python: https://python.org\n\n-[ Heading ]-\n\n\
         Body.\n",
    );
}
