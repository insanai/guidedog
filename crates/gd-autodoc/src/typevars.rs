//! TypeVars and NewTypes are documented as classes (autoclass, or a module member with
//! documentation): no signature, no bases, and `alias of TypeVar('T', bound=...)` or
//! `alias of <supertype>` after the documentation.

use std::sync::Arc;

use gd_pyscan::{Module, Node, Object, ObjectKind, repr_str, str_value};

use crate::Generator;
use crate::hints::{note_approximate, render_mode};
use crate::props::Props;
use crate::types::{Eval, eval_type, restify, stringify};

/// Whether a value is a TypeVar or NewType the source makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum AliasKind {
    #[default]
    None,
    TypeVar,
    NewType,
}

/// Whether an object is a TypeVar or NewType the source makes: a call of typing's (or
/// typing_extensions') `TypeVar` or `NewType`.
pub(crate) fn alias_kind(g: &mut Generator<'_>, o: &Object) -> AliasKind {
    let (ObjectKind::Value, false, Some(m)) = (o.kind, o.tuple, o.module) else {
        return AliasKind::None;
    };
    let m = Arc::clone(g.project.module(m));
    let value = m.node(o.value);
    let Some(function) = value.field("function").id().filter(|_| value.kind() == "call") else {
        return AliasKind::None;
    };
    let callee = g.project.reference(&m, o.owner, function);
    if callee.kind != ObjectKind::External {
        return AliasKind::None;
    }
    match callee.path.as_str() {
        "typing.TypeVar" | "typing_extensions.TypeVar" => AliasKind::TypeVar,
        "typing.NewType" | "typing_extensions.NewType" => AliasKind::NewType,
        _ => AliasKind::None,
    }
}

/// A call's positional arguments and keyword arguments.
pub(crate) fn call_arguments(call: Node<'_>) -> (Vec<Node<'_>>, Vec<Node<'_>>) {
    let mut positional = Vec::new();
    let mut keywords = Vec::new();
    for a in call.field("arguments").named_children() {
        match a.kind() {
            "keyword_argument" => keywords.push(a),
            "comment" | "list_splat" | "dictionary_splat" => {}
            _ => positional.push(a),
        }
    }
    (positional, keywords)
}

/// Fills a TypeVar's or NewType's properties: its name, its module, and the alias line.
pub(crate) fn typevar_props(g: &mut Generator<'_>, p: &mut Props, kind: AliasKind) {
    let Some(m) = p.obj.module else { return };
    let m = Arc::clone(g.project.module(m));
    let (positional, keywords) = call_arguments(m.node(p.obj.value));
    if let Some(first) = positional.first() {
        p.obj_name = str_value(*first).map(|s| s.into_owned()).unwrap_or_default();
    }
    p.obj_module = m.name.clone();
    p.alias_kind = kind;
    let mut e = Eval::default();
    if kind == AliasKind::NewType {
        if let Some(supertype) = positional.get(1)
            && let Some(v) = eval_type(g, &mut e, &m, supertype.id())
        {
            p.alias_text = restify(&v, render_mode(g));
        }
        return;
    }
    let mut parts = vec![repr_str(&p.obj_name)];
    for c in positional.iter().skip(1) {
        if let Some(v) = eval_type(g, &mut e, &m, c.id()) {
            parts.push(stringify(&v, render_mode(g)));
        }
    }
    for k in &keywords {
        if let Some(part) = typevar_keyword(g, &mut e, &m, *k) {
            parts.push(part);
        }
    }
    note_approximate(g, &e);
    p.alias_text = format!("TypeVar({})", parts.join(", "));
}

/// How one keyword argument of a TypeVar call shows in its alias line.
fn typevar_keyword(
    g: &mut Generator<'_>,
    e: &mut Eval,
    m: &Module,
    k: Node<'_>,
) -> Option<String> {
    let value = k.field("value");
    match k.field_text("name") {
        "bound" => {
            let v = eval_type(g, e, m, value.id())?;
            Some(format!("bound=\\ {}", restify(&v, render_mode(g))))
        }
        "covariant" if value.kind() == "true" => Some("covariant=True".into()),
        "contravariant" if value.kind() == "true" => Some("contravariant=True".into()),
        _ => None,
    }
}
