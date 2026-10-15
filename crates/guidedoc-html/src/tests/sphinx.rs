use guidedoc::{
    AdmonitionKind, AttributeKey as A, Builder, Emitter, Fail, Highlighter, LinkInfo, LinkKind,
    ListStyle, MathStyle, NodeId, NodeKind as K, Status,
};

use super::*;
use crate::code::in_line_ranges;

pub fn sphinx_sections(b: &mut Builder<'_>) -> Result<(), Fail> {
    let label = open(b, K::Target)?;
    set_text(b, label, "my-label")?;
    set_link(b, label, LinkKind::Internal, label + 1)?;
    shut(b)?;
    open_v(b, K::Section, 1)?;
    custom(b, "secnumber", "1.2")?;
    open_v(b, K::Heading, 1)?;
    text(b, "Intro")?;
    shut(b)?;
    para(b, "Body.")?;
    shut(b)
}

#[test]
fn ids_aliases_and_permalinks() {
    let r = render(sphinx_sections, RenderOptions { permalinks: true, ..frag() });
    assert_eq!(r.status, Status::Ok);
    assert_eq!(
        r.html,
        "<section id=\"intro\"><span id=\"my-label\"></span>\n<h1><span \
         class=\"section-number\">1.2. </span>Intro<a class=\"headerlink\" href=\"#intro\" \
         title=\"Link to this heading\">¶</a></h1>\n<p>Body.</p>\n</section>\n"
    );
    let options = RenderOptions { secnumber_suffix: " ", permalink_icon: "#", ..frag() };
    let plain = render(sphinx_sections, options);
    has(&plain, "<h1><span class=\"section-number\">1.2 </span>Intro</h1>");
    let options = RenderOptions { permalinks: true, permalink_icon: "#", ..frag() };
    let icon = render(sphinx_sections, options);
    has(&icon, "title=\"Link to this heading\">#</a>");
}

pub fn sphinx_captions(b: &mut Builder<'_>) -> Result<(), Fail> {
    open(b, K::Figure)?;
    attr(b, A::Id, "fig-one")?;
    custom(b, "number-label", "Fig. 1.2")?;
    link(b, K::Image, "a.png", LinkKind::External)?;
    shut(b)?;
    inline(b, K::Caption, "A figure")?;
    shut(b)?;
    open(b, K::Figure)?;
    custom(b, "figwidth", "image")?;
    link(b, K::Image, "b.png", LinkKind::External)?;
    shut(b)?;
    shut(b)?;
    open_v(b, K::Table, 1)?;
    attr(b, A::Id, "tab")?;
    custom(b, "number-label", "Table 3")?;
    inline(b, K::Title, "Numbers")?;
    open(b, K::TableBody)?;
    open(b, K::Row)?;
    open(b, K::Cell)?;
    para(b, "1")?;
    shut(b)?;
    shut(b)?;
    shut(b)?;
    shut(b)?;
    let math = leaf(b, K::MathBlock, "e^{i\\pi} = -1")?;
    b.add_attribute_string(math, A::Id, "equation-euler")?;
    b.add_custom_attribute(math, "number", "1")
}

#[test]
fn captions_are_numbered() {
    let r = render(sphinx_captions, RenderOptions { permalinks: true, ..frag() });
    assert_eq!(r.status, Status::Ok);
    has(
        &r,
        "<figure id=\"fig-one\">\n<img src=\"a.png\" alt=\"\" />\n<figcaption>\
         <span class=\"caption-number\">Fig. 1.2 </span><span class=\"caption-text\">\
         A figure</span><a class=\"headerlink\" href=\"#fig-one\" title=\"Link to this \
         image\">¶</a></figcaption>\n</figure>\n",
    );
    has(&r, "<figure style=\"width: fit-content\">");
    has(
        &r,
        "<table id=\"tab\">\n<caption><span class=\"caption-number\">Table 3 </span>\
         <span class=\"caption-text\">Numbers</span><a class=\"headerlink\" href=\"#tab\" \
         title=\"Link to this table\">¶</a></caption>",
    );
    has(
        &r,
        "<div id=\"equation-euler\" class=\"math\"><span class=\"eqno\">(1)\
         <a class=\"headerlink\" href=\"#equation-euler\" title=\"Link to this equation\">\
         ¶</a></span>\\[e^{i\\pi} = -1\\]</div>",
    );
    let unnumbered = render(sphinx_captions, frag());
    has(
        &unnumbered,
        "<figcaption><span class=\"caption-number\">Fig. 1.2 </span>\
         <span class=\"caption-text\">A figure</span></figcaption>",
    );
}

pub fn sphinx_code(b: &mut Builder<'_>) -> Result<(), Fail> {
    let code = leaf(b, K::CodeBlock, "def f():\n    return 1\n")?;
    b.set_value(code, 9);
    b.add_attribute_string(code, A::Language, "python")?;
    b.add_custom_attribute(code, "linenos", "1")?;
    b.add_custom_attribute(code, "emphasize-lines", "2")?;
    b.add_custom_attribute(code, "caption", "Example <code>")?;
    b.add_custom_attribute(code, "number-label", "Listing 1")?;
    b.add_custom_attribute(code, "name", "example")?;
    leaf(b, K::CodeBlock, "x < y\n").map(|_| ())
}

/// Marks keywords; it declines code without "def".
struct TestHighlighter;

impl Highlighter for TestHighlighter {
    fn highlight(&self, _language: &str, code: &str, out: &mut Emitter<'_>) -> bool {
        let Some(at) = code.find("def") else {
            return false;
        };
        out.emit("<span class=\"k\">def</span>");
        out.emit(&code[at + 3..]);
        true
    }
}

#[test]
fn code_blocks_have_captions_lines_and_highlighting() {
    let r = render(sphinx_code, RenderOptions { permalinks: true, ..frag() });
    assert_eq!(r.status, Status::Ok);
    assert_eq!(
        r.html,
        "<div id=\"example\" class=\"literal-block-wrapper docutils container\">\n\
         <div class=\"code-block-caption\"><span class=\"caption-number\">Listing 1 </span>\
         <span class=\"caption-text\">Example &lt;code&gt;</span><a class=\"headerlink\" \
         href=\"#example\" title=\"Link to this code\">¶</a></div>\n\
         <div class=\"highlight-python notranslate\"><div class=\"highlight\"><pre>\
         <span class=\"linenos\"> 9</span>def f():\n<span class=\"linenos\">10</span>\
         <span class=\"hll\">    return 1\n</span></pre></div></div>\n</div>\n\
         <pre><code>x &lt; y\n</code></pre>\n"
    );
    let lit = render(sphinx_code, RenderOptions { highlighter: Some(&TestHighlighter), ..frag() });
    has(&lit, "<span class=\"linenos\"> 9</span><span class=\"k\">def</span> f():\n");
    has(&lit, "<span class=\"hll\">    return 1\n</span>");
    has(&lit, "<pre><code>x &lt; y\n</code></pre>");
    assert!(in_line_ranges("1, 3-5", 4));
    assert!(in_line_ranges("7-", 90));
    assert!(!in_line_ranges("1,3-5", 2));
    let reference = render(sphinx_code, RenderOptions { reference_profile: true, ..frag() });
    has(&reference, "<pre><code class=\"language-python\">def f():\n");
}

fn author(b: &mut Builder<'_>, show: bool) -> Result<(), Fail> {
    open(b, K::Directive)?;
    attr(b, A::Name, "sectionauthor")?;
    custom(b, "author", "Ann <a@b>")?;
    if show {
        custom(b, "show", "1")?;
    }
    shut(b)
}

pub fn sphinx_blocks(b: &mut Builder<'_>) -> Result<(), Fail> {
    open_v(b, K::Toctree, 2)?;
    leaf(b, K::ToctreeEntry, "intro")?;
    shut(b)?;
    open(b, K::Container)?;
    attr(b, A::Class, "toctree-wrapper compound")?;
    open(b, K::Paragraph)?;
    attr(b, A::Class, "caption")?;
    text(b, "Contents")?;
    shut(b)?;
    shut(b)?;
    open(b, K::Only)?;
    custom(b, "expression", "html")?;
    para(b, "Only HTML.")?;
    shut(b)?;
    let index = open(b, K::Index)?;
    b.add_attribute_string(index, A::Id, "index-0")?;
    b.add_custom_attribute(index, "entries", "single: thing")?;
    shut(b)?;
    open(b, K::Paragraph)?;
    attr(b, A::Class, "centered")?;
    text(b, "Middle")?;
    shut(b)?;
    open_v(b, K::Admonition, AdmonitionKind::Todo as i32)?;
    para(b, "Finish.")?;
    shut(b)?;
    author(b, true)?;
    author(b, false)?;
    open(b, K::Directive)?;
    attr(b, A::Name, "todolist")?;
    para(b, "Collected.")?;
    shut(b)
}

#[test]
fn sphinx_blocks_render_like_sphinx() {
    let r = render(sphinx_blocks, frag());
    assert_eq!(r.status, Status::Ok);
    assert_eq!(
        r.html,
        "<div class=\"toctree-wrapper compound\">\n\
         <p class=\"caption\" role=\"heading\"><span class=\"caption-text\">Contents</span>\
         </p>\n</div>\n<p>Only HTML.</p>\n<span id=\"index-0\"></span>\n\
         <p class=\"centered\"><strong>Middle</strong></p>\n\
         <aside id=\"id1\" class=\"admonition todo admonition-todo\">\n\
         <p class=\"admonition-title\">Todo</p>\n<p>Finish.</p>\n</aside>\n\
         <p><span class=\"sectionauthor\">Section author: </span>Ann &lt;a&#64;b&gt;</p>\n\
         <p>Collected.</p>\n"
    );
}

pub fn sphinx_glossary_and_versions(b: &mut Builder<'_>) -> Result<(), Fail> {
    open(b, K::Glossary)?;
    open(b, K::DefinitionList)?;
    open(b, K::DefinitionItem)?;
    open(b, K::Term)?;
    attr(b, A::Id, "term-api")?;
    text(b, "API")?;
    shut(b)?;
    open(b, K::Definition)?;
    para(b, "Interface.")?;
    shut(b)?;
    shut(b)?;
    shut(b)?;
    shut(b)?;
    open(b, K::VersionModified)?;
    custom(b, "type", "versionadded")?;
    custom(b, "version", "2.1")?;
    para(b, "New option.")?;
    shut(b)?;
    open(b, K::VersionModified)?;
    custom(b, "type", "deprecated")?;
    custom(b, "version", "3.0")?;
    shut(b)
}

#[test]
fn glossaries_and_version_notes() {
    let r = render(sphinx_glossary_and_versions, frag());
    assert_eq!(r.status, Status::Ok);
    assert_eq!(
        r.html,
        "<dl class=\"simple glossary\">\n<dt id=\"term-api\">API</dt>\n<dd>\n\
         <p>Interface.</p>\n</dd>\n</dl>\n<div class=\"versionadded\">\n<p><span \
         class=\"versionmodified added\">Added in version 2.1: </span>New option.</p>\n\
         </div>\n<div class=\"deprecated\">\n<p><span class=\"versionmodified deprecated\">\
         Deprecated since version 3.0.</span></p>\n</div>\n"
    );
    // With permalinks, a glossary term links to itself, as Sphinx's writer does.
    let linked =
        render(sphinx_glossary_and_versions, RenderOptions { permalinks: true, ..frag() });
    has(
        &linked,
        "<dt id=\"term-api\">API<a class=\"headerlink\" href=\"#term-api\" \
         title=\"Link to this term\">¶</a></dt>",
    );
}

pub fn sphinx_desc(b: &mut Builder<'_>) -> Result<(), Fail> {
    open(b, K::Desc)?;
    custom(b, "domain", "py")?;
    custom(b, "objtype", "function")?;
    open(b, K::DescSignature)?;
    attr(b, A::Id, "mod.func")?;
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
    let index = open(b, K::Index)?;
    b.add_attribute_string(index, A::Id, "index-1")?;
    shut(b)?;
    open(b, K::DescContent)?;
    para(b, "Does things.")?;
    shut(b)?;
    shut(b)
}

#[test]
fn signatures_render_like_sphinx() {
    let r = render(sphinx_desc, RenderOptions { permalinks: true, ..frag() });
    assert_eq!(r.status, Status::Ok);
    assert_eq!(
        r.html,
        "<span id=\"index-1\"></span><dl class=\"py function\">\n<dt id=\"mod.func\" \
         class=\"sig sig-object py\"><span class=\"sig-prename descclassname\">mod.</span>\
         <span class=\"sig-name descname\">func</span><span class=\"sig-paren\">(</span>\
         <em class=\"sig-param\">a</em><span class=\"optional\">[</span>, \
         <em class=\"sig-param\">b=1</em><span class=\"optional\">]</span>\
         <span class=\"sig-paren\">)</span> <span class=\"sig-return\">\
         <span class=\"sig-return-icon\">&#x2192;</span> <span \
         class=\"sig-return-typehint\">int</span></span><a class=\"headerlink\" \
         href=\"#mod.func\" title=\"Link to this definition\">¶</a></dt>\n<dd>\n\
         <p>Does things.</p>\n</dd>\n</dl>\n"
    );
}

fn production(b: &mut Builder<'_>, token: &str, id: &str, body: &str) -> Result<(), Fail> {
    let p = open(b, K::Production)?;
    set_text(b, p, token)?;
    if !id.is_empty() {
        attr(b, A::Id, id)?;
    }
    text(b, body)?;
    shut(b)
}

pub fn sphinx_lists(b: &mut Builder<'_>) -> Result<(), Fail> {
    open_v(b, K::Hlist, 2)?;
    list(b, ListStyle::Bullet, true, &["a", "b", "c"])?;
    shut(b)?;
    open(b, K::ProductionList)?;
    production(b, "expr", "grammar-token-expr", "term \"+\" term")?;
    production(b, "t", "grammar-token-t", "digit")?;
    production(b, "", "", "| letter")?;
    shut(b)
}

#[test]
fn hlists_and_productions() {
    let r = render(sphinx_lists, frag());
    assert_eq!(r.status, Status::Ok);
    assert_eq!(
        r.html,
        "<table class=\"hlist\"><tr><td>\n<ul>\n<li>a</li>\n<li>b</li>\n</ul>\n</td><td>\n\
         <ul>\n<li>c</li>\n</ul>\n</td></tr>\n</table>\n\
         <pre class=\"productionlist\"><strong id=\"grammar-token-expr\">expr</strong> ::= \
         term &quot;+&quot; term\n<strong id=\"grammar-token-t\">t   </strong> ::= digit\n\
         \x20        | letter\n</pre>\n"
    );
}

fn role(b: &mut Builder<'_>, name: &str, content: &str) -> Result<NodeId, Fail> {
    let r = open(b, K::Role)?;
    set_text(b, r, name)?;
    text(b, content)?;
    Ok(r)
}

pub fn sphinx_inlines(b: &mut Builder<'_>) -> Result<(), Fail> {
    open(b, K::Paragraph)?;
    for reftype in ["ref", "py:func"] {
        let x = open(b, K::Xref)?;
        let name = b.add_text("target")?;
        b.set_link(x, LinkInfo { name, ..LinkInfo::UNRESOLVED })?;
        b.add_custom_attribute(x, "reftype", reftype)?;
        text(b, reftype)?;
        shut(b)?;
    }
    let g = open(b, K::Role)?;
    set_text(b, g, "guilabel")?;
    role(b, "accelerator", "C")?;
    shut(b)?;
    text(b, "ancel")?;
    shut(b)?;
    role(b, "file", "/etc/")?;
    inline(b, K::Emphasis, "name")?;
    shut(b)?;
    let names = ["menuselection", "command", "dfn", "samp", "program", "mimetype", "regexp"];
    for name in names.into_iter().chain(["manpage", "makevar", "custom"]) {
        role(b, name, name)?;
        shut(b)?;
    }
    let index = open(b, K::Index)?;
    b.add_attribute_string(index, A::Id, "index-2")?;
    shut(b)?;
    shut(b)
}

#[test]
fn sphinx_roles_and_references() {
    let r = render(sphinx_inlines, frag());
    assert_eq!(r.status, Status::Ok);
    assert_eq!(
        r.html,
        "<p><span class=\"xref std std-ref\">ref</span><span class=\"xref py py-func\">\
         py:func</span><span class=\"guilabel\"><span class=\"accelerator\">C</span>ancel\
         </span><code class=\"file docutils literal notranslate\">/etc/<em>name</em></code>\
         <span class=\"menuselection\">menuselection</span><strong class=\"command\">command\
         </strong><em class=\"dfn\">dfn</em><code class=\"samp docutils literal notranslate\">\
         samp</code><strong class=\"program\">program</strong><em class=\"mimetype\">mimetype\
         </em><code class=\"regexp docutils literal notranslate\">regexp</code>\
         <em class=\"manpage\">manpage</em><strong class=\"makevar\">makevar</strong>\
         <span class=\"custom\">custom</span><span id=\"index-2\"></span></p>\n"
    );
}

#[test]
fn everything_renders_standalone_with_highlighting() {
    let options = RenderOptions {
        permalinks: true,
        highlighter: Some(&TestHighlighter),
        ..RenderOptions::default()
    };
    let r = render_with(
        |b| {
            sphinx_sections(b)?;
            sphinx_captions(b)?;
            sphinx_code(b)?;
            sphinx_blocks(b)?;
            sphinx_glossary_and_versions(b)?;
            sphinx_desc(b)?;
            super::fields::sphinx_fields(b)?;
            sphinx_lists(b)?;
            sphinx_inlines(b)
        },
        options,
        guidedoc::default_policy(),
        |_| {},
    );
    assert_eq!(r.status, Status::Ok);
    assert!(r.html.len() > 1000);
}

fn sphinx_math(b: &mut Builder<'_>) -> Result<(), Fail> {
    open(b, K::Paragraph)?;
    leaf(b, K::Math, "a<b")?;
    shut(b)?;
    leaf(b, K::MathBlock, "a = b \\\\ c = d\n\ne = f")?;
    let raw = leaf(b, K::MathBlock, "x")?;
    b.add_custom_attribute(raw, "nowrap", "1")?;
    b.add_attribute_string(raw, A::Id, "equation-raw")?;
    b.add_custom_attribute(raw, "number", "2")
}

// With MathStyle::Sphinx, math is written as Sphinx's mathjax extension writes it.
#[test]
fn math_in_sphinx_style() {
    let options = RenderOptions { math: MathStyle::Sphinx, math_inline: ["$", "$"], ..frag() };
    let r = render(sphinx_math, options);
    assert_eq!(r.status, Status::Ok);
    has(&r, "<span class=\"math notranslate nohighlight\">$a&lt;b$</span>");
    has(
        &r,
        "<div class=\"math notranslate nohighlight\">\\[ \\begin{align}\
         \\begin{aligned}\\begin{split}a = b \\\\ c = d\\end{split}\\\\e = f\
         \\end{aligned}\\end{align} \\]</div>",
    );
    // nowrap: the text alone, without the number.
    has(&r, "<div id=\"equation-raw\" class=\"math notranslate nohighlight\">x</div>");
    let plain = render(sphinx_math, frag());
    has(&plain, "<span class=\"math\">\\(a&lt;b\\)</span>");
}
