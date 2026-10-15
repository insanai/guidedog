//! Every Sphinx-layer kind and attribute of AST.md.

use guidedoc::{
    AdmonitionKind, AttributeKey as Key, INVALID_NODE, LinkKind, ListInfo, ListStyle, NodeFlags,
    NodeKind as K, RenderOptions, Status, trusted_policy,
};

use super::{Tb, expect_compiles, refusing, render_policy, render_with};
use crate::sphinx::in_line_ranges;

type R = Result<(), guidedoc::Fail>;

fn role(t: &mut Tb<'_, '_>, name: &str, content: &str) -> R {
    let r = t.open(K::Role, 0)?;
    t.set_text(r, name)?;
    t.text(content)?;
    t.close(r)
}

fn build_sphinx(t: &mut Tb<'_, '_>) -> R {
    let label = t.open(K::Target, 0)?;
    t.set_text(label, "my-label")?;
    t.link(label, LinkKind::Internal, "", "", label + 1)?;
    t.close(label)?;
    let section = t.open(K::Section, 1)?;
    t.custom(section, "secnumber", "1.2")?;
    let heading = t.open(K::Heading, 1)?;
    t.text("Intro")?;
    t.close(heading)?;
    let p = t.open(K::Paragraph, 0)?;
    let reference = t.open(K::Reference, 0)?;
    t.link(reference, LinkKind::Internal, "", "", section)?;
    t.text("back to the label")?;
    t.close(reference)?;
    t.close(p)?;
    build_captions(t)?;
    build_blocks(t)?;
    build_desc(t)?;
    build_lists(t)?;
    build_inlines(t)?;
    t.close(section)
}

fn build_captions(t: &mut Tb<'_, '_>) -> R {
    let figure = t.open(K::Figure, 0)?;
    t.attr(figure, Key::Id, "fig-one")?;
    t.custom(figure, "number-label", "Fig. 1.2")?;
    t.custom(figure, "figwidth", "image")?;
    let image = t.open(K::Image, 0)?;
    t.link(image, LinkKind::External, "img.png", "", INVALID_NODE)?;
    t.close(image)?;
    t.wrap(K::Caption, "A figure")?;
    t.close(figure)?;
    let table = t.open(K::Table, 1)?;
    t.custom(table, "number-label", "Table 3")?;
    t.wrap(K::Title, "Numbers")?;
    let body = t.open(K::TableBody, 0)?;
    let row = t.open(K::Row, 0)?;
    let cell = t.open(K::Cell, 0)?;
    t.para("1")?;
    t.close(cell)?;
    t.close(row)?;
    t.close(body)?;
    t.close(table)?;
    let math = t.leaf(K::MathBlock, "e^{i \\pi} = -1", 0)?;
    t.attr(math, Key::Id, "equation-euler")?;
    t.custom(math, "number", "1")?;
    let code = t.leaf(K::CodeBlock, "def f():\n    return 1\n", 9)?;
    t.attr(code, Key::Language, "python")?;
    t.custom(code, "linenos", "1")?;
    t.custom(code, "emphasize-lines", "2")?;
    t.custom(code, "caption", "Example <code> #1")?;
    t.custom(code, "number-label", "Listing 1")?;
    let marked = t.leaf(K::CodeBlock, "a\nb\nc\n", 0)?;
    t.custom(marked, "emphasize-lines", "1,3-")
}

fn build_blocks(t: &mut Tb<'_, '_>) -> R {
    let toctree = t.open(K::Toctree, 2)?;
    t.leaf(K::ToctreeEntry, "intro", 0)?;
    t.close(toctree)?;
    let wrapper = t.open(K::Container, 0)?;
    t.attr(wrapper, Key::Class, "toctree-wrapper compound")?;
    let caption = t.open(K::Paragraph, 0)?;
    t.attr(caption, Key::Class, "caption")?;
    t.text("Contents")?;
    t.close(caption)?;
    t.close(wrapper)?;
    let only = t.open(K::Only, 0)?;
    t.custom(only, "expression", "latex")?;
    t.para("Only PDF.")?;
    t.close(only)?;
    let index = t.leaf(K::Index, "", 0)?;
    t.attr(index, Key::Id, "index-0")?;
    let centered = t.open(K::Paragraph, 0)?;
    t.attr(centered, Key::Class, "centered")?;
    t.text("Middle")?;
    t.close(centered)?;
    let todo = t.open(K::Admonition, AdmonitionKind::Todo as i32)?;
    t.para("Finish.")?;
    t.close(todo)?;
    for show in ["1", ""] {
        let d = t.open(K::Directive, 0)?;
        t.attr(d, Key::Name, "moduleauthor")?;
        t.custom(d, "author", "Ann <a@b>")?;
        if !show.is_empty() {
            t.custom(d, "show", show)?;
        }
        t.close(d)?;
    }
    let list = t.open(K::Directive, 0)?;
    t.attr(list, Key::Name, "todolist")?;
    t.para("Collected.")?;
    t.close(list)?;
    build_glossary(t)?;
    for type_name in ["versionadded", "deprecated"] {
        let vm = t.open(K::VersionModified, 0)?;
        t.custom(vm, "type", type_name)?;
        t.custom(vm, "version", "2.1")?;
        if type_name == "versionadded" {
            t.para("New option.")?;
        }
        t.close(vm)?;
    }
    Ok(())
}

fn build_glossary(t: &mut Tb<'_, '_>) -> R {
    let glossary = t.open(K::Glossary, 0)?;
    let dl = t.open(K::DefinitionList, 0)?;
    let item = t.open(K::DefinitionItem, 0)?;
    t.attr(item, Key::Id, "item-api")?;
    let term = t.open(K::Term, 0)?;
    t.attr(term, Key::Id, "term-api")?;
    t.text("API")?;
    t.close(term)?;
    let definition = t.open(K::Definition, 0)?;
    t.attr(definition, Key::Id, "about-api")?;
    t.para("Interface.")?;
    t.close(definition)?;
    t.close(item)?;
    t.close(dl)?;
    t.close(glossary)
}

fn build_desc(t: &mut Tb<'_, '_>) -> R {
    let desc = t.open(K::Desc, 0)?;
    t.custom(desc, "domain", "py")?;
    t.custom(desc, "objtype", "function")?;
    let sig = t.open(K::DescSignature, 0)?;
    t.attr(sig, Key::Id, "mod.func")?;
    t.wrap(K::DescAnnotation, "async ")?;
    t.wrap(K::DescAddname, "mod.")?;
    t.wrap(K::DescName, "func")?;
    let params = t.open(K::DescParameterlist, 0)?;
    t.wrap(K::DescParameter, "a")?;
    let optional = t.open(K::DescOptional, 0)?;
    t.wrap(K::DescParameter, "b=[1]")?;
    t.close(optional)?;
    t.close(params)?;
    t.wrap(K::DescReturns, "int")?;
    t.close(sig)?;
    let index = t.leaf(K::Index, "", 0)?;
    t.attr(index, Key::Id, "index-1")?;
    let content = t.open(K::DescContent, 0)?;
    t.para("Does things.")?;
    t.close(content)?;
    t.close(desc)
}

fn build_lists(t: &mut Tb<'_, '_>) -> R {
    for style in [ListStyle::Bullet, ListStyle::Arabic] {
        let hlist = t.open(K::Hlist, 2)?;
        let list = t.open(K::List, 0)?;
        t.b.set_list(list, ListInfo { style, delimiter: b'.', start: 1 })?;
        t.b.add_flags(list, NodeFlags::TIGHT);
        for text in ["a", "b", "c"] {
            let item = t.open(K::ListItem, 0)?;
            t.para(text)?;
            t.close(item)?;
        }
        t.close(list)?;
        t.close(hlist)?;
    }
    let productions = t.open(K::ProductionList, 0)?;
    let expr = t.open(K::Production, 0)?;
    t.set_text(expr, "expr")?;
    t.attr(expr, Key::Id, "grammar-token-expr")?;
    t.text("term \"+\" term")?;
    t.close(expr)?;
    t.wrap(K::Production, "| = letter")?;
    t.close(productions)
}

fn build_inlines(t: &mut Tb<'_, '_>) -> R {
    let p = t.open(K::Paragraph, 0)?;
    let x = t.open(K::Xref, 0)?;
    t.link(x, LinkKind::Unresolved, "", "target", INVALID_NODE)?;
    t.custom(x, "reftype", "ref")?;
    t.text("a reference")?;
    t.close(x)?;
    for name in [
        "kbd",
        "guilabel",
        "menuselection",
        "accelerator",
        "command",
        "dfn",
        "file",
        "samp",
        "program",
        "mimetype",
        "regexp",
        "manpage",
        "custom",
    ] {
        t.text(" ")?;
        role(t, name, name)?;
    }
    let index = t.leaf(K::Index, "", 0)?;
    t.attr(index, Key::Id, "index-2")?;
    t.close(p)
}

const SPHINX_PARTS: &[&str] = &[
    "#heading(level: 1, numbering: gd-secnum(\"1.2\"))[Intro]<intro>#metadata(none)<my-label>",
    "#gd-link(\"intro\")[back to the label]",
    concat!(
        "#metadata(none)<fig-one>#figure(numbering: none, image(\"img.png\"), ",
        "caption: [#strong[Fig\\. 1\\.2] A figure])"
    ),
    "#figure(numbering: none, caption: [#strong[Table 3] Numbers], kind: table, table(",
    concat!(
        "#metadata(none)<equation-euler>#gd-equation(\"1\")[\n#context { let gd-math-font = ",
        "if type(text.font) == str { (text.font,) } else { text.font }; ",
        "$ e^(i pi) = - 1 $ }\n]"
    ),
    concat!(
        "#figure(kind: raw, numbering: none, supplement: none, caption: figure.caption(",
        "position: top, [#strong[Listing 1] Example \\<code\\> \\#1]), gd-numbered(9, ",
        "raw(block: true, lang: \"python\", \"def f():\\n    return 1\"), marked: (2, )))"
    ),
    "#gd-numbered(none, raw(block: true, \"a\\nb\\nc\"), marked: (1, 3, ))",
    "#gd-rubric[Contents]",
    "Only PDF\\.",
    "#metadata(none)<index-0>",
    "#gd-align(center)[#strong[Middle]]",
    "#gd-admonition(\"todo\", [Todo])[",
    "Module author\\: Ann \\<a\\@b\\>\n",
    "Collected\\.",
    // Labels of arguments stand inside their brackets, where markup is allowed.
    concat!(
        "terms.item([#metadata(none)<item-api>#metadata(none)<term-api>API], ",
        "[#metadata(none)<about-api>\nInterface\\.\n]"
    ),
    "#gd-version(\"added\", [Added in version 2\\.1\\:])[\nNew option\\.\n]",
    "#gd-version(\"deprecated\", [Deprecated since version 2\\.1\\.])[\n]",
    concat!(
        "#metadata(none)<mod.func>#gd-signature[#emph[async ]mod\\.#strong[func]\\(",
        "#emph[a]\\[, #emph[b\\=\\[1\\]]\\]\\) → int]"
    ),
    "#metadata(none)<index-1>",
    "#gd-description[\nDoes things\\.\n]",
    "#gd-hlist(columns: 2, tight: true,\n[\na\n],\n[\nb\n],\n[\nc\n],\n)",
    "#gd-hlist(columns: 2, tight: true, start: 1,",
    concat!(
        "#gd-productions(\n[#metadata(none)<grammar-token-expr>expr], [\\:\\:\\=], ",
        "[term \\\"\\+\\\" term],\n[], [], [\\| \\= letter],\n)"
    ),
    "a reference",
    "#gd-kbd[kbd]",
    "#emph[guilabel]",
    "#underline[accelerator]",
    "#strong[#gd-code-text[command]]",
    "#emph[dfn]",
    "#gd-code-text[file]",
    "#emph[#gd-code-text[mimetype]]",
    " custom#metadata(none)<index-2>",
];

#[test]
fn sphinx_kinds_render() {
    let r = render_policy(build_sphinx, trusted_policy());
    assert_eq!(r.status, Status::Ok, "{:?}", r.reports);
    for part in SPHINX_PARTS {
        r.expect(part);
    }
    assert!(!r.output.contains("Toctree"));
    assert!(!r.output.contains("intro\"\n"));
    assert_eq!(r.output.matches("Module author").count(), 1);
    // The number is the heading's numbering, which the template sets apart from the
    // title; the HTML suffix does not apply.
    let options = RenderOptions { secnumber_suffix: ". ", ..RenderOptions::default() };
    let suffix = render_with(build_sphinx, trusted_policy(), options);
    suffix.expect("numbering: gd-secnum(\"1.2\"))[Intro]");
    assert!(in_line_ranges("1, 3-5", 4));
    assert!(!in_line_ranges("1,3-5", 2));
}

#[test]
fn sphinx_kinds_compile_with_typst() {
    let r = render_policy(build_sphinx, trusted_policy());
    assert_eq!(r.status, Status::Ok);
    let Some(html) = expect_compiles("sphinx", &r.output, true) else {
        return;
    };
    for part in [
        "Added in version 2.1:",
        "Deprecated since version 2.1.",
        "Module author: Ann &lt;a@b>",
        "<kbd>kbd</kbd>",
        "id=\"intro\"",
        "Does things.",
        "Fig. 1.2",
        "Listing 1",
    ] {
        assert!(html.contains(part), "html lacks {part:?}");
    }
}

fn build_undocumented(t: &mut Tb<'_, '_>) -> R {
    for _ in 0..80 {
        let desc = t.open(K::Desc, 0)?;
        let signature = t.open(K::DescSignature, 0)?;
        t.leaf(K::DescName, "An undocumented enumerator", 0)?;
        t.close(signature)?;
        let content = t.open(K::DescContent, 0)?;
        t.close(content)?;
        t.close(desc)?;
    }
    Ok(())
}

#[test]
fn sphinx_undocumented_signatures_can_paginate() {
    let r = render_policy(build_undocumented, refusing());
    assert_eq!(r.status, Status::Ok);
    r.expect("#gd-signature(attached: false)[");
    assert!(!r.output.contains("#gd-description[\n]"));
    expect_compiles("undocumented-signatures", &r.output, false);
}
