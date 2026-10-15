//! The problems every module shares: plain constructors over `gd_plugins::Problem`.

use gd_plugins::{Problem, ProblemData};

/// A problem with every field given and exit status 1; chain `.exit(n)` for another.
pub(crate) fn problem(code: &str, title: &str, path: &str, message: &str, hint: &str) -> Problem {
    Problem::from(ProblemData {
        code: code.to_string(),
        title: title.to_string(),
        path: path.to_string(),
        message: message.to_string(),
        hint: hint.to_string(),
        exit: 1,
    })
}

/// `pages.io` (exit 4): a path could not be read or written.
pub(crate) fn io_problem(path: &str, err: &std::io::Error) -> Problem {
    problem(
        "pages.io",
        "Cannot access this path",
        path,
        &err.to_string(),
        "Check that the path exists and that you may write it, then retry.",
    )
    .exit(4)
}

/// `pages.pack` (exit 4): the publication's Git objects could not be written.
pub(crate) fn pack_problem(e: &gd_git::Error) -> Problem {
    let path = e.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
    problem(
        "pages.pack",
        "The publication cannot be packed",
        &path,
        &e.message,
        "Check free disk space and that the site did not change while it was published, \
         then run the command again.",
    )
    .exit(4)
}

/// `pages.token` (exit 2): text that is not a GitHub token.
pub(crate) fn token_problem() -> Problem {
    problem(
        "pages.token",
        "This is not a GitHub token",
        "",
        "A token is one line of letters, digits, and underscores, such as github_pat_....",
        "Paste the whole token GitHub showed when you made it, and nothing else.",
    )
    .exit(2)
}
