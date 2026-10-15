//! `release publish` and `release check-run`: the GitHub side of a release, through the
//! GitHub CLI (`gh`), which reads GH_TOKEN.

use std::path::Path;
use std::process::Command;

use crate::args::{self, Spec};
use crate::fsutil::sorted_entries;
use crate::{Result, fail, process, repository_root};

const PUBLISH_HELP: &str = "\
Publishes the release files in DIR as the GitHub release TAG, never replacing a
published release.

Usage: cargo xtask release publish [--check] [--dry-run] TAG [DIR]

  --check     only fail when TAG is already published
  --dry-run   check SHA256SUMS and the release's state, and print the gh commands
              that would create, upload and publish, without running them

It checks DIR/SHA256SUMS against the files, creates a draft release with the notes in
.github/release-notes.md (or resumes an existing draft, replacing its files), uploads
every file, and then publishes the draft as the latest release. A release that is
already published is left untouched and the step fails, so assets never change after
people may have downloaded them. Needs gh, GH_TOKEN and GITHUB_REPOSITORY.
";

const CHECK_RUN_HELP: &str = "\
Checks that an earlier release workflow run may be published by
.github/workflows/release-recover.yml: it ran release.yml for the push of TAG, at the
commit TAG names, has finished, and every job but publishing succeeded.

Usage: cargo xtask release check-run RUN TAG

Needs gh, GH_TOKEN and GITHUB_REPOSITORY.
";

/// The jobs a release run must have passed before it may be published.
const GATES: [&str; 8] =
    ["version", "qualify", "build", "smoke", "windows", "windows-smoke", "source", "homebrew"];

/// The repository GitHub steps act on, from GITHUB_REPOSITORY.
fn repository() -> Result<String> {
    std::env::var("GITHUB_REPOSITORY").ok().filter(|r| !r.is_empty()).ok_or_else(|| {
        fail(
            "github-repository",
            "NO REPOSITORY",
            "GITHUB_REPOSITORY does not name the repository.",
            "Set GITHUB_REPOSITORY, such as GITHUB_REPOSITORY=insanai/guidedog.",
        )
    })
}

/// A release's state on GitHub.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Missing,
    Draft,
    Published,
}

/// Runs `release publish`.
pub(super) fn publish_main(args: &[String]) -> Result<()> {
    let spec = Spec {
        command: "release publish",
        flags: &["--check", "--dry-run"],
        options: &[],
        help: PUBLISH_HELP,
    };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(2, &spec)?;
    let tag = parsed.positional(0, "the tag to publish, such as v0.7.0", &spec)?;
    let root = repository_root();
    let repo = repository()?;
    let version = super::version::release_version(&root)?;
    if tag != format!("v{version}") {
        return Err(fail(
            "release-tag",
            "TAG DIFFERS",
            &format!("The tag {tag} does not match VERSION {version}."),
            &format!("Publish v{version}, or correct VERSION and tag again."),
        ));
    }
    let dry_run = parsed.has("--dry-run");
    let state = match release_state(&repo, tag) {
        Err(_) if dry_run => {
            println!("(dry run: the state of {tag} is unknown; taking it as missing)");
            State::Missing
        }
        state => state?,
    };
    if state == State::Published {
        return Err(fail(
            "release-published",
            "ALREADY PUBLISHED",
            &format!("The release {tag} is already published; its files are not replaced."),
            "Publish a new version instead.",
        ));
    }
    if parsed.has("--check") {
        println!("The release {tag} is not published ({state:?}).");
        return Ok(());
    }
    let dir = super::given_path(parsed.positional(1, "the folder of release files", &spec)?);
    super::files::verify_checksums(&dir)?;
    let commands = publish_commands(&root, &repo, tag, &version, &dir, state)?;
    for mut command in commands {
        if dry_run {
            println!("{}", process::describe(&command));
        } else {
            process::run(&mut command)?;
        }
    }
    if !dry_run {
        println!("Published {tag} from {}.", dir.display());
    }
    Ok(())
}

/// Whether the release for `tag` is missing, a draft, or published.
fn release_state(repo: &str, tag: &str) -> Result<State> {
    let mut gh = Command::new("gh");
    gh.args(["release", "view", tag, "--repo", repo, "--json", "isDraft", "--jq", ".isDraft"]);
    let out = process::capture(&mut gh)?;
    if out.success {
        return Ok(if out.stdout.trim() == "true" { State::Draft } else { State::Published });
    }
    if out.stderr.contains("release not found") {
        return Ok(State::Missing);
    }
    eprint!("{}", out.stderr);
    Err(fail(
        "release-state",
        "UNKNOWN RELEASE STATE",
        &format!("I cannot tell whether {tag} is published, so nothing was uploaded."),
        "Check GH_TOKEN and the network with `gh release list`, then run the step again.",
    ))
}

/// The gh commands that create (or resume) the draft, upload DIR and publish it.
fn publish_commands(
    root: &Path,
    repo: &str,
    tag: &str,
    version: &str,
    dir: &Path,
    state: State,
) -> Result<Vec<Command>> {
    let mut commands = Vec::new();
    if state == State::Draft {
        println!("Resuming the draft release {tag}.");
    } else {
        let mut create = Command::new("gh");
        create.args(["release", "create", tag, "--repo", repo, "--verify-tag", "--draft"]);
        create.args(["--title", &format!("Guidedog {version}"), "--notes-file"]);
        create.arg(root.join(".github/release-notes.md"));
        commands.push(create);
    }
    let mut upload = Command::new("gh");
    upload.args(["release", "upload", tag, "--repo", repo, "--clobber"]);
    let files = sorted_entries(dir)?;
    let visible = |p: &&std::path::PathBuf| {
        !p.file_name().unwrap_or_default().to_string_lossy().starts_with('.')
    };
    upload.args(files.iter().filter(visible));
    commands.push(upload);
    let mut edit = Command::new("gh");
    edit.args(["release", "edit", tag, "--repo", repo, "--draft=false"]);
    edit.args(["--prerelease=false", "--latest"]);
    commands.push(edit);
    Ok(commands)
}

/// Runs `release check-run`.
pub(super) fn check_run_main(args: &[String]) -> Result<()> {
    let spec =
        Spec { command: "release check-run", flags: &[], options: &[], help: CHECK_RUN_HELP };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(2, &spec)?;
    let run = parsed.positional(0, "the workflow run's id", &spec)?;
    let tag = parsed.positional(1, "the tag, such as v0.7.0", &spec)?;
    let repo = repository()?;
    let refuse = |why: String| {
        fail(
            "release-run",
            "RUN CANNOT BE PUBLISHED",
            &format!("Run {run} cannot be published: {why}"),
            "Recover from a release run that built this tag and passed every gate.",
        )
    };
    let info = gh_json(&format!("repos/{repo}/actions/runs/{run}"))?;
    let field = |name: &str| info.get(name).and_then(|v| v.as_str()).unwrap_or("").to_string();
    if field("path") != ".github/workflows/release.yml" {
        return Err(refuse(format!(
            "it ran {}, not .github/workflows/release.yml.",
            field("path")
        )));
    }
    if field("event") != "push" || field("head_branch") != tag {
        let ran = format!("{} of {}", field("event"), field("head_branch"));
        return Err(refuse(format!("it ran for {ran}, not for pushing {tag}.")));
    }
    let mut gh = Command::new("gh");
    gh.args(["api", &format!("repos/{repo}/commits/{tag}"), "--jq", ".sha"]);
    let commit = process::output(&mut gh)?.trim().to_string();
    if field("head_sha") != commit {
        return Err(refuse(format!("it built {}, but {tag} names {commit}.", field("head_sha"))));
    }
    if field("status") != "completed" {
        return Err(refuse(format!("it is still {}; wait for it.", field("status"))));
    }
    check_jobs(&repo, run).map_err(refuse)?;
    println!("Run {run} built {tag} at {commit} and passed every gate.");
    Ok(())
}

/// Fails with the reason unless the run has every gate job and every job but publish and
/// refresh succeeded.
fn check_jobs(repo: &str, run: &str) -> std::result::Result<(), String> {
    let mut gh = Command::new("gh");
    gh.args(["api", "--paginate", &format!("repos/{repo}/actions/runs/{run}/jobs")]);
    gh.args(["--jq", r#".jobs[] | [.name, (.conclusion // "none")] | @tsv"#]);
    let jobs =
        process::output(&mut gh).map_err(|_| "its jobs could not be listed.".to_string())?;
    print!("{jobs}");
    let rows: Vec<(&str, &str)> =
        jobs.lines().map(|line| line.split_once('\t').unwrap_or((line, ""))).collect();
    for gate in GATES {
        let present =
            rows.iter().any(|(name, _)| *name == gate || name.starts_with(&format!("{gate} (")));
        if !present {
            return Err(format!("it has no {gate} job."));
        }
    }
    let bad: Vec<String> = rows
        .iter()
        .filter(|(name, conclusion)| {
            !name.starts_with("publish")
                && !name.starts_with("refresh")
                && *conclusion != "success"
        })
        .map(|(name, conclusion)| format!("{name}\t{conclusion}"))
        .collect();
    if bad.is_empty() {
        Ok(())
    } else {
        Err(format!("these jobs did not succeed:\n{}", bad.join("\n")))
    }
}

/// GET a GitHub API path with gh, as JSON.
fn gh_json(path: &str) -> Result<gd_json::Value> {
    let text = process::output(Command::new("gh").args(["api", path]))?;
    gd_json::parse(&text).map_err(|err| {
        fail(
            "github-json",
            "UNREADABLE ANSWER",
            &format!("GitHub's answer for {path} is not JSON: {err}."),
            "Check GH_TOKEN and retry; `gh api` shows the raw answer.",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsutil::TempDir;

    #[test]
    fn publication_marks_each_version_as_the_latest_release() -> Result<()> {
        let dir = TempDir::new("publish-test")?;
        for version in ["1.0.0-beta.1", "1.0.0"] {
            let tag = format!("v{version}");
            let commands = publish_commands(
                dir.path(),
                "insanai/guidedog",
                &tag,
                version,
                dir.path(),
                State::Missing,
            )?;
            let Some(edit) = commands.last() else { panic!("no publication command") };
            let args: Vec<_> = edit.get_args().map(|arg| arg.to_string_lossy()).collect();
            assert!(args.iter().any(|arg| arg == "--prerelease=false"));
            assert!(args.iter().any(|arg| arg == "--latest"));
        }
        Ok(())
    }
}
