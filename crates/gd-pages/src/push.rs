//! The branch is read and moved with Git's smart HTTP protocol, as `git push` does: one
//! request reads the branch's commit, and one carries the pack and the update.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gd_fetch::{Body, ErrorKind, Header, Method, Request};
use gd_git::{Id, Kind, PackWriter, Tree, Update};
use gd_plugins::Problem;

use crate::client::{Call, Client, status_problem, transfer_problem};
use crate::problem::{io_problem, pack_problem, problem};

/// Reads the commit `refs/heads/BRANCH` of `owner/repo` names; `None` when the branch
/// does not exist. A repository of SHA-256 objects is refused.
pub fn branch_head(
    c: &Client,
    owner: &str,
    repo: &str,
    branch: &str,
) -> Result<Option<Id>, Problem> {
    let url = format!("{}/{owner}/{repo}.git/info/refs?service=git-receive-pack", c.roots.web);
    let request =
        Request { url: url.clone(), headers: vec![c.basic_auth()], ..Request::default() };
    let r = c.send(request, Duration::from_secs(60), gd_git::MAX_ADVERTISEMENT);
    let x = Call {
        path: &url,
        doing: "read the branch",
        needs: "Contents: read and write",
        ..Call::default()
    };
    match r.error.as_ref().map(|e| e.kind) {
        None => {}
        Some(ErrorKind::Status) => return Err(status_problem(&r, None, &x)),
        Some(_) => return Err(transfer_problem(&r, &url, x.doing)),
    }
    let a = gd_git::parse_advertisement(&r.body).map_err(|e| {
        problem(
            "pages.github",
            "GitHub's answer is not Git's",
            &url,
            &e.message,
            "Try again later; check https://www.githubstatus.com if it persists.",
        )
    })?;
    if a.has_capability("object-format") && !a.has_capability("object-format=sha1") {
        return Err(problem(
            "pages.sha256",
            "The repository uses SHA-256 objects",
            &url,
            "This publisher writes SHA-1 objects, Git's default, which the repository does \
             not take.",
            "Publish to a repository that uses SHA-1 objects.",
        )
        .exit(2));
    }
    Ok(a.find(&format!("refs/heads/{branch}")))
}

/// How a push ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushOutcome {
    /// GitHub moved the branch.
    Accepted,
    /// GitHub refused the update; nothing changed.
    Refused(Problem),
    /// The answer was lost after the request was sent: the branch may have moved.
    Unknown(Problem),
}

/// Sends the request file at `request` (of `size` bytes), which moves `branch` from
/// `old` (`None` for a new branch), and reads GitHub's report. A refusal is
/// `pages.conflict` when the branch no longer holds `old`: receive-pack's words say so
/// only sometimes, so the branch is read again when they do not.
pub fn push(
    c: &Client,
    owner: &str,
    repo: &str,
    branch: &str,
    old: Option<Id>,
    request: &Path,
    size: u64,
) -> PushOutcome {
    let url = format!("{}/{owner}/{repo}.git/git-receive-pack", c.roots.web);
    // Ten minutes, and a minute more for each 10 MB, at about 170 kB/s.
    let timeout = Duration::from_secs(600 + size / (10 * 1024 * 1024) * 60);
    let headers = vec![
        c.basic_auth(),
        Header::new("Content-Type", "application/x-git-receive-pack-request"),
        Header::new("Accept", "application/x-git-receive-pack-result"),
    ];
    let send = Request {
        method: Method::Post,
        url: url.clone(),
        headers,
        body: Body::File(request.to_path_buf()),
    };
    let r = c.send(send, timeout, gd_git::MAX_ADVERTISEMENT);
    let x = Call {
        path: &url,
        doing: "update the branch",
        needs: "Contents: read and write",
        ..Call::default()
    };
    match r.error.as_ref().map(|e| e.kind) {
        None => {}
        // GitHub refused the request before it changed anything.
        Some(ErrorKind::Status) => return PushOutcome::Refused(status_problem(&r, None, &x)),
        Some(ErrorKind::Redirect | ErrorKind::Unsupported | ErrorKind::Missing) => {
            return PushOutcome::Refused(transfer_problem(&r, &url, x.doing));
        }
        Some(_) => return PushOutcome::Unknown(transfer_problem(&r, &url, x.doing)),
    }
    match read_report(&r.body, &url, branch) {
        PushOutcome::Refused(p) => PushOutcome::Refused(moved(c, owner, repo, branch, old, p)),
        outcome => outcome,
    }
}

/// The problem of a refused push: a conflict when the branch no longer holds `old`,
/// which tells it more surely than receive-pack's words. Stock Git, for one, refuses a
/// stale update with only "failed to update ref".
fn moved(
    c: &Client,
    owner: &str,
    repo: &str,
    branch: &str,
    old: Option<Id>,
    refused: Problem,
) -> Problem {
    if refused.code == "pages.conflict" || refused.code == "pages.ruleset" {
        return refused;
    }
    match branch_head(c, owner, repo, branch) {
        Ok(head) if head != old => problem(
            "pages.conflict",
            "The branch moved",
            branch,
            &format!("Another publication moved {branch} after this one read it."),
            "Run the command again: it plans from the branch as it is now.",
        ),
        _ => refused,
    }
}

fn read_report(body: &[u8], url: &str, branch: &str) -> PushOutcome {
    let lost = |title: &str, message: &str| {
        let hint = "Run guidedog pages recover to learn whether the branch moved.";
        problem("pages.github", title, url, message, hint).exit(7)
    };
    let report = match gd_git::parse_report(body) {
        Ok(r) => r,
        Err(e) => return PushOutcome::Unknown(lost("GitHub's report is not Git's", &e.message)),
    };
    if let Some(error) = &report.unpack_error {
        return PushOutcome::Refused(problem(
            "pages.github",
            "GitHub refused the objects",
            url,
            &format!("GitHub could not unpack the publication: {error}."),
            "Run the command again; report it to Guidedog's authors if it persists.",
        ));
    }
    let name = format!("refs/heads/{branch}");
    match report.refs.iter().find(|s| s.name == name) {
        Some(s) => match &s.refusal {
            None => PushOutcome::Accepted,
            Some(reason) => PushOutcome::Refused(refusal(branch, reason)),
        },
        None => PushOutcome::Unknown(lost(
            "GitHub's report does not name the branch",
            "The report does not say whether the branch moved.",
        )),
    }
}

/// Explains why receive-pack refused to move the branch.
pub(crate) fn refusal(branch: &str, reason: &str) -> Problem {
    let lower = reason.to_lowercase();
    let says = |words: &[&str]| words.iter().any(|w| lower.contains(w));
    // GitHub says "cannot lock ref 'refs/heads/B': is at X but expected Y" for a stale
    // update and "... reference already exists" for a branch created meanwhile. Other
    // "cannot lock ref" refusals, such as a name that clashes with a folder of
    // branches, are not conflicts, so the words after it decide.
    let stale = ["fetch first", "stale", "non-fast-forward", "failed to lock", "but expected"];
    if says(&stale) || says(&["reference already exists"]) {
        return problem(
            "pages.conflict",
            "The branch moved",
            branch,
            &format!("Another publication moved {branch} after this one read it ({reason})."),
            "Run the command again: it plans from the branch as it is now.",
        );
    }
    if says(&["protected", "rule", "declined", "gh0"]) {
        return problem(
            "pages.ruleset",
            "A rule of the repository refused the update",
            branch,
            &format!("GitHub refused to move {branch}: {reason}."),
            "Let this token's account push to the branch in the repository's rulesets and \
             branch protection, or publish to another branch.",
        )
        .exit(2);
    }
    problem(
        "pages.refused",
        "GitHub refused to move the branch",
        branch,
        &format!("GitHub refused to move {branch}: {reason}."),
        "Check the repository's settings on GitHub, then run the command again.",
    )
}

/// A blob a publication sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blob {
    /// A file; the pack writer reads it again and refuses it if its size or ID changed.
    File {
        /// Where the file is.
        path: PathBuf,
        /// Its size in bytes, as [`gd_git::file_id`] measured it.
        size: u64,
        /// Its Git blob ID, as [`gd_git::file_id`] computed it.
        id: Id,
    },
    /// Bytes held in memory, such as the empty `.nojekyll` or a `CNAME`.
    Bytes(Vec<u8>),
}

/// What a pack carries: the blobs the branch lacks, the trees, and the commit.
#[derive(Debug, Clone, Copy)]
pub struct PackContents<'a> {
    /// The blobs the branch lacks, each once.
    pub blobs: &'a [Blob],
    /// The trees, as [`gd_git::build_trees`] returns them.
    pub trees: &'a [Tree],
    /// The commit, as [`gd_git::commit_bytes`] writes it.
    pub commit: &'a [u8],
}

/// Writes the objects of a publication into the folder `work` and the request that
/// moves `branch` from `old` (`None` for a new branch) to `candidate`, ready for
/// [`push`]. Returns the request file, which the caller removes, and its size; the pack
/// itself is removed. Fails with `pages.pack` when a file cannot be read, changed, or
/// the folder cannot be written.
pub fn write_pack(
    work: &Path,
    contents: &PackContents<'_>,
    branch: &str,
    old: Option<Id>,
    candidate: Id,
) -> Result<(PathBuf, u64), Problem> {
    let pack = work.join(format!("{candidate}.pack"));
    let request = work.join(format!("{candidate}.pack.request"));
    let _ = std::fs::remove_file(&pack);
    let _ = std::fs::remove_file(&request);
    let count = contents.blobs.len() + contents.trees.len() + 1;
    let mut w = PackWriter::begin(&pack, count).map_err(|e| pack_problem(&e))?;
    for blob in contents.blobs {
        let added = match blob {
            Blob::File { path, size, id } => w.add_file(path, *size, *id),
            Blob::Bytes(bytes) => w.add(Kind::Blob, bytes),
        };
        added.map_err(|e| pack_problem(&e))?;
    }
    for t in contents.trees {
        w.add(Kind::Tree, &t.data).map_err(|e| pack_problem(&e))?;
    }
    w.add(Kind::Commit, contents.commit).map_err(|e| pack_problem(&e))?;
    w.end().map_err(|e| pack_problem(&e))?;
    let reference = format!("refs/heads/{branch}");
    let agent = Client::agent();
    let update = Update {
        reference: &reference,
        old: old.unwrap_or(Id::ZERO),
        new: candidate,
        pack: &pack,
        agent: &agent,
    };
    let written = gd_git::write_request(&request, &update);
    let _ = std::fs::remove_file(&pack);
    written.map_err(|e| pack_problem(&e))?;
    let size = std::fs::metadata(&request)
        .map_err(|e| io_problem(&request.display().to_string(), &e))?
        .len();
    Ok((request, size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusals_are_conflicts_rules_or_other() {
        assert_eq!(refusal("gh-pages", "fetch first").code, "pages.conflict");
        assert_eq!(refusal("gh-pages", "failed to lock").code, "pages.conflict");
        assert_eq!(refusal("gh-pages", "protected branch hook declined").code, "pages.ruleset");
        assert_eq!(refusal("gh-pages", "GH013: Repository rule violations").code, "pages.ruleset");
        assert_eq!(refusal("gh-pages", "funny refname").code, "pages.refused");
        let github = "cannot lock ref 'refs/heads/gh-pages': is at 1a but expected 2b";
        assert_eq!(refusal("gh-pages", github).code, "pages.conflict");
        let created = "cannot lock ref 'refs/heads/gh-pages': reference already exists";
        assert_eq!(refusal("gh-pages", created).code, "pages.conflict");
        let folder = "cannot lock ref 'refs/heads/a/b': 'refs/heads/a' exists";
        assert_eq!(refusal("a/b", folder).code, "pages.refused");
        assert_eq!(refusal("gh-pages", "failed to update ref").code, "pages.refused");
    }

    #[test]
    fn reports_settle_or_lose_the_branch() {
        let pkt = |s: &str| format!("{:04x}{s}", s.len() + 4);
        let ok = format!("{}{}0000", pkt("unpack ok\n"), pkt("ok refs/heads/gh-pages\n"));
        assert_eq!(read_report(ok.as_bytes(), "u", "gh-pages"), PushOutcome::Accepted);
        let ng =
            format!("{}{}0000", pkt("unpack ok\n"), pkt("ng refs/heads/gh-pages stale info\n"));
        assert!(matches!(read_report(ng.as_bytes(), "u", "gh-pages"),
            PushOutcome::Refused(p) if p.code == "pages.conflict"));
        let other = format!("{}{}0000", pkt("unpack ok\n"), pkt("ok refs/heads/main\n"));
        assert!(matches!(read_report(other.as_bytes(), "u", "gh-pages"),
            PushOutcome::Unknown(p) if p.exit == 7));
        assert!(matches!(read_report(b"garbage", "u", "gh-pages"), PushOutcome::Unknown(_)));
    }
}
