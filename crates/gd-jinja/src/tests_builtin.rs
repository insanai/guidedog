//! The built-in tests: `{{ x is defined }}`, `select("odd")`, ...

use std::rc::Rc;

use crate::ast::{BinOp, CmpOp};
use crate::calls::Call;
use crate::convert::{Num, as_number, to_str, type_name, values_equal};
use crate::environment::Environment;
use crate::eval::{binary_op, compare_op, contains};
use crate::filters_text::{Builtin, Outcome};
use crate::value::{DictKind, Value};

pub(crate) fn register(env: &mut Environment) {
    let tests: &[(&str, Builtin)] = &[
        ("lower", test_case),
        ("upper", test_case),
        ("even", test_parity),
        ("odd", test_parity),
        ("divisibleby", test_divisibleby),
        ("sameas", test_sameas),
        ("in", |c| {
            Ok(Value::Bool(contains(c.state_ref(), &c.arg(1, "seq"), &c.arg(0, "value"))?))
        }),
        ("filter", |c| Ok(Value::Bool(registered(c, true)))),
        ("test", |c| Ok(Value::Bool(registered(c, false)))),
    ];
    for &(name, f) in tests {
        env.add_test(name, f);
    }
    let kinds = [
        "boolean",
        "callable",
        "defined",
        "undefined",
        "none",
        "false",
        "true",
        "float",
        "integer",
        "number",
        "string",
        "mapping",
        "sequence",
        "iterable",
        "escaped",
    ];
    for name in kinds {
        env.add_test(name, |c| Ok(Value::Bool(is_kind(&c.name, &c.arg(0, "value")))));
    }
    let comparisons = [
        ("eq", CmpOp::Eq),
        ("equalto", CmpOp::Eq),
        ("==", CmpOp::Eq),
        ("ne", CmpOp::Ne),
        ("!=", CmpOp::Ne),
        ("lt", CmpOp::Lt),
        ("lessthan", CmpOp::Lt),
        ("<", CmpOp::Lt),
        ("le", CmpOp::Le),
        ("<=", CmpOp::Le),
        ("gt", CmpOp::Gt),
        ("greaterthan", CmpOp::Gt),
        (">", CmpOp::Gt),
        ("ge", CmpOp::Ge),
        (">=", CmpOp::Ge),
    ];
    for (name, op) in comparisons {
        env.add_test(name, move |c| {
            Ok(Value::Bool(compare_op(c.state_ref(), op, &c.arg(0, "value"), &c.arg(1, "other"))?))
        });
    }
}

/// The type tests: whether v is of the kind the test called name checks.
fn is_kind(name: &str, v: &Value) -> bool {
    match name {
        "defined" => !v.is_undefined(),
        "undefined" => v.is_undefined(),
        "none" => v.is_none(),
        "boolean" => matches!(v, Value::Bool(_)),
        "true" => matches!(v, Value::Bool(true)),
        "false" => matches!(v, Value::Bool(false)),
        "integer" => matches!(v, Value::Int(_)),
        "float" => matches!(v, Value::Float(_)),
        "number" => as_number(v).is_some(),
        "string" => v.is_string(),
        "escaped" => v.is_markup(),
        "callable" => matches!(v, Value::Callable(_) | Value::Loop(_)),
        "mapping" => matches!(v, Value::Dict(d) if d.kind() == DictKind::Dict),
        "sequence" => match v {
            Value::Str(_) | Value::Markup(_) | Value::List(_) => true,
            Value::Dict(d) => d.kind() == DictKind::Dict,
            _ => false,
        },
        "iterable" => match v {
            Value::Str(_) | Value::Markup(_) | Value::List(_) | Value::Undefined(_) => true,
            Value::Dict(d) => d.kind() == DictKind::Dict,
            _ => false,
        },
        _ => false,
    }
}

fn test_case(c: &mut Call<'_, '_>) -> Outcome {
    let text = to_str(&c.arg(0, "value")).into_owned();
    let want_lower = &*c.name == "lower";
    let mut cased = text.chars().filter(|ch| ch.is_uppercase() || ch.is_lowercase()).peekable();
    let some = cased.peek().is_some();
    Ok(Value::Bool(some && cased.all(|ch| ch.is_lowercase() == want_lower)))
}

fn test_parity(c: &mut Call<'_, '_>) -> Outcome {
    let v = c.arg(0, "value");
    let even = match as_number(&v) {
        Some(Num::Int(i)) => i % 2 == 0,
        Some(Num::Float(f)) => f % 2.0 == 0.0,
        None => {
            let message =
                format!("unsupported operand type(s) for %: '{}' and 'int'", type_name(&v));
            return Err(c.error(&message));
        }
    };
    Ok(Value::Bool(even == (&*c.name == "even")))
}

fn test_divisibleby(c: &mut Call<'_, '_>) -> Outcome {
    let r = binary_op(c.state_ref(), BinOp::Mod, &c.arg(0, "value"), &c.arg(1, "num"))?;
    Ok(Value::Bool(values_equal(&r, &Value::Int(0))))
}

/// `a is sameas b`: the same object for containers and functions, else the same type
/// and value.
fn test_sameas(c: &mut Call<'_, '_>) -> Outcome {
    let (a, b) = (c.arg(0, "value"), c.arg(1, "other"));
    Ok(Value::Bool(match (&a, &b) {
        (Value::List(x), Value::List(y)) => Rc::ptr_eq(x, y),
        (Value::Dict(x), Value::Dict(y)) => Rc::ptr_eq(x, y),
        (Value::Callable(x), Value::Callable(y)) => Rc::ptr_eq(x, y),
        (Value::List(_) | Value::Dict(_) | Value::Callable(_), _) => false,
        _ => type_name(&a) == type_name(&b) && values_equal(&a, &b),
    }))
}

/// `'name' is filter` / `'name' is test`: whether one is registered.
fn registered(c: &Call<'_, '_>, filter: bool) -> bool {
    let Value::Str(name) = c.arg(0, "value") else { return false };
    let env = c.state_ref().env;
    if filter { env.filters.contains_key(&*name) } else { env.tests.contains_key(&*name) }
}
