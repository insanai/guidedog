# gd-jinja cheat sheet

A standalone Jinja 3.x template engine: tags, filters, tests, globals, template
inheritance, macros, autoescaping and Elm-style error reports, without Python. Standard
library only. Design notes: `docs/manual/internals/jinja.rst`.

```rust
use gd_jinja::{Autoescape, Environment, Loader, Options, Value};

let env = Environment::new(Options {
    autoescape: Autoescape::Always,
    loader: Some(Loader::filesystem(["themes/mine", "themes/basic"])),
    ..Options::default()
});
let ctx = Value::dict();
ctx.set("title", Value::from("Hello <World>"));
ctx.set("pages", Value::list([Value::from("a"), Value::from("b")]));
match env.load_template("page.html").and_then(|t| t.render(&ctx)) {
    Ok(html) => println!("{html}"),
    Err(err) => eprint!("{err}"), // the Elm-style report
}

// One call, default environment:
let out = gd_jinja::render_str("Hi {{ name }}", &ctx)?;
```

## Environment

| Call | Does |
|---|---|
| `Environment::new(options)` / `Environment::default()` | filters, tests, globals and a template cache |
| `env.load_template(name) -> Result<Template, Error>` | through the loader; cached |
| `env.parse_template(name, source)` | from a string; not cached |
| `env.add_template(name, source)` | parse and cache, so `extends`/`include` find it |
| `template.render(&ctx) -> Result<String, Error>` | `ctx` is a dict value, or `Value::default()` |
| `template.render_with(&ctx, &locals)` | with per-render functions and filters (`Locals`) |
| `env.render_str(source, &ctx)` | parse and render once, caching nothing |
| `env.render_str_with(source, &ctx, &locals)` | the same with `Locals` |
| `env.evaluate("a.b + 1", &ctx) -> Result<Value, Error>` | one expression (`compile_expression`) |
| `env.clear_cache()` | reload templates on the next `load_template` |
| `gd_jinja::render_str(source, &ctx)` | the one-call path |

A `Template<'env>` borrows its environment. An environment is not thread-safe; use one
per thread.

`Options` (plain fields, `Default`):

| Field | Default | Meaning |
|---|---|---|
| `autoescape` | `Never` | `Always`, or `ForExtensions(vec![".html".into()])` per template name |
| `trim_blocks`, `lstrip_blocks` | false | Jinja's whitespace options |
| `keep_trailing_newline` | false | keep the source's final newline |
| `undefined` | `Default` | `Strict`, `Chainable` or `Debug`, like Jinja's Undefined classes |
| `loader` | `None` | where `load_template`, `extends`, `include` and `import` read |
| `max_recursion` | 200 | nested macros, includes, blocks and recursive loops |
| `max_iterations` | 10,000,000 | loop iterations (and `range()` sizes) per render |
| `max_output` | 0 = unlimited | bytes per render, and per value a render builds, checked before allocating |
| `max_nesting` | 100 | how deep syntax nests: brackets, tags, operator/filter/attribute/elif chains |
| `stack_bytes` | 1 MiB (1.5 MiB debug) | machine stack parsing and rendering may use |
| `random_seed` | 0 = fixed | seeds `random` and `lipsum`; renders are deterministic |

## Loaders

| Loader | Serves |
|---|---|
| `Loader::memory([(name, source), ...])` | templates held in memory |
| `Loader::filesystem([dir1, dir2])` | the first directory holding the name; `..` is refused |
| `Loader::custom(\|name\| -> Option<String>)` | your own |

## Values

`Value` is an enum: `Undefined(Option<Rc<Undefined>>)`, `None`, `Bool`, `Int(i64)`,
`Float`, `Str(Rc<str>)`, `Markup(Rc<str>)`, `List(Rc<List>)` (list or tuple),
`Dict(Rc<Dict>)` (ordered; also namespaces and modules), `Callable`, `Loop`, `Cycler`.
`Value::default()` is Undefined. Containers are shared: cloning a value shares it, as in
Python.

| Build | Read |
|---|---|
| `Value::from("x" / 1 / 1.5 / true)`, `Value::None` | `v.get("key")`, `v.len()`, `v.keys()` |
| `Value::markup("<b>")`, `Value::undefined("name")` | `v.to_string()` (Python `str`), `v.repr()` |
| `Value::list(iter)`, `Value::tuple(iter)`, `v.push(x)` | `v.is_truthy()`, `v == w` (Python `==`) |
| `Value::dict()`, `v.set("k", x)`, `v.set_key(k, x)` | `v.type_name()`, `v.is_undefined()`, `v.is_none()`, `v.is_markup()` |
| `Value::function("name", \|call\| ...)` | `escape(&v)`, `escape_string(s)`, `to_str(&v)`, `repr(&v)` |

Integers are 64-bit: overflow is an error, not a wrap.

## Your own filters, tests and globals

Every native has one signature, `Fn(&mut Call) -> Result<Value, Error>`; for filters and
tests `call.args[0]` is the value being filtered or tested.

```rust
let mut env = Environment::default();
env.add_filter("shout", |c| {
    let times = c.arg_count(1, "times", 1)?;            // positional 1 or times=...
    Ok(Value::from(gd_jinja::to_str(&c.args[0]).to_uppercase().repeat(times)))
});
env.add_test("long", |c| Ok(Value::from(c.args[0].len() > 40)));
env.add_function("pathto", |_| Ok(Value::from("index.html")));
env.add_global("project", Value::from("Docs"));
```

| `Call` helper | Does |
|---|---|
| `arg(i, "name")`, `arg_or(i, "name", default)`, `keyword("name")` | positional, else keyword, else default |
| `arg_int`, `arg_count`, `arg_flag` | typed arguments |
| `has_arg(i, "name")` | was it passed? |
| `check_args("who", &["value", "a"], required)` | Python-style argument checking |
| `error("message")` | an error located where the call happens: `return Err(c.error(..))` |
| `autoescaping()` | is autoescape on where the call happens? |
| `call_filter(name, args, kwargs)`, `call_test(...)` | reuse registered ones |
| `c.this`, `c.name`, `c.kwargs` | method receiver, called name, keywords |

## Per-render functions that borrow your data

Environment functions are `'static`. For functions that need data living only for one
render, pass `Locals`: they may borrow, are visible in every template, block, macro and
call block of that render, and shadow globals and filters of the same name (context
variables still come first).

```rust
let pages = vec!["intro".to_string()];
let mut locals = gd_jinja::Locals::new();
locals.add_function("page", |c| Ok(Value::from(pages[c.arg_count(0, "i", 0)?].as_str())));
locals.add_filter("shout", |c| Ok(Value::from(gd_jinja::to_str(&c.args[0]).to_uppercase())));
let html = env.load_template("page.html")?.render_with(&ctx, &locals)?;
```

A local function kept past its render (stored in a dict the host keeps) cannot be called
any more, as a macro.

## Errors

`Error { kind, template, line, message, excerpt, hint }` with kinds `Syntax`, `Undefined`,
`Type`, `NotFound`, `Limit`, `Memory`, `Runtime`. `Display` renders the Elm-style report:

```
-- UNDEFINED VALUE -------------------------------------------------- page.html:3

'usr' is undefined

3| <h1>{{ usr.name }}</h1>

Hint: Did you mean 'user'?
```

Hints suggest close names for variables, filters, tests and tags. Nothing panics on bad
templates or data: deep nesting and runaway recursion are `Limit` errors
(`max_nesting`, `max_recursion`, and `stack_bytes` as the backstop, measured from the
first call on the environment); repr, equality, ordering and `tojson` follow values 200
levels deep (`MAX_VALUE_DEPTH`).

## Language coverage

Tags: `if/elif/else`, `for` (with `else`, `if` filter, `recursive`, unpacking, `break`,
`continue`), `set` (plain, tuple, block with filters, `ns.attr`), `with`, `filter`,
`macro` (defaults, `varargs`, `kwargs`, `caller`), `call`, `extends`, `block` (`super()`,
`scoped`, `required`, `self.name()`), `include` (lists, `ignore missing`, with/without
context), `import`, `from ... import ... as`, `raw`, `autoescape`, `do`, `trans` /
`pluralize` (pass-through `gettext`; register your own globals to translate). Whitespace
control with `-` and `+`, comments, `trim_blocks`, `lstrip_blocks`.

Filters: `abs attr batch capitalize center count d default dictsort e escape
filesizeformat first float forceescape format groupby indent int items join last length
list lower map max min pprint random reject rejectattr replace reverse round safe select
selectattr slice sort string striptags sum title tojson trim truncate unique upper
urlencode urlize wordcount wordwrap xmlattr`.

Tests: `boolean callable defined divisibleby eq equalto == escaped even false filter float
ge >= gt > greaterthan in integer iterable le <= lower lt < lessthan mapping ne != none
number odd sameas sequence string test true undefined upper`.

Globals: `range dict lipsum cycler joiner namespace gettext _ ngettext`. Methods of
Python's `str`, `list` and `dict` work as in Jinja.

## Differences from Jinja

- Line statements are not supported; `break`, `continue`, `do` and `trans` are always on.
- An unknown filter or test is reported when it runs, not when the template is parsed.
- Integers are 64-bit; `range(3)` prints as `[0, 1, 2]`.
- Case mapping is Unicode's simple mapping (`'ß'|upper` stays `ß`); `striptags` decodes
  numeric and common named entities only.
- `random` and `lipsum` use their own seeded generator; `pprint` is `repr` without
  wrapping; `urlize` follows Jinja 3.1's rules, not its exact regular expressions.
- A macro, `joiner` or block reference stored in your containers belongs to the render
  that made it: calling it from a later render is an error (import macros to share them).
- A list that contains itself is kept alive by its own reference (`Rc` cycle) until the
  process ends.

## Changes from the Odin engine

- Every step returns `Result` instead of setting a sticky failure flag; natives return
  `Result<Value, Error>` and use `?`.
- No arenas, allocator guards or ownership copying: values are `Rc`-shared, and values a
  template stores in your containers simply stay there. `Memory` errors remain for sizes
  that cannot be counted or reserved (`try_reserve`).
- `Options` hold real defaults (no 0-means-default), `Autoescape` is an enum, `Loader` an
  enum with a `Custom` closure, and a `Template` borrows its environment.
- The expression parser climbs precedence in one loop instead of one function per
  grammar level, so a bracket costs a few stack frames.
