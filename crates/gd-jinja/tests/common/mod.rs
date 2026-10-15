//! Test helpers: render a source with a context written as a Jinja dict literal.

#![allow(dead_code)]

use gd_jinja::{Environment, Error, Options, Value};

/// Renders source with ctx (a Jinja dict literal such as "{'a': 1}", or "" for none),
/// options, and templates added to the environment first.
pub fn run(
    source: &str,
    ctx: &str,
    options: Options,
    templates: &[(&str, &str)],
) -> Result<String, Error> {
    let env = Environment::new(options);
    for (name, text) in templates {
        env.add_template(name, text)?;
    }
    let data =
        if ctx.is_empty() { Value::default() } else { env.evaluate(ctx, &Value::default())? };
    env.render_str(source, &data)
}

/// Asserts that source renders to expected.
#[track_caller]
pub fn check(source: &str, expected: &str) {
    check_with(source, expected, "", Options::default(), &[]);
}

/// Asserts that source renders to expected with the context ctx.
#[track_caller]
pub fn check_ctx(source: &str, expected: &str, ctx: &str) {
    check_with(source, expected, ctx, Options::default(), &[]);
}

/// Asserts that source renders to expected with options.
#[track_caller]
pub fn check_opts(source: &str, expected: &str, options: Options) {
    check_with(source, expected, "", options, &[]);
}

/// Asserts that source renders to expected.
#[track_caller]
pub fn check_with(
    source: &str,
    expected: &str,
    ctx: &str,
    options: Options,
    templates: &[(&str, &str)],
) {
    match run(source, ctx, options, templates) {
        Ok(out) => assert_eq!(out, expected, "{source:?}"),
        Err(err) => panic!("{source:?}: unexpected error:\n{err}"),
    }
}

/// Asserts that rendering fails with a message containing want, and returns the error.
#[track_caller]
pub fn check_error(source: &str, want: &str) -> Error {
    check_error_with(source, want, "", Options::default(), &[])
}

/// Asserts that rendering fails with a message containing want, and returns the error.
#[track_caller]
pub fn check_error_with(
    source: &str,
    want: &str,
    ctx: &str,
    options: Options,
    templates: &[(&str, &str)],
) -> Error {
    match run(source, ctx, options, templates) {
        Ok(out) => panic!("{source:?}: expected an error containing {want:?}, got output {out:?}"),
        Err(err) => {
            assert!(
                err.message.contains(want),
                "{source:?}: error {:?} lacks {want:?}",
                err.message
            );
            err
        }
    }
}

/// Runs f on a thread with a large stack, for tests that recurse as deeply as the
/// default limits allow.
pub fn on_big_stack(f: impl FnOnce() + Send + 'static) {
    let worker = std::thread::Builder::new().stack_size(256 << 20).spawn(f);
    let finished = worker.map(std::thread::JoinHandle::join);
    assert!(matches!(finished, Ok(Ok(()))), "the test thread failed");
}
