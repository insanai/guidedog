use guidedoc::{
    AttributeKey as A, Builder, CORE_STACK, Conformance, Fail, LinkKind, ListInfo, ListStyle,
    Message, NodeKind as K, Severity, SourceSpan, Status, default_policy, trusted_policy,
};

use super::*;

pub const LONG: &str = "Guidedoc converts documents without calling an allocator, so embedders \
                        hand it memory once and it never asks for more.";

pub fn heading(b: &mut Builder<'_>, level: i32, s: &str) -> Result<(), Fail> {
    open_v(b, K::Heading, level)?;
    text(b, s)?;
    shut(b)
}

fn bullets() -> ListInfo {
    ListInfo { style: ListStyle::Bullet, delimiter: b'*', start: 1 }
}

#[test]
fn renderer_describes_itself() {
    let r = crate::renderer();
    assert_eq!(r.id, "text");
    assert_eq!(r.media_type, "text/plain");
    assert_eq!(r.version, crate::VERSION);
    assert_eq!(r.conformance, Conformance::Partial);
}

#[test]
fn titles_paragraphs_and_wrapping() {
    let r = render(
        |b| {
            inline(b, K::Title, "Guide")?;
            open_v(b, K::Section, 1)?;
            custom(b, "secnumber", "1")?;
            heading(b, 1, "Start")?;
            para(b, LONG)?;
            open_v(b, K::Section, 2)?;
            heading(b, 2, "Détails")?;
            open(b, K::Paragraph)?;
            text(b, "one")?;
            leaf(b, K::HardBreak, "")?;
            text(b, "two")?;
            shut(b)?;
            shut(b)?;
            shut(b)?;
            leaf(b, K::ThematicBreak, "").map(|_| ())
        },
        frag(),
    );
    assert_eq!(r.status, Status::Ok);
    let rule = "=".repeat(70);
    expect_text(
        &r,
        &format!(
            "Guide\n*****\n\n1. Start\n========\n\nGuidedoc converts documents without \
             calling an allocator, so embedders\nhand it memory once and it never asks for \
             more.\n\nDétails\n-------\n\none\ntwo\n\n{rule}\n"
        ),
    );
}

fn lists(b: &mut Builder<'_>) -> Result<(), Fail> {
    list(b, bullets(), true, &["a", "b"])?;
    let l = open(b, K::List)?;
    b.set_list(l, ListInfo { style: ListStyle::Arabic, delimiter: b'.', start: 9 })?;
    open(b, K::ListItem)?;
    para(b, "nine")?;
    list(b, ListInfo { delimiter: b'-', ..bullets() }, true, &["inner"])?;
    shut(b)?;
    open(b, K::ListItem)?;
    para(b, &format!("ten {LONG}"))?;
    shut(b)?;
    shut(b)?;
    let roman = ListInfo { style: ListStyle::LowerRoman, delimiter: b')', start: 3 };
    list(b, roman, true, &["x", "y"])?;
    let alpha = ListInfo { style: ListStyle::UpperAlpha, delimiter: b'(', start: 2 };
    list(b, alpha, true, &["p"])
}

fn definitions_and_fields(b: &mut Builder<'_>) -> Result<(), Fail> {
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
fn lists_and_definitions() {
    let r = render(
        |b| {
            lists(b)?;
            definitions_and_fields(b)
        },
        frag(),
    );
    assert_eq!(r.status, Status::Ok);
    expect_text(
        &r,
        "* a\n* b\n\n9. nine\n\n   * inner\n\n10. ten Guidedoc converts documents without \
         calling an allocator, so\n    embedders hand it memory once and it never asks for \
         more.\n\niii) x\niv) y\n\n(B) p\n\nterm : type\n   meaning\n\nAuthor:\n   Me\n\n\
         -a, --all=N\n   Everything.\n",
    );
}

/// Stands for a reader's content error, such as an unknown directive.
const UNKNOWN_CONSTRUCT: Message = Message {
    code: "test.unknown",
    severity: Severity::Error,
    status: Status::Unsupported,
    title: "UNKNOWN",
    message: "Unknown.",
    hint: "Remove it.",
};

fn kept_tree(b: &mut Builder<'_>) -> Result<(), Fail> {
    let none = guidedoc::Arg::None;
    b.report(UNKNOWN_CONSTRUCT, SourceSpan::default(), none.clone(), none);
    para(b, "kept")?;
    open(b, K::Paragraph)?;
    let x = open(b, K::Reference)?;
    set_link(b, x, LinkKind::Unresolved, guidedoc::INVALID_NODE)?;
    text(b, "missing")?;
    shut(b)?;
    shut(b)
}

#[test]
fn kept_documents_render_unresolved_references_as_text() {
    let refused = render(kept_tree, frag());
    assert_eq!(refused.status, Status::Unsupported);
    let keep = Policy { keep_documents_with_errors: true, ..trusted_policy() };
    let r = render_with(kept_tree, frag(), keep, |_| {});
    assert_eq!(r.status, Status::Ok);
    assert_eq!(r.code, "test.unknown");
    assert_eq!(r.warnings, 1);
    assert_eq!(r.text, "kept\n\nmissing\n");
}

#[test]
fn raw_content_and_references_follow_the_policy() {
    fn raw(b: &mut Builder<'_>) -> Result<(), Fail> {
        for format in ["text", "html"] {
            let id = leaf(b, K::RawBlock, "raw content\n")?;
            b.add_attribute_string(id, A::Format, format)?;
        }
        Ok(())
    }
    let refused = render_with(raw, frag(), default_policy(), |_| {});
    assert_eq!(refused.status, Status::Policy);
    assert_eq!(refused.code, "text.raw.refused");
    let allowed = render(raw, frag());
    assert_eq!(allowed.status, Status::Ok);
    assert_eq!(allowed.code, "text.raw.format");
    expect_text(&allowed, "raw content\n");
    let unresolved = render(
        |b| {
            open(b, K::Paragraph)?;
            let r = open(b, K::Reference)?;
            set_link(b, r, LinkKind::Unresolved, guidedoc::INVALID_NODE)?;
            text(b, "missing")?;
            shut(b)?;
            shut(b)
        },
        frag(),
    );
    assert_eq!(unresolved.status, Status::InvalidInput);
    assert_eq!(unresolved.code, "text.reference.unresolved");
}

const DEEP_QUOTES: usize = 200_000;

// With max_depth beyond any thread's stack, the stack guard turns a tree nested deeper
// than the stack budget holds into a Limit; rendering never overflows the stack.
#[test]
fn deep_trees_past_a_huge_depth_limit_hit_the_stack_limit() {
    let r = render_with(
        |b| {
            for _ in 0..DEEP_QUOTES {
                open(b, K::BlockQuote)?;
            }
            para(b, "deep")?;
            for _ in 0..DEEP_QUOTES {
                shut(b)?;
            }
            Ok(())
        },
        frag(),
        trusted_policy(),
        |request| request.limits.max_depth = 1_000_000,
    );
    assert_eq!(r.status, Status::Limit);
    assert_eq!(r.code, CORE_STACK.code);
}
