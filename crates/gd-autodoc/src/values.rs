//! `sphinx.util.inspect.object_description` for the value an expression makes, computed
//! from the source: literals and containers of them, references to constants, classes,
//! functions, and enum members, and instances of classes that keep object's repr.

use std::rc::Rc;

use gd_pyscan::{ClassRef, Module, Node, NodeId, Object, ObjectKind, StringKind, UnparseMode};
use gd_pyscan::{number_repr, repr_bytes, repr_str, string_value, unparse};

use crate::Generator;

/// Bounds the nodes one description visits. Depth 24 cuts reference cycles, but
/// constants that each use the next several times (`A = (B, B)`, `B = (C, C)`, ...)
/// multiply the work at every level. Past it a node is described as written.
pub(crate) const MAX_DESCRIBE_STEPS: usize = 10_000;

const MAX_DESCRIBE_DEPTH: usize = 24;

/// One description's walk: the generator, and the nodes visited so far.
struct Describer<'g, 'a> {
    g: &'g mut Generator<'a>,
    steps: usize,
}

/// Describes the value of expression `n` of module `m`, evaluated in the body of
/// `scope` if given. The flag is false when the value is only known at run time; the
/// text is then the expression as written.
pub(crate) fn describe(
    g: &mut Generator<'_>,
    m: &Module,
    scope: Option<ClassRef>,
    n: Option<NodeId>,
) -> (String, bool) {
    let mut d = Describer { g, steps: 0 };
    d.describe(m, scope, n, 0)
}

/// The expression as written: an inexact description.
fn as_written(n: Node<'_>) -> (String, bool) {
    (unparse(n, UnparseMode::Python), false)
}

impl Describer<'_, '_> {
    fn describe(
        &mut self,
        m: &Module,
        scope: Option<ClassRef>,
        n: Option<NodeId>,
        depth: usize,
    ) -> (String, bool) {
        self.steps += 1;
        let node = m.node(n);
        if node.is_null() || depth > MAX_DESCRIBE_DEPTH {
            return (String::new(), false);
        }
        if self.steps > MAX_DESCRIBE_STEPS {
            return as_written(node);
        }
        match node.kind() {
            "string" | "concatenated_string" => match string_value(node) {
                Some((v, StringKind::Str)) => (repr_str(&v), true),
                Some((v, StringKind::Bytes)) => (repr_bytes(&v), true),
                _ => as_written(node),
            },
            "integer" | "float" => (number_repr(node.text()), true),
            "true" | "false" | "none" => (node.text().to_string(), true),
            "ellipsis" => ("Ellipsis".into(), true),
            "parenthesized_expression" => {
                self.describe(m, scope, node.named_child(0).id(), depth + 1)
            }
            "unary_operator" => self.describe_unary(m, scope, node, depth),
            "binary_operator" => match self.integer_value(m, scope, node, depth) {
                Some(v) => (v.to_string(), true),
                None => as_written(node),
            },
            "tuple" | "list" | "set" => self.describe_sequence(m, scope, node, depth),
            "dictionary" => self.describe_dict(m, scope, node, depth),
            "identifier" | "attribute" => self.describe_reference(m, scope, node, depth),
            "call" => self.describe_call(m, scope, node, depth),
            "lambda" => {
                let name = match scope {
                    Some(c) => format!("{}.<lambda>", self.g.project.class(c).qualname),
                    None => "<lambda>".into(),
                };
                (format!("<function {name}>"), true)
            }
            _ => as_written(node),
        }
    }

    fn describe_unary(
        &mut self,
        m: &Module,
        scope: Option<ClassRef>,
        n: Node<'_>,
        depth: usize,
    ) -> (String, bool) {
        if let Some(v) = self.integer_value(m, scope, n, depth) {
            return (v.to_string(), true);
        }
        let arg = n.field("argument");
        if arg.kind() == "float" && n.field_text("operator") == "-" {
            return (format!("-{}", number_repr(arg.text())), true);
        }
        as_written(n)
    }

    /// Integer arithmetic on literals and constants: `+ - * // % << **` and unary
    /// `- + ~`, wrapping as the 128-bit integers it computes in.
    fn integer_value(
        &mut self,
        m: &Module,
        scope: Option<ClassRef>,
        n: Node<'_>,
        depth: usize,
    ) -> Option<i128> {
        self.steps += 1;
        if depth > MAX_DESCRIBE_DEPTH || self.steps > MAX_DESCRIBE_STEPS {
            return None;
        }
        match n.kind() {
            "integer" => number_repr(n.text()).parse().ok(),
            "parenthesized_expression" => {
                self.integer_value(m, scope, n.named_child(0), depth + 1)
            }
            "unary_operator" => {
                let x = self.integer_value(m, scope, n.field("argument"), depth + 1)?;
                match n.field_text("operator") {
                    "-" => Some(x.wrapping_neg()),
                    "+" => Some(x),
                    "~" => Some(!x),
                    _ => None,
                }
            }
            "binary_operator" => {
                let a = self.integer_value(m, scope, n.field("left"), depth + 1)?;
                let b = self.integer_value(m, scope, n.field("right"), depth + 1)?;
                binary_integer(n.field_text("operator"), a, b)
            }
            "identifier" => {
                let o = self.g.project.lookup_name(m.id, scope, n.text());
                let (ObjectKind::Value, false, Some(owner_module)) = (o.kind, o.tuple, o.module)
                else {
                    return None;
                };
                let module = Rc::clone(self.g.project.module(owner_module));
                let value = module.node(o.value);
                self.integer_value(&module, o.owner, value, depth + 1)
            }
            _ => None,
        }
    }

    fn describe_sequence(
        &mut self,
        m: &Module,
        scope: Option<ClassRef>,
        n: Node<'_>,
        depth: usize,
    ) -> (String, bool) {
        let mut items = Vec::new();
        let mut exact = true;
        for item in n.named_children().filter(|i| i.kind() != "comment") {
            let (s, ok) = self.describe(m, scope, item.id(), depth + 1);
            exact &= ok;
            items.push(s);
        }
        let text = match n.kind() {
            "list" => format!("[{}]", items.join(", ")),
            "set" => {
                sort_described(&mut items, |s| s.as_str());
                format!("{{{}}}", items.join(", "))
            }
            _ if items.len() == 1 => format!("({},)", items[0]),
            _ => format!("({})", items.join(", ")),
        };
        (text, exact)
    }

    fn describe_dict(
        &mut self,
        m: &Module,
        scope: Option<ClassRef>,
        n: Node<'_>,
        depth: usize,
    ) -> (String, bool) {
        let mut pairs: Vec<(String, String)> = Vec::new();
        let mut exact = true;
        for item in n.named_children() {
            if item.kind() != "pair" {
                exact &= item.kind() == "comment";
                continue;
            }
            let (k, key_exact) = self.describe(m, scope, item.field("key").id(), depth + 1);
            let (v, value_exact) = self.describe(m, scope, item.field("value").id(), depth + 1);
            exact &= key_exact && value_exact;
            pairs.push((k, v));
        }
        sort_described(&mut pairs, |(k, _)| k.as_str());
        let parts: Vec<String> = pairs.iter().map(|(k, v)| format!("{k}: {v}")).collect();
        (format!("{{{}}}", parts.join(", ")), exact)
    }

    fn describe_reference(
        &mut self,
        m: &Module,
        scope: Option<ClassRef>,
        n: Node<'_>,
        depth: usize,
    ) -> (String, bool) {
        if n.kind() == "attribute"
            && let Some(object) = n.field("object").id()
        {
            // An enum member: Color.RED.
            let owner = self.g.project.reference(m, scope, object);
            if let (ObjectKind::Class, Some(c)) = (owner.kind, owner.class)
                && is_enum(self.g, c)
            {
                let class = &self.g.project.class(c).name;
                return (format!("{class}.{}", n.field_text("attribute")), true);
            }
        }
        let Some(id) = n.id() else { return as_written(n) };
        let o = self.g.project.reference(m, scope, id);
        match o.kind {
            ObjectKind::Class | ObjectKind::Function | ObjectKind::Builtin => {
                (object_name(self.g, &o), true)
            }
            ObjectKind::Value if !o.tuple => {
                let Some(owner_module) = o.module else { return as_written(n) };
                let module = Rc::clone(self.g.project.module(owner_module));
                self.describe(&module, o.owner, o.value, depth + 1)
            }
            _ => as_written(n),
        }
    }

    fn describe_call(
        &mut self,
        m: &Module,
        scope: Option<ClassRef>,
        n: Node<'_>,
        depth: usize,
    ) -> (String, bool) {
        let callee = match n.field("function").id() {
            Some(f) => self.g.project.reference(m, scope, f),
            None => Object::default(),
        };
        let args = n.field("arguments");
        let count = (args.kind() == "argument_list").then(|| args.named_child_count());
        match (callee.kind, callee.class) {
            (ObjectKind::Class, Some(c)) if keeps_object_repr(self.g, c) => {
                let module = &self.g.project.module(c.module).name;
                let qualname = &self.g.project.class(c).qualname;
                return (format!("<{module}.{qualname} object>"), true);
            }
            (ObjectKind::Builtin, _) => {
                if let Some(text) = empty_builtin(&callee.path, count) {
                    return (text.to_string(), true);
                }
                if callee.path == "builtins.frozenset" {
                    let (inner, ok) = self.describe(m, scope, args.named_child(0).id(), depth + 1);
                    if inner.starts_with(['{', '[', '(']) {
                        let items = inner.trim_matches(['{', '}', '[', ']', '(', ')']);
                        return (format!("frozenset({{{items}}})"), ok);
                    }
                }
            }
            _ => {}
        }
        as_written(n)
    }
}

/// How `repr` writes a class, function, or builtin an expression names.
fn object_name(g: &Generator<'_>, o: &Object) -> String {
    match (o.kind, o.class, o.function) {
        (ObjectKind::Class, Some(c), _) => {
            let module = &g.project.module(c.module).name;
            format!("<class '{module}.{}'>", g.project.class(c).qualname)
        }
        (ObjectKind::Function, _, Some(f)) => {
            let qualname = &g.project.module(f.module).function(f.kind).qualname;
            format!("<function {qualname}>")
        }
        _ => {
            let name = o.path.trim_start_matches("builtins.");
            if gd_pyscan::is_builtin_exception(name) || is_builtin_type(name) {
                format!("<class '{name}'>")
            } else {
                format!("<built-in function {name}>")
            }
        }
    }
}

/// The repr of a builtin container called without arguments.
fn empty_builtin(path: &str, count: Option<usize>) -> Option<&'static str> {
    if count != Some(0) {
        return None;
    }
    Some(match path {
        "builtins.object" => "<object object>",
        "builtins.dict" => "{}",
        "builtins.list" => "[]",
        "builtins.tuple" => "()",
        "builtins.set" => "set()",
        "builtins.frozenset" => "frozenset()",
        _ => return None,
    })
}

fn binary_integer(op: &str, a: i128, b: i128) -> Option<i128> {
    match op {
        "+" => Some(a.wrapping_add(b)),
        "-" => Some(a.wrapping_sub(b)),
        "*" => Some(a.wrapping_mul(b)),
        "//" if b != 0 => Some(floor_div(a, b)),
        "%" if b != 0 => Some(a.wrapping_sub(b.wrapping_mul(floor_div(a, b)))),
        "<<" if (0..64).contains(&b) => Some(a.wrapping_shl(b as u32)),
        "**" if (0..=64).contains(&b) => Some((0..b).fold(1i128, |r, _| r.wrapping_mul(a))),
        _ => None,
    }
}

/// Python's `//`: the quotient rounded toward negative infinity.
fn floor_div(a: i128, b: i128) -> i128 {
    let q = a.wrapping_div(b);
    if a.wrapping_rem(b) != 0 && ((a < 0) != (b < 0)) { q - 1 } else { q }
}

/// Sorts described set items or dict keys as Python sorts their values: numbers by
/// value when both are numbers, else by text.
fn sort_described<T>(items: &mut [T], key: impl Fn(&T) -> &str) {
    items.sort_by(|a, b| {
        let (x, y) = (key(a), key(b));
        match (x.parse::<f64>(), y.parse::<f64>()) {
            (Ok(x), Ok(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
            _ => x.cmp(y),
        }
    });
}

/// Classes of `builtins` whose repr is `<class 'name'>`.
pub(crate) fn is_builtin_type(name: &str) -> bool {
    matches!(
        name,
        "object"
            | "type"
            | "str"
            | "bytes"
            | "bytearray"
            | "int"
            | "float"
            | "complex"
            | "bool"
            | "list"
            | "tuple"
            | "dict"
            | "set"
            | "frozenset"
            | "memoryview"
            | "range"
            | "slice"
            | "property"
            | "staticmethod"
            | "classmethod"
            | "super"
    )
}

/// Whether a class derives from one of enum's classes.
pub(crate) fn is_enum(g: &mut Generator<'_>, c: ClassRef) -> bool {
    let (order, _) = g.project.mro(c);
    order.iter().any(|e| e.path.starts_with("enum."))
}

/// Whether instances of a class in the roots print as `<module.Class object>`: no class
/// along its MRO defines `__repr__`, and every base is known.
fn keeps_object_repr(g: &mut Generator<'_>, c: ClassRef) -> bool {
    let (order, complete) = g.project.mro(c);
    complete
        && order
            .iter()
            .filter_map(|e| e.class)
            .all(|class| g.project.class(class).scope.find_def("__repr__").is_none())
}

/// The class an assignment instantiates, when its value is a call of one.
pub(crate) fn call_of_class(g: &mut Generator<'_>, o: &Object) -> Option<ClassRef> {
    if o.kind != ObjectKind::Value {
        return None;
    }
    let m = Rc::clone(g.project.module(o.module?));
    let value = m.node(o.value);
    if value.kind() != "call" {
        return None;
    }
    let callee = g.project.reference(&m, o.owner, value.field("function").id()?);
    callee.class.filter(|_| callee.kind == ObjectKind::Class)
}
