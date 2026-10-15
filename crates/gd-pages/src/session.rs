//! The pages publisher: session load (binding and identity checks) and plans.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use gd_git::{Id, Kind, Mode, Tree};
use gd_plugins::{
    Capabilities, Change, ChangeKind, CommandContext, CommandInfo, LoadOptions, Loaded, Outcome,
    Plan, Problem, Publisher, PublisherInfo, Snapshot, Source,
};

use crate::client::Client;
use crate::config::{Config, SiteSource, read_config};
use crate::github::{
    Account, RemoteFiles, Repository, Site, account, commit_tree, pages_site, read_repository,
    remote_files,
};
use crate::problem::{pack_problem, problem};
use crate::push::{SendFile, branch_head};
use crate::state::{Binding, State, StateLock, target_key};
use crate::target::{default_branch, short};
use crate::{ENV_PROFILE, ID, Settings, VERSION};

/// The trailer a publication commit carries, with the generation it publishes: it tells
/// this publisher's commits from others on the branch.
pub const MARK: &str = "Guidedog-Publication: ";

/// The GitHub Pages publisher. One value is one session's state: `load` reads the
/// configuration, checks the binding and the target's identity, and takes the target's
/// lock; dropping the value releases the lock and forgets the token.
#[derive(Debug, Default)]
pub struct Pages {
    settings: Settings,
    session: Option<Box<Session>>,
}

impl Clone for Pages {
    /// Each session starts from the unloaded prototype; a lock or token is never shared.
    fn clone(&self) -> Pages {
        Pages { settings: self.settings.clone(), session: None }
    }
}

impl Pages {
    /// A publisher that runs with `settings`.
    pub fn new(settings: Settings) -> Pages {
        Pages { settings, session: None }
    }
}

/// A loaded session: the configuration, the binding that authorizes its target, the
/// client (which holds the token), what GitHub says of the account and repository, the
/// target's key and lock, and the folder packs are written in.
#[derive(Debug)]
pub(crate) struct Session {
    pub config: Config,
    pub binding: Binding,
    pub client: Client,
    pub account: Account,
    pub repository: Repository,
    pub state: State,
    pub key: String,
    pub work: PathBuf,
    pub wait_seconds: i64,
    pub target: String,
    _lock: Option<StateLock>,
}

/// A plan's own data: the branch's commit as read, the new tree, and what is sent.
#[derive(Debug)]
pub(crate) struct PagesPlan {
    pub head: Option<Id>,
    pub tree: Id,
    pub trees: Vec<Tree>,
    pub send: Vec<SendFile>,
}

impl Publisher for Pages {
    fn info(&self) -> PublisherInfo {
        let caps = Capabilities { status: true, recover: true };
        PublisherInfo::new(ID, "GitHub Pages", VERSION, caps)
    }

    /// `guidedog publish pages` and `guidedog pages workflow`.
    fn commands(&self) -> Vec<CommandInfo> {
        crate::command::extension::commands()
    }

    fn run_command(
        &mut self,
        ctx: &mut CommandContext<'_>,
        name: &str,
        args: &[String],
    ) -> Result<i32, Problem> {
        crate::command::extension::run(&self.settings, ctx, name, args)
    }

    fn load(&mut self, options: &LoadOptions) -> Result<Loaded, Problem> {
        self.session = None;
        let session = Session::load(&self.settings, options)?;
        let c = &session.config;
        let mut source =
            Source { base_url: c.site_url.clone(), strict: c.strict, ..Source::default() };
        match &c.source {
            SiteSource::Project(p) => source.project = p.to_string_lossy().into_owned(),
            SiteSource::Built(p) => source.site = p.to_string_lossy().into_owned(),
        }
        let target = session.target.clone();
        self.session = Some(Box::new(session));
        Ok(Loaded { source, target })
    }

    fn plan(&mut self, snapshot: &Snapshot, plan: &mut Plan) -> Result<(), Problem> {
        self.loaded()?.plan(snapshot, plan)
    }

    fn publish(&mut self, _snapshot: &Snapshot, plan: &Plan) -> Outcome {
        match self.loaded() {
            Ok(s) => s.publish(plan),
            Err(p) => Outcome::failed(p),
        }
    }

    fn status(&mut self, operation: &str) -> Outcome {
        match self.loaded() {
            Ok(s) => s.status(operation),
            Err(p) => Outcome::failed(p),
        }
    }

    fn recover(&mut self) -> Outcome {
        match self.loaded() {
            Ok(s) => s.recover(),
            Err(p) => Outcome::failed(p),
        }
    }
}

impl Pages {
    fn loaded(&mut self) -> Result<&mut Session, Problem> {
        self.session.as_deref_mut().ok_or_else(|| {
            problem(
                "plugins.unmade",
                "The session is not loaded",
                ID,
                "No session.",
                "Load the session first.",
            )
            .exit(2)
        })
    }
}

/// `pages.unbound` (exit 2): no binding lets this machine publish to the target.
pub(crate) fn unbound(c: &Config) -> Problem {
    let repository = c.repository();
    problem(
        "pages.unbound",
        "This target is not authorized on this machine",
        &repository,
        &format!("No binding lets this machine publish to {repository}, branch {}.", c.branch),
        &format!(
            "Run guidedog-pages bind --config={} to review and authorize it.",
            c.shown_path()
        ),
    )
    .exit(2)
}

impl Session {
    fn load(settings: &Settings, options: &LoadOptions) -> Result<Session, Problem> {
        let config = read_config(Path::new(&options.config))?;
        let state = settings.state()?;
        let repository = config.repository();
        let Some(binding) = state.find_binding(&repository, &config.branch)? else {
            return Err(unbound(&config));
        };
        let profile = settings.profile_for(&config.profile);
        if profile != ENV_PROFILE && profile != binding.profile {
            return Err(problem(
                "pages.unbound",
                "The profile is not the bound one",
                &config.profile,
                &format!(
                    "{repository}, branch {} is bound to the profile {}, not {}.",
                    config.branch, binding.profile, config.profile
                ),
                "Set profile in pages.toml to the bound profile, or bind this profile with \
                 guidedog-pages bind.",
            )
            .exit(2));
        }
        let client = settings.client(settings.load_token(profile)?, options.cancel.clone());
        let account = account(&client)?;
        if account.id != binding.account_id {
            return Err(problem(
                "pages.account",
                "The token is another account's",
                &account.login,
                &format!(
                    "The token belongs to {}; the target is bound for {}.",
                    account.login, binding.account
                ),
                "Use the bound account's token, or bind the target again for this account.",
            )
            .exit(2));
        }
        let found = read_repository(&client, &config.owner, &config.repo)?;
        check_identity(&config, &binding, &found)?;
        let key = target_key(binding.repository_id, &config.branch);
        let lock = if options.dry_run { None } else { Some(state.lock(&key)?) };
        let work = settings.work_dir(&key)?;
        let target = format!("github.com/{}, branch {}", found.full_name, config.branch);
        Ok(Session {
            config,
            binding,
            client,
            account,
            repository: found,
            state,
            key,
            work,
            wait_seconds: options.wait_seconds,
            target,
            _lock: lock,
        })
    }

    fn plan(&mut self, snapshot: &Snapshot, plan: &mut Plan) -> Result<(), Problem> {
        self.refuse_unsettled()?;
        let (c, client) = (&self.config, &self.client);
        let site = pages_site(client, &c.owner, &c.repo)?;
        check_source(c, &site)?;
        let head = branch_head(client, &c.owner, &c.repo, &c.branch)?;
        let mut remote = RemoteFiles { complete: true, ..RemoteFiles::default() };
        let mut remote_tree = String::new();
        if let Some(head) = head {
            let hex = head.to_hex();
            let (tree, message) = commit_tree(client, &c.owner, &c.repo, &hex)?;
            if !message.contains(MARK) && self.binding.adopted.as_deref() != Some(&hex) {
                return Err(foreign(c, &self.repository, &hex));
            }
            remote = remote_files(client, &c.owner, &c.repo, &tree)?;
            remote_tree = tree;
        }
        let (ids, local) = local_tree(snapshot, &remote)?;
        plan.target = self.target.clone();
        plan.baseline = head.map(|h| h.to_hex()).unwrap_or_default();
        plan.unchanged = head.is_some() && local.tree.to_hex() == remote_tree;
        if !plan.unchanged {
            plan.changes = changes(snapshot, &ids, &remote);
        }
        let unreadable =
            if site.readable { "" } else { " (the token may not read the Pages settings)" };
        plan.summary = format!(
            "{} publishes to {} as {}{unreadable}",
            self.account.login, self.target, c.site_url
        );
        plan.data = Some(Box::new(PagesPlan { head, ..local }));
        Ok(())
    }

    /// Refuses to start while a publication of the target is unsettled.
    pub(crate) fn refuse_unsettled(&self) -> Result<(), Problem> {
        if self.state.read_journal(&self.key)?.is_none() {
            return Ok(());
        }
        Err(problem(
            "pages.unsettled",
            "An earlier publication is unsettled",
            &self.binding.repository,
            "A publication of this target lost its answer from GitHub, so whether it moved \
             the branch is not known yet.",
            &format!("Run guidedog-pages recover --config={} first.", self.config.shown_path()),
        )
        .exit(7))
    }
}

/// Refuses a repository that is not the bound one (made again, renamed, or transferred),
/// and a branch that is the repository's default.
fn check_identity(c: &Config, binding: &Binding, r: &Repository) -> Result<(), Problem> {
    if r.id != binding.repository_id {
        return Err(problem(
            "pages.identity",
            "The repository is not the one that was bound",
            &r.full_name,
            &format!(
                "{} now has the GitHub ID {}; the binding is for {}. It was deleted and \
                made again, or another took its name.",
                r.full_name, r.id, binding.repository_id
            ),
            "Check the repository on GitHub; if it is right, run guidedog-pages bind to \
             authorize it again.",
        )
        .exit(2));
    }
    if !r.full_name.eq_ignore_ascii_case(&binding.repository) {
        return Err(problem(
            "pages.identity",
            "The repository was renamed or moved",
            &r.full_name,
            &format!("The bound repository {} is now {}.", binding.repository, r.full_name),
            "Update repository in pages.toml and run guidedog-pages bind to authorize the new \
             name.",
        )
        .exit(2));
    }
    if r.default_branch == c.branch {
        return Err(default_branch(c, r));
    }
    Ok(())
}

fn foreign(c: &Config, r: &Repository, head: &str) -> Problem {
    problem(
        "pages.foreign",
        "The branch holds work Guidedog did not publish",
        &c.branch,
        &format!(
            "{}, branch {}, holds commit {}, which this publisher did not make.",
            r.full_name,
            c.branch,
            short(head)
        ),
        "Publish to a branch of its own, or run guidedog-pages bind --adopt after checking \
         that the branch may be replaced.",
    )
    .exit(2)
}

/// Refuses a Pages site that publishes from elsewhere: GitHub Actions, or another branch
/// or folder. A site the token may not read is not checked, and a repository without a
/// Pages site is published to, so that setup finds the branch it needs (GitHub sets
/// Pages up only for a branch that exists).
fn check_source(c: &Config, site: &Site) -> Result<(), Problem> {
    if !site.readable || !site.exists {
        return Ok(());
    }
    let repository = c.repository();
    let config = c.shown_path();
    if site.build_type == "workflow" {
        return Err(problem(
            "pages.source",
            "GitHub Pages publishes from GitHub Actions",
            &repository,
            "The site is deployed by a workflow; a branch publication would not be seen, and \
             two publishers would compete.",
            &format!(
                "Keep using the workflow, or run guidedog-pages setup --config={config} to \
                publish from the branch {} instead.",
                c.branch
            ),
        )
        .exit(2));
    }
    if site.branch != c.branch || site.path != "/" {
        return Err(problem(
            "pages.source",
            "GitHub Pages publishes from another branch",
            &repository,
            &format!(
                "The site is deployed from {}, folder {}, not from {}, folder /.",
                site.branch, site.path, c.branch
            ),
            &format!("Run guidedog-pages setup --config={config} to publish from {}.", c.branch),
        )
        .exit(2));
    }
    Ok(())
}

/// Takes each snapshot file's Git ID, adds `.nojekyll` (so that folders such as
/// `_static` are served), builds the trees, and chooses the blobs to send: those the
/// branch lacks. Returns each file's ID (hexadecimal) by path, and the plan's tree.
fn local_tree(
    snapshot: &Snapshot,
    remote: &RemoteFiles,
) -> Result<(HashMap<String, String>, PagesPlan), Problem> {
    let entries = snapshot.entries();
    let known: HashSet<&str> = remote.files.values().map(String::as_str).collect();
    let mut ids = HashMap::with_capacity(entries.len());
    let mut files = Vec::with_capacity(entries.len() + 1);
    let mut send = Vec::new();
    let mut sent = HashSet::new();
    for e in entries {
        let path = snapshot.root().join(&e.path);
        let (id, size) = gd_git::file_id(&path).map_err(|err| pack_problem(&err))?;
        if size != e.size {
            return Err(problem(
                "plugins.snapshot-changed",
                "The frozen site changed",
                &path.display().to_string(),
                "A file changed after the site was frozen.",
                "Run the command again.",
            ));
        }
        files.push(gd_git::File { path: e.path.clone(), mode: Mode::File, id });
        let hex = id.to_hex();
        if !known.contains(hex.as_str()) && sent.insert(id) {
            send.push(SendFile { path: Some(path), size, id });
        }
        ids.insert(e.path.clone(), hex);
    }
    if !ids.contains_key(".nojekyll") {
        let empty = gd_git::object_id(Kind::Blob, &[]);
        files.push(gd_git::File { path: ".nojekyll".into(), mode: Mode::File, id: empty });
        if !known.contains(empty.to_hex().as_str()) && sent.insert(empty) {
            send.push(SendFile { path: None, size: 0, id: empty });
        }
    }
    let trees = gd_git::build_trees(&files).map_err(|e| pack_problem(&e))?;
    let Some(root) = trees.last() else {
        return Err(Problem::memory());
    };
    let tree = root.id;
    Ok((ids, PagesPlan { head: None, tree, trees, send }))
}

/// Lists what publishing adds, changes, and removes. With an incomplete list of the
/// branch's files, every file is listed as changed and nothing as removed.
fn changes(
    snapshot: &Snapshot,
    ids: &HashMap<String, String>,
    remote: &RemoteFiles,
) -> Vec<Change> {
    let mut list = Vec::new();
    for e in snapshot.entries() {
        let was = remote.files.get(&e.path);
        if was.is_some() && was == ids.get(&e.path) {
            continue;
        }
        let kind = if was.is_some() || !remote.complete {
            ChangeKind::Changed
        } else {
            ChangeKind::Added
        };
        list.push(Change { path: e.path.clone(), kind });
    }
    if remote.complete {
        let local: HashSet<&str> = snapshot.entries().iter().map(|e| e.path.as_str()).collect();
        for path in remote.files.keys() {
            if !local.contains(path.as_str()) && path != ".nojekyll" {
                list.push(Change { path: path.clone(), kind: ChangeKind::Removed });
            }
        }
    }
    list.sort_by(|a, b| a.path.cmp(&b.path));
    list
}
