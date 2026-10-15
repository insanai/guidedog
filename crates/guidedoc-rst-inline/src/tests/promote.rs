use super::*;

#[test]
fn lone_section_becomes_the_title() {
    fixture("Doc Title\nbody\nPart\n", true, |f| {
        f.comment();
        f.target("top");
        let s = f.section(1, "Doc Title");
        f.para("body");
        let s2 = f.section(2, "Part");
        f.close(s2);
        f.close(s);
        assert_eq!(
            f.finish(),
            r#"document
  title
    text "Doc Title"
  comment "note"
  target "top" link=internal ->1
  paragraph
    text "body"
  section value=1
    heading value=1
      text "Part"
"#
        );
    });
}

#[test]
fn references_to_the_title_follow_it() {
    fixture("Doc Title\n`doc title`_\n", true, |f| {
        f.target("top");
        let s = f.section(1, "Doc Title");
        f.para("`doc title`_");
        f.close(s);
        assert_eq!(
            f.finish(),
            r#"document
  title
    text "Doc Title"
  target "top" link=internal ->1
  paragraph
    reference link=internal name="doc title" ->1
      text "doc title"
"#
        );
        assert_eq!(f.codes(), "");
    });
}

#[test]
fn lone_subsection_becomes_the_subtitle() {
    fixture("Main\nSub\ntext\nDeep\n", true, |f| {
        let s = f.section(1, "Main");
        f.comment();
        let s2 = f.section(2, "Sub");
        f.para("text");
        let s3 = f.section(3, "Deep");
        f.close(s3);
        f.close(s2);
        f.close(s);
        assert_eq!(
            f.finish(),
            r#"document
  title
    text "Main"
  subtitle
    text "Sub"
  comment "note"
  paragraph
    text "text"
  section value=1
    heading value=1
      text "Deep"
"#
        );
    });
}

#[test]
fn promotion_needs_a_single_section() {
    let cases: [fn(&mut Fx<'_>); 3] = [
        |f| {
            let s = f.section(1, "A");
            f.close(s);
            let s2 = f.section(1, "B");
            f.close(s2);
        },
        |f| {
            f.para("A");
            let s = f.section(1, "B");
            f.close(s);
        },
        |f| {
            let s = f.section(1, "A");
            f.close(s);
            f.comment();
        },
    ];
    for build in cases {
        fixture("A\nB\n", true, |f| {
            build(f);
            let out = f.finish();
            assert!(!out.contains("title"), "{out}");
        });
    }
}

#[test]
fn promotion_is_optional() {
    fixture("A\n", false, |f| {
        let s = f.section(1, "A");
        f.close(s);
        assert_eq!(
            f.finish(),
            "document\n  section value=1\n    heading value=1\n      text \"A\"\n"
        );
    });
}

#[test]
fn title_and_meta_directives_do_not_block_promotion() {
    fixture("A\n", true, |f| {
        let d = f.open(NodeKind::Directive, "");
        f.add_attribute(d, guidedoc::AttributeKey::Name, "meta");
        f.close(d);
        let s = f.section(1, "A");
        f.close(s);
        assert_eq!(f.finish(), "document\n  title\n    text \"A\"\n  directive @name=\"meta\"\n");
    });
}
