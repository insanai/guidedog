//! What a build lets through (its content policy) and how far each read and render may
//! go (its engine limits).

use gd_env::{BuildOptions, ContentRule, ContentRules};
use gd_host::Problem;
use guidedoc::{ContentPolicy, Limits, Policy};

/// How many diagnostics a build keeps for one document; more are counted in a note.
pub const DOCUMENT_REPORTS: usize = 1024;

/// The policy of a build of the user's own project: raw HTML and Typst, any URL and
/// untranslatable math are used as Sphinx uses them, and, as in Sphinx, a document with
/// an unknown directive or role is reported and still built.
pub fn trusted_policy() -> Policy {
    content_policy(ContentRules::ALLOW)
}

/// The policy of an `--untrusted` build of a project from elsewhere: raw content and
/// unsafe URLs (`javascript:` and the like) are left out with a warning. What such a
/// build reads is its confinement (`gd_env::Confinement::STRICT`), not its policy.
pub fn untrusted_policy() -> Policy {
    content_policy(ContentRules::OMIT)
}

/// The policy of a build with the content `rules` (`gd_env::BuildOptions::content_rules`):
/// raw content and unsafe URLs allowed or omitted as they say (an unstated rule
/// allows), untranslatable math allowed, and a document with an unknown directive or
/// role reported and still built.
pub fn content_policy(rules: ContentRules) -> Policy {
    let decide = |rule| match rule {
        ContentRule::Allow => ContentPolicy::Allow,
        ContentRule::Omit => ContentPolicy::Omit,
    };
    Policy {
        raw: decide(rules.raw_rule()),
        unsafe_urls: decide(rules.unsafe_url_rule()),
        keep_documents_with_errors: true,
        ..guidedoc::trusted_policy()
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
