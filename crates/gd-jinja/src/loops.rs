//! For loops, the `loop` variable and recursive loops.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::ast::{Compiled, ForDef, LoopId, Name};
use crate::calls::Call;
use crate::convert::values_equal;
use crate::error::ErrorKind;
use crate::eval::eval;
use crate::runtime::{
    Flow, Recursion, Result, ScopeId, State, assign, condition, exec_body, iterate,
};
use crate::value::{CallKind, Callable, Loop, Value};

pub(crate) fn exec_for(
    s: &mut State<'_>,
    t: &Rc<Compiled>,
    scope: ScopeId,
    id: LoopId,
) -> Result<Flow> {
    let def = t.loop_def(id);
    let iterable = eval(s, scope, &def.iter)?;
    let items = iterate(s, &iterable)?;
    let recurse = def.recursive.then(|| {
        Rc::new(Recursion { template: t.clone(), id, scope, ctx: s.ctx, render: s.render_id })
    });
    let (l, inner) = new_loop(s, scope, def, items, 0, recurse)?;
    run_loop(s, t, inner, def, &l)
}

/// Makes the loop variable of a for loop over items, in a new scope below scope,
/// keeping only the items that pass the loop's filter (`for x in xs if ...`).
fn new_loop(
    s: &mut State<'_>,
    scope: ScopeId,
    def: &ForDef,
    items: Vec<Value>,
    depth0: usize,
    recurse: Option<Rc<Recursion>>,
) -> Result<(Rc<Loop>, ScopeId)> {
    let inner = s.new_scope(Some(scope));
    let mut kept = items;
    if let Some(filter) = &def.filter {
        let mut passed = Vec::with_capacity(kept.len());
        for item in kept {
            s.count_iteration()?;
            assign(s, inner, &def.target, item.clone())?;
            if condition(s, inner, filter)? {
                passed.push(item);
            }
        }
        kept = passed;
    }
    let l =
        Loop { items: kept, index0: Cell::new(0), depth0, last_seen: RefCell::new(None), recurse };
    Ok((Rc::new(l), inner))
}

/// Runs the body of a for loop once for each of the loop's items.
fn run_loop(
    s: &mut State<'_>,
    t: &Rc<Compiled>,
    inner: ScopeId,
    def: &ForDef,
    l: &Rc<Loop>,
) -> Result<Flow> {
    let loop_name: Name = Rc::from("loop");
    for (i, item) in l.items.iter().enumerate() {
        s.count_iteration()?;
        l.index0.set(i);
        assign(s, inner, &def.target, item.clone())?;
        s.set_var(inner, &loop_name, Value::Loop(l.clone()));
        if exec_body(s, t, inner, &def.body)? == Flow::Break {
            break;
        }
    }
    if l.items.is_empty() {
        return exec_body(s, t, inner, &def.else_body);
    }
    Ok(Flow::Normal)
}

/// Runs the body of a recursive loop again: `loop(children)`.
pub(crate) fn call_loop(s: &mut State<'_>, l: &Loop, args: &[Value]) -> Result<Value> {
    let Some(rec) = l.recurse.clone() else {
        let message =
            "Tried to call non recursive loop.  Maybe you forgot the 'recursive' modifier.";
        return Err(s.error(ErrorKind::Type, message));
    };
    if rec.render != s.render_id {
        return Err(s.error(ErrorKind::Runtime, "the loop was made by a render that has ended"));
    }
    let [children] = args else {
        return Err(s.error(ErrorKind::Type, "loop() takes exactly 1 argument"));
    };
    let items = iterate(s, children)?;
    let def = rec.template.loop_def(rec.id);
    let (again, inner) = new_loop(s, rec.scope, def, items, l.depth0 + 1, Some(rec.clone()))?;
    s.enter_depth()?;
    let saved = s.enter(rec.ctx, &rec.template, s.autoescape);
    s.begin_capture();
    run_loop(s, &rec.template, inner, def, &again)?;
    let text = s.end_capture();
    s.restore(saved);
    s.leave_depth();
    Ok(s.captured(text))
}

/// Reads `loop.index` and friends.
pub(crate) fn loop_attribute(l: &Rc<Loop>, name: &str) -> Option<Value> {
    let n = l.items.len() as i64;
    let i = l.index0.get();
    let index = i as i64;
    Some(match name {
        "index" => Value::Int(index + 1),
        "index0" => Value::Int(index),
        "revindex" => Value::Int(n - index),
        "revindex0" => Value::Int(n - index - 1),
        "first" => Value::Bool(i == 0),
        "last" => Value::Bool(index == n - 1),
        "length" => Value::Int(n),
        "depth" => Value::Int(l.depth0 as i64 + 1),
        "depth0" => Value::Int(l.depth0 as i64),
        "previtem" if i > 0 => l.items[i - 1].clone(),
        "previtem" => Value::undefined_hint("there is no previous item"),
        "nextitem" if i + 1 < l.items.len() => l.items[i + 1].clone(),
        "nextitem" => Value::undefined_hint("there is no next item"),
        "cycle" | "changed" => {
            Callable::value(Rc::from(name), CallKind::Method(Value::Loop(l.clone())), 0)
        }
        _ => return None,
    })
}

/// `loop.cycle(...)` and `loop.changed(...)`.
pub(crate) fn loop_method(c: &mut Call<'_, '_>, l: &Loop) -> crate::filters_text::Outcome {
    match &*c.name {
        "cycle" if c.args.is_empty() => Err(c.error("no items for cycling given")),
        "cycle" => Ok(c.args[l.index0.get() % c.args.len()].clone()),
        "changed" => {
            let current = Value::list(c.args.iter().cloned());
            let mut last = l.last_seen.borrow_mut();
            if last.as_ref().is_some_and(|seen| values_equal(&current, seen)) {
                return Ok(Value::Bool(false));
            }
            *last = Some(current);
            Ok(Value::Bool(true))
        }
        name => Err(c.error(&format!("unknown loop method {name}"))),
    }
}
