//! What a build lets through (its content policy) and how far each read and render may
//! go (its engine limits).

use gd_env::BuildOptions;
use gd_host::Problem;
use guidedoc::{ContentPolicy, Limits, Policy};

/// How many diagnostics a build keeps for one document; more are counted in a note.
pub const DOCUMENT_REPORTS: usize = 1024;

/// The policy of a build of the user's own project: raw HTML and Typst, any URL and
/// untranslatable math are used as Sphinx uses them, and, as in Sphinx, a document with
/// an unknown directive or role is reported and still built.
pub fn trusted_policy() -> Policy {
    Policy { keep_documents_with_errors: true, ..guidedoc::trusted_policy() }
}

/// The policy of an `--untrusted` build of a project from elsewhere: raw content and
/// unsafe URLs (`javascript:` and the like) are left out with a warning. With
/// `gd_conf::confine_untrusted` it also reads files only below the source folder,
/// fetches nothing, runs no plugin, and reads no file a graph names.
pub fn untrusted_policy() -> Policy {
    Policy {
        raw: ContentPolicy::Omit,
        unsafe_urls: ContentPolicy::Omit,
        math: ContentPolicy::Allow,
        keep_documents_with_errors: true,
        ..guidedoc::default_policy()
    }
}

/// The limits every session of a build reads and renders with: the engine's defaults
/// with `--max-depth`, `--max-nodes` and `--stack-kib`. Documents are read and pages
/// rendered on threads of `gd_host::WORKER_STACK_BYTES` (the command line runs its
/// own work on one too), so the stack budget may be at most what such a thread holds.
///
/// Fails with `build.stack` (exit 2) when `--stack-kib` asks for more; a caller may
/// call it before building to refuse the options early.
pub fn build_limits(options: &BuildOptions) -> Result<Limits, Problem> {
    let mut limits = guidedoc::default_limits();
    // A page of a real manual can carry hundreds of diagnostics (CPython's os.rst has
    // 272), and a build counts every one for its summary and for -W.
    limits.max_reports = DOCUMENT_REPORTS;
    if let Some(depth) = options.max_depth {
        limits.max_depth = depth;
    }
    if let Some(nodes) = options.max_nodes {
        limits.max_nodes = nodes;
    }
    let stack = gd_host::WORKER_STACK_BYTES;
    let limit = gd_host::stack_budget_limit(stack);
    limits.stack_bytes = gd_host::default_stack_budget(stack);
    match options.stack_bytes {
        Some(asked) if asked > limit => Err(Problem::new(
            "build.stack",
            "STACK BUDGET TOO LARGE",
            "",
            format!(
                "A stack budget of {} KiB was asked for; the threads that read and render \
                 hold at most {} KiB.",
                asked >> 10,
                limit >> 10
            ),
            "Use a smaller --stack-kib.",
            2,
        )),
        Some(asked) if asked > 0 => {
            limits.stack_bytes = asked;
            Ok(limits)
        }
        _ => Ok(limits),
    }
}
