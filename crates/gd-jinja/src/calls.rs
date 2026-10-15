//! Calls: what native procedures receive, macros, call blocks, filters and tests.

use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::{Args, Compiled, Expr, ExprKind, MacroId, Name};
use crate::convert::{Num, as_number, quoted, to_str, truthy, type_name};
use crate::environment::UndefinedMode;
use crate::error::{Error, ErrorKind, Hinted, closest};
use crate::eval::eval;
use crate::runtime::{Macro, Result, ScopeId, State, exec_body, iterate};
use crate::value::{CallKind, Callable, Dict, DictKind, Native, Value};

/// One keyword argument of a call.
#[derive(Debug, Clone)]
pub struct Keyword {
    /// The parameter name.
    pub name: Rc<str>,
    /// The value passed.
    pub value: Value,
}

/// What a native procedure receives: the positional and keyword arguments (for a filter
/// or test, `args[0]` is the value being filtered or tested), the receiver of a bound
/// method, and the name it was called by. It borrows the render running it.
pub struct Call<'a, 'env> {
    /// Positional arguments.
    pub args: Vec<Value>,
    /// Keyword arguments, in the order given.
    pub kwargs: Vec<Keyword>,
    /// The receiver of a bound method; Undefined otherwise.
    pub this: Value,
    /// The name the procedure was registered or looked up by.
    pub name: Rc<str>,
    pub(crate) state: &'a mut State<'env>,
}

impl std::fmt::Debug for Call<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Call").field("name", &self.name).field("args", &self.args).finish()
    }
}

impl<'a, 'env> Call<'a, 'env> {
    pub(crate) fn new(
        state: &'a mut State<'env>,
        name: Rc<str>,
        args: Vec<Value>,
        kwargs: Vec<Keyword>,
    ) -> Self {
        Call { args, kwargs, this: Value::default(), name, state }
    }

    /// Positional argument `index`, else the keyword argument `name`, else Undefined.
    pub fn arg(&self, index: usize, name: &str) -> Value {
        self.arg_or(index, name, Value::default())
    }

    /// Positional argument `index`, else the keyword argument `name`, else `default`.
    pub fn arg_or(&self, index: usize, name: &str, default: Value) -> Value {
        match self.args.get(index) {
            Some(v) => v.clone(),
            None => self.keyword(name).unwrap_or(default),
        }
    }

    /// The keyword argument `name`, if it was passed.
    pub fn keyword(&self, name: &str) -> Option<Value> {
        self.kwargs.iter().find(|kw| &*kw.name == name).map(|kw| kw.value.clone())
    }

    /// Whether the argument was passed at all.
    pub fn has_arg(&self, index: usize, name: &str) -> bool {
        index < self.args.len() || self.kwargs.iter().any(|kw| &*kw.name == name)
    }

    /// Whether argument `index` / `name` is true, or `default` when it was not passed.
    pub fn arg_flag(&self, index: usize, name: &str, default: bool) -> bool {
        truthy(&self.arg_or(index, name, Value::Bool(default)))
    }

    /// An integer argument; bools and floats are accepted, and None gives `default`.
    pub fn arg_int(
        &self,
        index: usize,
        name: &str,
        default: i64,
    ) -> std::result::Result<i64, Error> {
        let v = self.arg_or(index, name, Value::Int(default));
        match as_number(&v) {
            Some(Num::Int(i)) => Ok(i),
            Some(Num::Float(f)) => Ok(f as i64),
            None if v.is_none() => Ok(default),
            None => Err(self.error(&format!("{name} must be an integer, not {}", type_name(&v)))),
        }
    }

    /// Like [`Call::arg_int`], with negative values read as 0.
    pub fn arg_count(
        &self,
        index: usize,
        name: &str,
        default: i64,
    ) -> std::result::Result<usize, Error> {
        Ok(usize::try_from(self.arg_int(index, name, default)?).unwrap_or(0))
    }

    /// Enforces a Python signature: at most `names.len()` positional arguments, only the
    /// named keywords, each once, and the first `required` present. `who` names the
    /// callable in errors.
    pub fn check_args(
        &self,
        who: &str,
        names: &[&str],
        required: usize,
    ) -> std::result::Result<(), Error> {
        if self.args.len() > names.len() {
            let (most, given) = (names.len(), self.args.len());
            let message = format!("{who}() takes at most {most} arguments ({given} given)");
            return Err(self.error(&message));
        }
        for kw in &self.kwargs {
            match names.iter().position(|n| *n == &*kw.name) {
                None => {
                    let name = quoted(&kw.name);
                    return Err(
                        self.error(&format!("{who}() got an unexpected keyword argument {name}"))
                    );
                }
                Some(index) if index < self.args.len() => {
                    let name = quoted(&kw.name);
                    return Err(
                        self.error(&format!("{who}() got multiple values for argument {name}"))
                    );
                }
                Some(_) => {}
            }
        }
        match names.iter().enumerate().take(required).find(|(i, n)| !self.has_arg(*i, n)) {
            Some((_, name)) => {
                Err(self.error(&format!("{who}() missing required argument {}", quoted(name))))
            }
            None => Ok(()),
        }
    }

    /// An error at the place the call happens, for the procedure to return.
    pub fn error(&self, message: &str) -> Error {
        self.state.located(Error::new(ErrorKind::Runtime, self.state.line, message))
    }

    /// Whether autoescape is on where the call happens.
    pub fn autoescaping(&self) -> bool {
        self.state.autoescape
    }

    /// Runs a registered filter by name.
    pub fn call_filter(
        &mut self,
        name: &str,
        args: Vec<Value>,
        kwargs: Vec<Keyword>,
    ) -> std::result::Result<Value, Error> {
        Ok(call_filter(self.state, name, args, kwargs)?)
    }

    /// Runs a registered test by name.
    pub fn call_test(
        &mut self,
        name: &str,
        args: Vec<Value>,
        kwargs: Vec<Keyword>,
    ) -> std::result::Result<bool, Error> {
        Ok(call_test(self.state, name, args, kwargs)?)
    }

    pub(crate) fn state(&mut self) -> &mut State<'env> {
        self.state
    }

    pub(crate) fn state_ref(&self) -> &State<'env> {
        self.state
    }
}

// ---------------------------------------------------------------------------------------
// Calls

/// Evaluates the arguments of a call, filter or test; input, when given, is the first
/// positional argument (the value being filtered or tested).
fn eval_arguments(
    s: &mut State<'_>,
    scope: ScopeId,
    a: &Args,
    input: Option<Value>,
) -> Result<(Vec<Value>, Vec<Keyword>)> {
    let mut args = Vec::with_capacity(a.args.len() + 1);
    args.extend(input);
    for e in &a.args {
        args.push(eval(s, scope, e)?);
    }
    let mut kwargs = Vec::with_capacity(a.kwargs.len());
    for (name, e) in &a.kwargs {
        kwargs.push(Keyword { name: name.clone(), value: eval(s, scope, e)? });
    }
    if let Some(star) = &a.star {
        let extra = eval(s, scope, star)?;
        args.extend(iterate(s, &extra)?);
    }
    if let Some(dstar) = &a.dstar {
        let d = match eval(s, scope, dstar)? {
            Value::Dict(d) => d,
            other => {
                let message =
                    format!("argument after ** must be a mapping, not {}", type_name(&other));
                return Err(s.error(ErrorKind::Type, message));
            }
        };
        for (k, v) in d.entries() {
            kwargs.push(Keyword { name: Rc::from(to_str(&k).as_ref()), value: v });
        }
    }
    Ok((args, kwargs))
}

pub(crate) fn eval_call(
    s: &mut State<'_>,
    scope: ScopeId,
    line: usize,
    callee: &Expr,
    a: &Args,
) -> Result<Value> {
    let f = eval(s, scope, callee)?;
    let (args, kwargs) = eval_arguments(s, scope, a, None)?;
    s.line = line;
    call_value(s, &f, args, kwargs)
}

/// Calls a callable value with evaluated arguments.
pub(crate) fn call_value(
    s: &mut State<'_>,
    callee: &Value,
    args: Vec<Value>,
    kwargs: Vec<Keyword>,
) -> Result<Value> {
    match callee {
        Value::Callable(c) => call_callable(s, c, args, kwargs),
        Value::Loop(l) => crate::loops::call_loop(s, l, &args),
        Value::Undefined(_) if s.env.options.undefined == UndefinedMode::Chainable => {
            Ok(callee.clone())
        }
        Value::Undefined(_) => Err(s.undefined_error(callee)),
        _ => {
            let message = format!("'{}' object is not callable", type_name(callee));
            Err(s.error(ErrorKind::Type, message))
        }
    }
}

fn call_callable(
    s: &mut State<'_>,
    c: &Rc<Callable>,
    args: Vec<Value>,
    kwargs: Vec<Keyword>,
) -> Result<Value> {
    if c.render != 0 && c.render != s.render_id {
        return Err(ended_callable(s, &c.name));
    }
    match &c.kind {
        CallKind::Native(f) => Ok(f(&mut Call::new(s, c.name.clone(), args, kwargs))?),
        CallKind::Method(this) => {
            let mut call = Call::new(s, c.name.clone(), args, kwargs);
            call.this = this.clone();
            Ok(crate::methods::dispatch_method(&mut call)?)
        }
        CallKind::Macro(m) => call_macro(s, &c.name, m, args, kwargs),
        CallKind::Block(r) => crate::inherit::call_block_reference(s, r),
        CallKind::Local => {
            // Copied out of the state, so the call may borrow the state mutably.
            let locals = s.locals;
            match locals.and_then(|l| l.function(&c.name)) {
                Some(f) => Ok(f(&mut Call::new(s, c.name.clone(), args, kwargs))?),
                None => Err(ended_callable(s, &c.name)),
            }
        }
    }
}

/// A macro, or a function of a render, kept past its render.
fn ended_callable(s: &State<'_>, name: &str) -> Box<Error> {
    let message = format!(
        "{} was made by a render that has ended, so it cannot be called any more.",
        quoted(name)
    );
    s.error(ErrorKind::Runtime, message).hinted(format!(
        "Call it during the render that makes it. To share a macro between templates, \
         import it: {{% from 'macros.html' import {name} %}}"
    ))
}

// ---------------------------------------------------------------------------------------
// Macros

pub(crate) fn make_macro(s: &State<'_>, t: &Rc<Compiled>, scope: ScopeId, id: MacroId) -> Value {
    let m = Macro { template: t.clone(), id, scope, ctx: s.ctx, autoescape: s.autoescape };
    s.render_callable(t.macro_def(id).name.clone(), CallKind::Macro(Rc::new(m)))
}

/// The attributes of a macro: name, arguments, catch_kwargs, catch_varargs, caller.
pub(crate) fn callable_attribute(c: &Callable, name: &str) -> Option<Value> {
    let CallKind::Macro(m) = &c.kind else { return None };
    let def = m.template.macro_def(m.id);
    Some(match name {
        "name" => Value::from(c.name.clone()),
        "arguments" => Value::tuple(def.params.iter().map(|(n, _)| Value::from(n.clone()))),
        "catch_kwargs" => Value::Bool(def.uses_kwargs),
        "catch_varargs" => Value::Bool(def.uses_varargs),
        "caller" => Value::Bool(def.uses_caller),
        _ => return None,
    })
}

fn call_macro(
    s: &mut State<'_>,
    name: &Rc<str>,
    m: &Macro,
    args: Vec<Value>,
    kwargs: Vec<Keyword>,
) -> Result<Value> {
    s.enter_depth()?;
    let scope = s.new_scope(Some(m.scope));
    bind_macro_args(s, name, m, scope, &args, &kwargs)?;
    let saved = s.enter(m.ctx, &m.template, m.autoescape);
    s.begin_capture();
    exec_body(s, &m.template, scope, &m.template.macro_def(m.id).body)?;
    let text = s.end_capture();
    s.restore(saved);
    s.leave_depth();
    Ok(if m.autoescape { Value::Markup(Rc::from(text)) } else { Value::from(text) })
}

/// Binds a macro's parameters, varargs, kwargs and caller in its scope.
fn bind_macro_args(
    s: &mut State<'_>,
    name: &str,
    m: &Macro,
    scope: ScopeId,
    args: &[Value],
    kwargs: &[Keyword],
) -> Result<()> {
    let def = m.template.macro_def(m.id);
    let extra = Dict::new(DictKind::Dict);
    let mut has_caller = false;
    for kw in kwargs {
        match def.params.iter().position(|(p, _)| *p == kw.name) {
            Some(i) if i < args.len() => {
                let (macro_name, arg) = (quoted(name), quoted(&kw.name));
                let message = format!("macro {macro_name} got multiple values for argument {arg}");
                return Err(s.error(ErrorKind::Type, message));
            }
            Some(_) => {}
            None if &*kw.name == "caller" && def.uses_caller => {
                has_caller = true;
                s.set_var(scope, &kw.name, kw.value.clone());
            }
            None => {
                has_caller |= &*kw.name == "caller";
                extra.set(Value::from(kw.name.clone()), kw.value.clone());
            }
        }
    }
    for (i, (param, default)) in def.params.iter().enumerate() {
        let v = if let Some(v) = args.get(i) {
            v.clone()
        } else if let Some(kw) = kwargs.iter().find(|kw| kw.name == *param) {
            kw.value.clone()
        } else if let Some(default) = default {
            s.set_var(scope, param, Value::default());
            eval(s, scope, default)?
        } else {
            let hint = format!("parameter {} was not provided", quoted(param));
            Value::undefined_with(param, "", &hint)
        };
        s.set_var(scope, param, v);
    }
    bind_macro_rest(s, name, m, scope, args, extra, has_caller)
}

fn bind_macro_rest(
    s: &mut State<'_>,
    name: &str,
    m: &Macro,
    scope: ScopeId,
    args: &[Value],
    extra: Rc<Dict>,
    has_caller: bool,
) -> Result<()> {
    let def = m.template.macro_def(m.id);
    let n = def.params.len();
    if args.len() > n && !def.uses_varargs {
        let message = format!("macro {} takes not more than {n} argument(s)", quoted(name));
        return Err(s.error(ErrorKind::Type, message));
    }
    if def.uses_varargs {
        let rest = args.get(n..).unwrap_or_default().to_vec();
        s.set_var(scope, &Rc::from("varargs"), Value::tuple(rest));
    }
    if let Some(first) = extra.keys().first().filter(|_| !def.uses_kwargs) {
        let message =
            format!("macro {} takes no keyword argument {}", quoted(name), quoted(&to_str(first)));
        return Err(s.error(ErrorKind::Type, message));
    }
    if def.uses_kwargs {
        s.set_var(scope, &Rc::from("kwargs"), Value::Dict(extra));
    }
    if def.uses_caller && !has_caller && !s.has_var(scope, "caller") {
        let undefined = Value::undefined_with("caller", "", "No caller defined");
        s.set_var(scope, &Rc::from("caller"), undefined);
    }
    Ok(())
}

/// `{% call(params) macro(args) %}body{% endcall %}`: calls the macro with the body as
/// its caller.
pub(crate) fn exec_call_block(
    s: &mut State<'_>,
    t: &Rc<Compiled>,
    scope: ScopeId,
    id: MacroId,
    call: &Expr,
) -> Result<()> {
    let ExprKind::Call(callee, a) = &call.kind else { return Ok(()) };
    let line = s.line;
    let f = eval(s, scope, callee)?;
    let (args, mut kwargs) = eval_arguments(s, scope, a, None)?;
    let caller: Name = Rc::from("caller");
    kwargs.push(Keyword { name: caller, value: make_macro(s, t, scope, id) });
    s.line = line;
    let v = call_value(s, &f, args, kwargs)?;
    s.write_output(&v)
}

// ---------------------------------------------------------------------------------------
// Filters and tests

/// Applies the filter expression e to input, or to its own operand when input is None.
pub(crate) fn apply_filter(
    s: &mut State<'_>,
    scope: ScopeId,
    e: &Expr,
    input: Option<Value>,
) -> Result<Value> {
    let ExprKind::Filter(operand, name, a) = &e.kind else { return Ok(Value::default()) };
    let input = match (input, operand) {
        (Some(v), _) => v,
        (None, Some(operand)) => eval(s, scope, operand)?,
        (None, None) => Value::default(),
    };
    let locals = s.locals;
    if let Some(f) = locals.and_then(|l| l.filter(name)) {
        let (args, kwargs) = eval_arguments(s, scope, a, Some(input))?;
        s.line = e.line;
        return Ok(f(&mut Call::new(s, name.clone(), args, kwargs))?);
    }
    let env = s.env;
    let f = find_registered(s, &env.filters, "filter", name)?;
    let (args, kwargs) = eval_arguments(s, scope, a, Some(input))?;
    s.line = e.line;
    Ok(f(&mut Call::new(s, name.clone(), args, kwargs))?)
}

/// Applies a filter chain whose innermost operand is missing to input.
pub(crate) fn apply_filter_chain(
    s: &mut State<'_>,
    scope: ScopeId,
    chain: &Expr,
    input: Value,
) -> Result<Value> {
    let ExprKind::Filter(operand, ..) = &chain.kind else { return Ok(input) };
    let inner = match operand {
        Some(inner_chain) => apply_filter_chain(s, scope, inner_chain, input)?,
        None => input,
    };
    apply_filter(s, scope, chain, Some(inner))
}

pub(crate) fn apply_test(s: &mut State<'_>, scope: ScopeId, e: &Expr) -> Result<Value> {
    let ExprKind::Test(operand, name, a) = &e.kind else { return Ok(Value::default()) };
    let input = eval(s, scope, operand)?;
    let env = s.env;
    let t = find_registered(s, &env.tests, "test", name)?;
    let (args, kwargs) = eval_arguments(s, scope, a, Some(input))?;
    s.line = e.line;
    let v = t(&mut Call::new(s, name.clone(), args, kwargs))?;
    Ok(Value::Bool(truthy(&v)))
}

/// Finds the filter or test (what) called name, or reports that there is none and
/// suggests a close match.
fn find_registered<'e>(
    s: &State<'_>,
    table: &'e HashMap<String, Native>,
    what: &str,
    name: &str,
) -> Result<&'e Native> {
    if let Some(f) = table.get(name) {
        return Ok(f);
    }
    let err = s.error(ErrorKind::Runtime, format!("No {what} named {}.", quoted(name)));
    let mut known: Vec<&str> = table.keys().map(String::as_str).collect();
    if let Some(locals) = s.locals.filter(|_| what == "filter") {
        known.extend(locals.filter_names());
    }
    known.sort_unstable();
    let near = closest(name, known);
    if near.is_empty() {
        return Err(err.hinted(format!("Register it with env.add_{what}(\"{name}\", procedure).")));
    }
    Err(err.hinted(format!("Did you mean the {what} {}?", quoted(near))))
}

/// Runs a registered filter by name (used by map, and by hosts).
pub(crate) fn call_filter(
    s: &mut State<'_>,
    name: &str,
    args: Vec<Value>,
    kwargs: Vec<Keyword>,
) -> Result<Value> {
    let locals = s.locals;
    if let Some(f) = locals.and_then(|l| l.filter(name)) {
        return Ok(f(&mut Call::new(s, Rc::from(name), args, kwargs))?);
    }
    let env = s.env;
    let f = find_registered(s, &env.filters, "filter", name)?;
    Ok(f(&mut Call::new(s, Rc::from(name), args, kwargs))?)
}

/// Runs a registered test by name (used by select and reject).
pub(crate) fn call_test(
    s: &mut State<'_>,
    name: &str,
    args: Vec<Value>,
    kwargs: Vec<Keyword>,
) -> Result<bool> {
    let env = s.env;
    let t = find_registered(s, &env.tests, "test", name)?;
    Ok(truthy(&t(&mut Call::new(s, Rc::from(name), args, kwargs))?))
}
