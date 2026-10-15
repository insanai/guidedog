//! Functions and filters of one render, which may borrow the caller's data.
//!
//! An environment's functions and filters are `'static`: they live as long as the
//! environment and can only own what they use. [`Locals`] holds natives for a single
//! render (`Template::render_with`, `Environment::render_str_with`), so they may borrow
//! anything that outlives the call: a database handle, a table of pages, the page being
//! rendered. They are visible everywhere during that render (included templates,
//! templates extended, blocks, macros and call blocks) and shadow the environment's
//! globals and filters of the same name; the context's own variables still come first.

use std::fmt;

use crate::calls::Call;
use crate::error::Error;
use crate::value::Value;

/// A native procedure of one render; it may borrow data that lives for `'a`.
pub type LocalFn<'a> = dyn Fn(&mut Call<'_, '_>) -> Result<Value, Error> + 'a;

/// The functions and filters one render adds to its environment's.
///
/// ```
/// use gd_jinja::{Environment, Locals, Value};
/// let titles = vec!["Intro".to_string(), "Usage".to_string()];
/// let mut locals = Locals::new();
/// locals.add_function("title", |c| {
///     let i = c.arg_count(0, "index", 0)?;
///     Ok(Value::from(titles.get(i).map_or("", String::as_str)))
/// });
/// let out = Environment::default().render_str_with("{{ title(1) }}", &Value::default(), &locals);
/// assert_eq!(out.unwrap(), "Usage");
/// ```
#[derive(Default)]
pub struct Locals<'a> {
    functions: Vec<(String, Box<LocalFn<'a>>)>,
    filters: Vec<(String, Box<LocalFn<'a>>)>,
}

impl fmt::Debug for Locals<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = |list: &[(String, Box<LocalFn<'_>>)]| {
            list.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>()
        };
        f.debug_struct("Locals")
            .field("functions", &names(&self.functions))
            .field("filters", &names(&self.filters))
            .finish()
    }
}

impl<'a> Locals<'a> {
    /// No functions or filters.
    pub fn new() -> Locals<'a> {
        Locals::default()
    }

    /// Adds a function called `name`, replacing one added before under that name.
    pub fn add_function(
        &mut self,
        name: &str,
        f: impl Fn(&mut Call<'_, '_>) -> Result<Value, Error> + 'a,
    ) -> &mut Locals<'a> {
        put(&mut self.functions, name, Box::new(f));
        self
    }

    /// Adds a filter called `name` (`call.args[0]` is the value filtered), replacing one
    /// added before under that name.
    pub fn add_filter(
        &mut self,
        name: &str,
        f: impl Fn(&mut Call<'_, '_>) -> Result<Value, Error> + 'a,
    ) -> &mut Locals<'a> {
        put(&mut self.filters, name, Box::new(f));
        self
    }

    /// The function called `name`.
    pub(crate) fn function(&self, name: &str) -> Option<&LocalFn<'a>> {
        find(&self.functions, name)
    }

    /// The filter called `name`.
    pub(crate) fn filter(&self, name: &str) -> Option<&LocalFn<'a>> {
        find(&self.filters, name)
    }

    /// The names of the functions, for "Did you mean" hints.
    pub(crate) fn function_names(&self) -> impl Iterator<Item = &str> {
        self.functions.iter().map(|(n, _)| n.as_str())
    }

    /// The names of the filters, for "Did you mean" hints.
    pub(crate) fn filter_names(&self) -> impl Iterator<Item = &str> {
        self.filters.iter().map(|(n, _)| n.as_str())
    }
}

fn put<'a>(list: &mut Vec<(String, Box<LocalFn<'a>>)>, name: &str, f: Box<LocalFn<'a>>) {
    match list.iter_mut().find(|(n, _)| n == name) {
        Some(slot) => slot.1 = f,
        None => list.push((name.to_string(), f)),
    }
}

fn find<'l, 'a>(list: &'l [(String, Box<LocalFn<'a>>)], name: &str) -> Option<&'l LocalFn<'a>> {
    list.iter().find(|(n, _)| n == name).map(|(_, f)| &**f)
}
