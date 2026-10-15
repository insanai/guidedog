//! The publisher's state on this machine, in a `pages` folder of Guidedog's
//! configuration folder (`gd_plugins::config_dir`), readable only by the user:
//!
//! ```text
//! profiles.toml     each credential profile's GitHub account; the tokens themselves
//!                   are in the credential store
//! bindings.toml     the targets the user authorized: profile, repository and its
//!                   GitHub ID, and branch
//! journal-KEY.toml  a publication in progress, until it is settled
//! last-KEY.toml     the last settled publication, for status
//! KEY.lock          held while one publication of a target runs
//! ```
//!
//! Every file is written beside its place and renamed over it, so a reader sees the old
//! file or the new one.

use std::fs::{self, File};
use std::path::{Path, PathBuf};

use gd_plugins::Problem;
use gd_toml::{Table, Value};

use crate::config::quoted;
use crate::files::{create_private_dir, write_atomically};
use crate::problem::{io_problem, problem};

const MAX_STATE_BYTES: u64 = 1024 * 1024;
const PROFILES_ABOUT: &str = "Credential profiles: each names a token in the credential store.";
const BINDINGS_ABOUT: &str =
    "Targets authorized on this machine. Change them with guidedog-pages bind and unbind.";

/// A stored token's account.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Profile {
    /// The profile's name.
    pub name: String,
    /// The GitHub login of the token's account.
    pub account: String,
    /// The account's GitHub ID.
    pub account_id: i64,
    /// How the token was stored: `device` or `token`.
    pub method: String,
    /// The date it was stored, `YYYY-MM-DD`.
    pub stored: String,
}

/// A target the user authorized for a profile.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Binding {
    /// The profile whose token publishes there.
    pub profile: String,
    /// `OWNER/REPO` as GitHub named it when it was bound.
    pub repository: String,
    /// The repository's GitHub ID, which a rename keeps and a new repository changes.
    pub repository_id: i64,
    /// The branch.
    pub branch: String,
    /// The account's login.
    pub account: String,
    /// The account's GitHub ID.
    pub account_id: i64,
    /// A commit the branch held, not made by Guidedog, that the user let publishing
    /// replace.
    pub adopted: Option<String>,
    /// The date it was bound.
    pub bound: String,
}

/// A publication in progress (or, as the last record, settled): what the branch held
/// and what it is moved to.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Journal {
    /// The candidate commit's ID, which names the operation.
    pub operation: String,
    /// `OWNER/REPO`.
    pub repository: String,
    /// The repository's GitHub ID.
    pub repository_id: i64,
    /// The branch.
    pub branch: String,
    /// The branch's commit before, `None` for a new branch.
    pub observed: Option<String>,
    /// The commit the branch is moved to.
    pub candidate: String,
    /// The generation (the snapshot's digest) it publishes.
    pub generation: String,
    /// GitHub accepted the update.
    pub accepted: bool,
    /// The date it started.
    pub started: String,
}

/// The publisher's state folder. One value per command; it holds no open files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    dir: PathBuf,
}

/// A lock on a target or the settings; dropping it releases the lock.
#[derive(Debug)]
pub struct StateLock {
    file: File,
}

impl Drop for StateLock {
    /// Unlocks before the file closes. Closing alone is not enough: the lock belongs to
    /// the open file, which a child process started meanwhile by another thread also
    /// holds until it runs its program, so the lock would outlive this value. Unlocking
    /// releases it for every holder of the open file at once.
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

/// Names a target's files: its repository ID and branch, with the branch's slashes
/// written as `~`.
pub fn target_key(repository_id: i64, branch: &str) -> String {
    format!("{repository_id}-{}", branch.replace('/', "~"))
}

impl State {
    /// Opens (creating it) the state folder `dir`, or, for `None`, the `pages` folder of
    /// Guidedog's configuration folder. Fails when it cannot be made.
    pub fn open(dir: Option<&Path>) -> Result<State, Problem> {
        let dir = match dir {
            Some(d) => d.to_path_buf(),
            None => gd_plugins::config_dir()?.join("pages"),
        };
        create_private_dir(&dir).map_err(|e| io_problem(&dir.display().to_string(), &e))?;
        Ok(State { dir })
    }

    /// The folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The stored profiles; none is an empty list.
    pub fn profiles(&self) -> Result<Vec<Profile>, Problem> {
        let root = self.read("profiles.toml")?.unwrap_or_default();
        Ok(tables(&root, "profiles").map(profile_of).collect())
    }

    /// The profile named `name`, if there is one.
    pub fn find_profile(&self, name: &str) -> Result<Option<Profile>, Problem> {
        Ok(self.profiles()?.into_iter().find(|p| p.name == name))
    }

    /// Adds `profile`, or replaces the profile of its name.
    pub fn save_profile(&self, profile: &Profile) -> Result<(), Problem> {
        self.rewrite_profiles(&profile.name, Some(profile))
    }

    /// Removes the profile named `name`; nothing happens when there is none.
    pub fn remove_profile(&self, name: &str) -> Result<(), Problem> {
        self.rewrite_profiles(name, None)
    }

    /// Rewrites the profiles without the one named `name`, adding `added` at the end.
    fn rewrite_profiles(&self, name: &str, added: Option<&Profile>) -> Result<(), Problem> {
        let _guard = self.metadata_lock("profiles")?;
        let mut text = header(PROFILES_ABOUT);
        for p in self.profiles()?.iter().filter(|p| p.name != name).chain(added) {
            write_profile(&mut text, p);
        }
        self.write("profiles.toml", &text)
    }

    /// The bindings; none is an empty list.
    pub fn bindings(&self) -> Result<Vec<Binding>, Problem> {
        let root = self.read("bindings.toml")?.unwrap_or_default();
        Ok(tables(&root, "bindings").map(binding_of).collect())
    }

    /// The binding of `repository` (`OWNER/REPO`, in any case) and `branch`.
    pub fn find_binding(
        &self,
        repository: &str,
        branch: &str,
    ) -> Result<Option<Binding>, Problem> {
        let same =
            |b: &Binding| b.repository.eq_ignore_ascii_case(repository) && b.branch == branch;
        Ok(self.bindings()?.into_iter().find(same))
    }

    /// Adds `binding`, replacing the binding of its repository (by ID or name) and branch.
    pub fn save_binding(&self, binding: &Binding) -> Result<(), Problem> {
        let replaced = |b: &Binding| {
            let same_id = b.repository_id == binding.repository_id;
            let same_name = b.repository.eq_ignore_ascii_case(&binding.repository);
            (same_id || same_name) && b.branch == binding.branch
        };
        self.rewrite_bindings(replaced, Some(binding)).map(|_| ())
    }

    /// Removes the binding of `repository` and `branch`; `false` when there was none.
    pub fn remove_binding(&self, repository: &str, branch: &str) -> Result<bool, Problem> {
        let removed =
            |b: &Binding| b.repository.eq_ignore_ascii_case(repository) && b.branch == branch;
        self.rewrite_bindings(removed, None)
    }

    /// Rewrites the bindings without those `dropped` matches, adding `added` at the end;
    /// `true` when one was dropped.
    fn rewrite_bindings(
        &self,
        dropped: impl Fn(&Binding) -> bool,
        added: Option<&Binding>,
    ) -> Result<bool, Problem> {
        let _guard = self.metadata_lock("bindings")?;
        let mut text = header(BINDINGS_ABOUT);
        let (dropped, kept): (Vec<Binding>, Vec<Binding>) =
            self.bindings()?.into_iter().partition(|b| dropped(b));
        for b in kept.iter().chain(added) {
            write_binding(&mut text, b);
        }
        self.write("bindings.toml", &text)?;
        Ok(!dropped.is_empty())
    }

    /// The journal of the target `key`, if a publication is unsettled. A journal that
    /// cannot be understood is `pages.journal` (exit 7).
    pub fn read_journal(&self, key: &str) -> Result<Option<Journal>, Problem> {
        let name = format!("journal-{key}.toml");
        let Some(root) = self.read(&name)? else {
            return Ok(None);
        };
        let j = journal_of(&root);
        if j.operation.is_empty() || j.candidate.is_empty() || j.repository_id == 0 {
            return Err(problem(
                "pages.journal",
                "The publication journal is damaged",
                &name,
                "The journal of an unsettled publication cannot be read.",
                "Check the branch on GitHub; then remove the journal file and publish again.",
            )
            .exit(7));
        }
        Ok(Some(j))
    }

    /// Writes the journal of `key`, synced, before a publication's push.
    pub fn write_journal(&self, key: &str, j: &Journal) -> Result<(), Problem> {
        self.write(&format!("journal-{key}.toml"), &journal_text(j))
    }

    /// Removes the journal of `key`, once its publication is settled.
    pub fn drop_journal(&self, key: &str) -> Result<(), Problem> {
        let path = self.dir.join(format!("journal-{key}.toml"));
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_problem(&path.display().to_string(), &e)),
        }
    }

    /// The last settled publication of `key`, if any.
    pub fn read_last(&self, key: &str) -> Result<Option<Journal>, Problem> {
        Ok(self.read(&format!("last-{key}.toml"))?.map(|root| journal_of(&root)))
    }

    /// Records `j` as the last settled publication of `key`.
    pub fn write_last(&self, key: &str, j: &Journal) -> Result<(), Problem> {
        self.write(&format!("last-{key}.toml"), &journal_text(j))
    }

    /// Takes the lock of the target `key`, held until the lock is dropped: one
    /// publication of a target runs at a time on this machine. A lock another process
    /// holds is refused at once (`pages.busy`), not waited for.
    pub fn lock(&self, key: &str) -> Result<StateLock, Problem> {
        let path = self.dir.join(format!("{key}.lock"));
        let shown = path.display().to_string();
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let file = options.open(&path).map_err(|e| io_problem(&shown, &e))?;
        match file.try_lock() {
            Ok(()) => Ok(StateLock { file }),
            Err(fs::TryLockError::WouldBlock) => Err(problem(
                "pages.busy",
                "Another publication of this target is running",
                &shown,
                "guidedog-pages is publishing to the same repository and branch in another \
                 process.",
                "Wait for it to finish, or stop it, then run the command again.",
            )),
            Err(fs::TryLockError::Error(e)) => Err(io_problem(&shown, &e)),
        }
    }

    fn metadata_lock(&self, name: &str) -> Result<StateLock, Problem> {
        self.lock(name).map_err(|p| {
            if p.code != "pages.busy" {
                return p;
            }
            problem(
                "pages.busy",
                "Another command is changing the publisher's settings",
                name,
                "The settings are locked while another command reads and changes them.",
                "Wait for that command to finish, then run this one again.",
            )
            .exit(4)
        })
    }

    /// Reads the state file `name`; `None` when it does not exist.
    fn read(&self, name: &str) -> Result<Option<Table>, Problem> {
        let path = self.dir.join(name);
        let shown = path.display().to_string();
        let text = match fs::metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io_problem(&shown, &e)),
            Ok(m) if m.len() > MAX_STATE_BYTES => {
                return Err(damaged(&shown, 0, "The file is over 1 MiB."));
            }
            Ok(_) => fs::read_to_string(&path).map_err(|e| io_problem(&shown, &e))?,
        };
        let root = gd_toml::parse(&text).map_err(|e| damaged(&shown, e.line, &e.message))?;
        if root.get("schema").and_then(Value::as_integer) != Some(1) {
            return Err(damaged(&shown, 0, "The file is not of schema 1."));
        }
        Ok(Some(root))
    }

    fn write(&self, name: &str, text: &str) -> Result<(), Problem> {
        write_atomically(&self.dir.join(name), text)
            .map_err(|(path, e)| io_problem(&path.display().to_string(), &e))
    }
}

fn tables<'a>(root: &'a Table, key: &str) -> impl Iterator<Item = &'a Table> {
    root.get(key).and_then(Value::as_array).unwrap_or(&[]).iter().filter_map(Value::as_table)
}

fn text_of(t: &Table, key: &str) -> String {
    t.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn int_of(t: &Table, key: &str) -> i64 {
    t.get(key).and_then(Value::as_integer).unwrap_or(0)
}

fn optional_text(t: &Table, key: &str) -> Option<String> {
    Some(text_of(t, key)).filter(|s| !s.is_empty())
}

fn profile_of(t: &Table) -> Profile {
    Profile {
        name: text_of(t, "name"),
        account: text_of(t, "account"),
        account_id: int_of(t, "account_id"),
        method: text_of(t, "method"),
        stored: text_of(t, "stored"),
    }
}

fn binding_of(t: &Table) -> Binding {
    Binding {
        profile: text_of(t, "profile"),
        repository: text_of(t, "repository"),
        repository_id: int_of(t, "repository_id"),
        branch: text_of(t, "branch"),
        account: text_of(t, "account"),
        account_id: int_of(t, "account_id"),
        adopted: optional_text(t, "adopted"),
        bound: text_of(t, "bound"),
    }
}

fn journal_of(t: &Table) -> Journal {
    Journal {
        operation: text_of(t, "operation"),
        repository: text_of(t, "repository"),
        repository_id: int_of(t, "repository_id"),
        branch: text_of(t, "branch"),
        observed: optional_text(t, "observed"),
        candidate: text_of(t, "candidate"),
        generation: text_of(t, "generation"),
        accepted: t.get("accepted").and_then(Value::as_bool).unwrap_or(false),
        started: text_of(t, "started"),
    }
}

fn header(about: &str) -> String {
    format!("# {about}\nschema = 1\n")
}

fn write_profile(out: &mut String, p: &Profile) {
    out.push_str(&format!(
        "\n[[profiles]]\nname = {}\naccount = {}\naccount_id = {}\nmethod = {}\nstored = {}\n",
        quoted(&p.name),
        quoted(&p.account),
        p.account_id,
        quoted(&p.method),
        quoted(&p.stored)
    ));
}

fn write_binding(out: &mut String, b: &Binding) {
    out.push_str(&format!(
        "\n[[bindings]]\nprofile = {}\nrepository = {}\nrepository_id = {}\nbranch = {}\n\
         account = {}\naccount_id = {}\nadopted = {}\nbound = {}\n",
        quoted(&b.profile),
        quoted(&b.repository),
        b.repository_id,
        quoted(&b.branch),
        quoted(&b.account),
        b.account_id,
        quoted(b.adopted.as_deref().unwrap_or("")),
        quoted(&b.bound)
    ));
}

fn journal_text(j: &Journal) -> String {
    let mut out = header("A publication of guidedog-pages. It holds no secret.");
    out.push_str(&format!(
        "operation = {}\nrepository = {}\nrepository_id = {}\nbranch = {}\nobserved = {}\n\
         candidate = {}\ngeneration = {}\naccepted = {}\nstarted = {}\n",
        quoted(&j.operation),
        quoted(&j.repository),
        j.repository_id,
        quoted(&j.branch),
        quoted(j.observed.as_deref().unwrap_or("")),
        quoted(&j.candidate),
        quoted(&j.generation),
        j.accepted,
        quoted(&j.started)
    ));
    out
}

/// `pages.state` (exit 2): a state file that cannot be understood.
fn damaged(path: &str, line: usize, message: &str) -> Problem {
    let place = if line > 0 { format!("{path}:{line}") } else { path.to_string() };
    problem(
        "pages.state",
        "The publisher's state is damaged",
        &place,
        message,
        "Fix the file, or remove it and run bind or auth again.",
    )
    .exit(2)
}

/// Today's date, `YYYY-MM-DD`, in UTC.
pub fn today() -> String {
    gd_clock::now().utc().date.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdir::TestDir;

    #[test]
    fn today_is_a_date() {
        assert_eq!(today().len(), 10);
    }

    #[test]
    fn profiles_bindings_and_journals_round_trip() {
        let dir = TestDir::new("state");
        let state = State::open(Some(dir.path())).unwrap();
        assert!(state.profiles().unwrap().is_empty());
        let p = Profile {
            name: "work".into(),
            account: "ada".into(),
            account_id: 42,
            method: "token".into(),
            stored: today(),
        };
        state.save_profile(&p).unwrap();
        state.save_profile(&Profile { account: "ada2".into(), ..p.clone() }).unwrap();
        assert_eq!(state.profiles().unwrap().len(), 1);
        assert_eq!(state.find_profile("work").unwrap().unwrap().account, "ada2");
        state.remove_profile("work").unwrap();
        assert!(state.find_profile("work").unwrap().is_none());
        let b = Binding {
            profile: "work".into(),
            repository: "Octo/Site".into(),
            repository_id: 7,
            branch: "gh-pages".into(),
            adopted: Some("abc".into()),
            ..Binding::default()
        };
        state.save_binding(&b).unwrap();
        let found = state.find_binding("octo/site", "gh-pages").unwrap().unwrap();
        assert_eq!(found, b);
        assert!(state.remove_binding("octo/site", "gh-pages").unwrap());
        assert!(!state.remove_binding("octo/site", "gh-pages").unwrap());
        let key = target_key(7, "docs/site");
        assert_eq!(key, "7-docs~site");
        let j = Journal {
            operation: "c".into(),
            repository_id: 7,
            candidate: "c".into(),
            observed: None,
            ..Journal::default()
        };
        state.write_journal(&key, &j).unwrap();
        assert_eq!(state.read_journal(&key).unwrap(), Some(j.clone()));
        state.drop_journal(&key).unwrap();
        assert_eq!(state.read_journal(&key).unwrap(), None);
        state.write_journal(&key, &Journal::default()).unwrap();
        assert_eq!(state.read_journal(&key).unwrap_err().code, "pages.journal");
        std::fs::write(dir.path().join("bindings.toml"), "schema = 2\n").unwrap();
        assert_eq!(state.bindings().unwrap_err().code, "pages.state");
    }

    #[test]
    fn a_target_is_locked_by_one_holder() {
        let dir = TestDir::new("state-lock");
        let state = State::open(Some(dir.path())).unwrap();
        let held = state.lock("7-gh-pages").unwrap();
        assert_eq!(state.lock("7-gh-pages").unwrap_err().code, "pages.busy");
        drop(held);
        assert!(state.lock("7-gh-pages").is_ok());
    }

    /// A child process started by another thread holds the lock file open until it
    /// runs its program; the lock must still be free as soon as it is dropped.
    #[cfg(unix)]
    #[test]
    fn a_dropped_lock_is_free_while_programs_start() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let dir = TestDir::new("state-spawn");
        let state = State::open(Some(dir.path())).unwrap();
        let stop = AtomicBool::new(false);
        std::thread::scope(|scope| {
            scope.spawn(|| {
                while !stop.load(Ordering::Relaxed) {
                    let _ = std::process::Command::new("true").status();
                }
            });
            for _ in 0..500 {
                drop(state.lock("7-gh-pages").unwrap());
                let again = state.lock("7-gh-pages");
                if again.is_err() {
                    stop.store(true, Ordering::Relaxed);
                }
                assert!(again.is_ok(), "the dropped lock is still held");
            }
            stop.store(true, Ordering::Relaxed);
        });
    }
}
