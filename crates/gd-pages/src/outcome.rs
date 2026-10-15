//! Publication outcomes: the journal, the push, the accepted result, the deployment's
//! status, and recovery after a lost answer.
//!
//! The plan records branch tip H. Publication makes candidate C with H as its parent and
//! asks GitHub to replace H with C only if the branch still holds H; it never forces a
//! branch. The journal records H and C before the request is sent, so a lost reply is
//! `Unknown` and recovery reads the branch before deciding what happened. `Accepted`
//! means the branch changed, not that Pages is live; `Live` needs a built deployment of
//! exactly C. The last record is saved before the journal is deleted, so a local disk
//! error cannot turn an accepted push into a failure that is safe to retry.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use gd_git::{Commit, Id, Kind, Person};
use gd_plugins::{EXIT_ACCEPTED, EXIT_UNKNOWN, Outcome, Plan, Problem, State};

use crate::client::sleep_cancellable;
use crate::config::Config;
use crate::github::{BuildState, pages_build, pages_site};
use crate::problem::problem;
use crate::push::{PackContents, PushOutcome, branch_head, push, write_pack};
use crate::session::{MARK, PagesPlan, Session};
use crate::state::{Journal, today};
use crate::target::short;

/// Removes a request file when the publication is done with it.
struct RemoveOnDrop(PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

impl Session {
    /// Carries out `plan`: commits the new tree on the branch's commit, writes the
    /// journal, sends one push that moves the branch only if it still has that commit,
    /// and waits for GitHub Pages to deploy the new commit.
    pub(crate) fn publish(&mut self, plan: &Plan) -> Outcome {
        let generation = plan.generation_hex();
        let url = self.config.site_url.clone();
        let Some(pp) = plan.data_as::<PagesPlan>() else {
            return Outcome::failed(
                problem(
                    "pages.plan",
                    "The plan is not this publisher's",
                    &self.target,
                    "The plan holds no pages data.",
                    "Make a new plan, then publish it.",
                )
                .exit(2),
            );
        };
        if plan.unchanged {
            let operation = pp.head.map(|h| h.to_hex()).unwrap_or_default();
            return Outcome { state: State::Unchanged, operation, generation, url, problem: None };
        }
        if let Err(p) = self.refuse_unsettled() {
            return Outcome::failed(p);
        }
        let commit = gd_git::commit_bytes(&self.publication_commit(pp, &generation));
        let candidate = gd_git::object_id(Kind::Commit, &commit);
        let contents = PackContents { send: &pp.send, trees: &pp.trees, commit: &commit };
        let (request, size) =
            match write_pack(&self.work, &contents, &self.config.branch, pp.head, candidate) {
                Ok(written) => written,
                Err(p) => return Outcome::failed(p),
            };
        let request = RemoveOnDrop(request);
        let j = self.journal(pp.head, candidate, generation);
        if let Err(p) = self.state.write_journal(&self.key, &j) {
            return Outcome::failed(p);
        }
        let c = &self.config;
        match push(&self.client, &c.owner, &c.repo, &c.branch, &request.0, size) {
            PushOutcome::Accepted => self.accepted(j),
            PushOutcome::Refused(p) => {
                let _ = self.state.drop_journal(&self.key);
                Outcome::failed(self.moved(&j, p))
            }
            PushOutcome::Unknown(p) => self.reconcile(j, p),
        }
    }

    fn journal(&self, head: Option<Id>, candidate: Id, generation: String) -> Journal {
        Journal {
            operation: candidate.to_hex(),
            repository: self.repository.full_name.clone(),
            repository_id: self.repository.id,
            branch: self.config.branch.clone(),
            observed: head.map(|h| h.to_hex()),
            candidate: candidate.to_hex(),
            generation,
            accepted: false,
            started: today(),
        }
    }

    /// The commit of a publication: the new tree on the branch's commit, by the token's
    /// account at its private address, at `SOURCE_DATE_EPOCH` or now.
    fn publication_commit(&self, pp: &PagesPlan, generation: &str) -> Commit {
        let a = &self.account;
        let name = if a.name.is_empty() { a.login.clone() } else { a.name.clone() };
        let who = Person { name, email: a.noreply(), time: now_seconds(), offset: 0 };
        Commit {
            tree: pp.tree,
            parents: pp.head.into_iter().collect(),
            author: who.clone(),
            committer: who,
            message: format!("Publish the site with Guidedog\n\n{MARK}{generation}\n"),
        }
    }

    /// Records that GitHub moved the branch to the journal's candidate, settles the
    /// journal, and waits for the deployment.
    fn accepted(&mut self, j: Journal) -> Outcome {
        let done = Journal { accepted: true, ..j };
        // The branch's state is known now: last records it for status, then the journal
        // is settled.
        let recorded = self.state.write_last(&self.key, &done);
        if let Err(p) = recorded.and_then(|()| self.state.drop_journal(&self.key)) {
            return self.recorded_later(&done, &p);
        }
        let c = &self.config;
        if let Ok(site) = pages_site(&self.client, &c.owner, &c.repo)
            && site.readable
            && !site.exists
        {
            return Outcome {
                state: State::Accepted,
                operation: done.candidate.clone(),
                generation: done.generation.clone(),
                url: c.site_url.clone(),
                problem: Some(no_site(c)),
            };
        }
        self.deployment(&done, self.config.wait_for(self.wait_seconds))
    }

    /// GitHub accepted the commit. A local disk failure cannot turn that into a claim
    /// that nothing was published, or discard the evidence recover needs.
    fn recorded_later(&self, j: &Journal, p: &Problem) -> Outcome {
        Outcome {
            state: State::Accepted,
            operation: j.operation.clone(),
            generation: j.generation.clone(),
            url: self.config.site_url.clone(),
            problem: Some(
                problem(
                    "pages.record",
                    "The publication needs recovery",
                    &p.path,
                    &format!(
                        "GitHub accepted commit {}, but its local record could not be \
                        settled: {}",
                        j.candidate, p.message
                    ),
                    &format!(
                        "Check free disk space and permissions, then run guidedog pages \
                        recover --config={}. Do not publish it again first.",
                        self.config.shown_path()
                    ),
                )
                .exit(EXIT_ACCEPTED),
            ),
        }
    }

    /// Settles a push whose answer was lost: it reads the branch again.
    fn reconcile(&mut self, j: Journal, lost: Problem) -> Outcome {
        let c = &self.config;
        let head = match branch_head(&self.client, &c.owner, &c.repo, &c.branch) {
            Ok(head) => head.map(|h| h.to_hex()),
            Err(_) => {
                return Outcome {
                    state: State::Unknown,
                    operation: j.operation.clone(),
                    generation: j.generation.clone(),
                    url: c.site_url.clone(),
                    problem: Some(self.unknown_problem(&lost)),
                };
            }
        };
        if head.as_deref() == Some(j.candidate.as_str()) {
            return self.accepted(j);
        }
        let _ = self.state.drop_journal(&self.key);
        if head == j.observed {
            let mut p = lost;
            p.title = "Nothing was published".to_string();
            p.message = format!("{} The branch did not move.", p.message);
            p.hint = "Run the command again.".to_string();
            return Outcome::failed(p);
        }
        let now = head.unwrap_or_default();
        Outcome::failed(problem(
            "pages.conflict",
            "The branch moved to another publication",
            &c.branch,
            &format!(
                "The branch now holds {}, neither the commit read before nor this \
                publication's.",
                short(&now)
            ),
            "Run the command again to publish this site on top of it.",
        ))
    }

    fn unknown_problem(&self, lost: &Problem) -> Problem {
        problem(
            "pages.unknown",
            "Whether the branch moved is not known",
            &self.config.branch,
            &format!("{} GitHub could not be asked again.", lost.message),
            &format!(
                "Run guidedog pages recover --config={} once GitHub can be reached.",
                self.config.shown_path()
            ),
        )
        .exit(EXIT_UNKNOWN)
    }

    /// Waits up to `wait` seconds for GitHub Pages to deploy the commit `j` moved the
    /// branch to: `Live` once it is built, `Failed` if its build failed, and otherwise
    /// `Pending` (a build is under way) or `Accepted` (none is listed, or the token may
    /// not read them).
    pub(crate) fn deployment(&self, j: &Journal, wait: u64) -> Outcome {
        let c = &self.config;
        let mut r = Outcome {
            state: State::Accepted,
            operation: j.candidate.clone(),
            generation: j.generation.clone(),
            url: c.site_url.clone(),
            problem: None,
        };
        let deadline = Instant::now() + Duration::from_secs(wait);
        let mut pause = Duration::from_secs(3);
        loop {
            match pages_build(&self.client, &c.owner, &c.repo, &j.candidate) {
                Err(_) | Ok(BuildState::Unreadable) => break,
                Ok(BuildState::NotListed) => {}
                Ok(BuildState::Listed(b)) => match b.status.as_str() {
                    "built" => {
                        r.state = State::Live;
                        return r;
                    }
                    "errored" => {
                        r.state = State::Failed;
                        r.problem = Some(build_failed(c, &b.error));
                        return r;
                    }
                    _ => r.state = State::Pending,
                },
            }
            if Instant::now() >= deadline
                || !sleep_cancellable(&self.client.cancel, pause.min(deadline - Instant::now()))
            {
                break;
            }
            pause = (pause * 2).min(Duration::from_secs(15));
        }
        r.problem = Some(problem(
            "pages.pending",
            "The deployment is not confirmed yet",
            &c.site_url,
            "GitHub accepted the publication; GitHub Pages has not reported its deployment yet.",
            &format!("Run guidedog pages status --config={} later.", c.shown_path()),
        )
        .exit(EXIT_ACCEPTED));
        r
    }

    /// Reads the deployment of the last publication (`operation` empty) or of the commit
    /// `operation` names, writing nothing.
    pub(crate) fn status(&mut self, operation: &str) -> Outcome {
        let last = match self.state.read_last(&self.key) {
            Ok(last) => last,
            Err(p) => return Outcome::failed(p),
        };
        let j = if operation.is_empty() {
            last
        } else {
            let generation = last.filter(|l| l.candidate == operation).map(|l| l.generation);
            Some(Journal {
                candidate: operation.to_string(),
                operation: operation.to_string(),
                generation: generation.unwrap_or_default(),
                ..Journal::default()
            })
        };
        let Some(j) = j else {
            return Outcome::failed(
                problem(
                    "pages.no-publication",
                    "Nothing was published yet",
                    &self.binding.repository,
                    "This machine has published nothing to this target.",
                    "Run the publish command first.",
                )
                .exit(2),
            );
        };
        self.deployment(&j, 0)
    }

    /// Settles a publication whose answer was lost: it reads the branch, and reports the
    /// publication accepted (and its deployment), not applied, or overtaken. It never
    /// moves the branch back.
    pub(crate) fn recover(&mut self) -> Outcome {
        let journal = match self.state.read_journal(&self.key) {
            Ok(j) => j,
            Err(p) => return Outcome::failed(p),
        };
        let Some(j) = journal else {
            return match self.state.read_last(&self.key) {
                Err(p) => Outcome::failed(p),
                Ok(None) => Outcome {
                    state: State::Unchanged,
                    url: self.config.site_url.clone(),
                    ..Outcome::default()
                },
                Ok(Some(last)) => self.deployment(&last, 0),
            };
        };
        let lost = problem(
            "pages.network",
            "The publication's answer was lost",
            &self.config.branch,
            "GitHub's answer to the publication did not arrive.",
            "Run the recover command again once GitHub can be reached.",
        )
        .exit(4);
        self.reconcile(j, lost)
    }

    /// The problem of a refused push: a conflict when the branch no longer has the commit
    /// the plan read, which tells it more surely than receive-pack's words.
    fn moved(&self, j: &Journal, refused: Problem) -> Problem {
        if refused.code == "pages.conflict" || refused.code == "pages.ruleset" {
            return refused;
        }
        let c = &self.config;
        match branch_head(&self.client, &c.owner, &c.repo, &c.branch) {
            Ok(head) if head.map(|h| h.to_hex()) != j.observed => problem(
                "pages.conflict",
                "The branch moved",
                &c.branch,
                &format!("Another publication moved {} after this one read it.", c.branch),
                "Run the command again: it plans from the branch as it is now.",
            ),
            _ => refused,
        }
    }
}

/// `SOURCE_DATE_EPOCH`, as reproducible builds set it, or the time now. A commit's time
/// is never before 1970.
fn now_seconds() -> i64 {
    match gd_clock::source_date_epoch() {
        Some(seconds) if seconds >= 0 => seconds,
        _ => gd_clock::now().unix.max(0),
    }
}

/// The problem of a publication to a repository without a Pages site: the branch holds
/// the site, and setup can now point GitHub Pages at it.
fn no_site(c: &Config) -> Problem {
    problem(
        "pages.no-site",
        "GitHub Pages is not set up yet",
        &c.repository(),
        &format!(
            "The branch {} holds the site, but the repository has no GitHub Pages site.",
            c.branch
        ),
        &format!(
            "Run guidedog pages setup --config={}, or choose Deploy from a branch, {}, / \
            (root) in the repository's Settings, Pages.",
            c.shown_path(),
            c.branch
        ),
    )
    .exit(EXIT_ACCEPTED)
}

fn build_failed(c: &Config, error: &str) -> Problem {
    let said = if error.is_empty() { String::new() } else { format!(": {error}") };
    problem(
        "pages.deployment",
        "GitHub Pages could not deploy it",
        &c.site_url,
        &format!(
            "The branch holds the publication, but its Pages build failed{said}. The site \
            readers see is the previous one."
        ),
        "Check the repository's Actions tab for the pages build and deployment run, then \
         publish again.",
    )
}
