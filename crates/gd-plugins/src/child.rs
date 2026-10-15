//! A plugin program running in protocol mode: framed requests and responses over its
//! standard streams, each whole exchange bounded by a deadline and a cancel probe.
//!
//! Two helper threads own the pipes: a writer that sends request lines and a reader
//! that frames response lines. The caller waits on their channels with a monotonic
//! deadline, so a program can neither block a write nor keep a request alive by
//! sending a byte now and then. Stopping a program that does not end kills it.

use std::io::{BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use gd_json::{Object, Value};

use crate::problem::{self, Problem};
use crate::wire::{self, Handshake};
use crate::{Cancel, MAX_REQUEST, MAX_RESPONSE, PROTOCOL_FLAG};

/// How often a waiting exchange polls the cancel probe.
const POLL: Duration = Duration::from_millis(5);

/// Which protocol a child speaks; it decides limits and how failures are explained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChildKind {
    Build,
    Publisher,
}

/// One running plugin program.
#[derive(Debug)]
pub(crate) struct Child {
    process: std::process::Child,
    requests: Option<Sender<Vec<u8>>>,
    written: Receiver<Result<(), Problem>>,
    lines: Receiver<Result<String, Problem>>,
    /// The ID of the last request sent.
    pub(crate) next: i64,
    /// The program stopped or broke the protocol: send it nothing more.
    pub(crate) broken: bool,
    kind: ChildKind,
    /// Bounds each whole exchange.
    pub(crate) timeout: Duration,
    /// Stops an exchange early.
    pub(crate) cancel: Cancel,
    deadline: Instant,
    stopped: bool,
}

impl Child {
    /// Starts `executable` with [`PROTOCOL_FLAG`]; its standard error stays the host's.
    pub(crate) fn start(
        executable: &str,
        kind: ChildKind,
        timeout: Duration,
        cancel: Cancel,
    ) -> Result<Child, Problem> {
        let mut process = Command::new(executable)
            .arg(PROTOCOL_FLAG)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| {
                Problem::new("plugins.start", "The plugin program cannot start", &e.to_string())
                    .hint(
                        "Check the registered path with guidedog plugin inspect, then \
                         register the program again if it moved.",
                    )
                    .path(executable)
                    .exit(4)
            })?;
        let (Some(stdin), Some(stdout)) = (process.stdin.take(), process.stdout.take()) else {
            let _ = process.kill();
            let _ = process.wait();
            return Err(problem::protocol("The plugin's standard streams cannot be opened."));
        };
        let (requests, written) = spawn_writer(stdin);
        let lines = spawn_reader(stdout);
        Ok(Child {
            process,
            requests: Some(requests),
            written,
            lines,
            next: 0,
            broken: false,
            kind,
            timeout,
            cancel,
            deadline: Instant::now(),
            stopped: false,
        })
    }

    /// Reads and checks the program's handshake, waiting at most `bound` (and the
    /// request timeout).
    pub(crate) fn handshake(&mut self, bound: Duration) -> Result<Handshake, Problem> {
        self.deadline = Instant::now() + self.timeout.min(bound);
        let line = self.receive_line()?;
        wire::parse_handshake(&line).inspect_err(|_| self.broken = true)
    }

    /// Sends `request` (an object whose `id` is `self.next + 1`) and returns the
    /// response with that ID. A refusal with a problem returns that problem and leaves
    /// the program usable; any other failure marks it broken.
    pub(crate) fn exchange(&mut self, request: Value) -> Result<Object, Problem> {
        if self.broken {
            return Err(problem::protocol("The plugin program stopped earlier."));
        }
        let id = self.next;
        self.deadline = Instant::now() + self.timeout;
        let line = wire::message_line(&request);
        let limit = if self.kind == ChildKind::Build { MAX_RESPONSE } else { MAX_REQUEST };
        if line.len() - 1 > limit {
            return Err(problem::protocol("A request is too long."));
        }
        let response = self
            .send(line.into_bytes())
            .and_then(|()| self.receive_line())
            .and_then(|line| wire::parse_message(&line));
        let o = response.inspect_err(|_| self.broken = true)?;
        match check_response(o, id) {
            Ok(o) => Ok(o),
            Err(Refused::Problem(p)) => Err(p),
            Err(Refused::Broken(p)) => {
                self.broken = true;
                Err(p)
            }
        }
    }

    /// The next request's ID, also recorded as the last one sent.
    pub(crate) fn next_id(&mut self) -> i64 {
        self.next += 1;
        self.next
    }

    fn send(&mut self, bytes: Vec<u8>) -> Result<(), Problem> {
        let Some(requests) = &self.requests else {
            return Err(problem::protocol("The plugin program stopped earlier."));
        };
        if requests.send(bytes).is_err() {
            self.broken = true;
            return Err(stopped_reading());
        }
        let result = self.wait(|c, slice| c.written.recv_timeout(slice));
        result.inspect_err(|_| self.broken = true)
    }

    fn receive_line(&mut self) -> Result<String, Problem> {
        let result = self.wait(|c, slice| c.lines.recv_timeout(slice));
        result.inspect_err(|_| self.broken = true)
    }

    /// Waits for a helper thread's message, polling the cancel probe and the deadline.
    fn wait<T>(
        &mut self,
        recv: impl Fn(&Child, Duration) -> Result<Result<T, Problem>, RecvTimeoutError>,
    ) -> Result<T, Problem> {
        loop {
            self.check()?;
            let left = self.deadline.saturating_duration_since(Instant::now());
            match recv(self, left.min(POLL)) {
                Ok(message) => return message,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Err(wire::closed()),
            }
        }
    }

    fn check(&self) -> Result<(), Problem> {
        if self.cancel.is_cancelled() {
            return Err(self.cancelled());
        }
        if Instant::now() >= self.deadline {
            return Err(self.timed_out());
        }
        Ok(())
    }

    fn cancelled(&self) -> Problem {
        match self.kind {
            ChildKind::Build => Problem::new(
                "plugins.cancelled",
                "The build plugin was cancelled",
                "The build stopped before replacing its output.",
            )
            .hint("Run guidedog build again when ready.")
            .exit(130),
            ChildKind::Publisher => Problem::new(
                "plugins.cancelled",
                "The publisher request was cancelled",
                "The program was stopped. A write it already sent cannot be undone.",
            )
            .hint("Run the publisher's recover command before publishing again.")
            .exit(130),
        }
    }

    fn timed_out(&self) -> Problem {
        match self.kind {
            ChildKind::Build => Problem::new(
                "plugins.timeout",
                "The build plugin did not answer in time",
                "Its complete request exceeded the deadline; the previous output remains \
                 installed.",
            )
            .hint(
                "Fix the plugin's blocking work; if it needs more time, use -D \
                 plugin_timeout=120.",
            )
            .exit(4),
            ChildKind::Publisher => Problem::new(
                "plugins.timeout",
                "The publisher did not answer in time",
                "The whole request exceeded its time limit, so the program was stopped. A \
                 write it already sent may have reached the target.",
            )
            .hint(
                "Run the publisher's recover command before publishing again. If it is \
                 healthy but needs longer, use guidedog publish --timeout=600.",
            )
            .exit(4),
        }
    }

    /// Closes the program's input, which ends it, and waits for it a while; a broken
    /// program, or one that does not end, is killed. Stopping twice is harmless.
    pub(crate) fn stop(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        self.requests = None;
        if self.broken {
            let _ = self.process.kill();
        }
        if !wait_for_exit(&mut self.process, Duration::from_secs(1)) {
            let _ = self.process.kill();
            wait_for_exit(&mut self.process, Duration::from_secs(2));
        }
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        self.stop();
    }
}

enum Refused {
    /// The program refused the request with a problem; it remains usable.
    Problem(Problem),
    /// The program broke the protocol or stopped.
    Broken(Problem),
}

impl From<Problem> for Refused {
    fn from(p: Problem) -> Refused {
        Refused::Broken(p)
    }
}

fn check_response(o: Object, id: i64) -> Result<Object, Refused> {
    if wire::int_field(&o, "id") != Some(id) {
        return Err(problem::protocol("A response answers another request.").into());
    }
    let Some(ok) = wire::bool_field(&o, "ok") else {
        return Err(problem::protocol("A response has no boolean ok field.").into());
    };
    if ok {
        return Ok(o);
    }
    match wire::obj_field(&o, "problem").and_then(wire::problem_of) {
        Some(p) => Err(Refused::Problem(p)),
        None => Err(problem::protocol("A refused request has no problem.").into()),
    }
}

fn stopped_reading() -> Problem {
    problem::protocol("The plugin stopped reading its input.")
}

fn spawn_writer(
    mut stdin: std::process::ChildStdin,
) -> (Sender<Vec<u8>>, Receiver<Result<(), Problem>>) {
    let (requests, incoming) = mpsc::channel::<Vec<u8>>();
    let (acks, written) = mpsc::channel();
    std::thread::spawn(move || {
        for bytes in incoming {
            let result = stdin.write_all(&bytes).and_then(|()| stdin.flush());
            let failed = result.is_err();
            if acks.send(result.map_err(|_| stopped_reading())).is_err() || failed {
                break;
            }
        }
    });
    (requests, written)
}

fn spawn_reader(stdout: std::process::ChildStdout) -> Receiver<Result<String, Problem>> {
    let (lines, received) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::with_capacity(16 * 1024, stdout);
        loop {
            let line = wire::read_line(&mut reader, MAX_RESPONSE);
            let failed = line.is_err();
            if lines.send(line).is_err() || failed {
                break;
            }
        }
    });
    received
}

/// Waits up to `limit` for `process` to end; whether it did.
fn wait_for_exit(process: &mut std::process::Child, limit: Duration) -> bool {
    let until = Instant::now() + limit;
    loop {
        match process.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(5)),
            _ => return false,
        }
    }
}
