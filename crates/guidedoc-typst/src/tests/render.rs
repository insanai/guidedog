//! Every node kind of AST.md, rendered and compiled.

use guidedoc::{
    AdmonitionKind, Align, AttributeKey as Key, CellInfo, Conformance, INVALID_NODE, LinkKind,
    ListInfo, ListStyle, NodeFlags, NodeKind as K, RenderOptions, Status, Text, kind_name,
    trusted_policy,
};

use super::{Tb, expect_compiles, render_policy, render_with};
use crate::{PREAMBLE, RENDERER, VERSION};

type R = Result<(), guidedoc::Fail>;

/// Uses every node kind of AST.md at least once.
fn build_everything(t: &mut Tb<'_, '_>) -> R {
    let title = t.open(K::Title, 0)?;
    t.text("Guide ")?;
    t.wrap(K::Emphasis, "one")?;
    t.close(title)?;
    t.wrap(K::Subtitle, "A subtitle")?;
    for name in ["header", "footer"] {
        let d = t.open(K::Directive, 0)?;
        t.attr(d, Key::Name, name)?;
        t.para(name)?;
        t.close(d)?;
    }
    let sectnum = t.open(K::Directive, 0)?;
    t.attr(sectnum, Key::Name, "sectnum")?;
    t.custom(sectnum, "depth", "2")?;
    t.close(sectnum)?;
    let meta = t.open(K::Directive, 0)?;
    t.attr(meta, Key::Name, "meta")?;
    t.custom(meta, "keywords", "guide")?;
    t.close(meta)?;
    let contents = t.open(K::Contents, 2)?;
    t.attr(contents, Key::Title, "Contents")?;
    t.close(contents)?;
    let intro = t.section(1, "Intro")?;
    let second_ref = build_inlines(t)?;
    let target = t.open(K::Target, 0)?;
    t.set_text(target, "target para")?;
    t.link(target, LinkKind::Internal, "", "", target + 1)?;
    t.close(target)?;
    t.para("The target paragraph.")?;
    t.leaf(K::Comment, "a comment", 0)?;
    let definition = t.open(K::SubstitutionDefinition, 0)?;
    t.set_text(definition, "sub")?;
    t.text("SUBSTITUTED")?;
    t.close(definition)?;
    build_blocks(t)?;
    build_tables(t)?;
    build_frames(t)?;
    t.close(intro)?;
    let second = t.section(1, "Second")?;
    t.b.resolve_link(second_ref, second, LinkKind::Internal, Text::default());
    let local = t.open(K::Contents, 1)?;
    t.b.add_flags(local, NodeFlags::LOCAL);
    t.close(local)?;
    let nested = t.section(2, "Nested")?;
    t.para("Nested text.")?;
    t.close(nested)?;
    t.close(second)?;
    let last = t.section(1, "Last")?;
    let h = t.open(K::Heading, 2)?;
    t.text("Loose heading")?;
    t.close(h)?;
    t.close(last)
}

/// Writes one paragraph with every inline kind. Returns the link row of a reference to
/// the second section, resolved once that section exists.
fn build_inlines(t: &mut Tb<'_, '_>) -> Result<u32, guidedoc::Fail> {
    let p = t.open(K::Paragraph, 0)?;
    t.text("Plain ")?;
    for kind in [
        K::Emphasis,
        K::Strong,
        K::Subscript,
        K::Superscript,
        K::TitleReference,
        K::Abbreviation,
        K::Role,
    ] {
        t.wrap(kind, kind_name(kind))?;
    }
    let code = t.open(K::Code, 0)?;
    t.set_text(code, "x = \"1\"")?;
    t.attr(code, Key::Language, "python")?;
    t.close(code)?;
    let link = t.open(K::Link, 0)?;
    t.link(link, LinkKind::External, "https://example.com/a?b=\"c\"", "", INVALID_NODE)?;
    t.text("example")?;
    t.close(link)?;
    let email = t.open(K::Link, 0)?;
    t.link(email, LinkKind::Email, "someone@example.com", "", INVALID_NODE)?;
    t.text("mail")?;
    t.close(email)?;
    let reference = t.open(K::Reference, 0)?;
    let row = t.link(reference, LinkKind::Unresolved, "", "second", INVALID_NODE)?;
    t.text("see the second section")?;
    t.close(reference)?;
    t.leaf(K::SoftBreak, "", 0)?;
    t.text("after a soft break")?;
    t.leaf(K::HardBreak, "", 0)?;
    t.leaf(K::Math, "x^2 + \\alpha", 0)?;
    let raw = t.leaf(K::RawInline, "#emph[raw inline]", 0)?;
    t.attr(raw, Key::Format, "typst")?;
    let target = t.open(K::InlineTarget, 0)?;
    t.set_text(target, "inline target")?;
    t.text("inline target")?;
    t.close(target)?;
    t.leaf(K::FootnoteReference, "1", 1)?;
    t.leaf(K::FootnoteReference, "1", 1)?;
    t.leaf(K::CitationReference, "CIT2026", 0)?;
    t.leaf(K::SubstitutionReference, "sub", 0)?;
    let image = t.open(K::Image, 0)?;
    t.attr(image, Key::Alt, "an inline image")?;
    t.link(image, LinkKind::External, "img.png", "", INVALID_NODE)?;
    t.close(image)?;
    t.close(p)?;
    Ok(row)
}

fn build_blocks(t: &mut Tb<'_, '_>) -> R {
    let quote = t.open(K::BlockQuote, 0)?;
    t.para("Quoted.")?;
    t.wrap(K::Attribution, "Someone")?;
    t.close(quote)?;
    for style in [ListStyle::Bullet, ListStyle::LowerAlpha, ListStyle::UpperRoman] {
        let list = t.open(K::List, 0)?;
        t.b.set_list(list, ListInfo { style, delimiter: b')', start: 3 })?;
        if style == ListStyle::Bullet {
            t.b.add_flags(list, NodeFlags::TIGHT);
        }
        for text in ["first", "second"] {
            let item = t.open(K::ListItem, 0)?;
            t.para(text)?;
            t.close(item)?;
        }
        t.close(list)?;
    }
    let dl = t.open(K::DefinitionList, 0)?;
    let item = t.open(K::DefinitionItem, 0)?;
    t.wrap(K::Term, "term")?;
    t.wrap(K::Classifier, "type")?;
    let definition = t.open(K::Definition, 0)?;
    t.para("The definition.")?;
    t.close(definition)?;
    t.close(item)?;
    t.close(dl)?;
    let fl = t.open(K::FieldList, 0)?;
    let field = t.open(K::Field, 0)?;
    t.wrap(K::FieldName, "Author")?;
    let body = t.open(K::FieldBody, 0)?;
    t.para("Me")?;
    t.close(body)?;
    t.close(field)?;
    t.close(fl)?;
    build_options(t)?;
    build_lines(t)?;
    t.leaf(K::CodeBlock, "def f():\n    return \"\\n\"\n", 5)?;
    let pre = t.open(K::Preformatted, 0)?;
    t.text("a  b\n\tc ")?;
    t.wrap(K::Strong, "bold")?;
    t.close(pre)?;
    t.leaf(K::DoctestBlock, ">>> 1 + 1\n2\n", 0)?;
    let math = "\\begin{aligned} a &= \\frac{b}{c} \\\\ d &= e \\end{aligned}";
    t.leaf(K::MathBlock, math, 0)?;
    let raw = t.leaf(K::RawBlock, "#text(fill: blue)[raw block]\n", 0)?;
    t.attr(raw, Key::Format, "typst")?;
    t.leaf(K::ThematicBreak, "", 0)?;
    Ok(())
}

fn build_options(t: &mut Tb<'_, '_>) -> R {
    let ol = t.open(K::OptionList, 0)?;
    let item = t.open(K::OptionItem, 0)?;
    let group = t.open(K::OptionGroup, 0)?;
    t.leaf(K::Option, "-a", 0)?;
    let option = t.open(K::Option, 0)?;
    t.set_text(option, "--all")?;
    t.leaf(K::OptionArgument, "FILE", i32::from(b'='))?;
    t.close(option)?;
    t.close(group)?;
    let description = t.open(K::OptionDescription, 0)?;
    t.para("Everything.")?;
    t.close(description)?;
    t.close(item)?;
    t.close(ol)
}

fn build_lines(t: &mut Tb<'_, '_>) -> R {
    let lb = t.open(K::LineBlock, 0)?;
    t.wrap(K::Line, "line one")?;
    let nested = t.open(K::LineBlock, 0)?;
    t.wrap(K::Line, "indented")?;
    t.close(nested)?;
    t.leaf(K::Line, "", 0)?;
    t.wrap(K::Line, "after a blank line")?;
    t.close(lb)
}

fn cell(t: &mut Tb<'_, '_>, text: &str, info: Option<CellInfo>) -> R {
    let c = t.open(K::Cell, 0)?;
    if let Some(info) = info {
        t.b.set_cell(c, info)?;
    }
    t.para(text)?;
    t.close(c)
}

fn build_tables(t: &mut Tb<'_, '_>) -> R {
    let table = t.open(K::Table, 3)?;
    t.attr(table, Key::Widths, "1 2 1")?;
    t.wrap(K::Title, "A table")?;
    let head = t.open(K::TableHead, 0)?;
    let row = t.open(K::Row, 0)?;
    for text in ["A", "B", "C"] {
        cell(t, text, None)?;
    }
    t.close(row)?;
    t.close(head)?;
    let body = t.open(K::TableBody, 0)?;
    let row = t.open(K::Row, 0)?;
    cell(t, "tall", Some(CellInfo { row_span: 2, col_span: 1, align: Align::None }))?;
    cell(t, "wide", Some(CellInfo { row_span: 1, col_span: 2, align: Align::Right }))?;
    t.close(row)?;
    let row = t.open(K::Row, 0)?;
    cell(t, "x", None)?;
    cell(t, "y", None)?;
    t.close(row)?;
    t.close(body)?;
    t.close(table)?;
    let figure = t.open(K::Figure, 0)?;
    let image = t.open(K::Image, 0)?;
    t.attr(image, Key::Alt, "a pixel")?;
    t.attr(image, Key::Width, "50%")?;
    t.link(image, LinkKind::External, "img.png", "", INVALID_NODE)?;
    t.close(image)?;
    t.wrap(K::Caption, "A caption")?;
    let legend = t.open(K::Legend, 0)?;
    t.para("A legend.")?;
    t.close(legend)?;
    t.close(figure)?;
    let block = t.open(K::Image, 0)?;
    t.attr(block, Key::Scale, "50%")?;
    t.attr(block, Key::Height, "20px")?;
    t.attr(block, Key::Align, "center")?;
    t.link(block, LinkKind::External, "img.png", "", INVALID_NODE)?;
    t.close(block)
}

fn build_frames(t: &mut Tb<'_, '_>) -> R {
    let note = t.open(K::Admonition, AdmonitionKind::Note as i32)?;
    t.para("A note.")?;
    t.close(note)?;
    let generic = t.open(K::Admonition, AdmonitionKind::Generic as i32)?;
    t.wrap(K::Title, "Custom title")?;
    t.para("Generic body.")?;
    t.close(generic)?;
    for kind in [K::Topic, K::Sidebar] {
        let frame = t.open(kind, 0)?;
        t.wrap(K::Title, kind_name(kind))?;
        if kind == K::Sidebar {
            t.wrap(K::Subtitle, "side subtitle")?;
        }
        t.para("Framed body.")?;
        t.close(frame)?;
    }
    t.wrap(K::Rubric, "A rubric")?;
    for kind in [K::Container, K::Compound] {
        let c = t.open(kind, 0)?;
        t.para(kind_name(kind))?;
        t.close(c)?;
    }
    build_notes(t)?;
    let other = t.open(K::Directive, 0)?;
    t.attr(other, Key::Name, "unknown-directive")?;
    t.wrap(K::Title, "Directive title")?;
    t.para("Directive body.")?;
    t.close(other)
}

fn build_notes(t: &mut Tb<'_, '_>) -> R {
    let footnote = t.open(K::Footnote, 1)?;
    t.set_text(footnote, "#")?;
    t.b.add_flags(footnote, NodeFlags::AUTO);
    t.para("The footnote.")?;
    t.close(footnote)?;
    let lonely = t.open(K::Footnote, 2)?;
    t.set_text(lonely, "2")?;
    t.para("Nobody cites me.")?;
    t.close(lonely)?;
    let citation = t.open(K::Citation, 0)?;
    t.set_text(citation, "CIT2026")?;
    t.para("A citation.")?;
    t.close(citation)?;
    let external = t.open(K::Target, 0)?;
    t.set_text(external, "python")?;
    t.link(external, LinkKind::External, "https://www.python.org/", "", INVALID_NODE)?;
    t.close(external)?;
    let notes = t.open(K::Directive, 0)?;
    t.attr(notes, Key::Name, "target-notes")?;
    t.close(notes)
}

#[test]
fn renderer_describes_itself() {
    let r = crate::renderer();
    assert_eq!(r.id, "typst");
    assert_eq!(r.name, "Typst source");
    assert_eq!(r.version, VERSION);
    assert_eq!(r.media_type, "text/x-typst");
    assert_eq!(r.conformance, Conformance::Partial);
    assert_eq!(RENDERER.id, r.id);
}

const EVERYTHING: &[&str] = &[
    "#gd-subtitle[A subtitle]\n\n#metadata(none)<contents>#outline(",
    "#set document(title: \"Guide one\")",
    "lang: \"en\"",
    "#set heading(numbering: gd-sectnum(2, \"\", \"\", 1))",
    "#set page(header: [",
    "#set page(footer: [",
    "#gd-title[Guide #emph[one]]",
    "#outline(title: [Contents], depth: 2)",
    "#heading(level: 1)[Intro]<intro>",
    "#emph[emphasis]",
    "#strong[strong]",
    "#sub[subscript]",
    "#super[superscript]",
    "#emph[title\\-reference]",
    "#raw(lang: \"python\", \"x = \\\"1\\\"\")",
    "#link(\"https://example.com/a?b=\\\"c\\\"\")[example]",
    "#link(\"mailto:someone@example.com\")[mail]",
    "#gd-link(\"second\")[see the second section]",
    "\nafter a soft break\\\n",
    "$x^(2) + alpha$",
    "#emph[raw inline]",
    "#metadata(none)<inline-target>",
    "#gd-footnote(\"1\")[\nThe footnote\\.\n]<id4>",
    "#gd-footnote(\"1\", label(\"id4\"))",
    "#gd-link(\"cit2026\")[\\[CIT2026\\]]",
    "SUBSTITUTED",
    "#box(image(\"img.png\", alt: \"an inline image\"))",
    "#metadata(none)<target-para>The target paragraph\\.",
    "#quote(block: true, attribution: [Someone])[\nQuoted\\.\n]",
    "#list(tight: true,\n[\nfirst\n],",
    "#enum(tight: false, start: 3, numbering: \"a)\"",
    "numbering: \"I)\"",
    "terms.item([term#gd-classifier[type]], [\nThe definition\\.\n]",
    "#gd-fields(\n[Author], [\nMe\n],",
    "[#raw(\"-a\"), #raw(\"--all=FILE\")], [",
    "#gd-lines[\nline one\n#gd-indent[#gd-lines[\nindented\n]]\n~ \\\nafter a blank line\n]",
    "#gd-numbered(5, raw(block: true, \"def f():\\n    return \\\"\\\\n\\\"\"))",
    "#gd-pre[a~~b\\\n~~~~~~~~c~#strong[bold]]",
    "#raw(block: true, lang: \"pycon\", \">>> 1 + 1\\n2\")",
    "$ a & = frac(b, c) \\ d & = e $",
    "#text(fill: blue)[raw block]\n",
    "#gd-rule()",
    "#figure(caption: [A table], kind: table, table(columns: (1fr, 2fr, 1fr, ),",
    "table.header(\n[\nA\n],",
    "table.hline(),\n),\n",
    "table.cell(rowspan: 2)[\ntall\n]",
    "table.cell(colspan: 2, align: right)[\nwide\n]",
    "#figure(image(\"img.png\", alt: \"a pixel\", width: 50%), caption: [A caption])",
    "#gd-legend[\nA legend\\.\n]",
    "#gd-align(center, gd-scale(50%, image(\"img.png\", height: 15pt)))",
    "#gd-admonition(\"note\", [Note])[\nA note\\.\n]",
    "#gd-admonition(\"admonition\", [Custom title])[\nGeneric body\\.\n]",
    "#gd-topic([topic])[\nFramed body\\.\n]",
    "#gd-sidebar([sidebar], [side subtitle])[",
    "#gd-rubric[A rubric]",
    "\ncontainer\n",
    "\ncompound\n",
    "#gd-note(\"2\")[\nNobody cites me\\.\n]<id5>",
    "#gd-citation(\"CIT2026\")[\nA citation\\.\n]<cit2026>",
    "[python: #link(\"https://www.python.org/\")]",
    "#gd-rubric[Directive title]\n\nDirective body\\.",
    "#outline(title: none, target: selector(heading).after(<second>, inclusive: false)\
     .before(<last>, inclusive: false), depth: 2)",
    "#heading(level: 2)[Loose heading]<loose-heading>",
];

#[test]
fn every_kind_renders() {
    let r = render_policy(build_everything, trusted_policy());
    assert_eq!(r.status, Status::Ok, "{:?}", r.reports);
    assert!(r.output.starts_with(PREAMBLE));
    for part in EVERYTHING {
        r.expect(part);
    }
    assert!(!r.output.contains("a comment"));
    assert_eq!(r.output.matches("<id4>").count(), 1);
    assert_eq!(r.output.matches("The footnote\\.").count(), 1);
}

#[test]
fn rendering_is_deterministic() {
    let a = render_policy(build_everything, trusted_policy());
    let b = render_policy(build_everything, trusted_policy());
    assert_eq!(a.status, Status::Ok);
    assert_eq!(a.output, b.output);
}

#[test]
fn body_only_output_has_no_preamble() {
    let options = RenderOptions { standalone: false, ..RenderOptions::default() };
    let r = render_with(build_everything, trusted_policy(), options);
    assert_eq!(r.status, Status::Ok);
    assert!(!r.output.contains("#let gd-"));
    assert!(!r.output.contains("#set page"));
    assert!(!r.output.contains("#set document"));
    assert!(r.output.starts_with("#metadata(none)<guide-one>#gd-title[Guide #emph[one]]"));
    let mut bytes = Vec::new();
    crate::emit_preamble(&mut guidedoc::Emitter::new(&mut bytes));
    assert_eq!(bytes, PREAMBLE.as_bytes());
}

/// The source map has an entry per mapped node, at offsets inside the output.
#[test]
fn source_maps_point_into_the_output() {
    let r = render_policy(build_everything, trusted_policy());
    assert!(r.source_map.len() > 50);
    assert!(r.source_map.iter().all(|e| e.offset as usize <= r.output.len()));
    assert!(r.source_map.windows(2).all(|w| w[0].offset <= w[1].offset));
}

#[test]
fn every_kind_compiles_with_typst() {
    let r = render_policy(build_everything, trusted_policy());
    assert_eq!(r.status, Status::Ok);
    expect_compiles("everything", &r.output, true);
}
