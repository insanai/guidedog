//! Systems without a supported credential store: every operation is a problem, and the
//! environment's token is the way to publish.

use gd_plugins::Problem;

use super::Secret;
use crate::problem::problem;

/// The store's name in messages.
pub(super) const STORE_NAME: &str = "a credential store";

fn unsupported() -> Problem {
    problem(
        "pages.store",
        "The credential store cannot be used",
        STORE_NAME,
        "This system has no credential store guidedog pages can use.",
        "Give a token to one command with GUIDEDOG_PAGES_TOKEN and --credential=env.",
    )
    .exit(4)
}

pub(super) fn store(_service: &str, _profile: &str, _token: &str) -> Result<(), Problem> {
    Err(unsupported())
}

pub(super) fn load(_service: &str, _profile: &str) -> Result<Secret, Problem> {
    Err(unsupported())
}

pub(super) fn delete(_service: &str, _profile: &str) -> Result<bool, Problem> {
    Err(unsupported())
}
