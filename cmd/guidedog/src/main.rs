//! guidedog: builds HTML sites and PDF books from reStructuredText and Markdown. The
//! whole command line is the gd-cli crate; this program chooses the plugins linked in
//! (GitHub Pages and discussion catalogs), and adds the crash report and the stack its conversions
//! are sized for.

#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> =
        std::env::args_os().map(|a| a.to_string_lossy().into_owned()).collect();
    // A crash, which is always a bug, ends with an explanation and exit status 70.
    gd_host::install_crash_reporter(&args, &gd_cli::version_text());
    // Conversions size their stack budgets for WORKER_STACK_BYTES (see gd-host); the
    // main thread's stack is the system's choice, so the command runs on a thread of
    // its own. Ctrl+C is caught by the commands that must settle before stopping
    // (build, convert, publish); the others, serve among them, stop at once.
    let status = gd_host::catch_crash(|| {
        gd_host::run_with_stack(gd_host::WORKER_STACK_BYTES, || {
            gd_cli::run_status(args.get(1..).unwrap_or(&[]), &linked_plugins())
        })
    });
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}

/// The plugins every `guidedog` links: the GitHub Pages publisher (`pages`), which also
/// adds `guidedog publish pages` and `guidedog pages workflow`, and the discussion
/// catalogs (`dsc`), which add `guidedog dsc init`, `new`, `promote`, `build`, and more.
fn linked_plugins() -> gd_plugins::Registry {
    // The plugins are valid by construction; an empty registry is the fallback a broken
    // build would still run with.
    gd_plugins::Registry::new(vec![gd_pages::plugin(), gd_discussion::plugin(), gd_ssg::plugin()])
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_linked_plugins_are_valid_and_share_the_version() {
        let registry = super::linked_plugins();
        assert!(registry.find("pages").is_some() && registry.find("dsc").is_some());
        let names: Vec<String> = registry.commands().map(|(_, c)| c.name()).collect();
        for expected in ["publish pages", "pages workflow", "pages init", "dsc init"] {
            assert!(names.iter().any(|name| name == expected), "{names:?}");
        }
        assert!(names.contains(&"dsc build".to_string()), "{names:?}");
        assert_eq!(gd_pages::VERSION, gd_cli::VERSION);
    }
}
