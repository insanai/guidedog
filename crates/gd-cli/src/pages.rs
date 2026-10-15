//! `guidedog publish pages` and `guidedog pages workflow`: GitHub Pages, the bundled
//! publisher (gd-pages). Both are gd-pages's own command line; guidedog passes it the
//! words and, for the guided publication, itself as the program that builds.

use crate::console::Console;

/// Runs `guidedog-pages publish` with `args`: the guided publication, doing whatever
/// setup is still missing. Unless `--guidedog` names another, the project is built with
/// this very program (so a custom binary builds with its linked plugins).
pub(crate) fn publish_pages(console: &mut Console<'_>, args: &[String]) -> i32 {
    let mut words = Vec::with_capacity(args.len() + 2);
    words.push("publish".to_string());
    words.extend_from_slice(args);
    let named = args.iter().any(|a| a == "--guidedog" || a.starts_with("--guidedog="));
    let from_environment =
        std::env::var_os(gd_pages::GUIDEDOG_VARIABLE).is_some_and(|v| !v.is_empty());
    if !named
        && !from_environment
        && let Ok(this) = std::env::current_exe()
    {
        words.push(format!("--guidedog={}", this.display()));
    }
    run_pages(console, &words)
}

/// Runs `guidedog-pages workflow` with `args`.
pub(crate) fn pages_workflow(console: &mut Console<'_>, args: &[String]) -> i32 {
    let mut words = Vec::with_capacity(args.len() + 1);
    words.push("workflow".to_string());
    words.extend_from_slice(args);
    run_pages(console, &words)
}

fn run_pages(console: &mut Console<'_>, words: &[String]) -> i32 {
    gd_host::catch_interrupts();
    let program = gd_pages::command::Program {
        cancel: gd_plugins::Cancel::from_fn(gd_host::was_interrupted),
        ..gd_pages::command::Program::default()
    };
    gd_pages::command::run_with(&program, words, console.out, console.err)
}
