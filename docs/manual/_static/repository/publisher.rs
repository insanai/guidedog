//! A publisher that publishes a built site to a local folder, replacing the folder
//! whole. It shows the whole publisher interface: load reads a configuration, plan
//! compares the snapshot with the folder, publish writes it, and status reads what the
//! folder holds. Run as a program, it serves the process protocol, so that
//! `guidedog plugin register folder /path/to/publisher` can use it.
//!
//! Its configuration, `folder.toml`, names the site (or a project for guidedog publish
//! to build) and the folder; relative paths are relative to the configuration:
//!
//! ```toml
//! site = "build/site"            # or: project = "docs"
//! target = "/srv/www/manual"
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use gd_plugins::{
    Capabilities, Change, ChangeKind, LoadOptions, Loaded, Outcome, Plan, Problem, Publisher,
    PublisherInfo, Snapshot, SnapshotMode, SnapshotOptions, Source, State,
};

/// The file in a published folder that names the generation it holds.
const MARK: &str = ".guidedog-publication";

/// The session's state: the target folder and, unless dry, its lock.
#[derive(Debug, Default)]
struct Folder {
    target: PathBuf,
    lock: Option<fs::File>,
}

impl Clone for Folder {
    /// Each session starts from the unloaded prototype; a lock is never shared.
    fn clone(&self) -> Folder {
        Folder { target: self.target.clone(), lock: None }
    }
}

impl Publisher for Folder {
    fn info(&self) -> PublisherInfo {
        let caps = Capabilities { status: true, recover: false };
        PublisherInfo::new("folder", "Local folder", "1.0.0", caps)
    }

    fn load(&mut self, options: &LoadOptions) -> Result<Loaded, Problem> {
        let config = Path::new(&options.config);
        let text = fs::read_to_string(config).map_err(|e| {
            config_problem(
                &options.config,
                &e.to_string(),
                "Write a folder.toml with site and target.",
            )
        })?;
        let root = gd_toml::parse(&text).ok();
        let field = |key| root.as_ref().and_then(|r| r.get(key)).and_then(|v| v.as_str());
        let (site, project, target) = (field("site"), field("project"), field("target"));
        let (Some(source), Some(target)) = (site.or(project), target) else {
            return Err(config_problem(
                &options.config,
                "It needs site (or project) and target, each a quoted path.",
                "Write site = \"build/site\" and target = \"/srv/www/site\".",
            ));
        };
        let base = configuration_folder(config)?;
        let source = base.join(source).to_string_lossy().into_owned();
        self.target = base.join(target);
        if !options.dry_run {
            self.lock = Some(lock_target(&self.target)?);
        }
        let source = if site.is_some() {
            Source { site: source, ..Source::default() }
        } else {
            Source { project: source, ..Source::default() }
        };
        Ok(Loaded { source, target: self.target.to_string_lossy().into_owned() })
    }

    fn plan(&mut self, snapshot: &Snapshot, plan: &mut Plan) -> Result<(), Problem> {
        plan.target = self.target.to_string_lossy().into_owned();
        plan.baseline = fs::read_to_string(self.target.join(MARK)).unwrap_or_default();
        plan.unchanged = plan.baseline == plan.generation_hex();
        if plan.unchanged {
            return Ok(());
        }
        plan.changes = snapshot
            .entries()
            .iter()
            .map(|e| {
                let exists = self.target.join(&e.path).exists();
                let kind = if exists { ChangeKind::Changed } else { ChangeKind::Added };
                Change { path: e.path.clone(), kind }
            })
            .collect();
        Ok(())
    }

    /// Copies the snapshot beside the target, then puts the copy in its place.
    fn publish(&mut self, snapshot: &Snapshot, plan: &Plan) -> Outcome {
        let generation = plan.generation_hex();
        let url = self.target.to_string_lossy().into_owned();
        if plan.unchanged {
            return Outcome { state: State::Unchanged, generation, url, ..Outcome::default() };
        }
        match self.replace(snapshot, &generation) {
            Ok(()) => Outcome {
                state: State::Live,
                operation: generation[..12].to_string(),
                generation,
                url,
                problem: None,
            },
            Err(p) => Outcome::failed(p),
        }
    }

    fn status(&mut self, _operation: &str) -> Outcome {
        let mark = fs::read_to_string(self.target.join(MARK)).unwrap_or_default();
        if mark.len() != 64 {
            return Outcome::failed(write_problem(&self.target));
        }
        Outcome {
            state: State::Live,
            operation: mark[..12].to_string(),
            url: self.target.to_string_lossy().into_owned(),
            generation: mark,
            problem: None,
        }
    }
}

impl Folder {
    fn replace(&self, snapshot: &Snapshot, generation: &str) -> Result<(), Problem> {
        let parent = self.target.parent().unwrap_or(Path::new("."));
        let mut options = SnapshotOptions::new(SnapshotMode::Copy);
        options.temp_dir = Some(parent.to_path_buf());
        options.no_index = true;
        // A private copy beside the target, so the final rename stays on one disk.
        let copy = gd_plugins::snapshot(snapshot.root(), &options)?;
        if copy.digest() != snapshot.digest() {
            return Err(write_problem(snapshot.root()));
        }
        let next = copy.root().to_path_buf();
        fs::write(next.join(MARK), generation).map_err(|_| write_problem(&next))?;
        let backup = parent.join(format!(".{MARK}-backup-{}", std::process::id()));
        let had_old = fs::rename(&self.target, &backup).is_ok();
        if fs::rename(&next, &self.target).is_err() {
            if had_old {
                let _ = fs::rename(&backup, &self.target);
            }
            return Err(write_problem(&self.target));
        }
        // The copy now lives at the target; dropping `copy` finds nothing left to remove.
        drop(copy);
        if had_old {
            let _ = fs::remove_dir_all(&backup);
        }
        Ok(())
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        if let Some(lock) = self.lock.take() {
            let _ = lock.unlock();
        }
    }
}

/// The configuration's folder, absolute; configured paths are relative to it.
fn configuration_folder(config: &Path) -> Result<PathBuf, Problem> {
    let base = config.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::path::absolute(base).map_err(|e| {
        Problem::new(
            "folder.config-path",
            "Cannot resolve the configuration folder",
            &e.to_string(),
        )
        .hint("Run from an existing directory and pass --config with the configuration's path.")
        .path(&config.to_string_lossy())
        .exit(4)
    })
}

/// Holds an exclusive lock beside the target for the session.
fn lock_target(target: &Path) -> Result<fs::File, Problem> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|_| write_problem(target))?;
    }
    let mut name = target.as_os_str().to_owned();
    name.push(".lock");
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(PathBuf::from(name))
        .map_err(|_| write_problem(target))?;
    if lock.try_lock().is_err() {
        return Err(Problem::new(
            "folder.busy",
            "Another publisher holds the folder",
            "A publication to this folder is already running.",
        )
        .hint("Wait for the other publication to finish, then run guidedog publish again.")
        .path(&target.to_string_lossy())
        .exit(4));
    }
    Ok(lock)
}

fn config_problem(path: &str, message: &str, hint: &str) -> Problem {
    Problem::new("folder.config", "The configuration is not valid", message)
        .hint(hint)
        .path(path)
        .exit(2)
}

fn write_problem(path: &Path) -> Problem {
    Problem::new(
        "folder.write",
        "Cannot publish to the folder",
        "A file or folder could not be written.",
    )
    .hint("Check the target folder's permissions.")
    .path(&path.to_string_lossy())
    .exit(4)
}

fn main() -> ExitCode {
    gd_plugins::serve_publisher(Folder::default())
}
