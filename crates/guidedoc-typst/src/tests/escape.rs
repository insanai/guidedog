//! Hostile text survives a round trip through Typst unchanged.

use guidedoc::{ListInfo, ListStyle, NodeKind as K, Status};

use super::{Tb, expect_compiles, refusing, render_policy};
use crate::{is_label_safe, write_markup_text, write_string_literal};

type R = Result<(), guidedoc::Fail>;

/// Text that would be Typst markup or code if it were not escaped.
const HOSTILE: [&str; 35] = [
    "//not a comment",
    "/* not a block comment */",
    "#let x = 1",
    "$x^2$",
    "<label>",
    "@ref",
    "\\",
    "\\\\",
    "\\u{41}",
    "= Heading",
    "- item",
    "+ item",
    "1. item",
    "/ term: description",
    "*strong*",
    "_emph_",
    "`raw`",
    "[link]",
    "#set text(red)",
    "~tilde~",
    "--- -- -?",
    "...",
    "'single' \"double\"",
    "https://example.com/path",
    "trailing\\",
    "{}",
    "#{1 + 1}",
    "\\#",
    "<script>alert(1)</script>",
    "a<b>c",
    "#import \"x.typ\"",
    "$ display $",
    "#[content]",
    "x];#panic()[",
    "\")#panic(\"",
];

/// Every printable ASCII character, space to tilde.
fn ascii_line() -> String {
    (0x20u8..0x7f).map(char::from).collect()
}

fn build_hostile(t: &mut Tb<'_, '_>) -> R {
    t.wrap(K::Title, "Title #let $x$ <l> @r \\")?;
    let section = t.section(1, "= #heading $x$ // <h>")?;
    let p = t.open(K::Paragraph, 0)?;
    t.text("ASCII:")?;
    t.text(&ascii_line())?;
    t.close(p)?;
    for h in HOSTILE {
        t.para(h)?;
    }
    let p = t.open(K::Paragraph, 0)?;
    for h in HOSTILE {
        t.leaf(K::SoftBreak, "", 0)?;
        t.text(h)?;
    }
    t.close(p)?;
    t.para("line\n= not a heading\n- not a list\n// not a comment")?;
    let p = t.open(K::Paragraph, 0)?;
    for h in HOSTILE {
        t.wrap(K::Emphasis, h)?;
        t.leaf(K::Code, h, 0)?;
    }
    t.close(p)?;
    let list = t.open(K::List, 0)?;
    t.b.set_list(list, ListInfo { style: ListStyle::Bullet, delimiter: b'-', start: 1 })?;
    let item = t.open(K::ListItem, 0)?;
    t.para("#let item = 1 // <x>")?;
    t.close(item)?;
    t.close(list)?;
    t.leaf(K::CodeBlock, "#let code = \"x\" // \\ \")#panic(\"\n", 0)?;
    t.close(section)
}

/// The text of an HTML page: tags become line breaks, entities are decoded.
fn html_text(html: &str) -> String {
    let body = html.find("<body>").map_or(html, |i| &html[i..]);
    let mut text = String::new();
    let mut in_tag = false;
    for c in body.chars() {
        match (in_tag, c) {
            (true, '>') => in_tag = false,
            (true, _) => {}
            (false, '<') => {
                in_tag = true;
                text.push('\n');
            }
            (false, c) => text.push(c),
        }
    }
    let pairs = [
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
        ("&#x27;", "'"),
        ("&nbsp;", " "),
        ("\u{a0}", " "),
        ("&amp;", "&"),
    ];
    pairs.iter().fold(text, |s, (from, to)| s.replace(from, to))
}

/// Every hostile string survives a round trip through Typst unchanged, in plain text,
/// after a line break, inside markup, and as code.
#[test]
fn text_escaping_round_trips_through_typst() {
    let r = render_policy(build_hostile, refusing());
    assert_eq!(r.status, Status::Ok, "{:?}", r.reports);
    let Some(html) = expect_compiles("hostile", &r.output, true) else {
        return;
    };
    let text = html_text(&html);
    for part in [
        format!("ASCII:{}", ascii_line()).as_str(),
        "Title #let $x$ <l> @r \\",
        "= #heading $x$ // <h>",
        "line = not a heading - not a list // not a comment",
        "#let item = 1 // <x>",
        "#let code = \"x\" // \\ \")#panic(\"",
    ] {
        assert!(text.contains(part), "{part:?} lost in round trip");
    }
    for h in HOSTILE {
        assert!(text.matches(h).count() >= 3, "{h:?} lost in round trip");
    }
}

#[test]
fn string_literals_escape_everything() {
    let mut out = String::new();
    write_string_literal(&mut out, "a\"b\\c\nd\te\x01f\x7f");
    assert_eq!(out, "\"a\\\"b\\\\c\\nd\\te\\u{01}f\\u{7f}\"");
}

#[test]
fn markup_text_escapes_punctuation() {
    let mut out = String::new();
    write_markup_text(&mut out, "a #b\n\tc\x01é*");
    assert_eq!(out, "a \\#b  cé\\*");
    assert!(is_label_safe("mod.func:x_y-1"));
    assert!(!is_label_safe("a b") && !is_label_safe("") && !is_label_safe("a>"));
}
