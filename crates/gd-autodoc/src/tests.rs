//! Unit tests of the parts with no project behind them.

use crate::names::match_signature;
use crate::options::process_options;
use crate::props::Hint;
use crate::typehints::merge_typehints;
use crate::{Config, DefaultOption, DescriptionTarget, DirectiveOption, Generator, ObjType};
use crate::{Output, Request};

#[test]
fn signatures_match_autodocs_expression() {
    let m = match_signature("mod::Class.method(a, b=1) -> int").expect("a signature");
    assert_eq!(m.explicit_module, Some("mod"));
    assert_eq!(m.path, "Class.");
    assert_eq!(m.base, "method");
    assert_eq!(m.args, Some("(a, b=1)"));
    assert_eq!(m.retann, Some("int"));
    let m = match_signature("pop(k[,d]) -> v, remove specified key").expect("a signature");
    assert_eq!(m.args, Some("(k[,d])"));
    assert_eq!(m.retann, Some("v, remove specified key"));
    assert!(match_signature("not a signature").is_none());
    assert!(match_signature("trailing.").is_none());
}

#[test]
fn options_take_defaults_and_negations() {
    let config = Config {
        autodoc_default_options: vec![
            DefaultOption { name: "members".into(), flag: true, ..DefaultOption::default() },
            DefaultOption { name: "exclude-members".into(), value: "a,b".into(), flag: false },
            DefaultOption { name: "undoc-members".into(), flag: true, ..DefaultOption::default() },
        ],
        ..Config::default()
    };
    let request = Request { directive: "autoclass".into(), ..Request::default() };
    let mut project = gd_pyscan::Project::new(&["."]).expect("grammar");
    let mut merged = std::collections::HashSet::new();
    let mut g = Generator {
        project: &mut project,
        merged: &mut merged,
        config: &config,
        options: Default::default(),
        request: &request,
        out: Output::default(),
        autodoc_module: String::new(),
        autodoc_class: String::new(),
        depth: 0,
        generated: 0,
    };
    let given =
        [DirectiveOption::new("exclude-members", "+c"), DirectiveOption::flag("no-undoc-members")];
    let o = process_options(&mut g, ObjType::Class, &given).expect("accepted");
    assert!(o.members.all);
    assert_eq!(o.exclude_members.names.join(","), "a,b,c");
    assert!(!o.undoc_members);
    assert!(
        process_options(&mut g, ObjType::Function, &[DirectiveOption::flag("members")]).is_none()
    );
    assert_eq!(g.out.messages[0].title, "AUTODOC OPTION NOT ACCEPTED");
}

fn lines(text: &[&str]) -> Vec<String> {
    text.iter().map(|s| s.to_string()).collect()
}

#[test]
fn typehints_join_the_field_list() {
    let hints = [
        Hint { name: "a".into(), text: "int".into() },
        Hint { name: "args".into(), text: "~typing.Any".into() },
        Hint { name: "return".into(), text: "str".into() },
    ];
    let mut body = lines(&[
        "Does it.",
        "",
        ":param a: The a.",
        "   More about a.",
        ":param \\*args: More.",
        "",
        "Afterwards.",
        "",
    ]);
    merge_typehints(&hints, DescriptionTarget::All, false, &mut body);
    assert_eq!(
        body.join("|"),
        "Does it.||:param a: The a.|   More about a.|:param \\*args: More.|:type a: int|\
         :type \\*args: ~typing.Any|:rtype: str||Afterwards.|"
    );
    // No field list: a new one ends the content.
    let mut body = lines(&["Just text.", ""]);
    merge_typehints(&hints, DescriptionTarget::All, false, &mut body);
    assert_eq!(
        body.join("|"),
        "Just text.||:type a: int|:param a:|:type args: ~typing.Any|:param args:|:rtype: str|"
    );
}
