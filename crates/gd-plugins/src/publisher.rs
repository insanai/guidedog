//! The publisher interface (GDS 0009): the `Publisher` trait, plans and outcomes.

use std::any::Any;
use std::fmt;
use std::time::Duration;

use crate::problem::{EXIT_ACCEPTED, EXIT_UNKNOWN, Problem};
use crate::{API_VERSION, Cancel, CommandContext, CommandInfo, Snapshot, Version};

/// A publisher sends a complete, frozen site to a destination such as GitHub Pages.
///
/// The value is one session's state: [`Publisher::load`] reads the configuration and
/// names the target, the other methods work on it, and dropping the value unloads it
/// (release locks, children and secrets in `Drop`). The SDK wrappers in
/// [`crate::PublisherSession`] hold every result to the contract, so a publisher that
/// reports no outcome, or the wrong generation, never passes for success.
pub trait Publisher: Send {
    /// What the publisher is and its optional operations; constant over its life.
    fn info(&self) -> PublisherInfo;

    /// Reads the configuration (`options.config`) and returns what to publish and
    /// where. A dry-run session must not take locks or credentials it does not need.
    fn load(&mut self, options: &LoadOptions) -> Result<Loaded, Problem>;

    /// Compares `snapshot` with the target, reading only, and fills in `plan`, whose
    /// generation is already the snapshot's digest.
    fn plan(&mut self, snapshot: &Snapshot, plan: &mut Plan) -> Result<(), Problem>;

    /// Carries out `plan`, made by this session from `snapshot`, which the SDK has
    /// checked again, and returns the verified outcome.
    fn publish(&mut self, snapshot: &Snapshot, plan: &Plan) -> Outcome;

    /// Reads the outcome of `operation` again (`""`: the last one), writing nothing.
    /// Called only when [`Capabilities::status`] is declared.
    fn status(&mut self, operation: &str) -> Outcome {
        let _ = operation;
        Outcome::failed(Problem::new("plugins.unsupported", "No status", "Not offered."))
    }

    /// Settles an operation interrupted at the target's boundary. Called only when
    /// [`Capabilities::recover`] is declared.
    fn recover(&mut self) -> Outcome {
        Outcome::failed(Problem::new("plugins.unsupported", "No recover", "Not offered."))
    }

    /// The commands the publisher adds to the host's command line, such as a guided
    /// `publish` target; constant over the value's life. The default adds none.
    fn commands(&self) -> Vec<CommandInfo> {
        Vec::new()
    }

    /// Runs the contributed command `name` (its words joined by spaces) with the
    /// arguments after its name, and returns the exit status. Runs on a fresh, unloaded
    /// value. The default refuses.
    fn run_command(
        &mut self,
        ctx: &mut CommandContext<'_>,
        name: &str,
        args: &[String],
    ) -> Result<i32, Problem> {
        let _ = (ctx, args);
        Err(crate::command::no_command(&self.info().id, name))
    }
}

/// A boxed publisher is a publisher; every operation forwards (forward new ones too).
impl Publisher for Box<dyn Publisher> {
    fn info(&self) -> PublisherInfo {
        (**self).info()
    }

    fn load(&mut self, options: &LoadOptions) -> Result<Loaded, Problem> {
        (**self).load(options)
    }

    fn plan(&mut self, snapshot: &Snapshot, plan: &mut Plan) -> Result<(), Problem> {
        (**self).plan(snapshot, plan)
    }

    fn publish(&mut self, snapshot: &Snapshot, plan: &Plan) -> Outcome {
        (**self).publish(snapshot, plan)
    }

    fn status(&mut self, operation: &str) -> Outcome {
        (**self).status(operation)
    }

    fn recover(&mut self) -> Outcome {
        (**self).recover()
    }

    fn commands(&self) -> Vec<CommandInfo> {
        (**self).commands()
    }

    fn run_command(
        &mut self,
        ctx: &mut CommandContext<'_>,
        name: &str,
        args: &[String],
    ) -> Result<i32, Problem> {
        (**self).run_command(ctx, name, args)
    }
}

/// A publisher's optional operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Capabilities {
    /// [`Publisher::status`].
    pub status: bool,
    /// [`Publisher::recover`].
    pub recover: bool,
}

impl Capabilities {
    /// The protocol names of the capabilities, in the order status, recover.
    pub fn names(self) -> impl Iterator<Item = &'static str> {
        [(self.status, "status"), (self.recover, "recover")]
            .into_iter()
            .filter_map(|(on, name)| on.then_some(name))
    }

    /// The capabilities `names` lists; unknown names are ignored.
    pub fn from_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Capabilities {
        let mut caps = Capabilities::default();
        for name in names {
            match name {
                "status" => caps.status = true,
                "recover" => caps.recover = true,
                _ => {}
            }
        }
        caps
    }

    /// Whether every capability of `self` is also in `other`.
    pub fn is_subset_of(self, other: Capabilities) -> bool {
        (!self.status || other.status) && (!self.recover || other.recover)
    }
}

/// A publisher's identity: ID, name, product version, the interface version it was
/// built against, and its optional operations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublisherInfo {
    /// The ID users select, such as `pages`.
    pub id: String,
    /// The name people see.
    pub name: String,
    /// The publisher's own release version.
    pub version: String,
    /// The publisher interface version, normally [`API_VERSION`].
    pub api: Version,
    /// Its optional operations.
    pub capabilities: Capabilities,
}

impl PublisherInfo {
    /// Describes a publisher built against [`API_VERSION`].
    pub fn new(id: &str, name: &str, version: &str, capabilities: Capabilities) -> PublisherInfo {
        PublisherInfo {
            id: id.to_string(),
            name: name.to_string(),
            version: version.to_string(),
            api: API_VERSION,
            capabilities,
        }
    }
}

/// A publisher session's settings. The default is a real (not dry) run with the
/// publisher's own wait and a five-minute request limit for programs. It is
/// `#[non_exhaustive]`: use [`LoadOptions::new`] or `Default`, then assign fields.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct LoadOptions {
    /// The publisher's configuration file, such as `pages.toml`.
    pub config: String,
    /// The session only plans: it never writes to the target.
    pub dry_run: bool,
    /// How long publish waits for a deployment to be confirmed: 0, the publisher's
    /// default; below 0, not at all (the outcome is then at most `Accepted`).
    pub wait_seconds: i64,
    /// Asks the publisher to stop; it cannot undo a write the target accepted.
    pub cancel: Cancel,
    /// Bounds each whole exchange with a program; zero means five minutes.
    pub request_timeout: Duration,
}

impl LoadOptions {
    /// Options for a real run with the configuration file `config`.
    pub fn new(config: &str) -> LoadOptions {
        LoadOptions { config: config.to_string(), ..LoadOptions::default() }
    }
}

/// What a session publishes: a Guidedog project to build as HTML (with warnings as
/// errors when `strict`), or a folder holding a site already built; exactly one of
/// `project` and `site` is set. `base_url` is where the site is seen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Source {
    /// A project folder to build.
    pub project: String,
    /// A built site's folder.
    pub site: String,
    /// The site's public URL.
    pub base_url: String,
    /// Build with warnings as errors.
    pub strict: bool,
}

/// What [`Publisher::load`] returns: the source, and the target described for people,
/// such as `"github.com/owner/repo, branch gh-pages"`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Loaded {
    /// What to publish.
    pub source: Source,
    /// Where it goes.
    pub target: String,
}

/// Where a publication stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum State {
    /// No outcome: the publisher reported nothing.
    #[default]
    NotStarted,
    /// The target already holds this generation; nothing was written.
    Unchanged,
    /// The host accepted the generation; its deployment is not confirmed.
    Accepted,
    /// The deployment of the generation is under way.
    Pending,
    /// The generation is confirmed live.
    Live,
    /// Nothing was published, or the deployment failed.
    Failed,
    /// The outcome is not known: inspect it before writing again.
    Unknown,
}

const STATES: [(State, &str); 7] = [
    (State::NotStarted, "not-started"),
    (State::Unchanged, "unchanged"),
    (State::Accepted, "accepted"),
    (State::Pending, "pending"),
    (State::Live, "live"),
    (State::Failed, "failed"),
    (State::Unknown, "unknown"),
];

impl State {
    /// The protocol's name, such as `"live"`.
    pub fn name(self) -> &'static str {
        STATES.iter().find(|(s, _)| *s == self).map_or("unknown", |(_, n)| n)
    }

    /// The state the protocol calls `name`.
    pub fn from_name(name: &str) -> Option<State> {
        STATES.iter().find(|(_, n)| *n == name).map(|(s, _)| *s)
    }
}

/// The outcome of publish, status or recover: a state, the operation's ID, the
/// generation it concerns (hexadecimal), the URL the site is seen at, and, unless the
/// outcome is verified, a problem that explains it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Outcome {
    /// Where the publication stands.
    pub state: State,
    /// The operation's ID, for status and recover.
    pub operation: String,
    /// The generation's digest in hexadecimal.
    pub generation: String,
    /// Where the site is seen.
    pub url: String,
    /// Why the outcome is not verified.
    pub problem: Option<Problem>,
}

impl Outcome {
    /// A `Failed` outcome with `problem`.
    pub fn failed(problem: Problem) -> Outcome {
        Outcome { state: State::Failed, problem: Some(problem), ..Outcome::default() }
    }

    /// The exit status a command returns: 0 when verified (`Live`, `Unchanged`),
    /// [`EXIT_ACCEPTED`] for `Accepted` or `Pending`, [`EXIT_UNKNOWN`] for `Unknown`, and
    /// the problem's status (at least 1) otherwise.
    pub fn exit_status(&self) -> i32 {
        match self.state {
            State::Live | State::Unchanged => 0,
            State::Accepted | State::Pending => EXIT_ACCEPTED,
            State::Unknown => EXIT_UNKNOWN,
            State::Failed | State::NotStarted => {
                self.problem.as_ref().map_or(1, |p| p.exit.max(1))
            }
        }
    }
}

/// What publishing a file does to the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChangeKind {
    /// A new file.
    Added,
    /// A file with other contents.
    Changed,
    /// A file the generation no longer has.
    Removed,
}

impl ChangeKind {
    /// The protocol's name: `"added"`, `"changed"` or `"removed"`.
    pub fn name(self) -> &'static str {
        match self {
            ChangeKind::Added => "added",
            ChangeKind::Changed => "changed",
            ChangeKind::Removed => "removed",
        }
    }

    /// The kind the protocol calls `name`.
    pub fn from_name(name: &str) -> Option<ChangeKind> {
        match name {
            "added" => Some(ChangeKind::Added),
            "changed" => Some(ChangeKind::Changed),
            "removed" => Some(ChangeKind::Removed),
            _ => None,
        }
    }
}

/// One file publishing adds, changes or removes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The file's path in the site.
    pub path: String,
    /// What happens to it.
    pub kind: ChangeKind,
}

/// What publishing a snapshot to a session's target would do. A plan is published only
/// by the session that made it, with the snapshot it was made from.
pub struct Plan {
    /// The target, for people.
    pub target: String,
    /// The target state compared with, as the publisher names it; empty for none.
    pub baseline: String,
    /// The target already holds the generation; `changes` is then empty.
    pub unchanged: bool,
    /// The changes, in the publisher's order.
    pub changes: Vec<Change>,
    /// What a person should see first, such as the account.
    pub summary: String,
    /// The publisher's own data for publish; see [`Plan::data_as`].
    pub data: Option<Box<dyn Any + Send>>,
    pub(crate) generation: [u8; 32],
    pub(crate) id: String,
    pub(crate) session: u64,
}

impl Plan {
    pub(crate) fn new(generation: [u8; 32], session: u64) -> Plan {
        Plan {
            target: String::new(),
            baseline: String::new(),
            unchanged: false,
            changes: Vec::new(),
            summary: String::new(),
            data: None,
            generation,
            id: String::new(),
            session,
        }
    }

    /// The generation the plan publishes: the snapshot's digest.
    pub fn generation(&self) -> [u8; 32] {
        self.generation
    }

    /// The generation in lowercase hexadecimal.
    pub fn generation_hex(&self) -> String {
        gd_sha::to_hex(&self.generation)
    }

    /// The plan's name in its session, for the process protocol; may be empty.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The publisher's data, if it is a `T`.
    pub fn data_as<T: 'static>(&self) -> Option<&T> {
        self.data.as_ref().and_then(|d| d.downcast_ref::<T>())
    }
}

impl fmt::Debug for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Plan")
            .field("id", &self.id)
            .field("generation", &self.generation_hex())
            .field("target", &self.target)
            .field("baseline", &self.baseline)
            .field("unchanged", &self.unchanged)
            .field("changes", &self.changes)
            .field("summary", &self.summary)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_have_protocol_names_and_exit_statuses() {
        for (state, name) in STATES {
            assert_eq!(state.name(), name);
            assert_eq!(State::from_name(name), Some(state));
        }
        assert_eq!(State::from_name("done"), None);
        let live = Outcome { state: State::Live, ..Outcome::default() };
        assert_eq!(live.exit_status(), 0);
        assert_eq!(Outcome { state: State::Pending, ..live.clone() }.exit_status(), 6);
        assert_eq!(Outcome::default().exit_status(), 1);
        let p = Problem::new("x", "X", "x").exit(4);
        assert_eq!(Outcome::failed(p).exit_status(), 4);
    }

    #[test]
    fn capabilities_round_trip_their_names() {
        let caps = Capabilities { status: true, recover: false };
        let names: Vec<_> = caps.names().collect();
        assert_eq!(names, ["status"]);
        assert_eq!(Capabilities::from_names(names), caps);
        assert!(caps.is_subset_of(Capabilities { status: true, recover: true }));
        assert!(!caps.is_subset_of(Capabilities::default()));
    }
}
