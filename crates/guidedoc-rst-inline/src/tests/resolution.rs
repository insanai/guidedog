use super::*;
use guidedoc::{Arg, AttributeKey, LinkKind, NodeFlag, Severity, SourceId};

#[test]
fn external_and_internal_targets() {
    fixture("Python_ and Here_ ok", false, |f| {
        f.para("Python_ and Here_ ok");
        f.target_with("python", "https://python.org", "", false);
        f.target("here");
        f.comment();
        f.target("chained");
        let p = f.open(NodeKind::Paragraph, "");
        f.close(p);
        assert_eq!(
            f.finish(),
            r#"document
  paragraph
    reference link=external dest="https://python.org" name="python"
      text "Python"
    text " and "
    reference link=internal name="here" ->9
      text "Here"
    text " ok"
  target +referenced "python" link=external dest="https://python.org"
  target +referenced "here" link=internal ->9
  comment "note"
  target "chained" link=internal ->12
  paragraph
"#
        );
        assert_eq!(f.codes(), "");
    });
}

#[test]
fn internal_target_at_the_end_points_at_itself() {
    fixture("x", false, |f| {
        f.target("last");
        assert_eq!(f.finish(), "document\n  target \"last\" link=internal ->1\n");
    });
}

#[test]
fn indirect_targets_follow_chains() {
    fixture("a_ b_", false, |f| {
        f.para("a_ b_");
        f.target_with("a", "", "b", false);
        f.target_with("b", "", "c", false);
        f.target_with("c", "https://c.org", "", false);
        let out = f.finish();
        assert_eq!(
            out,
            r#"document
  paragraph
    reference link=external dest="https://c.org" name="a"
      text "a"
    text " "
    reference link=external dest="https://c.org" name="b"
      text "b"
  target +referenced "a" link=external dest="https://c.org" name="b"
  target +referenced "b" link=external dest="https://c.org" name="c"
  target "c" link=external dest="https://c.org"
"#
        );
        assert_eq!(f.codes(), "");
    });
}

#[test]
fn indirect_target_cycles_and_dangling_names() {
    fixture("a_", false, |f| {
        f.para("a_");
        f.target_with("a", "", "b", false);
        f.target_with("b", "", "a", false);
        f.target_with("c", "", "nowhere", false);
        f.finish();
        assert_eq!(f.codes(), "rst.target.cycle,rst.target.unresolved");
    });
}

#[test]
fn unresolved_references_are_errors() {
    fixture("missing_ and `Other Name`_", false, |f| {
        f.para("missing_ and `Other Name`_");
        f.finish();
        assert_eq!(f.codes(), "rst.reference.unresolved,rst.reference.unresolved");
        let d = f.ctx.reports().items()[0].clone();
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.args[0], Arg::Span(SourceSpan::new(SourceId::MAIN, 0, 8)));
    });
}

#[test]
fn anonymous_references_pair_in_order() {
    fixture("one__ two__", false, |f| {
        f.para("one__ two__");
        f.target_with("", "https://one.org", "", true);
        f.target_with("", "", "", true);
        let p = f.open(NodeKind::Paragraph, "");
        f.close(p);
        assert_eq!(
            f.finish(),
            r#"document
  paragraph
    reference +anonymous link=external dest="https://one.org"
      text "one"
    text " "
    reference +anonymous link=internal ->9
      text "two"
  target +anonymous +referenced link=external dest="https://one.org"
  target +anonymous +referenced link=internal ->9
  paragraph
"#
        );
    });
}

#[test]
fn anonymous_mismatch_is_an_error() {
    fixture("one__ two__", false, |f| {
        f.para("one__ two__");
        f.target_with("", "https://one.org", "", true);
        f.finish();
        assert_eq!(f.codes(), "rst.reference.anonymous");
    });
}

#[test]
fn embedded_uris_define_targets() {
    fixture("`Site <https://s.org>`_ site_ `Alias <site_>`_ alias_", false, |f| {
        f.para("`Site <https://s.org>`_ site_ `Alias <site_>`_ alias_");
        assert_eq!(
            f.finish(),
            r#"document
  paragraph
    link link=external dest="https://s.org" name="site"
      text "Site"
    text " "
    reference link=external dest="https://s.org" name="site"
      text "site"
    text " "
    reference link=external dest="https://s.org" name="site" @target-name="alias"
      text "Alias"
    text " "
    reference link=external dest="https://s.org" name="alias"
      text "alias"
"#
        );
        assert_eq!(f.codes(), "");
    });
}

#[test]
fn duplicate_explicit_targets() {
    fixture("x", false, |f| {
        f.target_with("same", "https://a.org", "", false);
        f.target_with("same", "https://a.org", "", false);
        f.target_with("diff", "https://a.org", "", false);
        f.target_with("diff", "https://b.org", "", false);
        f.target("internal");
        let p = f.open(NodeKind::Paragraph, "");
        f.close(p);
        f.target("internal");
        f.finish();
        assert_eq!(f.codes(), "rst.target.repeated,rst.target.duplicate,rst.target.duplicate");
    });
}

#[test]
fn sections_are_implicit_targets() {
    fixture("Intro Text\n`intro  text`_\n", false, |f| {
        let s = f.section(1, "Intro Text");
        f.para("`intro  text`_");
        f.close(s);
        assert_eq!(
            f.finish(),
            r#"document
  section value=1
    heading value=1
      text "Intro Text"
    paragraph
      reference link=internal name="intro text" ->1
        text "intro  text"
"#
        );
    });
}

#[test]
fn explicit_names_win_over_section_titles() {
    fixture("Intro\nintro_\n", false, |f| {
        let s = f.section(1, "Intro");
        f.para("intro_");
        f.close(s);
        f.target_with("intro", "https://x.org", "", false);
        let out = f.finish();
        assert!(out.contains("reference link=external dest=\"https://x.org\""), "{out}");
        assert_eq!(f.codes(), "rst.target.implicit");
        assert_eq!(f.ctx.reports().items()[0].severity, Severity::Note);
    });
}

#[test]
fn repeated_section_titles_are_ambiguous() {
    fixture("Same\nsame_\n", false, |f| {
        let s1 = f.section(1, "Same");
        f.close(s1);
        let s2 = f.section(1, "Same");
        f.para("same_");
        f.close(s2);
        f.finish();
        assert_eq!(f.codes(), "rst.target.implicit,rst.reference.ambiguous");
    });
}

#[test]
fn names_and_inline_targets() {
    fixture("_`Spot` spot_ fig_ `spot two`_", false, |f| {
        f.para("_`Spot` spot_ fig_ `spot two`_");
        let fig = f.open(NodeKind::Figure, "");
        f.add_attribute(fig, AttributeKey::Name, "Fig");
        f.close(fig);
        let dir = f.open(NodeKind::Directive, "");
        f.add_attribute(dir, AttributeKey::Name, "header");
        f.add_custom(dir, "name", "Spot Two");
        f.close(dir);
        let out = f.finish();
        assert_eq!(
            out,
            r#"document
  paragraph
    inline-target +referenced "spot"
      text "Spot"
    text " "
    reference link=internal name="spot" ->2
      text "spot"
    text " "
    reference link=internal name="fig" ->13
      text "fig"
    text " "
    reference link=internal name="spot two" ->14
      text "spot two"
  figure @name="Fig"
  directive @name="header" @name="Spot Two"
"#
        );
        assert_eq!(f.codes(), "");
    });
}

#[test]
fn footnotes_are_numbered_and_referenced() {
    let src = "[#]_ [#note]_ [1]_ [*]_ [#]_ [*]_\none\nlabelled\nmanual\nsym\ntwo\nsym2\n";
    fixture(src, false, |f| {
        f.para("[#]_ [#note]_ [1]_ [*]_ [#]_ [*]_");
        let a = f.footnote("#", "one");
        let b = f.footnote("#note", "labelled");
        let c = f.footnote("1", "manual");
        let d = f.footnote("*", "sym");
        let e = f.footnote("#", "two");
        let g = f.footnote("*", "sym2");
        f.finish();
        assert_eq!(f.codes(), "");
        // 1 is taken by the manual footnote, so numbering starts at 2.
        let refs = [(a, 2), (b, 3), (c, 0), (d, 1), (e, 4), (g, 2)];
        for (target, value) in refs {
            assert_eq!(f.node(target).value, value);
        }
        let mut at = 2;
        for (target, value) in refs {
            while f.node(NodeId::from_index(at)).kind != NodeKind::FootnoteReference {
                at += 1;
            }
            let r = f.node(NodeId::from_index(at));
            assert_eq!(r.value, value);
            let link = f.ctx.b.snap.links[r.payload as usize];
            assert_eq!(link.kind, LinkKind::Internal);
            assert_eq!(link.resolved, target);
            assert!(f.node(target).flags.has(NodeFlag::Referenced));
            at += 1;
        }
    });
}

#[test]
fn footnote_reference_problems() {
    fixture("[#]_ [#]_ [*]_ [#nope]_ [2]_\none\n", false, |f| {
        f.para("[#]_ [#]_ [*]_ [#nope]_ [2]_");
        f.footnote("#", "one");
        f.finish();
        assert_eq!(
            f.codes(),
            concat!(
                "rst.footnote.references,rst.footnote.references,",
                "rst.reference.unresolved,rst.reference.unresolved"
            )
        );
    });
}

#[test]
fn footnote_symbols_repeat() {
    let numbers = [1, 2, 10, 11, 12];
    for (want, n) in ["*", "†", "♣", "**", "††"].into_iter().zip(numbers) {
        let mut out = String::new();
        crate::footnote_symbol(n, &mut out);
        assert_eq!(out, want);
    }
}

#[test]
fn citations_resolve_by_label() {
    fixture("[CIT2002]_ [nope]_\nbody\n", false, |f| {
        f.para("[CIT2002]_ [nope]_");
        f.citation("CIT2002", "body");
        let out = f.finish();
        assert!(
            out.contains(concat!(
                "citation-reference \"CIT2002\" link=internal ",
                "name=\"cit2002\" ->5"
            )),
            "{out}"
        );
        assert!(out.contains("citation +referenced \"CIT2002\""), "{out}");
        assert_eq!(f.codes(), "rst.reference.unresolved");
    });
}

#[test]
fn substitutions_resolve_case_sensitively_first() {
    fixture("|Name| |name| |NAME| |other|\nupper\nlower\n", false, |f| {
        f.para("|Name| |name| |NAME| |other|");
        f.definition("Name", "upper");
        f.definition("name", "lower");
        let out = f.finish();
        assert_eq!(
            out,
            r#"document
  paragraph
    substitution-reference "Name" link=internal name="name" ->9
    text " "
    substitution-reference "name" link=internal name="name" ->11
    text " "
    substitution-reference "NAME" link=internal name="name" ->11
    text " "
    substitution-reference "other" link=unresolved name="other"
  substitution-definition "Name"
    text "upper"
  substitution-definition "name"
    text "lower"
"#
        );
        assert_eq!(f.codes(), "rst.substitution.undefined");
    });
}

#[test]
fn substitution_cycles_are_errors() {
    fixture("|a|\nsee |b|\nsee |a|\nself |c|\n", false, |f| {
        f.para("|a|");
        f.definition("a", "see |b|");
        f.definition("b", "see |a|");
        f.definition("c", "self |c|");
        f.finish();
        assert_eq!(f.codes(), "rst.substitution.cycle,rst.substitution.cycle");
    });
}

#[test]
fn substitution_duplicates_keep_the_last() {
    fixture("|a|\none\ntwo\n", false, |f| {
        f.para("|a|");
        f.definition("a", "one");
        f.definition("a", "two");
        let out = f.finish();
        assert!(
            out.contains("substitution-reference \"a\" link=internal name=\"a\" ->5"),
            "{out}"
        );
        assert_eq!(f.codes(), "rst.substitution.duplicate");
    });
}

#[test]
fn substitution_nesting_is_bounded() {
    fixture("|a|\n|b|\n|c|\nend\n", false, |f| {
        f.ctx.limits.max_expansion = 2;
        f.para("|a|");
        f.definition("a", "|b|");
        f.definition("b", "|c|");
        f.definition("c", "end");
        f.finish();
        assert_eq!(f.codes(), "rst.substitution.depth");
    });
}
