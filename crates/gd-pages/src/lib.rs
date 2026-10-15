//! Publishes Guidedog sites to GitHub Pages from a local machine (GDS 0008). It is the
//! `pages` publisher of `gd-plugins`, linked into the `guidedog-pages` program and
//! reached by `guidedog publish --publisher=pages`.
//!
//! A publication is one Git push to a branch that holds nothing but the site: the
//! site's files become Git objects (`gd-git`), and one receive-pack request over HTTPS
//! (`gd-fetch`) carries the objects the branch lacks with the update of the branch from
//! the commit last seen to the new one. GitHub applies the update only if the branch
//! still has that commit, so a publication never overwrites a newer one. Pages then
//! deploys the branch; publish waits for that deployment, of the new commit, and
//! reports it `Live` only when GitHub says so.
//!
//! What may be published where is the user's decision, kept on their machine: a
//! [`Binding`] ([`bind`]) ties a credential profile to one repository, by its GitHub ID,
//! and one branch; a project's `pages.toml` can name a target, but never authorize one.
//! Tokens stay in the system's credential store ([`CredentialStore`]). A journal
//! written before the push lets `recover` settle a publication whose answer was lost.
//!
//! ```no_run
//! use gd_plugins::{LoadOptions, PublisherSession, SnapshotOptions, snapshot};
//!
//! # fn main() -> Result<(), gd_plugins::Problem> {
//! let plugin = gd_pages::plugin();
//! let options = LoadOptions::new("pages.toml");
//! let mut session = PublisherSession::load(&plugin, &options)?;
//! let site = snapshot(&session.source().site, &SnapshotOptions::default())?;
//! let plan = session.plan(&site)?;
//! let outcome = session.publish(&site, &plan);
//! std::process::exit(outcome.exit_status());
//! # }
//! ```

// In unit tests, a test failing on unexpected input is the point.
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod client;
pub mod command;
mod config;
mod credential;
mod device;
mod files;
mod github;
mod outcome;
mod problem;
mod push;
mod session;
mod state;
mod target;
#[cfg(test)]
mod testdir;

use std::path::PathBuf;

use gd_plugins::{Cancel, Plugin, Problem};

pub use client::{API_ROOT, Client, Roots, WEB_ROOT};
pub use config::{
    CONFIG_FILE, Config, SiteSource, default_site_url, is_valid_branch, is_valid_owner,
    is_valid_profile, is_valid_repo, read_config, render_config,
};
pub use credential::{
    CREDENTIAL_VARIABLE, Credential, CredentialStore, ENV_PROFILE, MAX_TOKEN, SERVICE, STORE_NAME,
    Secret, TOKEN_VARIABLE, env_token, is_valid_token,
};
pub use device::{CLIENT_ID, DeviceCode, start_device, wait_device};
pub use github::{
    Account, Build, BuildState, RemoteFiles, Repository, Site, account, commit_tree,
    configure_pages, pages_build, pages_site, read_repository, remote_files,
};
pub use push::{PushOutcome, branch_head, push};
pub use session::{MARK, Pages};
pub use state::{Binding, Journal, Profile, State, StateLock, target_key, today};
pub use target::{Review, bind, remember_token, review_target, setup};

/// The publisher's version, which its handshake declares: Guidedog's.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The publisher's ID in a registry.
pub const ID: &str = "pages";
/// When set, names the folder that holds builds and packs instead of the user's cache
/// folder, for tests and portable installations.
pub const CACHE_VARIABLE: &str = "GUIDEDOG_CACHE_DIR";

/// Where a run's state, credentials and GitHub are. The default is the real thing:
/// GitHub.com, the profile's stored token, and the user's configuration and cache
/// folders. Tests and embedders change fields with struct-update syntax.
#[derive(Debug, Clone, Default)]
pub struct Settings {
    /// Where requests go (GitHub.com by default).
    pub roots: Roots,
    /// Where the token comes from.
    pub credential: Credential,
    /// The credential store and its service name.
    pub store: CredentialStore,
    /// The state folder; `None` is `pages` in Guidedog's configuration folder.
    pub state_dir: Option<PathBuf>,
    /// The cache folder; `None` is `$GUIDEDOG_CACHE_DIR` or the user's cache folder.
    pub cache_dir: Option<PathBuf>,
}

impl Settings {
    /// Opens the state folder. Fails when it cannot be made.
    pub fn state(&self) -> Result<State, Problem> {
        State::open(self.state_dir.as_deref())
    }

    /// The token a run uses for `profile`: the given or environment token when the
    /// credential says so, and the profile's stored token otherwise.
    pub fn load_token(&self, profile: &str) -> Result<Secret, Problem> {
        match &self.credential {
            Credential::Token(token) => Ok(token.clone()),
            Credential::Env => env_token(),
            Credential::Stored if self.credential.is_unstored() || profile == ENV_PROFILE => {
                env_token()
            }
            Credential::Stored => self.store.load_token(profile),
        }
    }

    /// The profile a run's token is checked against: [`ENV_PROFILE`] when the token is
    /// not a stored profile's, and `configured` otherwise.
    pub fn profile_for<'a>(&self, configured: &'a str) -> &'a str {
        if self.credential.is_unstored() { ENV_PROFILE } else { configured }
    }

    /// A client of [`Settings::roots`] that sends `token`.
    pub fn client(&self, token: Secret, cancel: Cancel) -> Client {
        Client::new(token, self.roots.clone(), cancel)
    }

    /// The folder of the target `key`'s builds and packs, which it creates.
    pub fn work_dir(&self, key: &str) -> Result<PathBuf, Problem> {
        let base = match &self.cache_dir {
            Some(dir) => dir.clone(),
            None => user_cache_dir().ok_or_else(|| {
                problem::problem(
                    "pages.cache",
                    "No cache folder",
                    "cache folder",
                    "The system names no cache folder for this user.",
                    &format!("Set {CACHE_VARIABLE} to a private folder for Guidedog's builds."),
                )
                .exit(4)
            })?,
        };
        let dir = base.join("guidedog").join("pages").join(key);
        files::create_private_dir(&dir)
            .map_err(|e| problem::io_problem(&dir.display().to_string(), &e))?;
        Ok(dir)
    }
}

fn user_cache_dir() -> Option<PathBuf> {
    let var = |name| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(dir) = var(CACHE_VARIABLE) {
        return Some(dir);
    }
    if cfg!(windows) {
        return var("LOCALAPPDATA");
    }
    if cfg!(target_os = "macos") {
        return var("HOME").map(|h| h.join("Library/Caches"));
    }
    var("XDG_CACHE_HOME").or_else(|| var("HOME").map(|h| h.join(".cache")))
}

/// The pages publisher with the default [`Settings`], for a registry.
pub fn plugin() -> Plugin {
    Plugin::publisher(Pages::default())
}

/// The pages publisher with `settings`, for a registry.
pub fn plugin_with(settings: Settings) -> Plugin {
    Plugin::publisher(Pages::new(settings))
}

/// Where a project is built for the target of `config`: a folder of this publisher's
/// cache that no other build writes to, and for a dry run a new folder each time (the
/// caller removes it). Fails when the target is not bound on this machine.
pub fn build_dir(settings: &Settings, config: &Config, dry_run: bool) -> Result<PathBuf, Problem> {
    let state = settings.state()?;
    let Some(binding) = state.find_binding(&config.repository(), &config.branch)? else {
        return Err(session::unbound(config));
    };
    let work = settings.work_dir(&target_key(binding.repository_id, &config.branch))?;
    if dry_run {
        return files::make_unique_dir(&work, "dry-run-")
            .map_err(|e| problem::io_problem(&work.display().to_string(), &e));
    }
    Ok(work.join("site"))
}
