//! `PublisherSession`: one loaded publisher, with wrappers that hold its results to
//! the publication contract.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::problem::{self, Problem};
use crate::registry::validate_publisher;
use crate::{
    LoadOptions, Outcome, Plan, Plugin, Publisher, PublisherInfo, Registry, Snapshot, Source,
    State,
};

/// Numbers sessions, so that a plan is published only by the session that made it.
static SESSIONS: AtomicU64 = AtomicU64::new(1);

/// One publisher, loaded with its configuration and target. Dropping the session
/// unloads the publisher (and ends its program, for an installed one). One session
/// runs one operation at a time.
pub struct PublisherSession {
    publisher: Box<dyn Publisher>,
    info: PublisherInfo,
    options: LoadOptions,
    source: Source,
    target: String,
    serial: u64,
}

impl std::fmt::Debug for PublisherSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PublisherSession")
            .field("info", &self.info)
            .field("source", &self.source)
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

/// Loads the publisher `id` of `registry`: the one-call path to
/// [`PublisherSession::load`]. Fails when the registry has no such publisher.
pub fn load_publisher(
    registry: &Registry,
    id: &str,
    options: &LoadOptions,
) -> Result<PublisherSession, Problem> {
    let Some(plugin) = registry.find(id) else {
        return Err(Problem::new(
            "plugins.publisher",
            "No such publisher",
            &format!("This program has no publisher named {id:?}."),
        )
        .hint("Name a publisher the program lists, or register the program that provides it.")
        .path(id)
        .exit(2));
    };
    PublisherSession::load(plugin, options)
}

impl PublisherSession {
    /// Starts a fresh publisher value and has it read its configuration and target.
    /// Fails for a build plugin or a load problem; a failed load is unloaded before
    /// this returns.
    pub fn load(plugin: &Plugin, options: &LoadOptions) -> Result<PublisherSession, Problem> {
        plugin.validate()?;
        let Some(publisher) = plugin.make_publisher() else {
            return Err(problem::contract(
                plugin.id(),
                "Use this build plugin in conf.toml's plugins list, not guidedog publish.",
            ));
        };
        PublisherSession::load_value(publisher, options)
    }

    /// Loads one publisher value, checking its metadata first.
    pub(crate) fn load_value(
        mut publisher: Box<dyn Publisher>,
        options: &LoadOptions,
    ) -> Result<PublisherSession, Problem> {
        let info = publisher.info();
        validate_publisher(&info)?;
        let loaded = publisher.load(options)?;
        Ok(PublisherSession {
            publisher,
            info,
            options: options.clone(),
            source: loaded.source,
            target: loaded.target,
            serial: SESSIONS.fetch_add(1, Ordering::Relaxed),
        })
    }

    /// The publisher's identity and capabilities.
    pub fn info(&self) -> &PublisherInfo {
        &self.info
    }

    /// The options the session was loaded with.
    pub fn options(&self) -> &LoadOptions {
        &self.options
    }

    /// What the configuration says to publish.
    pub fn source(&self) -> &Source {
        &self.source
    }

    /// The target, described for people.
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Compares `snapshot` with the target and returns what publishing it would change.
    /// It only reads the target. A plan that calls the target unchanged yet lists
    /// changes breaks the contract.
    pub fn plan(&mut self, snapshot: &Snapshot) -> Result<Plan, Problem> {
        let mut plan = Plan::new(snapshot.digest(), self.serial);
        self.publisher.plan(snapshot, &mut plan)?;
        if plan.unchanged && !plan.changes.is_empty() {
            return Err(contract("plan", "lists changes for a target it calls unchanged"));
        }
        Ok(plan)
    }

    /// Publishes `plan`, made by this session from `snapshot`, after checking the
    /// snapshot's files again. A dry-run session never publishes. An outcome that names
    /// another generation, or no operation, is `Unknown`, never success.
    pub fn publish(&mut self, snapshot: &Snapshot, plan: &Plan) -> Outcome {
        if let Err(p) = self.check_plan(snapshot, plan) {
            return Outcome::failed(p);
        }
        if self.options.dry_run {
            return Outcome::failed(dry_run(
                &self.target,
                "The session was loaded for a dry run, which never publishes.",
                "Run the command again without --dry-run to publish.",
            ));
        }
        if let Err(p) = snapshot.verify() {
            return Outcome::failed(p);
        }
        let mut r = self.publisher.publish(snapshot, plan);
        let made = !matches!(r.state, State::Failed | State::NotStarted);
        if made && r.generation != plan.generation_hex() {
            r = contract_outcome(
                r,
                "publish",
                "reported another generation than the one it published",
            );
        }
        settle(r, "publish")
    }

    /// Reads the outcome of `operation` again (`""` for the last one), writing nothing.
    pub fn status(&mut self, operation: &str) -> Outcome {
        if !self.info.capabilities.status {
            return Outcome::failed(self.unsupported("status"));
        }
        let mut r = self.publisher.status(operation);
        if !operation.is_empty() && r.state != State::Failed && r.operation != operation {
            r = contract_outcome(r, "status", "reported another operation than the one requested");
        }
        settle(r, "status")
    }

    /// Settles an operation interrupted at the target's boundary: reads what the target
    /// holds and finishes or reports the operation, never forcing the target back.
    pub fn recover(&mut self) -> Outcome {
        if !self.info.capabilities.recover {
            return Outcome::failed(self.unsupported("recover"));
        }
        if self.options.dry_run {
            return Outcome::failed(dry_run(
                &self.target,
                "Recovery may write, and a dry run never does.",
                "Run recover without --dry-run.",
            ));
        }
        let r = self.publisher.recover();
        settle(r, "recover")
    }

    fn check_plan(&self, snapshot: &Snapshot, plan: &Plan) -> Result<(), Problem> {
        if plan.session != self.serial {
            return Err(plan_problem(
                &self.target,
                "The plan is another session's",
                "A plan is published only by the session that made it.",
                "Make a new plan with this session, then publish it.",
            ));
        }
        if plan.generation != snapshot.digest() {
            return Err(plan_problem(
                &self.target,
                "The plan is another snapshot's",
                "The plan was made for another generation of the site.",
                "Make a new plan from this snapshot, then publish it.",
            ));
        }
        Ok(())
    }

    fn unsupported(&self, operation: &str) -> Problem {
        let id = &self.info.id;
        Problem::new(
            "plugins.unsupported",
            "The publisher cannot do this",
            &format!("The publisher {id} has no {operation} operation."),
        )
        .hint("Use the publisher's own commands for this, or update it.")
        .path(id)
        .exit(2)
    }
}

/// Holds a result to the contract: an outcome other than `Failed` names its operation,
/// a verified one has no problem, and `Failed` has one. A result left as `NotStarted`,
/// or that breaks the contract, cannot pass for success.
fn settle(r: Outcome, operation: &str) -> Outcome {
    let mut out = r;
    match out.state {
        State::NotStarted => out = contract_outcome(out, operation, "returned no outcome"),
        State::Failed => {
            if out.problem.is_none() {
                out.problem = Some(
                    Problem::new(
                        "plugins.failed",
                        "Publication failed",
                        "The publisher reported a failure without saying why.",
                    )
                    .hint(
                        "Run the command again with the publisher's status command to learn more.",
                    )
                    .path(&out.url),
                );
            }
        }
        State::Unchanged | State::Accepted | State::Pending | State::Live | State::Unknown => {
            let explained = out.state == State::Unknown && out.problem.is_some();
            if out.operation.is_empty() && out.state != State::Unchanged && !explained {
                out = contract_outcome(
                    out,
                    operation,
                    "reported an outcome without its operation's ID",
                );
            } else if matches!(out.state, State::Live | State::Unchanged) && out.problem.is_some()
            {
                out.state = State::Unknown;
            }
        }
    }
    out
}

/// `r` made `Unknown`: what a publisher that broke its contract did at its target is
/// not known, so it is neither success nor a failure that may be retried.
fn contract_outcome(r: Outcome, operation: &str, what: &str) -> Outcome {
    Outcome { state: State::Unknown, problem: Some(contract(operation, what)), ..r }
}

fn contract(operation: &str, what: &str) -> Problem {
    Problem::new(
        "plugins.contract",
        "The publisher broke its contract",
        &format!("The publisher's {operation} {what}."),
    )
    .hint(
        "Report this to the publisher's authors with its version and the command you ran; \
         inspect the target or run recover before publishing again.",
    )
    .path(operation)
    .exit(5)
}

fn plan_problem(target: &str, title: &str, message: &str, hint: &str) -> Problem {
    Problem::new("plugins.plan", title, message).hint(hint).path(target).exit(2)
}

fn dry_run(target: &str, message: &str, hint: &str) -> Problem {
    Problem::new("plugins.dry-run", "This is a dry run", message).hint(hint).path(target).exit(2)
}

#[cfg(test)]
mod tests;
