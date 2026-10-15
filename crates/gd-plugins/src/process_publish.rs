//! The publisher process adapter: a `Publisher` that runs an installed program and
//! speaks `guidedog-publisher` 1.0 with it, one program per session.

use std::sync::Arc;
use std::time::Duration;

use gd_json::{Object, Value};

use crate::child::{Child, ChildKind};
use crate::problem::{self, Problem};
use crate::wire::{self, Handshake, object};
use crate::{
    Cancel, Capabilities, Change, ChangeKind, LoadOptions, Loaded, MachineEntry, Outcome,
    PROTOCOL_NAME, PROTOCOL_VERSION, Plan, Plugin, Publisher, PublisherInfo, SNAPSHOT_SCHEMA,
    Snapshot, Source, State, select_version,
};

/// Bounds a publisher request, including its write and response, when the options
/// leave it zero.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// Bounds the wait for a program's handshake.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// A publisher that runs the registered program `entry` in protocol mode, one program
/// a session: load starts it and checks its handshake against the entry, dropping the
/// session ends it. Verify the entry ([`crate::verify_entry`]) before offering it.
/// Fails for a build plugin's entry.
pub fn process_publisher(entry: &MachineEntry) -> Result<Plugin, Problem> {
    if entry.is_build() {
        return Err(problem::contract(
            &entry.id,
            "Use this build plugin in conf.toml's plugins list, not guidedog publish.",
        ));
    }
    let info = PublisherInfo::new(&entry.id, &entry.name, &entry.version, entry.capabilities);
    let (prototype, executable) = (info.clone(), entry.path.clone());
    let make = Arc::new(move || -> Box<dyn Publisher> {
        Box::new(ProcessPublisher {
            info: prototype.clone(),
            executable: executable.clone(),
            target: String::new(),
            child: None,
        })
    });
    Ok(Plugin::from_publisher_maker(info, &entry.path, make).with_program_sha256(&entry.sha256))
}

/// Runs the program at `executable` in protocol mode, reads its handshake, and ends
/// it, waiting at most `timeout`. Running it runs its code: call it only for a program
/// the user chose.
pub fn inspect_program(executable: &str, timeout: Duration) -> Result<Handshake, Problem> {
    if timeout.is_zero() {
        return Err(Problem::new(
            "plugins.timeout-option",
            "No finite timeout",
            "Inspecting a program needs a positive time limit.",
        )
        .hint("Pass a positive Duration, such as HANDSHAKE_TIMEOUT.")
        .path(executable)
        .exit(2));
    }
    let mut child = Child::start(executable, ChildKind::Publisher, timeout, Cancel::default())?;
    let handshake = child.handshake(timeout);
    child.broken = true; // nothing more is sent; end it now.
    handshake
}

struct ProcessPublisher {
    info: PublisherInfo,
    executable: String,
    target: String,
    child: Option<Child>,
}

impl ProcessPublisher {
    fn child(&mut self) -> Result<&mut Child, Problem> {
        self.child
            .as_mut()
            .ok_or_else(|| problem::protocol("The publisher program is not loaded."))
    }

    fn check_handshake(&self, h: &Handshake) -> Result<(), Problem> {
        let info = &self.info;
        if h.protocol != PROTOCOL_NAME {
            return Err(problem::contract(
                &info.id,
                "Use this build plugin in conf.toml's plugins list, not guidedog publish.",
            ));
        }
        if h.id != info.id || h.version != info.version {
            let message = format!(
                "It now says it is {} {}, not {} {}.",
                h.id, h.version, info.id, info.version
            );
            return Err(changed(&self.executable, &message));
        }
        let offered = Capabilities::from_names(h.capabilities.iter().map(String::as_str));
        if !info.capabilities.is_subset_of(offered)
            || !h.snapshot_schemas.contains(&SNAPSHOT_SCHEMA)
        {
            return Err(changed(
                &self.executable,
                "It no longer offers what it offered when it was registered.",
            ));
        }
        Ok(())
    }

    /// Sends a request answered by an outcome. When `writes` and the program fails to
    /// answer, the outcome is `Unknown`: it may have written before it stopped.
    fn outcome_exchange(&mut self, request: Value, writes: bool) -> Outcome {
        let child = match self.child() {
            Ok(child) => child,
            Err(p) => return Outcome::failed(p),
        };
        let uncertain = if writes { State::Unknown } else { State::Failed };
        let o = match child.exchange(request) {
            Ok(o) => o,
            Err(p) if child.broken && writes => {
                return Outcome { state: State::Unknown, problem: Some(p), ..Outcome::default() };
            }
            Err(p) => return Outcome::failed(p),
        };
        let Some(wire) = wire::obj_field(&o, "result") else {
            let p = problem::protocol("A response has no result.");
            return Outcome { state: uncertain, problem: Some(p), ..Outcome::default() };
        };
        let name = wire::str_field(wire, "state").unwrap_or_default();
        let Some(state) = State::from_name(name) else {
            let p = problem::protocol(&format!("A result has the unknown state {name:?}."));
            return Outcome { state: uncertain, problem: Some(p), ..Outcome::default() };
        };
        let text = |key| wire::str_field(wire, key).unwrap_or_default().to_string();
        Outcome {
            state,
            operation: text("operation"),
            generation: text("generation"),
            url: text("url"),
            problem: wire::obj_field(wire, "problem").and_then(wire::problem_of),
        }
    }
}

impl Publisher for ProcessPublisher {
    fn info(&self) -> PublisherInfo {
        self.info.clone()
    }

    fn load(&mut self, options: &LoadOptions) -> Result<Loaded, Problem> {
        let timeout = if options.request_timeout.is_zero() {
            DEFAULT_REQUEST_TIMEOUT
        } else {
            options.request_timeout
        };
        let child =
            Child::start(&self.executable, ChildKind::Publisher, timeout, options.cancel.clone())?;
        let handshake = self.child.insert(child).handshake(HANDSHAKE_TIMEOUT)?;
        self.check_handshake(&handshake)?;
        let Some(version) = select_version(&handshake) else {
            return Err(Problem::new(
                "plugins.version",
                "The publisher speaks another protocol",
                &format!(
                    "This Guidedog speaks protocol {PROTOCOL_VERSION}; the program does not."
                ),
            )
            .hint("Install a version of the publisher made for this Guidedog, and register it.")
            .path(&self.executable)
            .exit(2));
        };
        let child = self.child()?;
        let id = child.next_id();
        child.exchange(wire::select_request(id, version))?;
        let id = child.next_id();
        let o = child.exchange(object([
            ("id", id.into()),
            ("operation", "load".into()),
            ("config", options.config.as_str().into()),
            ("dry_run", options.dry_run.into()),
            ("wait_seconds", options.wait_seconds.into()),
        ]))?;
        let loaded = loaded_of(&o).inspect_err(|_| child.broken = true)?;
        self.target = loaded.target.clone();
        Ok(loaded)
    }

    fn plan(&mut self, snapshot: &Snapshot, plan: &mut Plan) -> Result<(), Problem> {
        let bounds = snapshot.bounds();
        let wire_snapshot = object([
            ("root", snapshot.root().to_string_lossy().as_ref().into()),
            ("digest", plan.generation_hex().into()),
            ("schema", SNAPSHOT_SCHEMA.into()),
            ("max_files", bounds.max_files.into()),
            ("max_file_bytes", (bounds.max_file_bytes as i64).into()),
            ("max_bytes", (bounds.max_bytes as i64).into()),
        ]);
        let child = self.child()?;
        let id = child.next_id();
        let o = child.exchange(object([
            ("id", id.into()),
            ("operation", "plan".into()),
            ("snapshot", wire_snapshot),
        ]))?;
        let Some(wire) = wire::obj_field(&o, "plan") else {
            return Err(problem::protocol("A plan response has no plan."));
        };
        fill_plan(wire, plan, &self.target)
    }

    fn publish(&mut self, _: &Snapshot, plan: &Plan) -> Outcome {
        let Ok(child) = self.child() else {
            return Outcome::failed(problem::protocol("The publisher program is not loaded."));
        };
        let id = child.next_id();
        let request = object([
            ("id", id.into()),
            ("operation", "publish".into()),
            ("plan", plan.id().into()),
            ("generation", plan.generation_hex().into()),
        ]);
        let mut r = self.outcome_exchange(request, true);
        if r.state == State::Unknown && r.generation.is_empty() {
            r.generation = plan.generation_hex();
        }
        r
    }

    fn status(&mut self, operation: &str) -> Outcome {
        let Ok(child) = self.child() else {
            return Outcome::failed(problem::protocol("The publisher program is not loaded."));
        };
        let id = child.next_id();
        let request = object([
            ("id", id.into()),
            ("operation", "status".into()),
            ("publication", operation.into()),
        ]);
        self.outcome_exchange(request, false)
    }

    fn recover(&mut self) -> Outcome {
        let Ok(child) = self.child() else {
            return Outcome::failed(problem::protocol("The publisher program is not loaded."));
        };
        let id = child.next_id();
        self.outcome_exchange(object([("id", id.into()), ("operation", "recover".into())]), true)
    }
}

impl Drop for ProcessPublisher {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            unload_child(child);
        }
    }
}

/// Asks a healthy program to unload, within one second and regardless of
/// cancellation, then stops it.
pub(crate) fn unload_child(child: &mut Child) {
    if !child.broken {
        child.cancel = Cancel::default();
        child.timeout = Duration::from_secs(1);
        let id = child.next_id();
        let _ = child.exchange(object([("id", id.into()), ("operation", "unload".into())]));
    }
    child.stop();
}

fn loaded_of(o: &Object) -> Result<Loaded, Problem> {
    let source = wire::obj_field(o, "source");
    let text = |key| source.and_then(|s| wire::str_field(s, key)).unwrap_or_default().to_string();
    let loaded = Loaded {
        source: Source {
            project: text("project"),
            site: text("site"),
            base_url: text("base_url"),
            strict: source.and_then(|s| wire::bool_field(s, "strict")).unwrap_or(false),
        },
        target: wire::str_field(o, "target").unwrap_or_default().to_string(),
    };
    let one_source = loaded.source.project.is_empty() != loaded.source.site.is_empty();
    if source.is_none() || !one_source || loaded.target.is_empty() {
        return Err(problem::protocol("A load response needs one project or site and a target."));
    }
    Ok(loaded)
}

fn fill_plan(wire: &Object, plan: &mut Plan, target: &str) -> Result<(), Problem> {
    let text = |key| wire::str_field(wire, key).unwrap_or_default().to_string();
    if text("generation") != plan.generation_hex() {
        return Err(problem::protocol("The plan is for another generation."));
    }
    let (Some(unchanged), Some(items)) =
        (wire::bool_field(wire, "unchanged"), wire::arr_field(wire, "changes"))
    else {
        return Err(malformed_plan());
    };
    if text("target") != target {
        return Err(malformed_plan());
    }
    let mut changes = Vec::with_capacity(items.len());
    for item in items {
        let change = item.as_object().and_then(|c| {
            let path = wire::str_field(c, "path")?;
            let kind = wire::str_field(c, "kind")?;
            Some((path, kind))
        });
        let Some((path, kind)) = change else {
            return Err(problem::protocol("A plan lists a malformed change."));
        };
        let Some(kind) = ChangeKind::from_name(kind) else {
            return Err(problem::protocol("A plan lists an unknown kind of change."));
        };
        changes.push(Change { path: path.to_string(), kind });
    }
    plan.id = text("id");
    plan.target = text("target");
    plan.baseline = text("baseline");
    plan.summary = text("summary");
    plan.unchanged = unchanged;
    plan.changes = changes;
    if plan.id.is_empty() {
        return Err(problem::protocol("A plan has no ID."));
    }
    Ok(())
}

fn malformed_plan() -> Problem {
    problem::protocol(
        "A plan needs a boolean unchanged field, a changes array, and the session's target.",
    )
}

fn changed(executable: &str, message: &str) -> Problem {
    Problem::new("plugins.changed", "The publisher program changed", message)
        .hint("Inspect the program, then register it again if you trust it.")
        .path(executable)
        .exit(2)
}
