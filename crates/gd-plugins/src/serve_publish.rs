//! Serving a publisher over the `guidedog-publisher` protocol.

use std::io::{BufRead, BufReader, Write};
use std::process::ExitCode;

use gd_json::{Object, Value};

use crate::problem::{self, Problem};
use crate::serve_build::{end_status, request_of, started_by_host};
use crate::wire::{self, Handshake, object};
use crate::{
    API_VERSION, LoadOptions, MAX_REQUEST, Outcome, PROTOCOL_NAME, PROTOCOL_VERSION, Plan,
    Publisher, PublisherSession, SNAPSHOT_SCHEMA, Snapshot, SnapshotMode, SnapshotOptions,
    digest_hex, snapshot,
};

/// The whole `main` of a publisher program: when started with
/// [`crate::PROTOCOL_FLAG`], it serves `publisher` over standard input and output
/// until the host unloads it; otherwise it writes a one-line usage note to standard
/// error and returns 2.
pub fn serve_publisher<P: Publisher + 'static>(publisher: P) -> ExitCode {
    if !started_by_host() {
        eprintln!("Run this publisher through guidedog publish after guidedog plugin register.");
        return ExitCode::from(2);
    }
    let input = BufReader::new(std::io::stdin().lock());
    ExitCode::from(serve_publisher_streams(publisher, input, std::io::stdout().lock()))
}

/// Serves `publisher` over `input` and `output`: writes the handshake, then answers
/// requests until the host unloads it or closes `input`. Returns 0 then, 1 when the
/// host broke the protocol or output failed, and 2 for invalid publisher metadata.
/// The server keeps only the current session, snapshot and plan.
pub fn serve_publisher_streams<P: Publisher + 'static>(
    publisher: P,
    mut input: impl BufRead,
    output: impl Write,
) -> u8 {
    let info = publisher.info();
    if crate::registry::validate_publisher(&info).is_err() {
        return 2;
    }
    let handshake = Handshake {
        protocol: PROTOCOL_NAME.to_string(),
        versions: vec![PROTOCOL_VERSION],
        api: API_VERSION,
        id: info.id.clone(),
        name: info.name.clone(),
        version: info.version.clone(),
        capabilities: info.capabilities.names().map(str::to_string).collect(),
        snapshot_schemas: vec![SNAPSHOT_SCHEMA],
        hooks: Vec::new(),
    };
    let mut server = Server {
        publisher: Some(Box::new(publisher)),
        session: None,
        snapshot: None,
        plan: None,
        plans: 0,
        output,
        selected: false,
        broken: false,
    };
    server.send(&handshake.to_json());
    while !server.broken {
        let line = match wire::read_line(&mut input, MAX_REQUEST) {
            Ok(line) => line,
            Err(p) => return end_status(&p),
        };
        let Some((id, operation, o)) = request_of(&line) else {
            return 1;
        };
        if !server.answer(id, &operation, &o) {
            return u8::from(server.broken);
        }
    }
    1
}

struct Server<W: Write> {
    publisher: Option<Box<dyn Publisher>>,
    session: Option<PublisherSession>,
    snapshot: Option<Snapshot>,
    plan: Option<Plan>,
    plans: u64,
    output: W,
    selected: bool,
    broken: bool,
}

impl<W: Write> Server<W> {
    /// Answers one request; false once the host has unloaded the publisher.
    fn answer(&mut self, id: i64, operation: &str, o: &Object) -> bool {
        if operation != "select" && !self.selected {
            let p = problem::protocol("The host made a request before it selected a version.");
            self.refuse(id, &p);
            return true;
        }
        match operation {
            "select" => self.select(id, o),
            "load" => self.load(id, o),
            "plan" => self.plan(id, o),
            "publish" => self.publish(id, o),
            "status" => {
                let publication = wire::str_field(o, "publication").unwrap_or_default();
                let r = self.session.as_mut().map(|s| s.status(publication));
                self.send_outcome(id, r);
            }
            "recover" => {
                let r = self.session.as_mut().map(PublisherSession::recover);
                self.send_outcome(id, r);
            }
            "unload" => {
                self.plan = None;
                self.snapshot = None;
                self.session = None;
                self.send(&wire::ok_reply(id, []));
                return false;
            }
            _ => self.refuse(id, &problem::protocol("The host asked for an unknown operation.")),
        }
        true
    }

    fn select(&mut self, id: i64, o: &Object) {
        let version = o.get("version").and_then(wire::version_of);
        match version {
            Some(v) if v.is_compatible_with(PROTOCOL_VERSION) => {
                self.selected = true;
                self.send(&wire::ok_reply(id, []));
            }
            _ => {
                let p = problem::protocol("The host selected a version this publisher lacks.");
                self.refuse(id, &p);
            }
        }
    }

    fn load(&mut self, id: i64, o: &Object) {
        let Some(publisher) = self.publisher.take().filter(|_| self.session.is_none()) else {
            return self.refuse(id, &problem::protocol("The publisher is loaded already."));
        };
        let options = LoadOptions {
            config: wire::str_field(o, "config").unwrap_or_default().to_string(),
            dry_run: wire::bool_field(o, "dry_run").unwrap_or(false),
            wait_seconds: wire::int_field(o, "wait_seconds").unwrap_or(0),
            ..LoadOptions::default()
        };
        match PublisherSession::load_value(publisher, &options) {
            Ok(session) => {
                let src = session.source();
                let source = object([
                    ("project", src.project.as_str().into()),
                    ("site", src.site.as_str().into()),
                    ("base_url", src.base_url.as_str().into()),
                    ("strict", src.strict.into()),
                ]);
                let reply =
                    wire::ok_reply(id, [("source", source), ("target", session.target().into())]);
                self.session = Some(session);
                self.send(&reply);
            }
            Err(p) => self.refuse(id, &p),
        }
    }

    fn plan(&mut self, id: i64, o: &Object) {
        self.plan = None;
        self.snapshot = None;
        let result = self.make_plan(o);
        match result {
            Ok(reply) => self.send(&wire::ok_reply(id, [("plan", reply)])),
            Err(p) => self.refuse(id, &p),
        }
    }

    /// Freezes the snapshot the host names, checks it is the one the host froze, and
    /// plans it; the reply's plan object.
    fn make_plan(&mut self, o: &Object) -> Result<Value, Problem> {
        let empty = Object::new();
        let wire_snapshot = wire::obj_field(o, "snapshot").unwrap_or(&empty);
        if wire::int_field(wire_snapshot, "schema") != Some(SNAPSHOT_SCHEMA) {
            return Err(problem::protocol("The host sent a snapshot of another schema."));
        }
        let root = wire::str_field(wire_snapshot, "root").unwrap_or_default();
        let digest = wire::str_field(wire_snapshot, "digest").unwrap_or_default();
        let bounds = bounds_of(wire_snapshot)?;
        let Some(session) = self.session.as_mut() else {
            return Err(not_loaded());
        };
        // The host owns the snapshot's folder; checking it here proves both see one site.
        let snap = snapshot(root, &bounds)?;
        if digest_hex(&snap.digest()) != digest {
            return Err(Problem::new(
                "plugins.snapshot-changed",
                "The frozen site changed",
                "The publisher read other files than the host froze.",
            )
            .hint("Run the command again, and let no other program write to its build folder.")
            .path(root));
        }
        let mut plan = session.plan(&snap)?;
        self.plans += 1;
        plan.id = format!("plan-{}", self.plans);
        let reply = plan_json(&plan);
        self.snapshot = Some(snap);
        self.plan = Some(plan);
        Ok(reply)
    }

    fn publish(&mut self, id: i64, o: &Object) {
        let name = wire::str_field(o, "plan").unwrap_or_default();
        let generation = wire::str_field(o, "generation").unwrap_or_default();
        let matches = self
            .plan
            .as_ref()
            .is_some_and(|plan| name == plan.id() && generation == plan.generation_hex());
        let r = match (&mut self.session, &self.snapshot, &self.plan) {
            (Some(session), Some(snap), Some(plan)) if matches => session.publish(snap, plan),
            _ => return self.refuse(id, &foreign_plan(name)),
        };
        self.send_outcome(id, Some(r));
    }

    fn send_outcome(&mut self, id: i64, r: Option<Outcome>) {
        let Some(r) = r else {
            return self.refuse(id, &not_loaded());
        };
        let result = object([
            ("state", r.state.name().into()),
            ("operation", r.operation.as_str().into()),
            ("generation", r.generation.as_str().into()),
            ("url", r.url.as_str().into()),
            ("problem", wire::problem_json(r.problem.as_ref())),
        ]);
        self.send(&wire::ok_reply(id, [("result", result)]));
    }

    fn refuse(&mut self, id: i64, p: &Problem) {
        self.send(&wire::refusal(id, p));
    }

    fn send(&mut self, value: &Value) {
        let line = wire::message_line(value);
        let written = self.output.write_all(line.as_bytes()).and_then(|()| self.output.flush());
        if written.is_err() {
            self.broken = true;
        }
    }
}

/// The bounds of a borrowed, index-free snapshot that the host's message names.
/// Older hosts omit them and keep the defaults.
fn bounds_of(wire_snapshot: &Object) -> Result<SnapshotOptions, Problem> {
    let number = |key| wire::int_field(wire_snapshot, key).unwrap_or(0);
    let (files, file_bytes, bytes) =
        (number("max_files"), number("max_file_bytes"), number("max_bytes"));
    if files < 0 || file_bytes < 0 || bytes < 0 {
        return Err(problem::protocol("Snapshot bounds cannot be negative."));
    }
    Ok(SnapshotOptions {
        mode: SnapshotMode::Borrow,
        temp_dir: None,
        max_files: usize::try_from(files).unwrap_or(usize::MAX),
        max_file_bytes: file_bytes as u64,
        max_bytes: bytes as u64,
        no_index: true,
    })
}

fn plan_json(plan: &Plan) -> Value {
    let changes = plan
        .changes
        .iter()
        .map(|c| object([("path", c.path.as_str().into()), ("kind", c.kind.name().into())]));
    object([
        ("id", plan.id().into()),
        ("generation", plan.generation_hex().into()),
        ("target", plan.target.as_str().into()),
        ("baseline", plan.baseline.as_str().into()),
        ("summary", plan.summary.as_str().into()),
        ("unchanged", plan.unchanged.into()),
        ("changes", Value::Array(changes.collect())),
    ])
}

fn foreign_plan(name: &str) -> Problem {
    Problem::new(
        "plugins.plan",
        "The plan is not this publisher's",
        "The host asked to publish a plan this publisher did not make.",
    )
    .hint("Make a new plan, then publish it.")
    .path(name)
    .exit(2)
}

fn not_loaded() -> Problem {
    Problem::new("plugins.unmade", "This was not made", "The session is not loaded.")
        .hint("Load the publisher before planning or publishing.")
        .path("session")
        .exit(2)
}
