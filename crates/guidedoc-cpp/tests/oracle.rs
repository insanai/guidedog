//! Conformance with the Odin implementation this crate ports. `data/oracle.tsv` holds,
//! for a corpus of declarations, cross-reference targets, namespace arguments, and role
//! expressions (many from Sphinx's `test_domain_cpp`), what the Odin C++ machinery
//! gave: ids of every version, names, parameter names, `str()` and display text, and
//! symbol paths. Each line is `kind TAB text TAB => TAB fields...`.

use guidedoc_cpp::{
    Ast, Directive, Element, Limits, Node, NodeId, namespace_path, parse_declaration,
    parse_expression_or_type, parse_xref_target,
};

const ORACLE: &str = include_str!("data/oracle.tsv");

fn stop_name(e: &guidedoc_cpp::CppError) -> &'static str {
    match e.kind {
        guidedoc_cpp::ErrorKind::Limit(guidedoc_cpp::Stop::Depth) => "limit:depth",
        guidedoc_cpp::ErrorKind::Limit(guidedoc_cpp::Stop::Stack) => "limit:stack",
        guidedoc_cpp::ErrorKind::Limit(guidedoc_cpp::Stop::Work) => "limit:work",
        guidedoc_cpp::ErrorKind::NoId => "noid",
        guidedoc_cpp::ErrorKind::Invalid => "invalid",
    }
}

fn encode_elements(elements: &[Element]) -> String {
    let parts: Vec<String> = elements
        .iter()
        .map(|e| {
            let ids: Vec<&str> = (1..=4).map(|v| e.id(v).unwrap_or("-")).collect();
            format!("{}|{}|{}|{}|{}", e.ident, e.args, e.params, e.display, ids.join("|"))
        })
        .collect();
    parts.join(";")
}

fn scope_path(scope: &str) -> String {
    if scope.is_empty() {
        return String::new();
    }
    namespace_path("", scope, &Limits::default()).unwrap_or_default()
}

fn declaration_line(directive: &str, scope: &str, def: &str) -> String {
    let Some(directive) = Directive::from_name(directive) else { return "invalid".into() };
    let decl = match parse_declaration(def, directive, &Limits::default()) {
        Ok(decl) => decl,
        Err(e) => return stop_name(&e).into(),
    };
    let symbol = match decl.symbol(&scope_path(scope)) {
        Ok(symbol) => symbol,
        Err(e) => return stop_name(&e).into(),
    };
    let ids: Vec<&str> = symbol.ids.iter().map(|id| id.as_deref().unwrap_or("-")).collect();
    let mut elements = symbol.elements.clone();
    for e in &mut elements {
        e.decl.clear();
    }
    format!(
        "ok\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        ids.join("\t"),
        symbol.full_name,
        symbol.display_name,
        symbol.template_params.join(","),
        symbol.function_params.join(","),
        decl.ast.text(decl.root, false),
        decl.ast.text(decl.root, true),
        encode_elements(&elements),
        decl.ast.is_fallback_used(),
    )
}

fn xref_line(target: &str, parens: bool) -> String {
    let x = match parse_xref_target(target, parens, &Limits::default()) {
        Ok(x) => x,
        Err(e) => return stop_name(&e).into(),
    };
    let names: Vec<String> = x.names.iter().map(|n| format!("{}|{}", n.ident, n.args)).collect();
    let mut line = format!(
        "ok\t{}\t{}\t{}\t{}\t",
        x.is_shorthand,
        names.join(";"),
        x.is_rooted,
        x.template_decls.join(";")
    );
    if x.is_shorthand {
        return line;
    }
    let Some(query) = &x.query else { return line };
    line.push_str(&query.replace('\x1d', "$"));
    line.push('\t');
    line.push_str(x.own.as_deref().unwrap_or(""));
    line
}

fn ns_line(base: &str, arg: &str) -> String {
    match namespace_path(&scope_path(base), arg, &Limits::default()) {
        Ok(path) if path.is_empty() => "root".into(),
        Ok(path) => format!("ok\t{}", encode_elements(&guidedoc_cpp::decode_path(&path))),
        Err(e) => stop_name(&e).into(),
    }
}

/// The Odin node kind an expression role's root has.
fn odin_kind(ast: &Ast<'_>, id: NodeId) -> &'static str {
    match ast.node(id) {
        Node::LitPointer => "Lit_Pointer",
        Node::LitBool(_) => "Lit_Bool",
        Node::LitNumber(_) => "Lit_Number",
        Node::LitString(_) => "Lit_String",
        Node::LitChar { .. } => "Lit_Char",
        Node::LitUser { .. } => "Lit_User",
        Node::This => "This",
        Node::Fold { .. } => "Fold",
        Node::Paren(_) => "Paren_Expr",
        Node::IdExpr(_) => "Id_Expr",
        Node::Postfix { .. } => "Postfix",
        Node::ExplicitCast { .. } => "Explicit_Cast",
        Node::TypeId { .. } => "Type_Id",
        Node::Unary { .. } => "Unary",
        Node::SizeofPack(_) => "Sizeof_Pack",
        Node::SizeofType(_) => "Sizeof_Type",
        Node::SizeofExpr(_) => "Sizeof_Expr",
        Node::Alignof(_) => "Alignof",
        Node::NoexceptExpr(_) => "Noexcept_Expr",
        Node::New { .. } => "New_Expr",
        Node::Delete { .. } => "Delete_Expr",
        Node::Cast { .. } => "Cast_Expr",
        Node::BinOp { .. } => "Bin_Op",
        Node::Conditional { .. } => "Conditional",
        Node::BracedInit { .. } => "Braced_Init",
        Node::Assignment { .. } => "Assignment",
        Node::Comma(_) => "Comma",
        Node::PackExpansion(_) => "Pack_Expansion",
        Node::Type { .. } => "Type",
        _ => "?",
    }
}

fn expr_line(text: &str) -> String {
    let expr = match parse_expression_or_type(text, &Limits::default()) {
        Ok(expr) => expr,
        Err(e) => return stop_name(&e).into(),
    };
    let (ast, root) = (&expr.ast, expr.root);
    let mut line = format!(
        "ok\t{}\t{}\t{}",
        ast.text(root, false),
        ast.text(root, true),
        odin_kind(ast, root)
    );
    for v in 2..=4 {
        line.push('\t');
        line.push_str(&ast.node_id(root, v).unwrap_or_else(|| "-".into()));
    }
    line
}

fn evaluate(kind: &str, text: &str) -> String {
    match kind {
        "xref" => xref_line(text, false),
        "xref()" => xref_line(text, true),
        "expr" => expr_line(text),
        _ if kind.starts_with("ns") => ns_line(kind.get(3..).unwrap_or(""), text),
        _ => match kind.split_once('@') {
            Some((directive, scope)) => declaration_line(directive, scope, text),
            None => declaration_line(kind, "", text),
        },
    }
}

#[test]
fn every_line_matches_the_odin_implementation() {
    let mut failures = Vec::new();
    let mut count = 0;
    for line in ORACLE.lines() {
        let Some((input, expected)) = line.split_once("\t=>\t") else { continue };
        let Some((kind, text)) = input.split_once('\t') else { continue };
        count += 1;
        let got = evaluate(kind, text);
        if got != expected {
            failures.push(format!("{kind} {text}\n  expected: {expected}\n  got:      {got}"));
        }
    }
    assert!(count > 600, "the oracle has {count} lines");
    assert!(failures.is_empty(), "{} of {count} differ:\n{}", failures.len(), failures.join("\n"));
}
