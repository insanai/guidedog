//! Tests of inline behaviour that the block reader usually exercises.

use super::*;
use crate::{Options, RoleCall, SmartQuotes, apply_host_role, resolve_with_substitutions};
use guidedoc::AttributeKey;

/// Parses `source` as one paragraph, resolves with `options`, and dumps.
fn resolved(source: &str, options: Options) -> String {
    fixture(source, false, |f| {
        f.state.options = options;
        f.para(source);
        f.finish()
    })
}

#[test]
fn smart_quotes_follow_the_language() {
    let on = Options { smart_quotes: SmartQuotes::On, ..Options::default() };
    let out = resolved("\"Hi\" -- it's 'x'... ``\"code\"``", on.clone());
    assert!(out.contains("text \"“Hi” – it’s ‘x’… \""), "{out}");
    assert!(out.contains("code \"\\\"code\\\"\""), "{out}");
    let de = Options { language: "de".into(), ..on.clone() };
    assert!(resolved("\"Hallo\"", de).contains("„Hallo“"));
    let escaped = resolved("\\\"no\\\"", on);
    assert!(escaped.contains("text \"\\\"no\\\"\""), "{escaped}");
}

#[test]
fn implicit_pep_and_rfc_references() {
    let options = Options { pep_references: true, rfc_references: true, ..Options::default() };
    let out = resolved("See PEP 8 and RFC 2822, not PEPS 8.", options);
    assert!(out.contains("dest=\"https://peps.python.org/pep-0008/\""), "{out}");
    assert!(out.contains("dest=\"https://datatracker.ietf.org/doc/html/rfc2822\""), "{out}");
    assert!(out.contains("text \", not PEPS 8.\""), "{out}");
}

#[test]
fn specialized_roles_become_their_constructs() {
    let source = ":target:`Spot` :named-reference:`spot` :uri-reference:`http://a.org` \
                  :substitution-reference:`s`";
    let out = fixture(source, false, |f| {
        f.para(source);
        f.definition("s", "spot");
        f.finish()
    });
    assert!(out.contains("inline-target +referenced \"spot\""), "{out}");
    assert!(out.contains("reference link=internal name=\"spot\" ->2"), "{out}");
    assert!(out.contains("link link=external dest=\"http://a.org\""), "{out}");
    assert!(out.contains("substitution-reference \"s\" link=internal name=\"s\""), "{out}");
}

#[test]
fn default_substitutions_come_from_the_dialect() {
    let source = "|version| |other|";
    fixture(source, false, |f| {
        f.para(source);
        let options = f.state.options.clone();
        let hook = |name: &str| (name == "version").then(|| "1.0".to_owned());
        resolve_with_substitutions(&mut f.ctx, &options, &hook).expect("resolve");
        assert!(f.dump().contains("text \"1.0\""), "{}", f.dump());
        assert_eq!(f.codes(), "rst.substitution.undefined");
    });
}

#[test]
fn role_hooks_come_first() {
    let source = ":loud:`hi there` :emphasis:`x`";
    fixture(source, false, |f| {
        let p = f.open(NodeKind::Paragraph, source);
        let lines = f.lines_of(source);
        let mut seen = Vec::new();
        let mut hook = |ctx: &mut ReadContext<'_>, call: &RoleCall<'_>| {
            seen.push(call.name.to_owned());
            if call.name != "loud" {
                return false;
            }
            let t = ctx.b.add_text(&call.text.to_uppercase()).expect("text");
            ctx.b.add_leaf(NodeKind::Text, call.span, t).is_ok()
        };
        parse_inline(&mut f.ctx, &mut f.state, &lines, &mut hook).expect("parse");
        f.close(p);
        assert_eq!(seen, ["loud", "emphasis"]);
        assert!(f.dump().contains("text \"HI THERE\""), "{}", f.dump());
    });
}

#[test]
fn host_roles_take_content_literally() {
    let source = "{code}`a\\b`";
    fixture(source, false, |f| {
        let p = f.open(NodeKind::Paragraph, source);
        let lines = f.lines_of(source);
        apply_host_role(&mut f.ctx, &mut f.state, &mut NoRoleHook, "code", &lines, 7..10)
            .expect("role");
        f.close(p);
        assert!(f.dump().contains("code \"a\\\\b\""), "{}", f.dump());
    });
}

#[test]
fn tabs_expand_to_the_source_column() {
    let source = "a\tb ``c\td``";
    let out = resolved(source, Options { tab_width: 4, ..Options::default() });
    assert!(out.contains("text \"a   b \""), "{out}");
    assert!(out.contains("code \"c   d\""), "{out}");
}

#[test]
fn bibliographic_fields_become_docinfo() {
    let source = "Author\nMe; You\nDedication\nTo all\nCustom\nx\n";
    let out = fixture(source, false, |f| {
        let list = f.open(NodeKind::FieldList, "");
        for (name, body) in [("Author", "Me; You"), ("Dedication", "To all"), ("Custom", "x")] {
            let field = f.open(NodeKind::Field, "");
            let n = f.open(NodeKind::FieldName, name);
            f.inlines(name);
            f.close(n);
            let b = f.open(NodeKind::FieldBody, "");
            f.para(body);
            f.close(b);
            f.close(field);
        }
        f.close(list);
        f.finish()
    });
    assert!(out.contains("field-list @class=\"docinfo\""), "{out}");
    assert!(out.contains("@docinfo=\"author\""), "{out}");
    assert!(out.contains("topic @class=\"dedication\""), "{out}");
    assert!(out.contains("field @class=\"custom\""), "{out}");
}

#[test]
fn section_subtitles_on_request() {
    let source = "Main\nSub\ntext\n";
    let out = fixture(source, false, |f| {
        f.state.options.sectsubtitle = true;
        let s = f.section(1, "Main");
        let s2 = f.section(2, "Sub");
        f.para("text");
        f.close(s2);
        f.close(s);
        f.finish()
    });
    assert!(out.contains("paragraph @class=\"section-subtitle\"\n      text \"Sub\""), "{out}");
    let _ = AttributeKey::Class;
}

#[test]
fn unescape_processes_marked_text() {
    let mut out = String::new();
    crate::unescape(&mut out, "a\0*b\0 c\nd");
    assert_eq!(out, "a*bc d");
}
