use super::*;
use crate::chars::{GeneralPunctuation, PunctClass, general_punctuation, is_space, punct_class};

#[test]
fn punctuation_categories_come_from_unicode_data() {
    use GeneralPunctuation as G;
    assert_eq!(general_punctuation('('), Some(G::Ps));
    assert_eq!(general_punctuation('\u{FF09}'), Some(G::Pe));
    assert_eq!(general_punctuation('\u{00AB}'), Some(G::Pi));
    assert_eq!(general_punctuation('\u{00BB}'), Some(G::Pf));
    assert_eq!(general_punctuation('\u{2014}'), Some(G::Pd));
    assert_eq!(general_punctuation('\u{3001}'), Some(G::Po));
    assert_eq!(general_punctuation('\u{203F}'), Some(G::Pc));
    assert_eq!(general_punctuation('a'), None);
    assert_eq!(general_punctuation('\u{10FFFF}'), None);
    // Markup after an opening bracket and before a closing one of any script.
    assert_eq!(punct_class('\u{FF08}'), PunctClass::Open);
    assert_eq!(punct_class('\u{2E42}'), PunctClass::Open); // low double quote, Ps
    assert_eq!(punct_class('\u{203F}'), PunctClass::None);
}

#[test]
fn white_space_is_pythons_isspace() {
    let spaces = [
        ' ', '\t', '\r', '\u{1C}', '\u{1F}', '\u{85}', '\u{A0}', '\u{1680}', '\u{200A}',
        '\u{2029}', '\u{3000}',
    ];
    for r in spaces {
        assert!(is_space(r), "white space {r:?}");
    }
    for r in ['\0', 'a', '\u{1B}', '\u{180E}', '\u{200B}', '\u{200E}', '\u{2060}', '\u{FEFF}'] {
        assert!(!is_space(r), "not white space {r:?}");
    }
    // A zero-width space is not white space, so it neither ends nor starts markup
    // (Docutils: the literal ends at the closing ``, and x \u{200b}*a* is text).
    expect_inline(
        "backquote (``\u{200b}`\u{200b}``)",
        "text \"backquote (\"\ncode \"\u{200b}`\u{200b}\"\ntext \")\"\n",
    );
    expect_inline("x \u{200b}*a*\u{200b} y", "text \"x \u{200b}*a*\u{200b} y\"\n");
    expect_inline("\x1c*b*\x1c", "text \"\x1c\"\nemphasis\n  text \"b\"\ntext \"\x1c\"\n");
}
