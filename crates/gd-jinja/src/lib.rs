//! gd-jinja: a standalone Jinja 3.x template engine.
//!
//! It supports Jinja's tags, filters, tests and globals, template inheritance, macros,
//! autoescaping, and Elm-style error reports, without a Python environment.
//!
//! ```
//! use gd_jinja::{Environment, Options, Autoescape, Value};
//!
//! let env = Environment::new(Options { autoescape: Autoescape::Always, ..Options::default() });
//! let ctx = Value::dict();
//! ctx.set("name", Value::from("<World>"));
//! let out = env.render_str("Hello {{ name }}!", &ctx).unwrap();
//! assert_eq!(out, "Hello &lt;World&gt;!");
//! ```
//!
//! Every failure is an [`Error`] whose `Display` is the Elm-style report; nothing panics
//! on bad templates or data. See the crate README for the cheat sheet.

#![forbid(unsafe_code)]

mod ast;
mod calls;
mod convert;
mod environment;
mod error;
mod escape;
mod eval;
mod filters_html;
mod filters_seq;
mod filters_text;
mod format;
mod globals;
mod i18n;
mod inherit;
mod lexer;
mod limits;
mod loader;
mod locals;
mod loops;
mod methods;
mod parser;
mod parser_stmt;
mod runtime;
mod stack;
mod tests_builtin;
mod text;
mod value;

pub use calls::{Call, Keyword};
pub use convert::{MAX_VALUE_DEPTH, repr, to_str};
pub use environment::{Autoescape, Environment, Options, Template, UndefinedMode};
pub use error::{Error, ErrorKind};
pub use escape::{escape, escape_string};
pub use loader::{LoadFn, Loader};
pub use locals::{LocalFn, Locals};
pub use runtime::State;
pub use stack::{DEFAULT_MAX_NESTING, DEFAULT_MAX_RECURSION, DEFAULT_STACK_BYTES};
pub use value::{Callable, Cycler, Dict, DictKind, List, Loop, Native, Undefined, Value};

/// Renders source once with ctx (a dict value, or Undefined for none) in a default
/// environment: the one-call path.
///
/// # Errors
///
/// Returns the parse or render error, whose `Display` is an Elm-style report.
pub fn render_str(source: &str, ctx: &Value) -> Result<String, Error> {
    Environment::default().render_str(source, ctx)
}
