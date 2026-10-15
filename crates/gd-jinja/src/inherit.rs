//! Template inheritance (extends, blocks, super, self) and include / import.

use std::rc::Rc;

use crate::ast::{BlockId, Compiled, Expr, Name, StmtKind};
use crate::convert::{quoted, to_str, type_name};
use crate::error::ErrorKind;
use crate::eval::eval;
use crate::runtime::{BlockRef, BlockReference, ContextId, Result, ScopeId, State, exec_body};
use crate::value::{CallKind, Dict, Value};

/// Runs a template (and the templates it extends) in ctx, writing to the current output.
pub(crate) fn render_in_context(
    s: &mut State<'_>,
    ctx: ContextId,
    t: &Rc<Compiled>,
) -> Result<()> {
    let saved = s.enter(ctx, t, t.autoescape);
    let out_id = s.out_id();
    let c = s.context_mut(ctx);
    c.template = Some(t.clone());
    c.top_out = out_id;
    register_blocks(s, ctx, t);
    let root = s.context(ctx).root;
    exec_body(s, t, root, &t.body)?;
    // Each extends tag sets a parent; the parent's body then renders the child's blocks.
    while let Some(parent) = s.context_mut(ctx).parent.take() {
        s.context_mut(ctx).template = Some(parent.clone());
        s.current = Some(parent.clone());
        s.autoescape = parent.autoescape;
        exec_body(s, &parent, root, &parent.body)?;
    }
    s.restore(saved);
    Ok(())
}

/// Adds the blocks of t to the block chains of ctx.
fn register_blocks(s: &mut State<'_>, ctx: ContextId, t: &Rc<Compiled>) {
    let blocks = &mut s.context_mut(ctx).blocks;
    for (i, def) in t.blocks.iter().enumerate() {
        let r = BlockRef { template: t.clone(), id: BlockId(i as u32) };
        match blocks.iter_mut().find(|(name, _)| *name == def.name) {
            Some((_, refs)) => refs.push(r),
            None => blocks.push((def.name.clone(), vec![r])),
        }
    }
}

/// The template names an extends, include or import tag's value gives.
fn template_names(s: &State<'_>, v: &Value) -> Result<Vec<String>> {
    match v {
        Value::Str(name) | Value::Markup(name) => Ok(vec![name.to_string()]),
        Value::List(l) => Ok(l.items().iter().map(|item| to_str(item).into_owned()).collect()),
        Value::Undefined(_) => Err(s.undefined_error(v)),
        _ => {
            let message = format!("template name must be a string, not {}", type_name(v));
            Err(s.error(ErrorKind::Type, message))
        }
    }
}

/// Loads the first of names that exists; None when none does and missing ones are
/// ignored.
fn select_template(
    s: &State<'_>,
    names: &[String],
    ignore_missing: bool,
) -> Result<Option<Rc<Compiled>>> {
    for name in names {
        match s.env.load_compiled(name) {
            Ok(t) => return Ok(Some(t)),
            Err(err) if err.kind != ErrorKind::NotFound => return Err(Box::new(err)),
            Err(_) => {}
        }
    }
    if ignore_missing {
        return Ok(None);
    }
    let message = match names {
        [name] => format!("template {} not found", quoted(name)),
        _ => format!("none of the templates given were found: {}", names.join(", ")),
    };
    Err(s.error(ErrorKind::NotFound, message))
}

/// Finds the template an extends, include or import tag names.
fn find_template(
    s: &mut State<'_>,
    scope: ScopeId,
    e: &Expr,
    ignore_missing: bool,
) -> Result<Option<Rc<Compiled>>> {
    let v = eval(s, scope, e)?;
    let names = template_names(s, &v)?;
    select_template(s, &names, ignore_missing)
}

pub(crate) fn exec_extends(s: &mut State<'_>, scope: ScopeId, e: &Expr) -> Result<()> {
    if s.current_context().parent.is_some() {
        return Err(s.error(ErrorKind::Runtime, "extended multiple times"));
    }
    let Some(parent) = find_template(s, scope, e, false)? else { return Ok(()) };
    let ctx = s.ctx;
    s.context_mut(ctx).parent = Some(parent.clone());
    register_blocks(s, ctx, &parent);
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Blocks

pub(crate) fn exec_block_tag(
    s: &mut State<'_>,
    t: &Rc<Compiled>,
    scope: ScopeId,
    id: BlockId,
) -> Result<()> {
    if s.output_dropped() {
        return Ok(());
    }
    let def = t.block(id);
    let parent_scope = if def.scoped { scope } else { s.current_context().root };
    render_block(s, &def.name, 0, parent_scope)
}

/// Renders level `level` of the block called name.
fn render_block(
    s: &mut State<'_>,
    name: &Name,
    level: usize,
    parent_scope: ScopeId,
) -> Result<()> {
    let (r, inner) = block_scope(s, name, level, parent_scope)?;
    let saved = s.enter(s.ctx, &r.template, r.template.autoescape);
    exec_body(s, &r.template, inner, &r.template.block(r.id).body)?;
    s.restore(saved);
    Ok(())
}

/// Finds level of the block called name and makes the scope its body runs in, where
/// super() renders the next level.
fn block_scope(
    s: &mut State<'_>,
    name: &Name,
    level: usize,
    parent_scope: ScopeId,
) -> Result<(BlockRef, ScopeId)> {
    let refs = s.current_context().blocks.iter().find(|(n, _)| n == name).map(|(_, r)| r);
    let count = refs.map_or(0, Vec::len);
    let Some(r) = refs.and_then(|refs| refs.get(level)).cloned() else {
        let message = format!("there is no parent block called {}.", quoted(name));
        return Err(s.error(ErrorKind::Runtime, message));
    };
    if r.template.block(r.id).required {
        return Err(
            s.error(ErrorKind::Runtime, format!("Required block {} not found.", quoted(name)))
        );
    }
    let inner = s.new_scope(Some(parent_scope));
    let super_value = if level + 1 < count {
        let reference = BlockReference {
            name: name.clone(),
            level: level + 1,
            scope: parent_scope,
            ctx: s.ctx,
        };
        s.render_callable(Rc::from("super"), CallKind::Block(Rc::new(reference)))
    } else {
        let hint = format!("there is no parent block called {}.", quoted(name));
        Value::undefined_with("super", "", &hint)
    };
    s.set_var(inner, &Rc::from("super"), super_value);
    Ok((r, inner))
}

/// Renders a block into a value: super() and self.name().
pub(crate) fn call_block_reference(s: &mut State<'_>, r: &BlockReference) -> Result<Value> {
    s.enter_depth()?;
    let saved = std::mem::replace(&mut s.ctx, r.ctx);
    s.begin_capture();
    render_block(s, &r.name, r.level, r.scope)?;
    let text = s.end_capture();
    s.ctx = saved;
    s.leave_depth();
    Ok(s.captured(text))
}

/// The `self` object: one callable per block.
pub(crate) fn self_reference(s: &State<'_>) -> Value {
    let ctx = s.current_context();
    let name = ctx.template.as_ref().map_or_else(|| Rc::from(""), |t| t.name.clone());
    let module = Dict::module(name);
    for (block, _) in &ctx.blocks {
        let reference =
            BlockReference { name: block.clone(), level: 0, scope: ctx.root, ctx: s.ctx };
        let f = s.render_callable(block.clone(), CallKind::Block(Rc::new(reference)));
        module.set(Value::from(block.clone()), f);
    }
    Value::Dict(module)
}

// ---------------------------------------------------------------------------------------
// Include and import

pub(crate) fn exec_include(
    s: &mut State<'_>,
    scope: ScopeId,
    e: &Expr,
    ignore_missing: bool,
    with_context: bool,
) -> Result<()> {
    if s.output_dropped() {
        return Ok(());
    }
    let Some(t) = find_template(s, scope, e, ignore_missing)? else { return Ok(()) };
    s.enter_depth()?;
    let ctx = template_context(s, scope, with_context);
    render_in_context(s, ctx, &t)?;
    s.leave_depth();
    Ok(())
}

/// The context an included or imported template renders in, which sees the includer's
/// variables when the tag says "with context".
fn template_context(s: &mut State<'_>, scope: ScopeId, with_context: bool) -> ContextId {
    let root = s.new_scope(with_context.then_some(scope));
    s.new_context(root)
}

pub(crate) fn exec_import(s: &mut State<'_>, scope: ScopeId, kind: &StmtKind) -> Result<()> {
    let (template, with_context) = match kind {
        StmtKind::Import { template, with_context, .. }
        | StmtKind::FromImport { template, with_context, .. } => (template, *with_context),
        _ => return Ok(()),
    };
    let line = s.line;
    let Some(t) = find_template(s, scope, template, false)? else { return Ok(()) };
    s.enter_depth()?;
    let ctx = template_context(s, scope, with_context);
    s.begin_capture();
    render_in_context(s, ctx, &t)?;
    s.end_capture();
    s.leave_depth();
    s.line = line;
    let root = s.context(ctx).root;
    bind_module(s, scope, kind, &t, root);
    Ok(())
}

/// Makes the module an import tag imports from the variables the imported template
/// defined (root), and binds it, or the names imported from it, in scope.
fn bind_module(s: &mut State<'_>, scope: ScopeId, kind: &StmtKind, t: &Compiled, root: ScopeId) {
    let module = Dict::module(t.name.clone());
    for (key, value) in s.scope_vars(root) {
        if !key.starts_with('_') {
            module.set(Value::from(key.clone()), value.clone());
        }
    }
    match kind {
        StmtKind::Import { name, .. } => s.set_var(scope, name, Value::Dict(module)),
        StmtKind::FromImport { names, .. } => {
            for (name, alias) in names {
                let value = module.get_str(name).unwrap_or_else(|| {
                    let hint = format!(
                        "the template {} (imported on line {}) does not export the requested \
                         name {}",
                        quoted(&t.name),
                        s.line,
                        quoted(name)
                    );
                    Value::undefined_with(name, "", &hint)
                });
                s.set_var(scope, alias, value);
            }
        }
        _ => {}
    }
}
