//! Targets: what a repository and branch are, read before they are authorized, and the
//! binding that authorizes them.

use gd_plugins::{Cancel, Problem};

use crate::config::Config;
use crate::credential::Secret;
use crate::github::{
    Account, Repository, Site, account, commit_tree, configure_pages, pages_site, read_repository,
};
use crate::problem::problem;
use crate::push::branch_head;
use crate::session::MARK;
use crate::state::{Binding, Profile, State, today};
use crate::{Client, Settings};

/// What a target is, read before it is authorized: the token's account, the
/// repository, its Pages settings, and what the branch holds.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Review {
    /// The token's account.
    pub account: Account,
    /// The repository.
    pub repository: Repository,
    /// Its Pages settings.
    pub site: Site,
    /// The branch's commit, `None` when it does not exist.
    pub head: Option<String>,
    /// The branch holds a commit this publisher did not make.
    pub others: bool,
}

/// Reads the target of `config` through `c`. Fails when the branch is the repository's
/// default (`pages.branch`), or when GitHub cannot be read.
pub fn review_target(c: &Client, config: &Config) -> Result<Review, Problem> {
    let account = account(c)?;
    let repository = read_repository(c, &config.owner, &config.repo)?;
    if repository.default_branch == config.branch {
        return Err(default_branch(config, &repository));
    }
    let site = pages_site(c, &config.owner, &config.repo)?;
    let mut review = Review { account, repository, site, head: None, others: false };
    if let Some(head) = branch_head(c, &config.owner, &config.repo, &config.branch)? {
        let head = head.to_hex();
        let (_, message) = commit_tree(c, &config.owner, &config.repo, &head)?;
        review.others = !message.contains(MARK);
        review.head = Some(head);
    }
    Ok(review)
}

/// `pages.branch` (exit 2): the branch holds the repository's sources.
pub(crate) fn default_branch(config: &Config, r: &Repository) -> Problem {
    problem(
        "pages.branch",
        "The branch is the repository's default",
        &config.branch,
        &format!(
            "{} is the default branch of {}, which holds sources, not a published site.",
            config.branch, r.full_name
        ),
        "Publish to a branch of its own, such as branch = \"gh-pages\".",
    )
    .exit(2)
}

/// Authorizes the target of `config` for `profile`, as `review` found it. A branch that
/// holds work this publisher did not make is adopted (replaced by the next publication)
/// only when `adopt` is set; otherwise it is `pages.foreign`.
pub fn bind(
    state: &State,
    config: &Config,
    profile: &str,
    review: &Review,
    adopt: bool,
) -> Result<(), Problem> {
    let head = review.head.as_deref().unwrap_or_default();
    if review.others && !adopt {
        return Err(problem(
            "pages.foreign",
            "The branch holds work Guidedog did not publish",
            &config.branch,
            &format!(
                "{}, branch {}, holds commit {}, which this publisher did not make; \
                 publishing replaces all its files.",
                review.repository.full_name,
                config.branch,
                short(head)
            ),
            "Bind another branch, or add --adopt once you have checked that the branch's \
             files may be replaced.",
        )
        .exit(2));
    }
    state.save_binding(&Binding {
        profile: profile.to_string(),
        repository: review.repository.full_name.clone(),
        repository_id: review.repository.id,
        branch: config.branch.clone(),
        account: review.account.login.clone(),
        account_id: review.account.id,
        adopted: review.others.then(|| head.to_string()),
        bound: today(),
    })
}

/// Makes GitHub Pages publish the branch of `config` from its root, creating the Pages
/// site when `site` says there is none. It needs the Pages and Administration
/// permissions.
pub fn setup(c: &Client, config: &Config, site: &Site) -> Result<(), Problem> {
    configure_pages(c, &config.owner, &config.repo, &config.branch, !site.exists)
}

/// Checks `token` with GitHub and stores it as `profile`, by `method` (`device` or
/// `token`). It refuses to give a profile of one account another account's token
/// (`pages.account`) unless `replace` is set.
pub fn remember_token(
    settings: &Settings,
    profile: &str,
    token: &Secret,
    method: &str,
    replace: bool,
) -> Result<Account, Problem> {
    let client = settings.client(token.clone(), Cancel::default());
    let a = account(&client)?;
    let state = settings.state()?;
    if let Some(old) = state.find_profile(profile)?
        && old.account_id != a.id
        && !replace
    {
        return Err(problem(
            "pages.account",
            "The profile is another account's",
            profile,
            &format!(
                "The profile {profile} holds {}'s token; this token is {}'s.",
                old.account, a.login
            ),
            "Use another profile name, or add --replace to give the profile to this account.",
        )
        .exit(2));
    }
    settings.store.store_token(profile, token)?;
    state.save_profile(&Profile {
        name: profile.to_string(),
        account: a.login.clone(),
        account_id: a.id,
        method: method.to_string(),
        stored: today(),
    })?;
    Ok(a)
}

/// The first 12 characters of a commit ID, as people read it.
pub(crate) fn short(id: &str) -> &str {
    id.get(..12).unwrap_or(id)
}

/// What `review` found, as lines for people: the account, the repository with its
/// GitHub ID, the branch and what it holds, and the Pages settings.
pub fn review_lines(config: &Config, review: &Review) -> Vec<String> {
    let r = &review.repository;
    let visibility = if r.private { "private" } else { "public" };
    let mut lines = vec![
        format!("Account     {}", review.account.login),
        format!("Repository  {} ({visibility}, GitHub ID {})", r.full_name, r.id),
        format!("Branch      {}", config.branch),
    ];
    lines.push(match &review.head {
        None => "The branch does not exist yet; the first publication makes it.".to_string(),
        Some(head) if review.others => format!(
            "The branch holds commit {}, which Guidedog did not publish: publishing replaces \
             all its files.",
            short(head)
        ),
        Some(_) => "The branch holds earlier Guidedog publications.".to_string(),
    });
    lines.extend(site_lines(config, &review.site));
    lines
}

/// A repository's Pages settings as lines for people.
pub fn site_lines(config: &Config, s: &Site) -> Vec<String> {
    let mut lines = vec![if !s.readable {
        "Pages       the token may not read the Pages settings".to_string()
    } else if !s.exists {
        "Pages       not set up yet".to_string()
    } else if s.build_type == "workflow" {
        "Pages       deployed by a GitHub Actions workflow".to_string()
    } else {
        let note = if s.publishes(&config.branch) { "" } else { " (not this branch)" };
        format!("Pages       from {}, folder {}{note}", s.branch, s.path)
    }];
    if !s.cname.is_empty() {
        lines.push(format!("Domain      {}", s.cname));
    }
    lines
}
