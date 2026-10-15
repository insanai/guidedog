//! The one-call API as the Sphinx layer uses it, checked against the ids Sphinx 9.1
//! writes for its `test-domain-cpp`.

use guidedoc_cpp::{
    Directive, ErrorKind, Limits, Node, ObjectType, Symbol, namespace_path, parse_declaration,
    parse_expression_or_type, parse_xref_target,
};

fn symbol(directive: Directive, scope: &str, def: &str) -> Symbol {
    let decl = parse_declaration(def, directive, &Limits::default());
    let symbol = decl.and_then(|d| d.symbol(scope));
    assert!(symbol.is_ok(), "{def}: {symbol:?}");
    symbol.unwrap_or_default()
}

fn ids(s: &Symbol) -> Vec<&str> {
    s.ids.iter().rev().filter_map(|id| id.as_deref()).collect()
}

#[test]
fn declarations_and_ids_in_nested_scopes() {
    let foo = symbol(Directive::Class, "", "Foo : public Bar");
    assert_eq!(ids(&foo), ["_CPPv43Foo", "_CPPv33Foo", "_CPPv23Foo", "Foo"]);
    assert_eq!(foo.full_name, "Foo");
    let f = symbol(Directive::Function, &foo.path, "void f(int i) const");
    assert_eq!(f.newest_id(), "_CPPv4NK3Foo1fEi");
    assert_eq!(f.ids[0].as_deref(), Some("Foo::f__iC"));
    assert_eq!(f.full_name, "Foo::f");
    assert_eq!(f.function_params, ["i"]);
    let name = symbol(Directive::Member, &foo.path, "std::string name");
    assert_eq!(name.ids[0].as_deref(), Some("Foo::name__ss"));
    let ns = namespace_path("", "ns", &Limits::default()).unwrap_or_default();
    let get = symbol(Directive::Function, &ns, "const std::string &get(double d, char *p)");
    assert_eq!(get.ids[0].as_deref(), Some("ns::get__double.cP"));
    assert_eq!(get.full_name, "ns::get");
    let apply = symbol(Directive::Function, &ns, "template<typename T> void apply(T t)");
    assert_eq!(&ids(&apply)[..2], ["_CPPv4I0EN2ns5applyEv1T", "_CPPv3I0EN2ns5applyE1T"]);
    assert_eq!(
        (apply.template_params, apply.function_params),
        (vec!["T".into()], vec!["t".into()])
    );
    assert_eq!(symbol(Directive::Function, "", "void none()").newest_id(), "_CPPv44nonev");
}

#[test]
fn ids_of_every_version() {
    let cases = [
        (
            Directive::Function,
            "",
            "void paren_5::operator()(int)",
            vec![
                "_CPPv4N7paren_5clEi",
                "_CPPv3N7paren_5clEi",
                "_CPPv2N7paren_5clEi",
                "paren_5::call-operator__i",
            ],
        ),
        (
            Directive::Function,
            "",
            "int operator+(int, int)",
            vec!["_CPPv4plii", "_CPPv3plii", "_CPPv2plii", "add-operator__i.i"],
        ),
        (
            Directive::Function,
            "",
            "int operator\"\"_lit()",
            vec!["_CPPv4li4_litv", "_CPPv3li4_litv", "_CPPv2li4_litv"],
        ),
        (Directive::Var, "", "char c = '\\\\'", vec!["_CPPv41c", "_CPPv31c", "_CPPv21c", "c__c"]),
        (
            Directive::EnumStruct,
            "",
            "protected E : std::underlying_type<S>::type",
            vec!["_CPPv41E", "_CPPv31E", "_CPPv21E"],
        ),
    ];
    for (directive, scope, def, expected) in cases {
        assert_eq!(ids(&symbol(directive, scope, def)), expected, "{def}");
    }
    let b = symbol(Directive::Class, "", "B");
    let cast = symbol(Directive::Function, &b.path, "operator int()");
    assert_eq!(
        ids(&cast),
        ["_CPPv4N1BcviEv", "_CPPv3N1BcviEv", "_CPPv2N1BcviEv", "B::castto-i-operator"]
    );
    let ns = namespace_path("", "ns", &Limits::default()).unwrap_or_default();
    let anon = symbol(Directive::Class, &ns, "@a::A");
    assert_eq!(ids(&anon), ["_CPPv4N2nsUt1_a1AE", "_CPPv3N2nsUt1_a1AE"]);
    assert_eq!(
        (anon.full_name.as_str(), anon.display_name.as_str()),
        ("ns::@a::A", "ns::[anonymous]::A")
    );
    let c = symbol(
        Directive::Class,
        &ns,
        "template<typename T, int V, template<typename> typename U> C",
    );
    assert_eq!(
        ids(&c),
        ["_CPPv4I0_iI0E0EN2ns1CE", "_CPPv3I0_iI0E0EN2ns1CE", "_CPPv2I0_iI0E0EN2ns1CE"]
    );
    assert_eq!(c.template_params, ["T", "V", "U"]);
}

#[test]
fn the_last_path_element_records_its_declaration() {
    let ns = namespace_path("", "ns", &Limits::default()).unwrap_or_default();
    let c = symbol(Directive::Class, &ns, "C");
    assert_eq!(c.elements.len(), 2);
    assert_eq!(c.elements[1].decl, "_CPPv4N2ns1CE");
    assert_eq!(guidedoc_cpp::decode_path(&c.path), c.elements);
    assert_eq!(namespace_path(&c.path, "nullptr", &Limits::default()).as_deref(), Ok(""));
}

#[test]
fn invalid_declarations_say_so() {
    for (directive, def) in
        [(Directive::Function, "void f("), (Directive::Member, "int"), (Directive::Class, "")]
    {
        let error = parse_declaration(def, directive, &Limits::default()).err();
        assert_eq!(error.map(|e| e.kind), Some(ErrorKind::Invalid), "{def}");
    }
    // The operator's text keeps its space, which no id table knows.
    let decl = parse_declaration("int operator [ ](int)", Directive::Function, &Limits::default());
    let error = decl.and_then(|d| d.symbol("")).err();
    assert_eq!(error.map(|e| e.code), Some("cpp.no-id"));
}

#[test]
fn spans_point_into_the_untrimmed_text() {
    let error = parse_declaration("  void f(  ", Directive::Function, &Limits::default()).err();
    assert_eq!(error.map(|e| e.span), Some(2..9));
}

#[test]
fn xref_targets_for_the_host() {
    let x = parse_xref_target("A<int>::f", true, &Limits::default()).unwrap_or_default();
    assert!(x.is_shorthand && !x.is_rooted);
    assert_eq!(x.encoded_names(), "A\x1f<int>\nf\x1f");
    let x =
        parse_xref_target("void A::f(int) const", false, &Limits::default()).unwrap_or_default();
    assert!(!x.is_shorthand);
    assert_eq!(x.query.as_deref(), Some("_CPPv4\x1dK\x1di"));
    assert_eq!(x.own.as_deref(), Some("1f"));
    let x = parse_xref_target("template<typename T> A<T>::f", false, &Limits::default())
        .unwrap_or_default();
    assert_eq!(x.encoded_template_decls(), "template<typename T> ");
    let error = parse_xref_target("A B C", false, &Limits::default()).err();
    assert_eq!(error.map(|e| e.kind), Some(ErrorKind::Invalid));
}

#[test]
fn expressions_and_types_for_roles() {
    let e = parse_expression_or_type("sizeof(item) + 1", &Limits::default());
    let e = e
        .as_ref()
        .map(|e| (e.ast.text(e.root, false), matches!(e.ast.node(e.root), Node::BinOp { .. })));
    assert_eq!(e, Ok(("sizeof(item) + 1".to_string(), true)));
    let t = parse_expression_or_type("const item &", &Limits::default());
    assert!(t.is_ok_and(|t| matches!(t.ast.node(t.root), Node::Type { .. })));
    let fallback = parse_expression_or_type("@#$", &Limits::default()).err();
    assert_eq!(fallback.map(|e| e.kind), Some(ErrorKind::Invalid));
}

#[test]
fn the_tree_answers_what_a_describer_asks() {
    let decl = parse_declaration(
        "template<typename T> int *ns::f(T x) const",
        Directive::Function,
        &Limits::default(),
    );
    let Ok(decl) = decl else { panic!("does not parse") };
    assert_eq!(decl.object_type(), ObjectType::Function);
    assert_eq!(decl.directive().name(), "function");
    let ast = &decl.ast;
    let name = decl.name().map(|n| ast.text(n, false));
    assert_eq!(name.as_deref(), Some("ns::f"));
    assert_eq!(ast.count(decl.template_lists()), 1);
    let Node::Declaration { decl: ty, .. } = *ast.node(decl.root) else {
        panic!("no declaration")
    };
    let Node::Type { decl: d, .. } = *ast.node(ty) else { panic!("no type") };
    assert!(ast.is_function_type(d) && ast.space_after_specs(d));
    let params = ast.function_params(d).map(|q| ast.node(q).clone());
    assert!(matches!(params, Some(Node::ParamQual(q)) if q.is_const));
}
