//! One render in progress: its state, scopes, contexts, output, and the statements.
//!
//! Every step returns `Result`: the first error ends the render, carrying the template,
//! line and excerpt where it happened. Scopes and contexts live in flat vectors owned by
//! the render and are named by id. A macro, a block reference or a recursive loop keeps
//! the ids of the scope and context it was made in; it belongs to its render
//! (`Callable::render`), so the ids stay valid for every call that may use them.

use std::rc::Rc;

use crate::ast::{BlockId, Compiled, Expr, ExprKind, LoopId, MacroId, Name, Stmt, StmtKind};
use crate::convert::{quoted, to_str, truthy, type_name};
use crate::environment::{Environment, UndefinedMode};
use crate::error::{Error, ErrorKind, Hinted, closest, source_line};
use crate::escape::write_escaped;
use crate::eval::{eval, lookup};
use crate::stack::StackGuard;
use crate::value::{CallKind, Callable, Dict, DictKind, Undefined, Value};

/// What the renderer's steps return. The error is boxed so that the results passed up
/// through every level of recursion stay small.
pub(crate) type Result<T> = std::result::Result<T, Box<Error>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScopeId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ContextId(u32);

/// Variables. The root scope of a render also reads the host's context dict.
struct Scope {
    vars: Vec<(Name, Value)>,
    dict: Option<Rc<Dict>>,
    parent: Option<ScopeId>,
}

/// One template render (the main template, an include or an import): the variables of
/// its top level and the block chain built by extends.
pub(crate) struct Context {
    pub root: ScopeId,
    pub blocks: Vec<(Name, Vec<BlockRef>)>,
    pub parent: Option<Rc<Compiled>>,
    /// The output buffer the template writes to; a child template that extends another
    /// writes nothing there itself.
    pub top_out: usize,
    pub template: Option<Rc<Compiled>>,
}

#[derive(Clone)]
pub(crate) struct BlockRef {
    pub template: Rc<Compiled>,
    pub id: BlockId,
}

/// A macro or call-block body together with where it was defined.
pub(crate) struct Macro {
    pub template: Rc<Compiled>,
    pub id: MacroId,
    pub scope: ScopeId,
    pub ctx: ContextId,
    pub autoescape: bool,
}

/// Lets loop(...) re-enter a recursive for loop.
pub(crate) struct Recursion {
    pub template: Rc<Compiled>,
    pub id: LoopId,
    pub scope: ScopeId,
    pub ctx: ContextId,
    pub render: u64,
}

/// super() and self.name(): a level of the block called name.
pub(crate) struct BlockReference {
    pub name: Name,
    pub level: usize,
    pub scope: ScopeId,
    pub ctx: ContextId,
}

/// How a run of statements ends, when it does not fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flow {
    Normal,
    Break,
    Continue,
}

/// What a render saves while it runs code of another template or context.
pub(crate) struct Saved {
    ctx: ContextId,
    current: Option<Rc<Compiled>>,
    autoescape: bool,
}

/// One render in progress. Native procedures reach it through [`crate::Call`].
pub struct State<'env> {
    pub(crate) env: &'env Environment,
    pub(crate) out: String,
    out_id: usize,
    next_out_id: usize,
    captures: Vec<(String, usize)>,
    pub(crate) autoescape: bool,
    depth: usize,
    iterations: usize,
    pub(crate) ctx: ContextId,
    pub(crate) current: Option<Rc<Compiled>>,
    pub(crate) line: usize,
    pub(crate) rng: u64,
    pub(crate) render_id: u64,
    stack: StackGuard,
    scopes: Vec<Scope>,
    contexts: Vec<Context>,
}

impl std::fmt::Debug for State<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State").field("line", &self.line).finish_non_exhaustive()
    }
}

/// Numbers renders, so that what one render makes can tell it is called by another.
static NEXT_RENDER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// The random seed when Options::random_seed is 0.
const DEFAULT_SEED: u64 = 0x853c_49e6_748f_ea9b;

impl<'env> State<'env> {
    /// A render of env with a root scope that reads ctx (the host's context dict), in a
    /// context of its own.
    pub(crate) fn new(env: &'env Environment, ctx: Option<Rc<Dict>>) -> State<'env> {
        let seed = env.options.random_seed;
        let mut s = State {
            env,
            out: String::new(),
            out_id: 0,
            next_out_id: 0,
            captures: Vec::new(),
            autoescape: false,
            depth: 0,
            iterations: 0,
            ctx: ContextId(0),
            current: None,
            line: 0,
            rng: if seed == 0 { DEFAULT_SEED } else { seed },
            render_id: NEXT_RENDER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            stack: env.stack_guard(),
            scopes: Vec::new(),
            contexts: Vec::new(),
        };
        let root = s.new_scope(None);
        s.scopes[0].dict = ctx;
        s.ctx = s.new_context(root);
        s
    }

    // -----------------------------------------------------------------------------------
    // Scopes and contexts

    pub(crate) fn new_scope(&mut self, parent: Option<ScopeId>) -> ScopeId {
        self.scopes.push(Scope { vars: Vec::new(), dict: None, parent });
        ScopeId(self.scopes.len() as u32 - 1)
    }

    pub(crate) fn root_scope(&self) -> ScopeId {
        ScopeId(0)
    }

    pub(crate) fn set_var(&mut self, scope: ScopeId, name: &Name, value: Value) {
        let vars = &mut self.scopes[scope.0 as usize].vars;
        match vars.iter_mut().find(|(n, _)| n == name) {
            Some(slot) => slot.1 = value,
            None => vars.push((name.clone(), value)),
        }
    }

    pub(crate) fn has_var(&self, scope: ScopeId, name: &str) -> bool {
        self.scopes[scope.0 as usize].vars.iter().any(|(n, _)| &**n == name)
    }

    /// Finds name in scope and the scopes around it, then in the host's context dict.
    pub(crate) fn find_var(&self, scope: ScopeId, name: &str) -> Option<Value> {
        let mut at = Some(scope);
        while let Some(id) = at {
            let sc = &self.scopes[id.0 as usize];
            if let Some((_, v)) = sc.vars.iter().find(|(n, _)| &**n == name) {
                return Some(v.clone());
            }
            if let Some(v) = sc.dict.as_ref().and_then(|d| d.get_str(name)) {
                return Some(v);
            }
            at = sc.parent;
        }
        None
    }

    /// The variables a scope defines itself, in the order they were set.
    pub(crate) fn scope_vars(&self, scope: ScopeId) -> &[(Name, Value)] {
        &self.scopes[scope.0 as usize].vars
    }

    pub(crate) fn new_context(&mut self, root: ScopeId) -> ContextId {
        let top_out = self.out_id;
        let ctx = Context { root, blocks: Vec::new(), parent: None, top_out, template: None };
        self.contexts.push(ctx);
        ContextId(self.contexts.len() as u32 - 1)
    }

    pub(crate) fn context(&self, id: ContextId) -> &Context {
        &self.contexts[id.0 as usize]
    }

    pub(crate) fn context_mut(&mut self, id: ContextId) -> &mut Context {
        &mut self.contexts[id.0 as usize]
    }

    pub(crate) fn current_context(&self) -> &Context {
        self.context(self.ctx)
    }

    /// Switches to running template t in context ctx; restore puts back what it saves.
    pub(crate) fn enter(&mut self, ctx: ContextId, t: &Rc<Compiled>, autoescape: bool) -> Saved {
        let current = self.current.replace(t.clone());
        let saved = Saved { ctx: self.ctx, current, autoescape: self.autoescape };
        self.ctx = ctx;
        self.autoescape = autoescape;
        saved
    }

    pub(crate) fn restore(&mut self, saved: Saved) {
        self.ctx = saved.ctx;
        self.current = saved.current;
        self.autoescape = saved.autoescape;
    }

    // -----------------------------------------------------------------------------------
    // Errors

    /// An error at the line being rendered.
    pub(crate) fn error(&self, kind: ErrorKind, message: impl Into<String>) -> Box<Error> {
        Box::new(self.located(Error::new(kind, self.line, message)))
    }

    /// err with the template and excerpt of the line being rendered.
    pub(crate) fn located(&self, mut err: Error) -> Error {
        if let Some(t) = &self.current {
            err.template = t.name.to_string();
            err.excerpt = source_line(&t.source, self.line).to_string();
        }
        err
    }

    /// The error that using an undefined value produces, with a hint.
    pub(crate) fn undefined_error(&self, v: &Value) -> Box<Error> {
        let u = v.undefined_info().unwrap_or_default();
        let err = self.error(ErrorKind::Undefined, undefined_message(&u));
        if !u.hint.is_empty() {
            return err;
        }
        err.hinted(self.undefined_hint(&u))
    }

    /// Explains an undefined variable and suggests a close name.
    fn undefined_hint(&self, u: &Undefined) -> String {
        let name = &u.name;
        if !u.owner.is_empty() {
            return format!(
                "Check the attribute name, or give it a fallback: \
                 {{{{ value.{name} | default('') }}}}."
            );
        }
        let names = self.visible_names();
        let near = closest(name, names.iter().map(String::as_str));
        if !near.is_empty() {
            return format!("Did you mean {}?", quoted(near));
        }
        format!(
            "Pass {} in the render context, set it with {{% set {name} = ... %}}, or guard it \
             with {{% if {name} is defined %}}.",
            quoted(name)
        )
    }

    /// The variables a template could have meant, sorted.
    fn visible_names(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut at = Some(self.current_context().root);
        while let Some(id) = at {
            let sc = &self.scopes[id.0 as usize];
            out.extend(sc.vars.iter().map(|(n, _)| n.to_string()));
            if let Some(d) = &sc.dict {
                out.extend(d.keys().iter().filter_map(|k| k.as_str().map(String::from)));
            }
            at = sc.parent;
        }
        out.extend(self.env.globals.keys().cloned());
        out.sort();
        out
    }

    /// Fails when v is undefined and the environment uses strict undefined values.
    pub(crate) fn strict_check(&self, v: &Value) -> Result<()> {
        if v.is_undefined() && self.env.options.undefined == UndefinedMode::Strict {
            return Err(self.undefined_error(v));
        }
        Ok(())
    }

    /// Truthiness, with strict undefined's error.
    pub(crate) fn is_true(&self, v: &Value) -> Result<bool> {
        self.strict_check(v)?;
        Ok(truthy(v))
    }

    /// Counts one loop iteration against Options::max_iterations.
    pub(crate) fn count_iteration(&mut self) -> Result<()> {
        self.iterations += 1;
        let max = self.env.options.max_iterations;
        if self.iterations > max {
            let message = format!("loop iteration limit of {max} reached");
            return Err(self.error(ErrorKind::Limit, message));
        }
        Ok(())
    }

    /// Enters one more level of macro call, include, block or recursive loop; the caller
    /// calls leave_depth after.
    pub(crate) fn enter_depth(&mut self) -> Result<()> {
        let max = self.env.options.max_recursion;
        if self.depth >= max {
            let message = format!(
                "recursion limit of {max} reached (macros, includes, blocks and recursive loops)"
            );
            return Err(self.error(ErrorKind::Limit, message).hinted(
                "Give the recursion a condition that ends it, or raise Options::max_recursion \
                 (and Options::stack_bytes with it).",
            ));
        }
        self.depth += 1;
        Ok(())
    }

    pub(crate) fn leave_depth(&mut self) {
        self.depth -= 1;
    }

    /// Fails once the stack has grown past Options::stack_bytes.
    pub(crate) fn check_stack(&self) -> Result<()> {
        if !self.stack.exhausted() {
            return Ok(());
        }
        let limit = self.stack.limit;
        let message = format!(
            "the template recurses too deeply for the stack budget of {limit} bytes (macros, \
             includes, recursive loops and nested expressions)"
        );
        Err(self.error(ErrorKind::Limit, message).hinted(
            "Make the recursion shallower or the expressions within it simpler, or raise \
             Options::stack_bytes and render on a thread with that much more stack.",
        ))
    }

    // -----------------------------------------------------------------------------------
    // Output

    /// Whether output goes nowhere: a child template that extends another writes only
    /// through its blocks.
    pub(crate) fn output_dropped(&self) -> bool {
        let ctx = self.current_context();
        ctx.parent.is_some() && self.out_id == ctx.top_out
    }

    pub(crate) fn emit(&mut self, text: &str) -> Result<()> {
        if self.output_dropped() {
            return Ok(());
        }
        let limit = self.env.options.max_output;
        if limit > 0 && text.len() > limit.saturating_sub(self.out.len()) {
            let message = format!("output limit of {limit} bytes reached");
            return Err(self.error(ErrorKind::Limit, message));
        }
        self.out.push_str(text);
        Ok(())
    }

    /// Prints an expression result, escaping it when autoescape is on.
    pub(crate) fn write_output(&mut self, v: &Value) -> Result<()> {
        if self.output_dropped() {
            return Ok(());
        }
        match v {
            Value::Undefined(_) => match self.env.options.undefined {
                UndefinedMode::Default | UndefinedMode::Chainable => Ok(()),
                UndefinedMode::Strict => Err(self.undefined_error(v)),
                UndefinedMode::Debug => self.emit(&debug_undefined(v)),
            },
            Value::Markup(text) => self.emit(text),
            Value::Str(text) if !self.autoescape => self.emit(text),
            _ if self.autoescape => {
                let text = to_str(v);
                let mut escaped = String::with_capacity(text.len() + 8);
                write_escaped(&mut escaped, &text);
                self.emit(&escaped)
            }
            _ => self.emit(&to_str(v)),
        }
    }

    /// Names the output buffer being written now.
    pub(crate) fn out_id(&self) -> usize {
        self.out_id
    }

    /// Redirects output into a fresh buffer until end_capture.
    pub(crate) fn begin_capture(&mut self) {
        let saved = std::mem::take(&mut self.out);
        self.captures.push((saved, self.out_id));
        self.next_out_id += 1;
        self.out_id = self.next_out_id;
    }

    /// Restores the output begin_capture redirected, returning what was captured.
    pub(crate) fn end_capture(&mut self) -> String {
        let Some((saved, id)) = self.captures.pop() else { return String::new() };
        self.out_id = id;
        std::mem::replace(&mut self.out, saved)
    }

    /// Captured output as a value: Markup when autoescape is on.
    pub(crate) fn captured(&self, text: String) -> Value {
        if self.autoescape { Value::Markup(Rc::from(text)) } else { Value::from(text) }
    }

    /// Makes a callable that only this render may call.
    pub(crate) fn render_callable(&self, name: Rc<str>, kind: CallKind) -> Value {
        Callable::value(name, kind, self.render_id)
    }
}

fn undefined_message(u: &Undefined) -> String {
    if !u.hint.is_empty() {
        return u.hint.to_string();
    }
    if u.owner.is_empty() {
        return format!("{} is undefined", quoted(&u.name));
    }
    format!("'{}' has no attribute {}", u.owner, quoted(&u.name))
}

fn debug_undefined(v: &Value) -> String {
    let u = v.undefined_info().unwrap_or_default();
    if !u.hint.is_empty() {
        return format!("{{{{ undefined value printed: {} }}}}", u.hint);
    }
    if u.owner.is_empty() {
        return format!("{{{{ {} }}}}", u.name);
    }
    format!("{{{{ no such element: {}[{}] }}}}", u.owner, quoted(&u.name))
}

// ---------------------------------------------------------------------------------------
// Statements

/// Runs statements of template t in order, stopping at break or continue.
pub(crate) fn exec_body(
    s: &mut State<'_>,
    t: &Rc<Compiled>,
    scope: ScopeId,
    body: &[Stmt],
) -> Result<Flow> {
    for n in body {
        s.line = n.line;
        s.check_stack()?;
        let flow = exec(s, t, scope, n)?;
        if flow != Flow::Normal {
            return Ok(flow);
        }
    }
    Ok(Flow::Normal)
}

/// Runs one statement of template t. Like eval, it only dispatches, so that its frame
/// stays small on the stack of every nested tag.
fn exec(s: &mut State<'_>, t: &Rc<Compiled>, scope: ScopeId, n: &Stmt) -> Result<Flow> {
    use crate::{calls, inherit};
    let done = |r: Result<()>| r.map(|()| Flow::Normal);
    match &n.kind {
        StmtKind::Data(start, end) => done(s.emit(t.data(*start, *end))),
        StmtKind::Output(e) => exec_output(s, scope, e),
        StmtKind::If(test, body, else_body) => exec_if(s, t, scope, test, body, else_body),
        StmtKind::For(id) => crate::loops::exec_for(s, t, scope, *id),
        StmtKind::Set(target, value) => exec_set(s, scope, target, value),
        StmtKind::SetBlock(target, filter, body) => {
            exec_set_block(s, t, scope, target, filter.as_ref(), body)
        }
        StmtKind::Macro(id) => {
            let m = calls::make_macro(s, t, scope, *id);
            s.set_var(scope, &t.macro_def(*id).name, m);
            Ok(Flow::Normal)
        }
        StmtKind::CallBlock(id, call) => done(calls::exec_call_block(s, t, scope, *id, call)),
        StmtKind::FilterBlock(filter, body) => exec_filter_block(s, t, scope, filter, body),
        StmtKind::With(pairs, body) => exec_with(s, t, scope, pairs, body),
        StmtKind::Block(id) => done(inherit::exec_block_tag(s, t, scope, *id)),
        StmtKind::Extends(e) => done(inherit::exec_extends(s, scope, e)),
        StmtKind::Include { template, ignore_missing, with_context } => {
            done(inherit::exec_include(s, scope, template, *ignore_missing, *with_context))
        }
        StmtKind::Import { .. } | StmtKind::FromImport { .. } => {
            done(inherit::exec_import(s, scope, &n.kind))
        }
        StmtKind::Autoescape(e, body) => exec_autoescape(s, t, scope, e, body),
        StmtKind::Do(e) => eval(s, scope, e).map(|_| Flow::Normal),
        StmtKind::Break => Ok(Flow::Break),
        StmtKind::Continue => Ok(Flow::Continue),
        StmtKind::Trans(trans) => done(crate::i18n::exec_trans(s, scope, trans)),
    }
}

#[inline(never)]
fn exec_output(s: &mut State<'_>, scope: ScopeId, e: &Expr) -> Result<Flow> {
    let v = eval(s, scope, e)?;
    s.write_output(&v)?;
    Ok(Flow::Normal)
}

#[inline(never)]
fn exec_if(
    s: &mut State<'_>,
    t: &Rc<Compiled>,
    scope: ScopeId,
    test: &Expr,
    body: &[Stmt],
    else_body: &[Stmt],
) -> Result<Flow> {
    let branch = if condition(s, scope, test)? { body } else { else_body };
    exec_body(s, t, scope, branch)
}

#[inline(never)]
fn exec_set(s: &mut State<'_>, scope: ScopeId, target: &Expr, value: &Expr) -> Result<Flow> {
    let v = eval(s, scope, value)?;
    assign(s, scope, target, v)?;
    Ok(Flow::Normal)
}

#[inline(never)]
fn exec_set_block(
    s: &mut State<'_>,
    t: &Rc<Compiled>,
    scope: ScopeId,
    target: &Expr,
    filter: Option<&Expr>,
    body: &[Stmt],
) -> Result<Flow> {
    let mut v = capture_body(s, t, scope, body)?;
    if let Some(chain) = filter {
        v = crate::calls::apply_filter_chain(s, scope, chain, v)?;
    }
    assign(s, scope, target, v)?;
    Ok(Flow::Normal)
}

#[inline(never)]
fn exec_filter_block(
    s: &mut State<'_>,
    t: &Rc<Compiled>,
    scope: ScopeId,
    filter: &Expr,
    body: &[Stmt],
) -> Result<Flow> {
    let input = capture_body(s, t, scope, body)?;
    let v = crate::calls::apply_filter_chain(s, scope, filter, input)?;
    s.emit(&to_str(&v))?;
    Ok(Flow::Normal)
}

#[inline(never)]
fn exec_with(
    s: &mut State<'_>,
    t: &Rc<Compiled>,
    scope: ScopeId,
    pairs: &[(Expr, Expr)],
    body: &[Stmt],
) -> Result<Flow> {
    let inner = with_scope(s, scope, pairs)?;
    exec_body(s, t, inner, body)
}

#[inline(never)]
fn exec_autoescape(
    s: &mut State<'_>,
    t: &Rc<Compiled>,
    scope: ScopeId,
    e: &Expr,
    body: &[Stmt],
) -> Result<Flow> {
    let on = truthy(&eval(s, scope, e)?);
    let saved = std::mem::replace(&mut s.autoescape, on);
    let flow = exec_body(s, t, scope, body)?;
    s.autoescape = saved;
    Ok(flow)
}

/// Evaluates a test (if, elif, the inline if, a loop filter) to a boolean.
pub(crate) fn condition(s: &mut State<'_>, scope: ScopeId, e: &Expr) -> Result<bool> {
    let v = eval(s, scope, e)?;
    s.is_true(&v)
}

/// Runs body into a buffer of its own, in a scope of its own, and returns the text.
fn capture_body(
    s: &mut State<'_>,
    t: &Rc<Compiled>,
    scope: ScopeId,
    body: &[Stmt],
) -> Result<Value> {
    s.begin_capture();
    let inner = s.new_scope(Some(scope));
    exec_body(s, t, inner, body)?;
    let text = s.end_capture();
    Ok(s.captured(text))
}

/// The scope of a with block, holding the values it assigns.
fn with_scope(s: &mut State<'_>, scope: ScopeId, pairs: &[(Expr, Expr)]) -> Result<ScopeId> {
    let inner = s.new_scope(Some(scope));
    let mut values = Vec::with_capacity(pairs.len());
    for (_, value) in pairs {
        values.push(eval(s, scope, value)?);
    }
    for ((target, _), v) in pairs.iter().zip(values) {
        assign(s, inner, target, v)?;
    }
    Ok(inner)
}

/// Stores v into a name, unpacks it into a tuple of targets, or sets a namespace
/// attribute.
pub(crate) fn assign(s: &mut State<'_>, scope: ScopeId, target: &Expr, v: Value) -> Result<()> {
    match &target.kind {
        ExprKind::Name(name) => s.set_var(scope, name, v),
        ExprKind::Tuple(targets) | ExprKind::List(targets) => {
            for (t, item) in targets.iter().zip(unpack(s, &v, targets.len())?) {
                assign(s, scope, t, item)?;
            }
        }
        ExprKind::NsRef(ns, attr) => match lookup(s, scope, ns) {
            Value::Dict(d) if d.kind() == DictKind::Namespace => {
                d.set(Value::from(attr.clone()), v);
            }
            _ => {
                let message = "cannot assign attribute on non-namespace object";
                return Err(s.error(ErrorKind::Type, message));
            }
        },
        // The parser accepts only the targets above; anything else would drop the value.
        _ => return Err(s.error(ErrorKind::Runtime, "cannot assign to an expression")),
    }
    Ok(())
}

/// The items of v for a tuple of `want` targets.
fn unpack(s: &State<'_>, v: &Value, want: usize) -> Result<Vec<Value>> {
    if !matches!(v, Value::List(_) | Value::Dict(_) | Value::Str(_) | Value::Markup(_)) {
        let message = format!("cannot unpack non-iterable {} object", type_name(v));
        return Err(s.error(ErrorKind::Type, message));
    }
    let items = iterate(s, v)?;
    let got = items.len();
    if got > want {
        let message = format!("too many values to unpack (expected {want})");
        return Err(s.error(ErrorKind::Type, message));
    }
    if got < want {
        let message = format!("not enough values to unpack (expected {want}, got {got})");
        return Err(s.error(ErrorKind::Type, message));
    }
    Ok(items)
}

/// The items a for loop sees: list items, dict keys, string characters.
pub(crate) fn iterate(s: &State<'_>, v: &Value) -> Result<Vec<Value>> {
    match v {
        Value::List(l) => Ok(l.snapshot()),
        Value::Dict(d) if d.kind() == DictKind::Dict => Ok(d.keys()),
        Value::Str(text) | Value::Markup(text) => {
            Ok(text.chars().map(|c| Value::from(c.to_string())).collect())
        }
        Value::Undefined(_) => s.strict_check(v).map(|()| Vec::new()),
        _ => {
            let message = format!("'{}' object is not iterable", type_name(v));
            Err(s.error(ErrorKind::Type, message))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assignment targets the parser never builds fail loudly instead of dropping the
    /// value. (Statements and expressions are separate types, so the other misplaced
    /// nodes of the Odin engine cannot be built at all.)
    #[test]
    fn misplaced_assignment_targets_are_errors() {
        let env = Environment::default();
        let mut s = State::new(&env, None);
        let root = s.root_scope();
        let name = Expr { line: 1, kind: ExprKind::Name(Rc::from("x")) };
        let target = Expr { line: 1, kind: ExprKind::Getattr(Box::new(name), Rc::from("a")) };
        let err = assign(&mut s, root, &target, Value::Int(1)).expect_err("assign");
        assert!(err.message.contains("cannot assign"), "{}", err.message);
    }
}
