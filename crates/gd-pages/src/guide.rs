//! The guided publish: one run that does whatever is still missing, then publishes.
//!
//! ```text
//! pages.toml    missing: repository from `git remote get-url origin` (or given),
//!               project from the nearest conf.toml (or given); written once
//! token         missing: GitHub's device flow (stored in the credential store), or
//!               the GitHub CLI's token for this run only
//! binding       missing: the target's review is shown, and the user asked once
//! Pages source  another branch or Actions: setup, asked once
//! publish       build, one push, wait for the deployment; a repository without a
//!               Pages site is set up after the first push, which makes the branch
//! ```
//!
//! Every step is skipped when it is already done, so a configured machine publishes
//! without a question. A run that may not ask (`interactive` false) never prompts: a
//! missing step that needs the user fails with a hint naming the command or flag.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gd_plugins::{
    Cancel, ChangeKind, LoadOptions, Outcome, Plan, Plugin, Problem, PublisherSession, State,
};

use crate::config::{Config, SiteSource, is_valid_owner, is_valid_repo, read_config};
use crate::credential::{Credential, Secret};
use crate::device::{CLIENT_ID, start_device, wait_device};
use crate::github::{Site, pages_site};
use crate::infer::{find_project, gh_token, origin_url, parse_github_remote};
use crate::problem::{not_confirmed, problem};
use crate::push::branch_head;
use crate::target::{bind, remember_token, review_lines, review_target, setup, short};
use crate::{Client, Pages, Settings, freeze_site, guidedog_program, render_config};

/// How the guided publish talks to a person. The command line answers on the terminal;
/// tests answer from a script.
pub trait Prompt {
    /// Shows a line of progress, a review, or a plan.
    fn show(&mut self, text: &str);
    /// Asks a yes-or-no question; `false` unless the person says yes.
    fn confirm(&mut self, question: &str) -> bool;
    /// Asks the person to choose one of `choices`; `None` when they choose none.
    fn choose(&mut self, question: &str, choices: &[&str]) -> Option<usize>;
}

/// A guided publish's settings. The default publishes `pages.toml` in the current
/// folder with the real GitHub, and never asks.
#[derive(Debug, Clone)]
pub struct GuideOptions {
    /// Where state, credentials and GitHub are.
    pub settings: Settings,
    /// Stops transfers and waits.
    pub cancel: Cancel,
    /// The configuration, written when it does not exist.
    pub config: PathBuf,
    /// The folder the repository and project are inferred from.
    pub dir: PathBuf,
    /// `OWNER/REPO` for a new configuration; empty: from the `origin` remote.
    pub repository: String,
    /// The project for a new configuration; `None`: the nearest `conf.toml`.
    pub project: Option<PathBuf>,
    /// The branch for a new configuration.
    pub branch: String,
    /// The profile for a new configuration.
    pub profile: String,
    /// The device flow's client ID; empty: the one this build has ([`CLIENT_ID`]).
    pub client_id: String,
    /// The program that builds; empty: [`guidedog_program`]'s choice.
    pub guidedog: String,
    /// The GitHub CLI's program, offered as a token source.
    pub gh: String,
    /// As `LoadOptions::wait_seconds`: 0 the configuration's wait, below 0 none.
    pub wait_seconds: i64,
    /// The person may be asked.
    pub interactive: bool,
    /// Confirmations are answered yes without asking (`--yes`); a step that needs the
    /// person (signing in, replacing others' work) still fails.
    pub assume_yes: bool,
    /// List every change of the plan.
    pub verbose: bool,
}

impl Default for GuideOptions {
    fn default() -> GuideOptions {
        GuideOptions {
            settings: Settings::default(),
            cancel: Cancel::default(),
            config: PathBuf::from(crate::CONFIG_FILE),
            dir: PathBuf::from("."),
            repository: String::new(),
            project: None,
            branch: "gh-pages".to_string(),
            profile: "default".to_string(),
            client_id: String::new(),
            guidedog: String::new(),
            gh: "gh".to_string(),
            wait_seconds: 0,
            interactive: false,
            assume_yes: false,
            verbose: false,
        }
    }
}

/// Publishes the site, doing first whatever is missing (configuration, token,
/// binding, Pages source), and returns the publication's outcome. Fails before
/// publishing when a step cannot be done or was declined; the problem's hint names
/// the step's command or flag.
pub fn publish_guided(
    options: &GuideOptions,
    prompt: &mut dyn Prompt,
) -> Result<Outcome, Problem> {
    let mut guide = Guide { o: options, prompt, settings: options.settings.clone() };
    let config = guide.ensure_config()?;
    guide.ensure_token(&config)?;
    guide.ensure_binding(&config)?;
    guide.ensure_source(&config)?;
    guide.publish(&config)
}

struct Guide<'a> {
    o: &'a GuideOptions,
    prompt: &'a mut dyn Prompt,
    /// The run's settings: a token chosen for this run only replaces the credential.
    settings: Settings,
}

/// `pages.guide` (exit 2): a step needs the person, and this run may not ask.
fn needs_person(step: &str, message: &str, hint: &str) -> Problem {
    problem("pages.guide", "This step needs your answer", step, message, hint).exit(2)
}

impl Guide<'_> {
    /// Asks `question` before `step`; `--yes` answers yes. A run that may not ask fails
    /// with `hint`, and a declined question with `pages.unconfirmed`.
    fn ask(&mut self, step: &str, question: &str, hint: &str) -> Result<(), Problem> {
        if self.may(question) {
            return Ok(());
        }
        if !self.o.interactive {
            return Err(needs_person(step, &format!("{question} (not asked: no terminal)"), hint));
        }
        Err(not_confirmed(
            step,
            "Run the command again and answer y once you have checked what it shows.",
        ))
    }

    /// Whether the person agrees to `question`: yes with `--yes`, their answer when
    /// they may be asked, and no otherwise.
    fn may(&mut self, question: &str) -> bool {
        if self.o.assume_yes {
            self.prompt.show(&format!("{question} yes (--yes)"));
            return true;
        }
        self.o.interactive && self.prompt.confirm(question)
    }

    fn client(&self, config: &Config) -> Result<Client, Problem> {
        let token = self.settings.load_token(self.settings.profile_for(&config.profile))?;
        Ok(self.settings.client(token, self.o.cancel.clone()))
    }

    /// Reads the configuration, writing it first when there is none.
    fn ensure_config(&mut self) -> Result<Config, Problem> {
        if self.o.config.exists() {
            return read_config(&self.o.config);
        }
        let (owner, repo) = self.repository()?;
        let project = match &self.o.project {
            Some(p) => self.o.dir.join(p),
            None => find_project(&self.o.dir).ok_or_else(|| {
                needs_person(
                    "project",
                    "No Guidedog project (a folder with conf.toml) was found here or above.",
                    "Pass --project=DIR, or write pages.toml with guidedog-pages init \
                     --site=DIR to publish a site you build yourself.",
                )
            })?,
        };
        let shown = relative_to(&project, self.o.config.parent().unwrap_or(Path::new(".")));
        let source = SiteSource::Project(shown);
        let text = render_config(&owner, &repo, &self.o.branch, &source, &self.o.profile);
        let path = self.o.config.display().to_string();
        std::fs::write(&self.o.config, text).map_err(|e| crate::problem::io_problem(&path, &e))?;
        self.prompt
            .show(&format!("Wrote {path} for {owner}/{repo}, project {}.", project.display()));
        read_config(&self.o.config)
    }

    /// The repository to publish to: given, or the `origin` remote's, confirmed.
    fn repository(&mut self) -> Result<(String, String), Problem> {
        if !self.o.repository.is_empty() {
            return match self.o.repository.split_once('/') {
                Some((o, r)) if is_valid_owner(o) && is_valid_repo(r) => {
                    Ok((o.to_string(), r.to_string()))
                }
                _ => Err(crate::command::usage(
                    "Not a repository",
                    &self.o.repository,
                    "Write --repository=OWNER/REPOSITORY, as GitHub shows it.",
                )),
            };
        }
        let remote = origin_url(&self.o.dir).unwrap_or_default();
        let Some((owner, repo)) = parse_github_remote(&remote) else {
            return Err(needs_person(
                "repository",
                "The repository to publish to is not known: the origin remote is not on \
                 GitHub.",
                "Pass --repository=OWNER/REPO.",
            ));
        };
        self.ask(
            "repository",
            &format!("Publish to github.com/{owner}/{repo}, branch {}?", self.o.branch),
            &format!("Pass --repository={owner}/{repo} (or --yes) to accept it."),
        )?;
        Ok((owner, repo))
    }

    /// Makes sure a token is at hand: the profile's, or one the person signs in for.
    fn ensure_token(&mut self, config: &Config) -> Result<(), Problem> {
        let profile = self.settings.profile_for(&config.profile);
        match self.settings.load_token(profile) {
            Ok(_) => Ok(()),
            Err(p) if p.code == "pages.no-token" => self.sign_in(config, true),
            // Without a usable store, only a token for this run can help.
            Err(p) if p.code == "pages.store" && self.o.interactive => {
                self.sign_in(config, false).map_err(|_| p)
            }
            Err(p) => Err(p),
        }
    }

    /// Offers the ways to a token: the device flow when a token can be stored
    /// (`storable`) and the build has a client ID, and the GitHub CLI's token.
    fn sign_in(&mut self, config: &Config, storable: bool) -> Result<(), Problem> {
        let profile = &config.profile;
        if !self.o.interactive {
            return Err(needs_person(
                "sign in",
                &format!(
                    "No token is stored for the profile {profile}, and signing in needs \
                    you."
                ),
                &format!(
                    "Run guidedog-pages auth login --profile={profile} (or auth import), or \
                    give a token with GUIDEDOG_PAGES_TOKEN and --credential=env."
                ),
            ));
        }
        let client_id =
            if self.o.client_id.is_empty() { CLIENT_ID } else { self.o.client_id.as_str() };
        let gh = gh_token(&self.o.gh);
        let mut choices = Vec::new();
        if storable && !client_id.is_empty() {
            choices.push("Sign in on github.com with a code (the token is stored)");
        }
        if gh.is_some() {
            choices.push("Use the GitHub CLI's token for this run only (nothing is stored)");
        }
        let choice = (!choices.is_empty())
            .then(|| self.prompt.choose("How should guidedog-pages reach GitHub?", &choices))
            .flatten();
        match (choice.map(|i| choices[i]), gh) {
            (Some(c), Some(token)) if c.starts_with("Use the GitHub CLI") => {
                self.prompt.show("Using the GitHub CLI's token for this run; it is not stored.");
                self.settings.credential = Credential::Token(token);
                Ok(())
            }
            (Some(_), _) => self.device_login(client_id, profile),
            (None, _) => Err(needs_person(
                "sign in",
                &format!("No token is stored for the profile {profile}."),
                &format!(
                    "Store one with guidedog-pages auth import --profile={profile}, or \
                    auth login --client-id=ID with a GitHub App you registered."
                ),
            )),
        }
    }

    fn device_login(&mut self, client_id: &str, profile: &str) -> Result<(), Problem> {
        let client = self.settings.client(Secret::default(), self.o.cancel.clone());
        let scope = "public_repo";
        let d = start_device(&client, client_id, scope)?;
        self.prompt.show(&format!(
            "Open {} on any device and enter the code {}.",
            d.verification_uri, d.user_code
        ));
        self.prompt.show("Waiting for GitHub (Ctrl+C stops)...");
        let token = wait_device(&client, client_id, &d)?;
        let a = remember_token(&self.settings, profile, &token, "device", false)?;
        self.prompt
            .show(&format!("Signed in as {}; the token is stored as profile {profile}.", a.login));
        Ok(())
    }

    /// Binds the target when this machine has not been allowed to publish there.
    fn ensure_binding(&mut self, config: &Config) -> Result<(), Problem> {
        let state = self.settings.state()?;
        if state.find_binding(&config.repository(), &config.branch)?.is_some() {
            return Ok(());
        }
        let review = review_target(&self.client(config)?, config)?;
        for line in review_lines(config, &review) {
            self.prompt.show(&line);
        }
        let shown = config.shown_path();
        let mut adopt = false;
        if review.others {
            let head = review.head.as_deref().unwrap_or_default();
            let question =
                format!("Replace the files of commit {} on {}?", short(head), config.branch);
            // Replacing someone's work is never assumed, not even with --yes.
            adopt = self.o.interactive && !self.o.assume_yes && self.prompt.confirm(&question);
            if !adopt {
                // Refused as `pages.foreign`, with the hint to bind with --adopt.
                return bind(&state, config, &config.profile, &review, false);
            }
        }
        self.ask(
            "bind",
            "Let this machine publish there with this account?",
            &format!(
                "Run guidedog-pages bind --config={shown} --yes after checking the target, \
                or pass --yes."
            ),
        )?;
        bind(&state, config, &config.profile, &review, adopt)?;
        self.prompt
            .show(&format!("Bound {}, branch {}.", review.repository.full_name, config.branch));
        Ok(())
    }

    /// Points GitHub Pages at the branch when it publishes from elsewhere.
    fn ensure_source(&mut self, config: &Config) -> Result<(), Problem> {
        let client = self.client(config)?;
        let site = pages_site(&client, &config.owner, &config.repo)?;
        if !site.readable || !site.exists || site.publishes(&config.branch) {
            return Ok(());
        }
        if branch_head(&client, &config.owner, &config.repo, &config.branch)?.is_none() {
            return Err(problem(
                "pages.source",
                "GitHub Pages publishes from elsewhere",
                &config.repository(),
                "Pages deploys another source, and the branch does not exist yet, so Pages \
                 cannot be pointed at it.",
                "Choose None as the source in the repository's Settings, Pages, then publish \
                 again; or keep the current source.",
            )
            .exit(2));
        }
        self.setup(&client, config, &site)
    }

    fn setup(&mut self, client: &Client, config: &Config, site: &Site) -> Result<(), Problem> {
        for line in crate::site_lines(config, site) {
            self.prompt.show(&line);
        }
        self.ask(
            "setup",
            &setup_question(config),
            &format!(
                "Run guidedog-pages setup --config={} --yes, or pass --yes.",
                config.shown_path()
            ),
        )?;
        setup(client, config, site)?;
        self.prompt.show(&format!("GitHub Pages now publishes from {}.", config.branch));
        Ok(())
    }

    /// Builds, plans, publishes and waits; sets Pages up after the first push when the
    /// repository had no Pages site.
    fn publish(&mut self, config: &Config) -> Result<Outcome, Problem> {
        let plugin = Plugin::publisher(Pages::new(self.settings.clone()));
        let mut options = LoadOptions::new(&config.path.to_string_lossy());
        options.wait_seconds = self.o.wait_seconds;
        options.cancel = self.o.cancel.clone();
        let mut session = PublisherSession::load(&plugin, &options)?;
        let program = guidedog_program(&self.o.guidedog);
        let prompt = &mut *self.prompt;
        let snap = freeze_site(&self.settings, config, &program, false, &mut |l| prompt.show(l))?;
        let plan = session.plan(&snap)?;
        for line in plan_lines(&plan, self.o.verbose) {
            self.prompt.show(&line);
        }
        let mut outcome = session.publish(&snap, &plan);
        if outcome.problem.as_ref().is_some_and(|p| p.code == "pages.no-site") {
            outcome = self.first_setup(config, &mut session, outcome)?;
        }
        if outcome.state == State::Live {
            self.prompt.show(&format!("Live at {}", outcome.url));
        }
        Ok(outcome)
    }

    /// After the first push to a repository without Pages: sets Pages up (asked once)
    /// and waits for the deployment. Without an answer the accepted outcome stays.
    fn first_setup(
        &mut self,
        config: &Config,
        session: &mut PublisherSession,
        accepted: Outcome,
    ) -> Result<Outcome, Problem> {
        if !self.may(&setup_question(config)) {
            return Ok(accepted);
        }
        let client = self.client(config)?;
        setup(&client, config, &Site { readable: true, ..Site::default() })?;
        self.prompt.show(&format!("GitHub Pages now publishes from {}.", config.branch));
        let wait = config.wait_for(self.o.wait_seconds);
        Ok(self.wait_live(session, &accepted.operation, Duration::from_secs(wait)))
    }

    fn wait_live(
        &self,
        session: &mut PublisherSession,
        operation: &str,
        wait: Duration,
    ) -> Outcome {
        let deadline = Instant::now() + wait;
        let mut pause = Duration::from_secs(3);
        loop {
            let r = session.status(operation);
            let waiting = matches!(r.state, State::Accepted | State::Pending);
            if !waiting || Instant::now() >= deadline || self.o.cancel.is_cancelled() {
                return r;
            }
            std::thread::sleep(pause.min(deadline - Instant::now()));
            pause = (pause * 2).min(Duration::from_secs(15));
        }
    }
}

/// The question before Pages is set up to publish the configured branch.
fn setup_question(config: &Config) -> String {
    format!("Make GitHub Pages publish {} from {}, folder /?", config.repository(), config.branch)
}

/// `path` relative to `base` when it is inside it, as a configuration writes it.
fn relative_to(path: &Path, base: &Path) -> PathBuf {
    let absolute = |p: &Path| std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    let (path, base) = (absolute(path), absolute(base));
    path.strip_prefix(&base).map(Path::to_path_buf).unwrap_or(path)
}

/// What a plan publishes, as lines for people: the summary, the counts, and the
/// changes (the first 20 unless `verbose`).
pub fn plan_lines(plan: &Plan, verbose: bool) -> Vec<String> {
    let mut lines = vec![plan.summary.clone()];
    if plan.unchanged {
        lines.push("The branch already holds this site: nothing to publish.".to_string());
        return lines;
    }
    let count = |k: ChangeKind| plan.changes.iter().filter(|c| c.kind == k).count();
    lines.push(format!(
        "{} added, {} changed, {} removed",
        count(ChangeKind::Added),
        count(ChangeKind::Changed),
        count(ChangeKind::Removed)
    ));
    for (i, c) in plan.changes.iter().enumerate() {
        if i == 20 && !verbose {
            lines.push(format!(
                "  ... and {} more (--verbose lists them all)",
                plan.changes.len() - 20
            ));
            break;
        }
        let verb = match c.kind {
            ChangeKind::Added => "add",
            ChangeKind::Changed => "change",
            ChangeKind::Removed => "remove",
        };
        lines.push(format!("  {verb:<6} {}", c.path));
    }
    lines
}
