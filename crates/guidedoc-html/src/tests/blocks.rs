use guidedoc::{
    AdmonitionKind, Align, AttributeKey as A, Builder, CellInfo, Fail, LinkInfo, LinkKind,
    ListInfo, ListStyle, NodeFlags, NodeKind as K, Status,
};

use super::*;

fn titled(b: &mut Builder<'_>) -> Result<(), Fail> {
    inline(b, K::Title, "My <Doc>")?;
    open(b, K::Directive)?;
    attr(b, A::Name, "meta")?;
    custom(b, "description", "A guide")?;
    shut(b)?;
    para(b, "Body")
}

#[test]
fn standalone_documents_are_complete() {
    let options = RenderOptions { language: "de", ..RenderOptions::default() };
    let r = render(titled, options);
    assert_eq!(r.status, Status::Ok);
    assert!(r.html.starts_with("<!DOCTYPE html>\n<html lang=\"de\">\n"));
    has(&r, "<meta charset=\"utf-8\" />");
    has(&r, "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />");
    has(&r, "<title>My &lt;Doc&gt;</title>");
    has(&r, "<meta name=\"description\" content=\"A guide\" />");
    has(&r, "prefers-color-scheme: dark");
    has(
        &r,
        "<body>\n<main>\n<h1 id=\"my-doc\" class=\"title\">My &lt;Doc&gt;</h1>\n<p>Body</p>\n",
    );
    assert!(r.html.ends_with("</main>\n</body>\n</html>\n"));
    let options =
        RenderOptions { title: "Override", stylesheet: "body{}", ..RenderOptions::default() };
    let custom_style = render(titled, options);
    has(&custom_style, "<title>Override</title>");
    has(&custom_style, "<style>\nbody{}\n</style>");
    lacks(&custom_style, "prefers-color-scheme");
    let heading = render(
        |b| {
            open_v(b, K::Heading, 2)?;
            text(b, "First")?;
            shut(b)
        },
        RenderOptions::default(),
    );
    has(&heading, "<html lang=\"en\">");
    has(&heading, "<title>First</title>");
    let directive = render(
        |b| {
            open(b, K::Directive)?;
            attr(b, A::Name, "title")?;
            attr(b, A::Title, "From directive")?;
            shut(b)
        },
        RenderOptions::default(),
    );
    has(&directive, "<title>From directive</title>");
    let empty = render(|b| para(b, "x"), RenderOptions::default());
    has(&empty, "<title>Document</title>");
    let fragment = render(titled, frag());
    lacks(&fragment, "<html");
    lacks(&fragment, "<meta");
}

fn sections(b: &mut Builder<'_>) -> Result<(), Fail> {
    open_v(b, K::Section, 1)?;
    open_v(b, K::Heading, 1)?;
    text(b, "Intro")?;
    shut(b)?;
    para(b, "a")?;
    open_v(b, K::Section, 2)?;
    open_v(b, K::Heading, 2)?;
    text(b, "Intro")?;
    shut(b)?;
    shut(b)?;
    shut(b)?;
    open_v(b, K::Heading, 3)?;
    text(b, "Hello, World")?;
    shut(b)
}

#[test]
fn sections_and_headings_get_anchors() {
    let r = render(sections, frag());
    assert_eq!(
        r.html,
        "<section id=\"intro\">\n<h1>Intro</h1>\n<p>a</p>\n<section id=\"id1\">\n\
         <h2>Intro</h2>\n</section>\n</section>\n<h3 id=\"hello-world\">Hello, World</h3>\n"
    );
    let reference = render(sections, RenderOptions { reference_profile: true, ..frag() });
    lacks(&reference, "id=");
    has(&reference, "<h3>Hello, World</h3>");
    let prefixed = render(sections, RenderOptions { anchor_prefix: "ch1-", ..frag() });
    has(&prefixed, "<section id=\"ch1-intro\">");
    let titled = render(
        |b| {
            inline(b, K::Title, "Doc")?;
            open_v(b, K::Section, 1)?;
            open_v(b, K::Heading, 1)?;
            text(b, "Part")?;
            shut(b)?;
            shut(b)?;
            open_v(b, K::Heading, 9)?; // Heading levels past six clamp to h6.
            text(b, "Deep")?;
            shut(b)
        },
        frag(),
    );
    has(&titled, "<h2>Part</h2>");
    has(&titled, "<h6 id=\"deep\">Deep</h6>");
}

fn note_reference(b: &mut Builder<'_>, kind: K, label: &str, symbol: bool) -> Result<u32, Fail> {
    let id = open_v(b, kind, i32::from(kind == K::FootnoteReference))?;
    set_text(b, id, label)?;
    if symbol {
        b.add_flags(id, NodeFlags::SYMBOL);
    }
    let row = b.set_link(id, LinkInfo::UNRESOLVED)?;
    shut(b)?;
    Ok(row)
}

fn notes(b: &mut Builder<'_>) -> Result<(), Fail> {
    open(b, K::Paragraph)?;
    text(b, "See")?;
    let row1 = note_reference(b, K::FootnoteReference, "1", false)?;
    let row2 = note_reference(b, K::FootnoteReference, "1", false)?;
    let row3 = note_reference(b, K::CitationReference, "CIT2002", false)?;
    let row4 = note_reference(b, K::FootnoteReference, "*", true)?;
    shut(b)?;
    let note = open_v(b, K::Footnote, 1)?;
    set_text(b, note, "1")?;
    para(b, "Note text.")?;
    shut(b)?;
    let citation = open(b, K::Citation)?;
    set_text(b, citation, "CIT2002")?;
    para(b, "Book.")?;
    shut(b)?;
    let symbol = open_v(b, K::Footnote, 1)?;
    set_text(b, symbol, "*")?;
    b.add_flags(symbol, NodeFlags::SYMBOL);
    para(b, "Starred.")?;
    shut(b)?;
    let keep = guidedoc::Text::default();
    b.resolve_link(row1, note, LinkKind::Internal, keep);
    b.resolve_link(row2, note, LinkKind::Internal, keep);
    b.resolve_link(row3, citation, LinkKind::Internal, keep);
    b.resolve_link(row4, symbol, LinkKind::Internal, keep);
    Ok(())
}

#[test]
fn footnotes_and_citations_link_both_ways() {
    let r = render(notes, frag());
    assert_eq!(r.status, Status::Ok);
    has(&r, "<a class=\"footnote-reference\" id=\"id1\" href=\"#id5\">[1]</a>");
    has(&r, "<a class=\"footnote-reference\" id=\"id2\" href=\"#id5\">[1]</a>");
    has(&r, "<a class=\"citation-reference\" id=\"id3\" href=\"#cit2002\">[CIT2002]</a>");
    has(&r, "<a class=\"footnote-reference\" id=\"id4\" href=\"#id6\">[*]</a>");
    has(
        &r,
        "<aside id=\"id6\" class=\"footnote\">\n<p class=\"label\">\
         <span class=\"label-text\">[*]</span>",
    );
    has(
        &r,
        "<aside id=\"id5\" class=\"footnote\">\n<p class=\"label\">\
         <span class=\"label-text\">[1]</span> <a class=\"backref\" href=\"#id1\">↩1\
         </a> <a class=\"backref\" href=\"#id2\">↩2</a></p>\n<p>Note text.</p>\n</aside>\n",
    );
    has(
        &r,
        "<aside id=\"cit2002\" class=\"citation\">\n<p class=\"label\"><span \
         class=\"label-text\">[CIT2002]</span> <a class=\"backref\" href=\"#id3\">↩</a>",
    );
}

fn cell(b: &mut Builder<'_>, info: CellInfo, paragraphs: &[&str]) -> Result<(), Fail> {
    let c = open(b, K::Cell)?;
    b.set_cell(c, info)?;
    for p in paragraphs {
        para(b, p)?;
    }
    shut(b)
}

#[test]
fn tables_keep_spans_and_widths() {
    let r = render(
        |b| {
            open_v(b, K::Table, 3)?;
            attr(b, A::Widths, "2 1 1")?;
            inline(b, K::Title, "Caption")?;
            open(b, K::TableHead)?;
            open(b, K::Row)?;
            cell(b, CellInfo { col_span: 2, ..CellInfo::default() }, &["A"])?;
            cell(b, CellInfo::default(), &["B"])?;
            shut(b)?;
            shut(b)?;
            open(b, K::TableBody)?;
            open(b, K::Row)?;
            let tall = CellInfo { row_span: 2, col_span: 1, align: Align::Right };
            cell(b, tall, &["x", "y"])?;
            cell(b, CellInfo::default(), &[])?;
            shut(b)?;
            shut(b)?;
            shut(b)
        },
        frag(),
    );
    assert_eq!(r.status, Status::Ok);
    assert_eq!(
        r.html,
        "<div class=\"table-wrapper\">\n<table>\n<caption>Caption</caption>\n<colgroup>\
         <col style=\"width: 50%\" /><col style=\"width: 25%\" /><col style=\"width: 25%\" />\
         </colgroup>\n<thead>\n<tr>\n<th colspan=\"2\">A</th>\n<th>B</th>\n</tr>\n</thead>\n\
         <tbody>\n<tr>\n<td rowspan=\"2\" style=\"text-align: right\">\n<p>x</p>\n<p>y</p>\n\
         </td>\n<td></td>\n</tr>\n</tbody>\n</table>\n</div>\n"
    );
}

#[test]
fn admonitions_have_titles_and_classes() {
    let r = render(
        |b| {
            open_v(b, K::Admonition, AdmonitionKind::Note as i32)?;
            attr(b, A::Class, "extra wide")?;
            para(b, "Careful.")?;
            shut(b)?;
            open_v(b, K::Admonition, AdmonitionKind::Seealso as i32)?;
            para(b, "Other.")?;
            shut(b)?;
            open(b, K::Admonition)?;
            inline(b, K::Title, "Custom")?;
            para(b, "Generic.")?;
            shut(b)
        },
        frag(),
    );
    assert_eq!(
        r.html,
        "<aside class=\"admonition note extra wide\">\n<p class=\"admonition-title\">Note</p>\n\
         <p>Careful.</p>\n</aside>\n<aside class=\"admonition seealso\">\n\
         <p class=\"admonition-title\">See also</p>\n<p>Other.</p>\n</aside>\n\
         <aside class=\"admonition\">\n<p class=\"admonition-title\">Custom</p>\n\
         <p>Generic.</p>\n</aside>\n"
    );
}

fn contents_tree(b: &mut Builder<'_>, depth: i32, local: bool, numbers: bool) -> Result<(), Fail> {
    if numbers {
        open(b, K::Directive)?;
        attr(b, A::Name, "sectnum")?;
        custom(b, "start", "3")?;
        custom(b, "prefix", "Part ")?;
        shut(b)?;
    }
    if !local {
        open_v(b, K::Contents, depth)?;
        shut(b)?;
    }
    open_v(b, K::Section, 1)?;
    open_v(b, K::Heading, 1)?;
    text(b, "Alpha")?;
    shut(b)?;
    if local {
        let c = open_v(b, K::Contents, depth)?;
        b.add_flags(c, NodeFlags::LOCAL);
        attr(b, A::Title, "")?;
        shut(b)?;
    }
    open_v(b, K::Section, 2)?;
    open_v(b, K::Heading, 2)?;
    text(b, "Beta")?;
    shut(b)?;
    shut(b)?;
    shut(b)?;
    open_v(b, K::Section, 1)?;
    open_v(b, K::Heading, 1)?;
    text(b, "Gamma")?;
    shut(b)?;
    shut(b)
}

#[test]
fn contents_list_sections() {
    let full = render(|b| contents_tree(b, 0, false, false), frag());
    has(
        &full,
        "<nav id=\"contents\" class=\"contents\">\n<p class=\"contents-title\">Contents</p>\n\
         <ul>\n<li><a id=\"id1\" href=\"#alpha\">Alpha</a>\n<ul>\n\
         <li><a id=\"id2\" href=\"#beta\">Beta</a></li>\n</ul>\n</li>\n\
         <li><a id=\"id3\" href=\"#gamma\">Gamma</a></li>\n</ul>\n</nav>\n",
    );
    let shallow = render(|b| contents_tree(b, 1, false, false), frag());
    lacks(&shallow, "#beta");
    has(&shallow, "#gamma");
    let local = render(|b| contents_tree(b, 0, true, false), frag());
    has(
        &local,
        "<nav id=\"id1\" class=\"contents\">\n<ul>\n\
         <li><a id=\"id2\" href=\"#beta\">Beta</a></li>\n</ul>\n</nav>\n",
    );
    let numbered = render(|b| contents_tree(b, 0, false, true), frag());
    // Titles link back to their entries, around their numbers (Docutils' toc-backref).
    let back = "\" role=\"doc-backlink\"><span class=\"section-number\">Part ";
    has(
        &numbered,
        &format!("<h1><a class=\"toc-backref\" href=\"#id1{back}3</span> Alpha</a></h1>"),
    );
    has(
        &numbered,
        &format!("<h2><a class=\"toc-backref\" href=\"#id2{back}3.1</span> Beta</a></h2>"),
    );
    has(
        &numbered,
        &format!("<h1><a class=\"toc-backref\" href=\"#id3{back}4</span> Gamma</a></h1>"),
    );
    // A local table lists only the sections it holds, whose titles alone link back.
    has(&local, "<h1>Alpha</h1>");
    has(&local, "<h2><a class=\"toc-backref\" href=\"#id2\" role=\"doc-backlink\">Beta</a>");
    has(
        &numbered,
        "<a id=\"id2\" href=\"#beta\"><span class=\"section-number\">Part 3.1</span> Beta",
    );
    let headings = render(
        |b| {
            open(b, K::Contents)?;
            shut(b)?;
            open_v(b, K::Heading, 2)?;
            text(b, "One")?;
            shut(b)?;
            open_v(b, K::Heading, 3)?;
            text(b, "Two")?;
            shut(b)
        },
        frag(),
    );
    has(&headings, "<li><a href=\"#one\">One</a>\n<ul>\n<li><a href=\"#two\">Two</a></li>");
}

fn rst_blocks(b: &mut Builder<'_>) -> Result<(), Fail> {
    open(b, K::LineBlock)?;
    inline(b, K::Line, "one")?;
    open(b, K::Line)?;
    shut(b)?;
    shut(b)?;
    open(b, K::DefinitionList)?;
    open(b, K::DefinitionItem)?;
    inline(b, K::Term, "term")?;
    inline(b, K::Classifier, "type")?;
    open(b, K::Definition)?;
    para(b, "meaning")?;
    shut(b)?;
    shut(b)?;
    shut(b)?;
    open(b, K::FieldList)?;
    open(b, K::Field)?;
    inline(b, K::FieldName, "Author")?;
    open(b, K::FieldBody)?;
    para(b, "Me")?;
    shut(b)?;
    shut(b)?;
    shut(b)?;
    options(b)
}

fn options(b: &mut Builder<'_>) -> Result<(), Fail> {
    open(b, K::OptionList)?;
    open(b, K::OptionItem)?;
    open(b, K::OptionGroup)?;
    leaf(b, K::Option, "-a")?;
    let opt = open(b, K::Option)?;
    set_text(b, opt, "--all")?;
    let arg = leaf(b, K::OptionArgument, "N")?;
    b.set_value(arg, i32::from(b'='));
    shut(b)?;
    shut(b)?;
    open(b, K::OptionDescription)?;
    para(b, "Everything.")?;
    shut(b)?;
    shut(b)?;
    shut(b)
}

#[test]
fn rst_blocks_render_semantically() {
    let r = render(rst_blocks, frag());
    assert_eq!(
        r.html,
        "<div class=\"line-block\">\n<div class=\"line\">one</div>\n<div class=\"line\"><br />\
         </div>\n</div>\n<dl>\n<dt>term <span class=\"classifier-delimiter\">:</span> \
         <span class=\"classifier\">type</span></dt>\n<dd>\n<p>meaning</p>\n</dd>\n</dl>\n\
         <dl class=\"field-list\">\n<dt>Author</dt>\n<dd>\n<p>Me</p>\n</dd>\n</dl>\n\
         <dl class=\"option-list\">\n<dt><kbd class=\"option\">-a</kbd>, \
         <kbd class=\"option\">--all=<var>N</var></kbd></dt>\n<dd>\n<p>Everything.</p>\n\
         </dd>\n</dl>\n"
    );
}

fn rst_containers(b: &mut Builder<'_>) -> Result<(), Fail> {
    leaf(b, K::DoctestBlock, ">>> 1 < 2\nTrue")?;
    open(b, K::Preformatted)?;
    text(b, "keep  ")?;
    inline(b, K::Emphasis, "this")?;
    shut(b)?;
    open(b, K::BlockQuote)?;
    attr(b, A::Class, "epigraph")?;
    para(b, "Quote.")?;
    inline(b, K::Attribution, "Someone")?;
    shut(b)?;
    inline(b, K::Rubric, "Aside")?;
    open(b, K::Topic)?;
    inline(b, K::Title, "Topic")?;
    para(b, "t")?;
    shut(b)?;
    open(b, K::Sidebar)?;
    inline(b, K::Title, "Side")?;
    inline(b, K::Subtitle, "Sub")?;
    shut(b)?;
    open(b, K::Container)?;
    attr(b, A::Class, "custom")?;
    para(b, "c")?;
    shut(b)?;
    open(b, K::Compound)?;
    para(b, "d")?;
    shut(b)?;
    leaf(b, K::MathBlock, "x^2")?;
    leaf(b, K::Comment, "hidden")?;
    leaf(b, K::Target, "hidden")?;
    leaf(b, K::ThematicBreak, "").map(|_| ())
}

#[test]
fn rst_containers_render_semantically() {
    let r = render(rst_containers, frag());
    assert_eq!(
        r.html,
        "<pre class=\"doctest\">&gt;&gt;&gt; 1 &lt; 2\nTrue</pre>\n\
         <pre class=\"parsed-literal\">keep  <em>this</em></pre>\n\
         <blockquote class=\"epigraph\">\n<p>Quote.</p>\n<p class=\"attribution\">— \
         Someone</p>\n</blockquote>\n<p class=\"rubric\">Aside</p>\n<aside class=\"topic\">\n\
         <p class=\"topic-title\">Topic</p>\n<p>t</p>\n</aside>\n<aside class=\"sidebar\">\n\
         <p class=\"sidebar-title\">Side</p>\n<p class=\"sidebar-subtitle\">Sub</p>\n</aside>\n\
         <div class=\"container custom\">\n<p>c</p>\n</div>\n<div class=\"compound\">\n\
         <p>d</p>\n</div>\n<div class=\"math\">\\[x^2\\]</div>\n<hr />\n"
    );
}

fn directive(b: &mut Builder<'_>, name: &str, body: &str) -> Result<(), Fail> {
    open(b, K::Directive)?;
    attr(b, A::Name, name)?;
    if !body.is_empty() {
        para(b, body)?;
    }
    shut(b)
}

fn figures_and_code(b: &mut Builder<'_>) -> Result<(), Fail> {
    directive(b, "header", "Top")?;
    open(b, K::Figure)?;
    link(b, K::Image, "fig.png", LinkKind::External)?;
    attr(b, A::Alt, "A \"figure\"")?;
    attr(b, A::Width, "200")?;
    attr(b, A::Height, "5em")?;
    shut(b)?;
    inline(b, K::Caption, "Cap")?;
    open(b, K::Legend)?;
    para(b, "Leg")?;
    shut(b)?;
    shut(b)?;
    let code = leaf(b, K::CodeBlock, "print(1 < 2)\n")?;
    b.set_value(code, 5);
    b.add_attribute_string(code, A::Language, "python")?;
    let l = open(b, K::List)?;
    b.set_list(l, ListInfo { style: ListStyle::LowerAlpha, delimiter: b'.', start: 3 })?;
    open(b, K::ListItem)?;
    para(b, "c")?;
    shut(b)?;
    shut(b)
}

fn target_notes(b: &mut Builder<'_>) -> Result<(), Fail> {
    open(b, K::Paragraph)?;
    let reference = open(b, K::Reference)?;
    let destination = b.add_text("https://example.org/a b")?;
    let name = b.add_text("site")?;
    b.set_link(
        reference,
        LinkInfo { destination, name, kind: LinkKind::External, ..LinkInfo::UNRESOLVED },
    )?;
    text(b, "site")?;
    shut(b)?;
    shut(b)?;
    let target = open(b, K::Target)?;
    set_text(b, target, "site")?;
    let destination = b.add_text("https://example.org/a b")?;
    b.set_link(
        target,
        LinkInfo { destination, kind: LinkKind::External, ..LinkInfo::UNRESOLVED },
    )?;
    shut(b)?;
    directive(b, "target-notes", "")?;
    directive(b, "custom-thing", "inside")?;
    directive(b, "footer", "Bottom")
}

#[test]
fn figures_code_and_directives() {
    let r = render(
        |b| {
            figures_and_code(b)?;
            target_notes(b)
        },
        frag(),
    );
    assert_eq!(r.status, Status::Ok);
    assert_eq!(
        r.html,
        "<header>\n<p>Top</p>\n</header>\n<figure>\n\
         <img src=\"fig.png\" alt=\"A &quot;figure&quot;\" width=\"200\" \
         style=\"height: 5em\" />\n<figcaption>Cap</figcaption>\n<div class=\"legend\">\n\
         <p>Leg</p>\n</div>\n</figure>\n<div class=\"highlight-python notranslate\">\
         <div class=\"highlight\"><pre><span class=\"linenos\">5</span>print(1 &lt; 2)\n\
         </pre></div></div>\n<ol start=\"3\" type=\"a\">\n<li>\n<p>c</p>\n</li>\n</ol>\n\
         <p><a href=\"https://example.org/a%20b\">site</a> <a class=\"footnote-reference\" \
         id=\"id2\" href=\"#id1\">[1]</a></p>\n<div class=\"target-notes\">\n\
         <aside class=\"footnote\" id=\"id1\">\n<p class=\"label\"><span \
         class=\"label-text\">[1]</span> <a class=\"backref\" href=\"#id2\">↩</a></p>\n\
         <p><a href=\"https://example.org/a%20b\">https://example.org/a b</a></p>\n\
         </aside>\n</div>\n\
         <div class=\"directive\" data-directive=\"custom-thing\">\n<p>inside</p>\n</div>\n\
         <footer>\n<p>Bottom</p>\n</footer>\n"
    );
}

fn labels(b: &mut Builder<'_>, names: &[&str]) -> Result<(), Fail> {
    let next = guidedoc::NodeId::from_index(b.snap.nodes.len() + names.len());
    for name in names {
        let label = open(b, K::Target)?;
        set_text(b, label, name)?;
        set_link(b, label, LinkKind::Internal, next)?;
        shut(b)?;
    }
    Ok(())
}

#[test]
fn aliases_go_inside_or_before_elements() {
    let r = render(
        |b| {
            labels(b, &["a", "b"])?;
            para(b, "Text")?;
            labels(b, &["c", "d"])?;
            list(b, ListStyle::Bullet, false, &["x"])
        },
        frag(),
    );
    assert_eq!(r.status, Status::Ok);
    assert_eq!(
        r.html,
        "<p id=\"a\"><span id=\"b\"></span>Text</p>\n\
         <span id=\"d\"></span><ul id=\"c\">\n<li>\n<p>x</p>\n</li>\n</ul>\n"
    );
}
