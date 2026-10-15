use super::*;
use crate::{RoleBase, standard_role};
use guidedoc::SourceId;

#[test]
fn standard_roles() {
    expect_inline(":emphasis:`a`", "emphasis\n  text \"a\"\n");
    expect_inline(":strong:`a`", "strong\n  text \"a\"\n");
    expect_inline(":literal:`a\\*b`", "code \"a*b\"\n");
    expect_inline(":code:`a\\*b`", "code \"a\\\\*b\"\n");
    expect_inline(":math:`\\alpha^2`", "math \"\\\\alpha^2\"\n");
    expect_inline(
        ":sub:`2` :subscript:`2`",
        "subscript\n  text \"2\"\ntext \" \"\nsubscript\n  text \"2\"\n",
    );
    expect_inline(
        ":sup:`2` :superscript:`2`",
        "superscript\n  text \"2\"\ntext \" \"\nsuperscript\n  text \"2\"\n",
    );
    expect_inline(
        ":t:`A` :title:`B` :title-reference:`C`",
        concat!(
            "title-reference\n  text \"A\"\ntext \" \"\ntitle-reference\n  text \"B\"\n",
            "text \" \"\ntitle-reference\n  text \"C\"\n"
        ),
    );
    expect_inline(
        ":ab:`a` :abbreviation:`b` :ac:`c` :acronym:`d`",
        concat!(
            "abbreviation\n  text \"a\"\ntext \" \"\nabbreviation\n  text \"b\"\ntext \" \"\n",
            "abbreviation\n  text \"c\"\ntext \" \"\nabbreviation\n  text \"d\"\n"
        ),
    );
}

#[test]
fn role_names_ignore_case_and_may_follow_the_text() {
    expect_inline(":Emphasis:`a`", "emphasis\n  text \"a\"\n");
    expect_inline("`a`:STRONG:", "strong\n  text \"a\"\n");
    expect_inline("`a`:sub:.", "subscript\n  text \"a\"\ntext \".\"\n");
}

#[test]
fn roles_turn_line_breaks_into_spaces() {
    expect_inline(":emphasis:`a\nb`", "emphasis\n  text \"a b\"\n");
}

#[test]
fn default_role() {
    expect_inline("`title`", "title-reference\n  text \"title\"\n");
    assert_eq!(inline_dump("`x`", "emphasis", &[]), "emphasis\n  text \"x\"\n");
    assert_eq!(inline_dump("`x`", "missing", &[]), "text \"`x`\"\n| rst.role.unknown");
}

#[test]
fn pep_and_rfc_roles() {
    expect_inline(
        ":PEP:`8`",
        "link link=external dest=\"https://peps.python.org/pep-0008/\"\n  text \"PEP 8\"\n",
    );
    expect_inline(
        ":pep-reference:`287`",
        concat!(
            "link link=external dest=\"https://peps.python.org/pep-0287/\"\n",
            "  text \"PEP 287\"\n"
        ),
    );
    expect_inline(
        ":RFC:`2822`",
        concat!(
            "link link=external dest=\"https://datatracker.ietf.org/doc/html/rfc2822\"\n",
            "  text \"RFC 2822\"\n"
        ),
    );
    expect_inline(
        ":rfc-reference:`0822#section-3`",
        concat!(
            "link link=external dest=\"https://datatracker.ietf.org/doc/html/rfc822#section-3\"\n",
            "  text \"RFC 822\"\n"
        ),
    );
    expect_inline(":PEP:`x`", "text \":PEP:`x`\"\n| rst.role.pep");
    expect_inline(":PEP:`10000`", "text \":PEP:`10000`\"\n| rst.role.pep");
    expect_inline(":RFC:`0`", "text \":RFC:`0`\"\n| rst.role.rfc");
}

#[test]
fn raw_role_needs_a_format() {
    expect_inline(":raw:`<b>`", "text \":raw:`<b>`\"\n| rst.role.raw");
    let roles = [role("html", RoleBase::Raw, "", "html", "")];
    assert_eq!(
        inline_dump(":html:`<b>\\x</b>`", "", &roles),
        "raw-inline \"<b>\\\\x</b>\" @format=\"html\" @class=\"html\"\n"
    );
}

#[test]
fn custom_roles() {
    let roles = [
        role("custom", RoleBase::Custom, "", "", ""),
        role("red", RoleBase::Emphasis, "red text", "", ""),
        role("python", RoleBase::Code, "py", "", "python"),
        role("formula", RoleBase::Math, "", "", ""),
        role("ref", RoleBase::PepReference, "pep", "", ""),
    ];
    assert_eq!(
        inline_dump(":custom:`a`", "", &roles),
        "role \"custom\" @class=\"custom\"\n  text \"a\"\n"
    );
    assert_eq!(
        inline_dump(":RED:`a`", "", &roles),
        "emphasis @class=\"red text\"\n  text \"a\"\n"
    );
    assert_eq!(
        inline_dump(":python:`print(1)`", "", &roles),
        "code \"print(1)\" @language=\"python\" @class=\"py\"\n"
    );
    assert_eq!(inline_dump(":formula:`x`", "", &roles), "math \"x\" @class=\"formula\"\n");
    assert_eq!(
        inline_dump(":ref:`8`", "", &roles),
        concat!(
            "link link=external dest=\"https://peps.python.org/pep-0008/\" @class=\"pep\"\n",
            "  text \"PEP 8\"\n"
        )
    );
    assert_eq!(
        inline_dump("`a`", "custom", &roles),
        "role \"custom\" @class=\"custom\"\n  text \"a\"\n"
    );
}

#[test]
fn spans_point_at_the_source() {
    let source = "x *em* y\nz";
    fixture(source, false, |f| {
        f.para(source);
        f.ctx.b.finish().expect("finish");
        let want = [
            (NodeKind::Document, 0, 10),
            (NodeKind::Paragraph, 0, 10),
            (NodeKind::Text, 0, 2),
            (NodeKind::Emphasis, 2, 6),
            (NodeKind::Text, 3, 5),
            (NodeKind::Text, 6, 8),
            (NodeKind::SoftBreak, 9, 9),
            (NodeKind::Text, 9, 10),
        ];
        assert_eq!(f.ctx.b.snap.nodes.len(), want.len());
        for (i, (kind, start, end)) in want.into_iter().enumerate() {
            let node = f.node(NodeId::from_index(i));
            assert_eq!(node.kind, kind);
            assert_eq!(node.span, SourceSpan::new(SourceId::MAIN, start, end));
        }
    });
}

#[test]
fn lines_keep_their_own_offsets() {
    let source = "  *a\n  b*";
    fixture(source, false, |f| {
        let p = f.open(NodeKind::Paragraph, source);
        let lines = [Line { text: "*a", offset: 2 }, Line { text: "b*", offset: 7 }];
        parse_inline(&mut f.ctx, &mut f.state, &lines, &mut NoRoleHook).expect("parse");
        f.close(p);
        f.ctx.b.finish().expect("finish");
        let span = |i: usize| f.node(NodeId::from_index(i)).span;
        assert_eq!(span(2), SourceSpan::new(SourceId::MAIN, 2, 9));
        assert_eq!(span(4), SourceSpan::new(SourceId::MAIN, 7, 7));
        assert_eq!(span(5), SourceSpan::new(SourceId::MAIN, 7, 8));
    });
}

#[test]
fn standard_roles_are_found_by_alias() {
    assert_eq!(standard_role("PEP"), Some(RoleBase::PepReference));
    assert_eq!(standard_role("Sup"), Some(RoleBase::Superscript));
    assert_eq!(standard_role("named-reference"), Some(RoleBase::Custom));
    assert_eq!(standard_role("no-such-role"), None);
}

fn role(name: &str, base: RoleBase, classes: &str, format: &str, language: &str) -> Role {
    Role {
        name: name.into(),
        base,
        classes: classes.into(),
        format: format.into(),
        language: language.into(),
    }
}
