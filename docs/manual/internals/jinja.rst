gd-jinja: design of the template engine
=======================================

gd-jinja is Guidedog's template engine for Jinja templates. This note records how the crate is built and why.

Pipeline
--------

1. ``lexer``: the source (newlines normalized, borrowed when it needs no change) becomes
   tokens whose text borrows the source (``Cow``), decoded only for string literals with
   escapes. Whitespace control (``-``, ``+``, ``trim_blocks``, ``lstrip_blocks``) and
   ``raw`` are handled here.
2. ``parser`` / ``parser_stmt`` / ``i18n``: tokens become a ``Compiled`` template.
   Expressions and statements are separate enums of boxed trees. Definitions that a
   render refers back to later (macros, blocks, for loops) live in side tables of the
   ``Compiled`` template and are named by newtype ids (``MacroId``, ``BlockId``,
   ``LoopId``). Template text is a byte range of the template's source.
3. ``runtime`` / ``eval`` / ``calls`` / ``loops`` / ``inherit``: a ``State`` renders a
   ``Compiled`` template. Statements return ``Result<Flow>`` and expressions
   ``Result<Value>``.

Values
------

``Value`` is an enum. Strings are ``Rc<str>``. Lists and dicts are ``Rc`` with interior
mutability, because Jinja's semantics need shared mutable containers: ``{% set b = a %}``
aliases, ``a is sameas b``, ``items.append(x)`` through any alias, and templates storing
values in the host's containers. Dicts keep insertion order (keys and values in vectors,
with a hash index of string keys). Dropping nested lists and dicts is iterative, so a
value nested thousands of levels deep cannot overflow the stack when it goes.

Scopes and contexts
-------------------

Scopes (variables) and contexts (one per template render: its root scope and block chain)
live in two vectors owned by the render and are named by ``ScopeId`` and ``ContextId``.
Scopes are tiny and use linear search. A macro, ``super()`` reference, ``self.name()`` or
recursive ``loop`` value keeps the ids it was made with. It is stamped with the id of its
render. Calling it from another render is an error ("made by a render that has
ended"). So a stored value never has to be copied out of its render.

Errors
------

The first error ends a render: every step returns ``Result`` and ``?`` carries the error
up. Inside the renderer the error is boxed (``Result<T, Box<Error>>``) so that the
results passed up through every level of recursion stay a few words wide. The public
``Error`` has plain fields (kind, template, line, message, excerpt, hint) and renders the
Elm-style report through ``Display``. Native procedures return ``Result<Value, Error>``;
``Call::error`` locates an error at the line being rendered.

Per-render functions
--------------------

An environment's functions, filters and loader are ``'static`` closures: they live as long
as the environment and can only own what they use. A host whose functions need data that
lives only for one render (a build's tables, the page being written) passes them as
``Locals<'a>`` to ``Template::render_with(&ctx, &locals)`` or
``Environment::render_str_with(source, &ctx, &locals)``. ``Locals::add_function`` and
``add_filter`` take ``impl Fn(&mut Call) -> Result<Value, Error> + 'a``, so the closures
may borrow anything that outlives the call.

The render's ``State`` holds ``Option<&Locals>``. Every template, include, extended
template, block, macro and call block of the render runs in that state, so locals are
visible everywhere in it. Name lookup is: the scopes and the context dict, then the
locals, then the environment's globals; filters look in the locals first. A local
function looked up becomes a ``CallKind::Local`` callable tagged with the render's id. It
finds its closure again by name when called. A copy kept past its render is refused, just
as a macro kept past its render is. "Did you mean" hints include the locals' names.

Limits and the stack
--------------------

Three limits keep hostile templates from overflowing the stack:

- ``max_nesting`` bounds the depth of syntax the parser builds. Every bracket, nested tag
  and link of an operator, filter, test, attribute or ``elif`` chain is one level.
- ``max_recursion`` bounds macro calls, includes, blocks and recursive loops.
- ``stack_bytes`` bounds the machine stack, measured from the address of a local in the
  first public call on the environment (a ``Drop`` guard stops the measurement). It is the
  backstop for recursion whose levels are each deep.

For the first two limits to come first, a level must be cheap in stack. Measured on
x86-64 Linux, a level of macro recursion takes about 1.9 KiB in a release build and
5.8 KiB in a debug one, and a bracket about 2 KiB and 3.5 KiB. The default budget (1 MiB,
1.5 MiB in debug builds) therefore holds 200 levels of recursion and 100 of nesting and
fits a 2 MiB spawned thread. Three choices keep frames small:

- ``eval`` and ``exec`` only dispatch: each node kind is handled by a helper marked
  ``#[inline(never)]``, so the large match does not reserve room for every arm's locals.
- The renderer's error is boxed (see above): an unboxed ``Result<Value, Error>`` is about
  five times wider, and debug builds keep a slot for every temporary.
- The expression parser climbs precedence in one loop (``parse_level``) instead of one
  function per level of Jinja's grammar, so a bracket costs about five frames rather than
  thirteen. Nesting is still counted per chain link.

Values have no syntax, so repr, equality, ordering and ``tojson`` stop at
``MAX_VALUE_DEPTH`` (200) instead. Values whose size is known before they are built (a
repetition, padding, a join) are sized first: past ``max_output`` they are a ``Limit``
error, a size that cannot be counted or reserved (``try_reserve``) is a ``Memory`` error.

Memory
------

- Values are owned: no arena or allocator guard is needed, and the standard allocator aborts rather than refusing small requests.
- Lists that contain themselves leak (an ``Rc`` cycle); Jinja's own behaviour relies on a
  garbage collector.

Why not minijinja
-----------------

Evaluated on 2026-10-15 against minijinja 3.0.0, the most widely used Rust Jinja engine.
**Recommendation: keep our own engine (gd-jinja). Do not adopt minijinja, and do not build a
hybrid.**

The decisive reason is bounds and safety. On hostile input, minijinja 3.0.0 overflows the
stack and aborts the process in cases where gd-jinja returns a ``Limit``
error, and it has no output or memory limit. The second reason is that minijinja behaves
differently from Jinja2 in scoping and escaping in ways its extension API cannot change. The
third is that the thin layer needed to restore gd-jinja's behaviour is already about 900 lines,
and it would keep growing toward a re-implementation of the builtins anyway. Speed and error
quality are about equal, so neither changes the decision.
