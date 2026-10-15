//! Expressions: names, attributes, items, slices and operators.

use std::rc::Rc;

use crate::ast::{BinOp, CmpOp, Expr, ExprKind, UnaryOp};
use crate::calls::{apply_filter, apply_test, eval_call};
use crate::convert::{
    Num, as_int, as_number, char_count, char_slice, compare_values, object_name, to_str,
    type_name, values_equal,
};
use crate::environment::UndefinedMode;
use crate::error::{ErrorKind, Hinted};
use crate::escape::{escaped_text, markup_concat};
use crate::runtime::{Result, ScopeId, State, condition};
use crate::value::{CallKind, Callable, Dict, DictKind, List, Value};

/// Resolves a variable through the scopes, the host context and the globals.
pub(crate) fn lookup(s: &State<'_>, scope: ScopeId, name: &str) -> Value {
    if let Some(v) = s.find_var(scope, name) {
        return v;
    }
    if s.locals.is_some_and(|l| l.function(name).is_some()) {
        return s.render_callable(Rc::from(name), CallKind::Local);
    }
    if let Some(v) = s.env.globals.get(name) {
        return v.clone();
    }
    if name == "self" {
        return crate::inherit::self_reference(s);
    }
    Value::undefined(name)
}

/// Evaluates an expression. It is on the stack once per level of every nested
/// expression, macro call and include, so it only dispatches: each kind is evaluated by
/// a helper of its own, keeping this frame small in unoptimized builds too.
pub(crate) fn eval(s: &mut State<'_>, scope: ScopeId, e: &Expr) -> Result<Value> {
    s.line = e.line;
    s.check_stack()?;
    match &e.kind {
        ExprKind::Const(v) => Ok(v.clone()),
        ExprKind::Name(name) => Ok(lookup(s, scope, name)),
        ExprKind::List(items) => eval_sequence(s, scope, items, false),
        ExprKind::Tuple(items) => eval_sequence(s, scope, items, true),
        ExprKind::Dict(pairs) => eval_dict(s, scope, pairs),
        ExprKind::Getattr(obj, name) => eval_getattr(s, scope, obj, name),
        ExprKind::Getitem(obj, key) => eval_getitem(s, scope, obj, key),
        ExprKind::Slice(..) | ExprKind::NsRef(..) => Err(misplaced(s)),
        ExprKind::Call(callee, args) => eval_call(s, scope, e.line, callee, args),
        ExprKind::Filter(..) => apply_filter(s, scope, e, None),
        ExprKind::Test(..) => apply_test(s, scope, e),
        ExprKind::Unary(op, operand) => eval_unary(s, scope, *op, operand),
        ExprKind::Binary(op, left, right) => eval_binary(s, scope, *op, left, right),
        ExprKind::Concat(items) => eval_concat(s, scope, items),
        ExprKind::Compare(first, rest) => eval_compare(s, scope, first, rest),
        ExprKind::Cond { test, then, otherwise } => {
            eval_cond(s, scope, e.line, test, then, otherwise.as_deref())
        }
    }
}

/// A slice or assignment target outside its place; the parser never builds one.
#[inline(never)]
fn misplaced(s: &State<'_>) -> Box<crate::Error> {
    s.error(ErrorKind::Runtime, "cannot evaluate a slice or assignment target here")
}

#[inline(never)]
fn eval_sequence(s: &mut State<'_>, scope: ScopeId, items: &[Expr], tuple: bool) -> Result<Value> {
    Ok(Value::List(List::new(eval_all(s, scope, items)?, tuple)))
}

#[inline(never)]
fn eval_dict(s: &mut State<'_>, scope: ScopeId, pairs: &[(Expr, Expr)]) -> Result<Value> {
    let d = Dict::new(DictKind::Dict);
    for (key, value) in pairs {
        let k = eval(s, scope, key)?;
        d.set(k, eval(s, scope, value)?);
    }
    Ok(Value::Dict(d))
}

#[inline(never)]
fn eval_getattr(s: &mut State<'_>, scope: ScopeId, obj: &Expr, name: &str) -> Result<Value> {
    let v = eval(s, scope, obj)?;
    get_attr(s, &v, name)
}

#[inline(never)]
fn eval_unary(s: &mut State<'_>, scope: ScopeId, op: UnaryOp, operand: &Expr) -> Result<Value> {
    let v = eval(s, scope, operand)?;
    unary_op(s, op, &v)
}

#[inline(never)]
fn eval_cond(
    s: &mut State<'_>,
    scope: ScopeId,
    line: usize,
    test: &Expr,
    then: &Expr,
    otherwise: Option<&Expr>,
) -> Result<Value> {
    if condition(s, scope, test)? {
        return eval(s, scope, then);
    }
    match otherwise {
        Some(other) => eval(s, scope, other),
        None => Ok(Value::undefined_hint(&format!(
            "the inline if-expression on line {line} evaluated to false and no else section \
             was defined."
        ))),
    }
}

fn eval_all(s: &mut State<'_>, scope: ScopeId, items: &[Expr]) -> Result<Vec<Value>> {
    items.iter().map(|item| eval(s, scope, item)).collect()
}

#[inline(never)]
fn eval_getitem(s: &mut State<'_>, scope: ScopeId, obj: &Expr, key: &Expr) -> Result<Value> {
    let v = eval(s, scope, obj)?;
    let ExprKind::Slice(start, stop, step) = &key.kind else {
        let k = eval(s, scope, key)?;
        return get_item(s, &v, &k);
    };
    let mut bounds = [Value::None, Value::None, Value::None];
    for (bound, part) in bounds.iter_mut().zip([start, stop, step]) {
        if let Some(part) = part {
            *bound = eval(s, scope, part)?;
        }
    }
    if v.is_undefined() && s.env.options.undefined != UndefinedMode::Chainable {
        return Err(s.undefined_error(&v));
    }
    slice_value(s, &v, &bounds)
}

// ---------------------------------------------------------------------------------------
// Attribute and item access

/// Jinja's environment.getattr: attribute first, then item.
pub(crate) fn get_attr(s: &State<'_>, obj: &Value, name: &str) -> Result<Value> {
    if obj.is_undefined() {
        return chain_undefined(s, obj);
    }
    let found = attribute_of(obj, name).or_else(|| item_of(obj, &Value::str(name)));
    Ok(found.unwrap_or_else(|| Value::undefined_with(name, &object_name(obj), "")))
}

/// Jinja's environment.getitem: item first, then attribute for string keys.
pub(crate) fn get_item(s: &State<'_>, obj: &Value, key: &Value) -> Result<Value> {
    if obj.is_undefined() {
        return chain_undefined(s, obj);
    }
    if let Some(v) = item_of(obj, key) {
        return Ok(v);
    }
    if let Value::Str(name) = key
        && let Some(v) = attribute_of(obj, name)
    {
        return Ok(v);
    }
    Ok(Value::undefined_with(&to_str(key), &object_name(obj), ""))
}

/// Reading from an undefined value: an error, unless undefined values chain.
fn chain_undefined(s: &State<'_>, obj: &Value) -> Result<Value> {
    if s.env.options.undefined == UndefinedMode::Chainable {
        return Ok(obj.clone());
    }
    Err(s.undefined_error(obj))
}

/// Python attributes: methods of builtin types and object fields.
pub(crate) fn attribute_of(obj: &Value, name: &str) -> Option<Value> {
    use crate::methods::{is_dict_method, is_list_method, is_string_method};
    let method = || Some(bound_method(obj, name));
    match obj {
        Value::Dict(d) if d.kind() != DictKind::Dict => d.get_str(name),
        Value::Dict(_) if is_dict_method(name) => method(),
        Value::List(l) => match l.fields.iter().position(|f| *f == name) {
            Some(i) => l.item(i),
            None if is_list_method(name, l.is_tuple()) => method(),
            None => None,
        },
        Value::Str(_) | Value::Markup(_) if is_string_method(name, obj.is_markup()) => method(),
        Value::Loop(l) => crate::loops::loop_attribute(l, name),
        Value::Cycler(c) => match name {
            "current" => Some(c.items.get(c.pos.get()).cloned().unwrap_or(Value::None)),
            "next" | "reset" => method(),
            _ => None,
        },
        Value::Callable(c) => crate::calls::callable_attribute(c, name),
        Value::Int(_) | Value::Float(_) | Value::Bool(_) => match name {
            "real" => Some(obj.clone()),
            "imag" => Some(Value::Int(0)),
            "is_integer" | "conjugate" | "bit_length" => method(),
            _ => None,
        },
        _ => None,
    }
}

/// `obj[key]` for dicts, sequences and strings.
pub(crate) fn item_of(obj: &Value, key: &Value) -> Option<Value> {
    // A Python index: negative counts from the end.
    let index = |n: usize| {
        let i = as_int(key)?;
        let i = if i < 0 { i + n as i64 } else { i };
        usize::try_from(i).ok().filter(|&i| i < n)
    };
    match obj {
        Value::Dict(d) if d.kind() == DictKind::Dict => d.get(key),
        Value::List(l) => l.item(index(l.len())?),
        Value::Str(text) | Value::Markup(text) => {
            let i = index(char_count(text))?;
            Some(Value::str(char_slice(text, i, i + 1)))
        }
        _ => None,
    }
}

/// A method of a builtin value, bound to it.
pub(crate) fn bound_method(obj: &Value, name: &str) -> Value {
    Callable::value(Rc::from(name), CallKind::Method(obj.clone()), 0)
}

/// Python slicing of lists, tuples and strings.
fn slice_value(s: &State<'_>, obj: &Value, bounds: &[Value; 3]) -> Result<Value> {
    if !matches!(obj, Value::List(_) | Value::Str(_) | Value::Markup(_)) {
        let message = format!("'{}' object is not subscriptable", type_name(obj));
        return Err(s.error(ErrorKind::Type, message));
    }
    let indices = slice_indices(s, obj.len() as i64, bounds)?;
    Ok(match obj {
        Value::List(l) => {
            let items = l.items();
            Value::List(List::new(indices.map(|i| items[i].clone()).collect(), l.is_tuple()))
        }
        _ => {
            let chars: Vec<char> = to_str(obj).chars().collect();
            let text: String = indices.map(|i| chars[i]).collect();
            if obj.is_markup() { Value::Markup(Rc::from(text)) } else { Value::from(text) }
        }
    })
}

/// The indices a slice picks from a sequence of n items.
fn slice_indices(
    s: &State<'_>,
    n: i64,
    bounds: &[Value; 3],
) -> Result<impl Iterator<Item = usize>> {
    let bad = || s.error(ErrorKind::Type, "slice indices must be integers or None");
    let step = match &bounds[2] {
        Value::None => 1,
        v => as_int(v).ok_or_else(bad)?,
    };
    if step == 0 {
        return Err(s.error(ErrorKind::Runtime, "slice step cannot be zero"));
    }
    let backward = step < 0;
    let bound = |v: &Value, default: i64| -> Option<i64> {
        if v.is_none() {
            return Some(default);
        }
        let x = as_int(v)?;
        let x = if x < 0 { x + n } else { x };
        Some(if backward { x.clamp(-1, n - 1) } else { x.clamp(0, n) })
    };
    let lo = bound(&bounds[0], if backward { n - 1 } else { 0 }).ok_or_else(bad)?;
    let hi = bound(&bounds[1], if backward { -1 } else { n }).ok_or_else(bad)?;
    let mut i = lo;
    Ok(std::iter::from_fn(move || {
        let inside = if backward { i > hi } else { i < hi };
        let at = i;
        i += step;
        inside.then_some(at as usize)
    }))
}

// ---------------------------------------------------------------------------------------
// Operators

fn unary_op(s: &State<'_>, op: UnaryOp, v: &Value) -> Result<Value> {
    if op == UnaryOp::Not {
        return Ok(Value::Bool(!s.is_true(v)?));
    }
    if v.is_undefined() {
        return Err(s.undefined_error(v));
    }
    let negate = op == UnaryOp::Neg;
    match as_number(v) {
        Some(Num::Int(i)) if negate => {
            i.checked_neg().map(Value::Int).ok_or_else(|| overflow_error(s))
        }
        Some(Num::Int(i)) => Ok(Value::Int(i)),
        Some(Num::Float(f)) => Ok(Value::Float(if negate { -f } else { f })),
        None => {
            let symbol = if negate { "-" } else { "+" };
            let message = format!("bad operand type for unary {symbol}: '{}'", type_name(v));
            Err(s.error(ErrorKind::Type, message))
        }
    }
}

#[inline(never)]
fn eval_binary(
    s: &mut State<'_>,
    scope: ScopeId,
    op: BinOp,
    left: &Expr,
    right: &Expr,
) -> Result<Value> {
    let l = eval(s, scope, left)?;
    match op {
        BinOp::And if !s.is_true(&l)? => Ok(l),
        BinOp::Or if s.is_true(&l)? => Ok(l),
        BinOp::And | BinOp::Or => eval(s, scope, right),
        _ => {
            let r = eval(s, scope, right)?;
            binary_op(s, op, &l, &r)
        }
    }
}

/// Applies an arithmetic operator to two values.
pub(crate) fn binary_op(s: &State<'_>, op: BinOp, left: &Value, right: &Value) -> Result<Value> {
    for side in [left, right] {
        if side.is_undefined() {
            return Err(s.undefined_error(side));
        }
    }
    if let (Some(a), Some(b)) = (as_number(left), as_number(right)) {
        return numeric_op(s, op, a, b);
    }
    match op {
        BinOp::Add => return add_values(s, left, right),
        BinOp::Mul => {
            if let Some(n) = as_int(right) {
                return crate::limits::repeat_value(s, left, n);
            }
            if let Some(n) = as_int(left) {
                return crate::limits::repeat_value(s, right, n);
            }
        }
        BinOp::Mod if left.is_string() => {
            return crate::format::percent_format(s, left.is_markup(), &to_str(left), right);
        }
        _ => {}
    }
    Err(unsupported(s, op.symbol(), left, right))
}

fn unsupported(s: &State<'_>, symbol: &str, left: &Value, right: &Value) -> Box<crate::Error> {
    let (l, r) = (type_name(left), type_name(right));
    s.error(ErrorKind::Type, format!("unsupported operand type(s) for {symbol}: '{l}' and '{r}'"))
}

fn numeric_op(s: &State<'_>, op: BinOp, a: Num, b: Num) -> Result<Value> {
    let ints = match (a, b) {
        (Num::Int(x), Num::Int(y)) => Some((x, y)),
        _ => None,
    };
    let (lf, rf) = (a.f(), b.f());
    let checked = |r: Option<i64>| r.map(Value::Int).ok_or_else(|| overflow_error(s));
    match (op, ints) {
        (BinOp::Add, Some((x, y))) => checked(x.checked_add(y)),
        (BinOp::Sub, Some((x, y))) => checked(x.checked_sub(y)),
        (BinOp::Mul, Some((x, y))) => checked(x.checked_mul(y)),
        (BinOp::Add, None) => Ok(Value::Float(lf + rf)),
        (BinOp::Sub, None) => Ok(Value::Float(lf - rf)),
        (BinOp::Mul, None) => Ok(Value::Float(lf * rf)),
        (BinOp::Div, _) if rf == 0.0 => Err(s.error(ErrorKind::Runtime, "division by zero")),
        (BinOp::Div, _) => Ok(Value::Float(lf / rf)),
        (BinOp::FloorDiv | BinOp::Mod, _) if rf == 0.0 => {
            Err(s.error(ErrorKind::Runtime, "integer division or modulo by zero"))
        }
        (BinOp::FloorDiv, Some((i64::MIN, -1))) => Err(overflow_error(s)),
        (BinOp::FloorDiv, Some((x, y))) => Ok(Value::Int(floor_div(x, y))),
        (BinOp::FloorDiv, None) => Ok(Value::Float((lf / rf).floor())),
        (BinOp::Mod, Some((x, y))) => {
            Ok(Value::Int(x.wrapping_sub(floor_div(x, y).wrapping_mul(y))))
        }
        (BinOp::Mod, None) => {
            let m = lf % rf;
            Ok(Value::Float(if m != 0.0 && ((m < 0.0) != (rf < 0.0)) { m + rf } else { m }))
        }
        (BinOp::Pow, Some((x, y))) if y >= 0 => checked(int_pow(x, y)),
        (BinOp::Pow, _) if lf == 0.0 && rf < 0.0 => {
            Err(s.error(ErrorKind::Runtime, "0.0 cannot be raised to a negative power"))
        }
        (BinOp::Pow, _) => Ok(Value::Float(lf.powf(rf))),
        (BinOp::And | BinOp::Or, _) => Err(s.error(ErrorKind::Type, "unsupported operator")),
    }
}

/// Python's integer //: the quotient rounded toward negative infinity.
fn floor_div(a: i64, b: i64) -> i64 {
    let q = a.wrapping_div(b);
    if a.wrapping_rem(b) != 0 && ((a < 0) != (b < 0)) { q - 1 } else { q }
}

fn add_values(s: &State<'_>, left: &Value, right: &Value) -> Result<Value> {
    if (left.is_markup() || right.is_markup()) && left.is_string() && right.is_string() {
        return Ok(markup_concat(left, right));
    }
    match (left, right) {
        (Value::Str(a), Value::Str(b)) => Ok(Value::from(format!("{a}{b}"))),
        (Value::Str(_), _) => {
            let message =
                format!("can only concatenate str (not \"{}\") to str", type_name(right));
            Err(s.error(ErrorKind::Type, message))
        }
        (Value::List(a), Value::List(b)) if a.is_tuple() == b.is_tuple() => {
            let mut items = a.snapshot();
            items.extend(b.snapshot());
            Ok(Value::List(List::new(items, a.is_tuple())))
        }
        (Value::List(_), _) => {
            let (l, r) = (type_name(left), type_name(right));
            let message = format!("can only concatenate {l} (not \"{r}\") to {l}");
            Err(s.error(ErrorKind::Type, message))
        }
        _ => Err(unsupported(s, "+", left, right)),
    }
}

/// The ~ operator: strings joined, Markup-aware when autoescaping.
#[inline(never)]
fn eval_concat(s: &mut State<'_>, scope: ScopeId, items: &[Expr]) -> Result<Value> {
    let mut values = Vec::with_capacity(items.len());
    for item in items {
        let v = eval(s, scope, item)?;
        s.strict_check(&v)?;
        values.push(v);
    }
    let escaping = s.autoescape && values.iter().any(Value::is_markup);
    let parts: Vec<_> =
        values.iter().map(|v| if escaping { escaped_text(v) } else { to_str(v) }).collect();
    let mut out = String::new();
    crate::limits::reserve_text(s, &mut out, parts.iter().map(|p| p.len()).sum())?;
    parts.iter().for_each(|p| out.push_str(p));
    Ok(if escaping { Value::Markup(Rc::from(out)) } else { Value::from(out) })
}

#[inline(never)]
fn eval_compare(
    s: &mut State<'_>,
    scope: ScopeId,
    first: &Expr,
    rest: &[(CmpOp, Expr)],
) -> Result<Value> {
    let mut left = eval(s, scope, first)?;
    for (op, operand) in rest {
        let right = eval(s, scope, operand)?;
        if !compare_op(s, *op, &left, &right)? {
            return Ok(Value::Bool(false));
        }
        left = right;
    }
    Ok(Value::Bool(true))
}

/// Applies a comparison operator.
pub(crate) fn compare_op(s: &State<'_>, op: CmpOp, left: &Value, right: &Value) -> Result<bool> {
    match op {
        CmpOp::Eq | CmpOp::Ne => {
            s.strict_check(left)?;
            s.strict_check(right)?;
            return Ok(values_equal(left, right) == (op == CmpOp::Eq));
        }
        CmpOp::In | CmpOp::NotIn => return Ok(contains(s, right, left)? == (op == CmpOp::In)),
        _ => {}
    }
    for side in [left, right] {
        if side.is_undefined() {
            return Err(s.undefined_error(side));
        }
    }
    let Some(order) = compare_values(left, right) else {
        let (l, r) = (type_name(left), type_name(right));
        let message =
            format!("'{}' not supported between instances of '{l}' and '{r}'", op.symbol());
        return Err(s.error(ErrorKind::Type, message));
    };
    Ok(match op {
        _ if order == 2 => false,
        CmpOp::Lt => order < 0,
        CmpOp::Le => order <= 0,
        CmpOp::Gt => order > 0,
        _ => order >= 0,
    })
}

/// Python's `item in container`.
pub(crate) fn contains(s: &State<'_>, container: &Value, item: &Value) -> Result<bool> {
    match container {
        Value::Str(text) | Value::Markup(text) => match item.as_str() {
            Some(needle) => Ok(text.contains(needle)),
            None => {
                let what = type_name(item);
                let message = format!("'in <string>' requires string as left operand, not {what}");
                Err(s.error(ErrorKind::Type, message))
            }
        },
        Value::List(l) => Ok(l.items().iter().any(|v| values_equal(v, item))),
        Value::Dict(d) => Ok(d.get(item).is_some()),
        Value::Undefined(_) => s.strict_check(container).map(|()| false),
        _ => {
            let what = type_name(container);
            Err(s.error(ErrorKind::Type, format!("argument of type '{what}' is not iterable")))
        }
    }
}

pub(crate) fn overflow_error(s: &State<'_>) -> Box<crate::Error> {
    s.error(ErrorKind::Runtime, "integer overflow: results must fit in 64 bits")
        .hinted("Use a float literal (2.0 ** 100) for very large numbers.")
}

/// Raises base to a non-negative power by squaring; None on overflow.
fn int_pow(base: i64, exp: i64) -> Option<i64> {
    let (mut result, mut b, mut e) = (1i64, base, exp);
    while e > 0 {
        if e & 1 == 1 {
            result = result.checked_mul(b)?;
        }
        e >>= 1;
        if e > 0 {
            b = b.checked_mul(b)?;
        }
    }
    Some(result)
}
