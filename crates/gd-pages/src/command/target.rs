//! `init`, `bind`, `unbind`, `setup`, and `doctor`.

use std::path::{Path, PathBuf};

use gd_plugins::Problem;

use super::display::{Console, confirm, not_confirmed, short};
use super::publish::{guidedog_missing, guidedog_version};
use super::{Context, usage};
use crate::config::{
    Config, SiteSource, is_valid_branch, is_valid_owner, is_valid_repo, read_config, render_config,
};
use crate::credential::Secret;
use crate::github::Site;
use crate::problem::problem;
use crate::target::{Review, review_target};

/// Writes a `pages.toml`; it changes nothing at GitHub.
pub(crate) fn init(ctx: &Context<'_>, console: &mut Console<'_>) -> i32 {
    let o = ctx.o;
    let (owner, repo) = o.repository.split_once('/').unwrap_or(("", ""));
    if !is_valid_owner(owner) || !is_valid_repo(repo) {
        return console.report(&usage(
            "Not a repository",
            &o.repository,
            "Write --repository=OWNER/REPOSITORY, as GitHub shows it.",
        ));
    }
    let source = match (o.project.is_empty(), o.site.is_empty()) {
        (false, true) => SiteSource::Project(PathBuf::from(&o.project)),
        (true, false) => SiteSource::Built(PathBuf::from(&o.site)),
        _ => {
            let given = if o.project.is_empty() { &o.site } else { &o.project };
            return console.report(&usage(
                "Name one source",
                given,
                "Give --project=DIR to \
                build a Guidedog project, or --site=DIR to publish a site you build yourself.",
            ));
        }
    };
    if !is_valid_branch(&o.branch) {
        return console.report(&usage(
            "Not a branch name",
            &o.branch,
            "Name a branch of its own, such as --branch=gh-pages.",
        ));
    }
    let path = Path::new(&o.config);
    if path.exists() && !o.force {
        return console.report(
            &problem(
                "pages.exists",
                "The configuration exists",
                &o.config,
                "A configuration is there already.",
                "Edit it, or add --force to write a new one \
            over it.",
            )
            .exit(2),
        );
    }
    let text = render_config(owner, repo, &o.branch, &source, &o.profile);
    if let Err(e) = std::fs::write(path, text) {
        return console.report(
            &problem(
                "pages.io",
                "Cannot write the configuration",
                &o.config,
                &e.to_string(),
                "Check the folder's permissions.",
            )
            .exit(4),
        );
    }
    console.success(&format!("Wrote {}", o.config));
    console.say(&format!(
        "Next: guidedog-pages auth login --profile={}, then guidedog-pages \
        bind --config={}.",
        o.profile, o.config
    ));
    0
}

/// Reads the configuration and the token it names.
fn config_and_token(ctx: &Context<'_>) -> Result<(Config, Secret), Problem> {
    let c = read_config(Path::new(&ctx.o.config))?;
    let token = ctx.settings.load_token(ctx.settings.profile_for(&c.profile))?;
    Ok((c, token))
}

/// Reads the configuration and reviews its target.
fn review(ctx: &Context<'_>) -> Result<(Config, crate::Client, Review), Problem> {
    let (c, token) = config_and_token(ctx)?;
    let client = ctx.settings.client(token, ctx.cancel.clone());
    let r = review_target(&client, &c)?;
    Ok((c, client, r))
}

/// Shows the target and authorizes it on this machine, once the user confirms.
pub(crate) fn bind(ctx: &Context<'_>, console: &mut Console<'_>) -> i32 {
    let (c, _, r) = match review(ctx) {
        Ok(found) => found,
        Err(p) => return console.report(&p),
    };
    let visibility = if r.repository.private { "private" } else { "public" };
    console.print("\n");
    console.line(&format!("Account     {}", r.account.login));
    console.line(&format!(
        "Repository  {} ({visibility}, GitHub ID {})",
        r.repository.full_name, r.repository.id
    ));
    console.line(&format!("Branch      {}", c.branch));
    match &r.head {
        None => console.line("The branch does not exist yet; the first publication makes it."),
        Some(head) if r.others => console.line(&format!(
            "The branch holds commit {}, which \
            Guidedog did not publish: publishing replaces all its files.",
            short(head)
        )),
        Some(_) => console.line("The branch holds earlier Guidedog publications."),
    }
    describe_site(console, &c, &r.site);
    if r.repository.private {
        console.say(
            "A site published from a private repository is not necessarily private: \
            check its visibility in the repository's Pages settings.",
        );
    }
    if !confirm(ctx.o, "Let this machine publish there with this account?") {
        return console.report(&not_confirmed("binding the target"));
    }
    let bound =
        ctx.settings.state().and_then(|s| crate::bind(&s, &c, &c.profile, &r, ctx.o.adopt));
    if let Err(p) = bound {
        return console.report(&p);
    }
    console.success(&format!(
        "Bound {}, branch {}, to profile {}",
        r.repository.full_name, c.branch, c.profile
    ));
    if !r.site.publishes(&c.branch) {
        console.say(&format!(
            "Next: guidedog-pages setup --config={} points GitHub Pages at the \
            branch.",
            ctx.o.config
        ));
    }
    0
}

fn describe_site(console: &mut Console<'_>, c: &Config, s: &Site) {
    if !s.readable {
        console.line("Pages       the token may not read the Pages settings");
    } else if !s.exists {
        console.line("Pages       not set up yet");
    } else if s.build_type == "workflow" {
        console.line("Pages       deployed by a GitHub Actions workflow");
    } else {
        let note = if s.branch == c.branch && s.path == "/" { "" } else { " (not this branch)" };
        console.line(&format!("Pages       from {}, folder {}{note}", s.branch, s.path));
    }
    if !s.cname.is_empty() {
        console.line(&format!("Domain      {}", s.cname));
    }
    console.line("");
}

/// Removes the target's binding from this machine.
pub(crate) fn unbind(ctx: &Context<'_>, console: &mut Console<'_>) -> i32 {
    let c = match read_config(Path::new(&ctx.o.config)) {
        Ok(c) => c,
        Err(p) => return console.report(&p),
    };
    let repository = c.repository();
    match ctx.settings.state().and_then(|s| s.remove_binding(&repository, &c.branch)) {
        Ok(true) => console.success(&format!(
            "Unbound {repository}, branch {}: this machine no \
            longer publishes there",
            c.branch
        )),
        Ok(false) => console.success(&format!(
            "{repository}, branch {}, was not bound on this \
            machine",
            c.branch
        )),
        Err(p) => return console.report(&p),
    }
    0
}

/// Points GitHub Pages at the configured branch, once the user confirms.
pub(crate) fn setup(ctx: &Context<'_>, console: &mut Console<'_>) -> i32 {
    let (c, client, r) = match review(ctx) {
        Ok(found) => found,
        Err(p) => return console.report(&p),
    };
    describe_site(console, &c, &r.site);
    if r.site.publishes(&c.branch) {
        console.success(&format!(
            "GitHub Pages already publishes {} from {}",
            r.repository.full_name, c.branch
        ));
        return 0;
    }
    if r.head.is_none() {
        return console.report(
            &problem(
                "pages.no-branch",
                "The branch does not exist yet",
                &c.branch,
                "GitHub sets Pages up only for a branch that exists.",
                &format!(
                    "Run \
            guidedog-pages publish --config={} first; it makes the branch. Then run setup \
            again.",
                    ctx.o.config
                ),
            )
            .exit(2),
        );
    }
    if r.site.build_type == "workflow" {
        console.say(
            "The site is now deployed by a GitHub Actions workflow; after this, its \
            deployments are not seen. Disable that workflow too.",
        );
    }
    let question = format!(
        "Make GitHub Pages publish {} from {}, folder /?",
        r.repository.full_name, c.branch
    );
    if !confirm(ctx.o, &question) {
        return console.report(&not_confirmed("changing the Pages source"));
    }
    if let Err(p) = crate::setup(&client, &c, &r.site) {
        return console.report(&p);
    }
    console.success(&format!(
        "GitHub Pages now publishes {} from {}",
        r.repository.full_name, c.branch
    ));
    0
}

/// Checks everything a publication needs, writing nothing, and says what to fix.
pub(crate) fn doctor(ctx: &Context<'_>, console: &mut Console<'_>) -> i32 {
    match diagnose(ctx, console) {
        Ok(()) => 0,
        Err(p) => console.report(&p),
    }
}

fn diagnose(ctx: &Context<'_>, console: &mut Console<'_>) -> Result<(), Problem> {
    let c = read_config(Path::new(&ctx.o.config))?;
    console.success(&format!(
        "{} is valid: {}, branch {}, profile {}",
        c.shown_path(),
        c.repository(),
        c.branch,
        c.profile
    ));
    let folder = c.source.folder();
    if !folder.exists() {
        return Err(problem(
            "pages.source-missing",
            "The source is missing",
            &folder.display().to_string(),
            "There is nothing to build or publish here.",
            "Point project or site in pages.toml at the right folder.",
        )
        .exit(2));
    }
    if matches!(c.source, SiteSource::Project(_)) {
        let found = guidedog_version(ctx.o).ok_or_else(|| guidedog_missing(ctx.o))?;
        console.success(&format!("Builds with {found}"));
    }
    let (_, _, r) = review(ctx)?;
    console.success(&format!(
        "The token is {}'s, and it can read {}",
        r.account.login, r.repository.full_name
    ));
    if !r.repository.can_push {
        return Err(problem(
            "pages.permission",
            "The token may not push",
            &r.repository.full_name,
            "GitHub says this account may not push to the repository.",
            "Give the token Contents: read and write for the repository.",
        )
        .exit(2));
    }
    let binding = ctx.settings.state()?.find_binding(&r.repository.full_name, &c.branch)?;
    let Some(binding) = binding.filter(|b| b.repository_id == r.repository.id) else {
        return Err(problem(
            "pages.unbound",
            "This target is not authorized",
            &r.repository.full_name,
            "No binding on this machine lets it publish there.",
            &format!("Run guidedog-pages bind --config={}.", ctx.o.config),
        )
        .exit(2));
    };
    console.success(&format!("Bound to profile {} on {}", binding.profile, binding.bound));
    describe_site(console, &c, &r.site);
    if r.site.readable && !r.site.exists && r.head.is_none() {
        console.success(
            "Ready for a first publication: run publish, then setup to deploy the \
            branch",
        );
        return Ok(());
    }
    if r.site.readable && !r.site.publishes(&c.branch) {
        return Err(problem(
            "pages.source",
            "GitHub Pages does not publish this branch",
            &r.repository.full_name,
            "Pages is not set up to publish the branch from its root.",
            &format!("Run guidedog-pages setup --config={}.", ctx.o.config),
        )
        .exit(2));
    }
    console.success(&format!("Ready to publish to {}", c.site_url));
    Ok(())
}
