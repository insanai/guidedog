//! The MyST syntax without directive and role handlers; the Sphinx layer's tests run it
//! with handlers.

mod common;

use std::time::{Duration, Instant};

use common::{dump, myst_dump};
use guidedoc::{Workspace, convert, read_document, read_extension};
use guidedoc_commonmark::{
    MYST_READER, READER, Rule, Settings, Substitution, SyntaxExtension as X, SyntaxExtensions,
};

#[track_caller]
fn expect_myst(markdown: &str, expected: &str, settings: &Settings) {
    assert_eq!(myst_dump(markdown, settings), expected, "{markdown:?}");
}

fn with(extensions: &[X]) -> Settings {
    Settings { enable_extensions: SyntaxExtensions::of(extensions), ..Settings::default() }
}

#[test]
fn headings_make_sections_and_nested_headings_rubrics() {
    let s = Settings::default();
    expect_myst(
        "# A\n\n## B\n\ntext\n\n> # Q\n\n# C\n",
        "document\n  section value=1\n    heading value=1\n      text \"A\"\n    \
         section value=2\n      heading value=2\n        text \"B\"\n      paragraph\n        \
         text \"text\"\n      block-quote\n        rubric\n          text \"Q\"\n  \
         section value=1\n    heading value=1\n      text \"C\"\n",
        &s,
    );
    expect_myst(
        "# A\n### C\n",
        "document\n  section value=1\n    heading value=1\n      text \"A\"\n    \
         section value=2\n      heading value=2\n        text \"C\"\n",
        &s,
    );
}

#[test]
fn targets_comments_and_block_breaks() {
    expect_myst(
        "(my label)=\n# T\n\n% one\n% two\ntext\n+++ cell\n",
        "document\n  target \"my label\" link=internal ->2\n  section value=1 +referenced\n    \
         heading value=1\n      text \"T\"\n    comment \"one\\ntwo\"\n    paragraph\n      \
         text \"text\"\n    comment \"cell\" @class=\"block_break\"\n",
        &Settings::default(),
    );
}

#[test]
fn footnotes_collect_in_reference_order() {
    expect_myst(
        "[^1] and [^x] [^x]\n\n[^x]: X *note*.\n\n    more\n[^1]: One.\n[^u]: Unused.\n",
        "document\n  paragraph\n    footnote-reference value=1 \"1\" link=internal name=\"1\" \
         ->8\n    text \" and \"\n    footnote-reference value=2 +auto \"#x\" link=internal \
         name=\"x\" ->11\n    text \" \"\n    footnote-reference value=2 +auto \"#x\" \
         link=internal name=\"x\" ->11\n  thematic-break @class=\"footnotes\"\n  footnote \
         value=1 +referenced \"1\"\n    paragraph\n      text \"One.\"\n  footnote value=2 \
         +auto +referenced \"#x\"\n    paragraph\n      text \"X \"\n      emphasis\n        \
         text \"note\"\n      text \".\"\n    paragraph\n      text \"more\"\n",
        &Settings::default(),
    );
    let no_transition = Settings { footnote_transition: false, ..Settings::default() };
    expect_myst(
        "[^a]\n\n[^a]: A.\n",
        "document\n  paragraph\n    footnote-reference value=1 +auto \"#a\" link=internal \
         name=\"a\" ->3\n  footnote value=1 +auto +referenced \"#a\"\n    paragraph\n      \
         text \"A.\"\n",
        &no_transition,
    );
}

#[test]
fn links_resolve_as_myst_does() {
    expect_myst(
        "[a](https://x.org) [b](other.md) [c](sub/doc.rst#sec) [d](label) [e](#frag) \
         [](ftp://f.org) [g](ab:c)\n",
        "document\n  paragraph\n    link link=external dest=\"https://x.org\"\n      \
         text \"a\"\n    text \" \"\n    xref link=unresolved name=\"other\" @reftype=\"doc\" \
         @refexplicit=\"1\"\n      text \"b\"\n    text \" \"\n    xref link=unresolved \
         name=\"sub/doc\" @reftype=\"doc\" @refexplicit=\"1\" @anchor=\"sec\"\n      \
         text \"c\"\n    text \" \"\n    xref link=unresolved name=\"label\" @reftype=\"any\" \
         @refexplicit=\"1\"\n      text \"d\"\n    text \" \"\n    link link=external \
         dest=\"#frag\"\n      text \"e\"\n    text \" \"\n    link link=external \
         dest=\"ftp://f.org\"\n    text \" \"\n    xref link=unresolved name=\"ab:c\" \
         @reftype=\"any\" @refexplicit=\"1\"\n      text \"g\"\n",
        &Settings::default(),
    );
    let every = Settings { url_schemes: None, ..Settings::default() };
    expect_myst(
        "[g](ab:c)\n",
        "document\n  paragraph\n    link link=external dest=\"ab:c\"\n      text \"g\"\n",
        &every,
    );
}

#[test]
fn heading_anchors_link_sections() {
    let s = Settings { heading_anchors: 2, ..Settings::default() };
    expect_myst(
        "See [](#my-title-1) and [x](#my-title).\n\n# My title!\n\n## My title\n",
        "document\n  paragraph\n    text \"See \"\n    link link=internal ->12\n      \
         text \"My title\"\n    text \" and \"\n    link link=internal ->9\n      text \"x\"\n    \
         text \".\"\n  section value=1 @slug=\"my-title\"\n    heading value=1\n      \
         text \"My title!\"\n    section value=2 @slug=\"my-title-1\"\n      heading \
         value=2\n        \
         text \"My title\"\n",
        &s,
    );
}

#[test]
fn roles_and_directives_need_handlers() {
    expect_myst(
        "Role {sub}`x` and {u}``a`b``.\n",
        "document\n  paragraph\n    text \"Role \"\n    role \"sub\"\n      text \"x\"\n    \
         text \" and \"\n    role \"u\"\n      text \"a`b\"\n    text \".\"\n",
        &Settings::default(),
    );
    expect_myst("```{note}\nbody\n```\n", "document\n", &Settings::default());
}

#[test]
fn dollar_math_follows_its_settings() {
    let s = Settings::default();
    expect_myst(
        "a $x$ b $ y $ c 1$z$2\n",
        "document\n  paragraph\n    text \"a \"\n    math \"x\"\n    text \" b \"\n    \
         math \" y \"\n    text \" c 1\"\n    math \"z\"\n    text \"2\"\n",
        &s,
    );
    let strict =
        Settings { dmath_allow_space: false, dmath_allow_digits: false, ..Settings::default() };
    expect_myst(
        "b $ y $ c 1$z$2\n",
        "document\n  paragraph\n    text \"b $ y $ c 1$z$2\"\n",
        &strict,
    );
    expect_myst(
        "$$\na = 1\n$$\n\n$$b$$ (lab)\n",
        "document\n  math-block \"a = 1\"\n  math-block \"b\" @label=\"lab\"\n",
        &s,
    );
    expect_myst(
        "\\begin{equation*}\nx\n\\end{equation*}\n",
        "document\n  math-block \"\\\\begin{equation*}\\nx\\n\\\\end{equation*}\" \
         @class=\"amsmath\" @nowrap=\"1\"\n",
        &with(&[X::Amsmath]),
    );
    expect_myst("a $x$\n", "document\n  paragraph\n    text \"a $x$\"\n", &with(&[]));
}

#[test]
fn tables_definition_and_field_lists() {
    expect_myst(
        "text\n| a | b |\n|:-|--:|\n| 1 |\n",
        "document\n  paragraph\n    text \"text\"\n  table value=2 @class=\"colwidths-auto\"\n    \
         table-head\n      row +header\n        cell +header span=1x1 \
         @class=\"text-left\"\n          \
         paragraph\n            text \"a\"\n        cell +header span=1x1 \
         @class=\"text-right\"\n          paragraph\n            text \"b\"\n    \
         table-body\n      \
         row\n        cell span=1x1 @class=\"text-left\"\n          paragraph\n            \
         text \"1\"\n        cell span=1x1 @class=\"text-right\"\n",
        &Settings::default(),
    );
    expect_myst(
        "Term\n\n: Def\n\n  more\n~ Two\n",
        "document\n  definition-list @class=\"simple myst\"\n    definition-item\n      \
         term\n        \
         text \"Term\"\n      definition\n        paragraph\n          text \"Def\"\n        \
         paragraph\n          text \"more\"\n      definition\n        paragraph\n          \
         text \"Two\"\n",
        &with(&[X::Deflist]),
    );
    expect_myst(
        ":a b: c\n:*d*:\n  e\n",
        "document\n  field-list @class=\"myst\"\n    field\n      field-name\n        \
         text \"a b\"\n      field-body\n        paragraph\n          text \"c\"\n    \
         field\n      \
         field-name\n        emphasis\n          text \"d\"\n      field-body\n        \
         paragraph\n          text \"e\"\n",
        &with(&[X::Fieldlist]),
    );
}

#[test]
fn front_matter_fields_meta_and_errors() {
    expect_myst(
        "---\ntitle: T\nlist: [1, 'b']\nsubstitutions:\n  s: \"**b**\"\nhtml_meta:\n  \
         keywords: a, b\n---\n{{ s }} x\n",
        "document\n  field-list\n    field\n      field-name\n        text \"title\"\n      \
         field-body\n        paragraph\n          text \"T\"\n    field\n      \
         field-name\n        \
         text \"list\"\n      field-body\n        paragraph\n          \
         text \"[1, \\\"b\\\"]\"\n  directive @name=\"meta\" @keywords=\"a, b\"\n  paragraph\n    \
         strong\n      text \"b\"\n    text \" x\"\n",
        &with(&[X::Substitution]),
    );
    expect_myst("---\na: {\n---\ntext\n", "myst.topmatter", &Settings::default());
}

#[test]
fn substitutions_evaluate_names_and_filters() {
    let mut s = with(&[X::Substitution]);
    s.substitutions =
        vec![Substitution::new("a", "*x*"), Substitution::new("b", "{{ a }} and {{ b }}")];
    s.docname = "guide/intro".into();
    expect_myst(
        "{{ a }}, {{ env.docname | upper }}, {{ 'p' ~ a | trim }}.\n\n{{ a }}\n",
        "document\n  paragraph\n    emphasis\n      text \"x\"\n    text \", \"\n    \
         text \"GUIDE/INTRO\"\n    text \", \"\n    text \"p\"\n    emphasis\n      text \
         \"x\"\n    \
         text \".\"\n  paragraph\n    emphasis\n      text \"x\"\n",
        &s,
    );
    let circular = myst_dump("{{ b }}\n", &s);
    assert!(circular.contains("text \" and \""), "{circular}");
}

#[test]
fn optional_typography_and_links() {
    expect_myst(
        "(c) +- ... -- \"q\" don't www.example.com\n",
        "document\n  paragraph\n    text \"\u{a9} \u{b1} \u{2026} \u{2013} \u{201c}q\u{201d} \
         don\u{2019}t \"\n    link link=external dest=\"http://www.example.com\"\n      \
         text \"www.example.com\"\n",
        &with(&[X::Replacements, X::Smartquotes, X::Linkify]),
    );
    let unchecked = "<input class=\\\"task-list-item-checkbox\\\" disabled=\\\"disabled\\\" \
                     type=\\\"checkbox\\\">";
    let checked = "<input class=\\\"task-list-item-checkbox\\\" checked=\\\"checked\\\" \
                   disabled=\\\"disabled\\\" type=\\\"checkbox\\\">";
    expect_myst(
        "- [ ] a\n- [x] b\n",
        &format!(
            "document\n  list +tight list=bullet,-,1 @class=\"contains-task-list\"\n    \
             list-item @class=\"task-list-item\"\n      paragraph\n        raw-inline \
             \"{unchecked}\" @format=\"html\"\n        text \" \"\n        text \"a\"\n    \
             list-item @class=\"task-list-item\"\n      paragraph\n        raw-inline \
             \"{checked}\" @format=\"html\"\n        text \" \"\n        text \"b\"\n"
        ),
        &with(&[X::Tasklist]),
    );
}

#[test]
fn html_admonitions_and_images() {
    expect_myst(
        "<div class=\"admonition tip extra\" name=\"n\">\n*body*\n</div>\n\n\
         <img src=\"a.png\" alt=\"A\" width=\"20px\">\n",
        "document\n  admonition value=8 @class=\"extra\" @name=\"n\"\n    paragraph\n      \
         emphasis\n        text \"body\"\n  image link=external dest=\"a.png\" @alt=\"A\" \
         @width=\"20px\"\n",
        &with(&[X::HtmlAdmonition, X::HtmlImage]),
    );
    expect_myst(
        "<div class=\"admonition\">\n<p class=\"title\">T</p>\nx\n</div>\n",
        "document\n  admonition\n    title\n      text \"T\"\n    paragraph\n      text \"x\"\n",
        &with(&[X::HtmlAdmonition]),
    );
}

#[test]
fn syntax_can_be_disabled() {
    let s = Settings {
        disable_syntax: vec!["emphasis".into(), "table".into()],
        ..Settings::default()
    };
    expect_myst(
        "*a*\n\n| a |\n|---|\n",
        "document\n  paragraph\n    text \"*a*\"\n  paragraph\n    text \"| a |\"\n    \
         soft-break\n    text \"|---|\"\n",
        &s,
    );
    let only = Settings { commonmark_only: true, ..Settings::default() };
    expect_myst(
        "# T\n\n(x)=\n$a$ {r}`b`\n",
        "document\n  section value=1\n    heading value=1\n      text \"T\"\n    paragraph\n      \
         text \"(x)=\"\n      soft-break\n      text \"$a$ {r}\"\n      code \"b\"\n",
        &only,
    );
    // The "commonmark" reader: no MyST.
    assert_eq!(
        dump("# T\n$a$\n"),
        "document\n  heading value=1\n    text \"T\"\n  paragraph\n    text \"$a$\"\n"
    );
}

#[test]
fn colon_fences_and_markdown_figures() {
    expect_myst(
        ":::: plain\n::: x\n::::\n",
        "document\n  code-block \"::: x\\n\" @language=\"plain\"\n",
        &with(&[X::ColonFence]),
    );
    expect_myst(
        "```{figure-md} fig\n:class: c\n\n![alt](a.png)\n\nThe *caption*\n```\n",
        "document\n  figure @class=\"c\" @name=\"fig\"\n    image link=external \
         dest=\"a.png\"\n      text \"alt\"\n    caption\n      text \"The \"\n      \
         emphasis\n        text \"caption\"\n",
        &Settings::default(),
    );
}

const MYST_SAMPLE: &str = "---\ntitle: T\nsubstitutions:\n  s: \"*v*\"\n---\n(t)=\n# Title\n\n\
    Text {{ s }} with $x$, {r}`c`, [^n], [a](#title), www.example.com, (c) \"q\".\n\n\
    % comment\n+++\n| a | b |\n|---|---|\n| 1 | 2 |\n\nTerm\n: Def\n\n:f: body\n\n\
    - [ ] task\n\n$$\ny\n$$ (e)\n\n\\begin{equation}\nz\n\\end{equation}\n\n\
    :::{note}\nx\n:::\n\n<div class=\"admonition tip\">\ntip\n</div>\n\n\
    <img src=\"a.png\" alt=\"a\">\n\n[^n]: A *note*.\n";

fn all_syntax() -> Settings {
    Settings {
        enable_extensions: SyntaxExtensions::ALL,
        heading_anchors: 2,
        ..Settings::default()
    }
}

#[test]
fn myst_sample_converts_with_every_extension() {
    let settings = all_syntax();
    let registry = common::registry();
    let mut ws = Workspace::new();
    let mut request = common::reference_request(MYST_SAMPLE);
    request.reader = "myst";
    request.read.extension = Some(read_extension(&settings));
    let html = convert(&mut ws, &registry, &request).map(str::to_owned);
    let html = html.unwrap_or_else(|f| panic!("{}: {}", f.code(), ws.describe(&f.diagnostic)));
    assert!(html.contains("<h1"), "{html}");
}

#[test]
fn hostile_myst_inputs_end_in_results() {
    let settings = Settings { heading_anchors: 6, ..all_syntax() };
    let registry = common::registry();
    let patterns = [
        "{r}`",
        "{{",
        "$",
        "$$",
        "[^a]",
        "[^a]: x\n",
        ":::",
        "% c\n",
        "(a)=\n",
        "| a |\n",
        ": d\n",
        ":a: b\n",
        "\"'",
        "www.a.com ",
        "<div class=\"admonition\">",
        "---\n",
        "# h\n",
        "\\begin{equation}",
        "- [ ] ",
        "```{note}\n",
    ];
    for pattern in patterns {
        let text = pattern.repeat(20_000);
        let mut ws = Workspace::new();
        let mut request = common::reference_request(&text);
        request.reader = "myst";
        request.read.extension = Some(read_extension(&settings));
        let start = Instant::now();
        let _ = convert(&mut ws, &registry, &request);
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "took {:?} for {pattern:?}",
            start.elapsed()
        );
    }
}

#[test]
fn settings_default_as_myst_parser() {
    assert_eq!(READER.id, "commonmark");
    assert_eq!(MYST_READER.id, "myst");
    let s = Settings::default();
    assert!(!s.commonmark_only);
    assert_eq!(s.enable_extensions, SyntaxExtensions::of(&[X::Dollarmath]));
    assert_eq!(s.url_schemes.as_ref().map(Vec::len), Some(4));
    assert!(s.linkify_fuzzy_links && s.footnote_transition);
    assert!(s.dmath_allow_labels && s.dmath_allow_space && s.dmath_allow_digits);
    assert!(!s.dmath_double_inline);
    assert_eq!(s.sub_delimiters, [b'{', b'}']);
    assert_eq!(s.words_per_minute, 200);
    assert_eq!(s.heading_anchors, 0);
    for e in X::ALL {
        assert_eq!(X::named(e.name()), Some(e));
    }
    assert_eq!(Rule::named("html_block"), Some(Rule::HtmlBlock));
}

/// The two reader ids are two profiles: "commonmark" is pure CommonMark and takes no
/// settings; "myst" is MyST by default and takes only `Settings`.
#[test]
fn reader_ids_name_their_profiles() {
    assert_eq!(
        dump("# T\n\n$x$\n"),
        "document\n  heading value=1\n    text \"T\"\n  paragraph\n    text \"$x$\"\n"
    );
    let got = myst_dump("# T\n\n$x$\n", &Settings::default());
    assert!(got.contains("section") && got.contains("math"), "{got}");
    let settings = Settings::default();
    let mut ws = Workspace::new();
    let mut request = common::reference_request("# T\n");
    request.read.extension = Some(read_extension(&settings));
    let r = read_document(&mut ws, &READER, &request).map(|_| ());
    assert_eq!(
        r.map_err(|f| (f.status, f.code())),
        Err((guidedoc::Status::InvalidInput, "core.read.settings_none"))
    );
    let wrong = guidedoc::Limits::default();
    request.read.extension = Some(read_extension(&wrong));
    let r = read_document(&mut ws, &MYST_READER, &request).map(|_| ());
    assert_eq!(
        r.map_err(|f| (f.status, f.code())),
        Err((guidedoc::Status::InvalidInput, "core.read.settings_type"))
    );
}
