//! The preview server: a listener polled on the caller's thread, which also owns the
//! rebuild, and a small fixed pool of worker threads that answer connections.
//!
//! Workers ask the caller's thread to refresh before they serve an HTML page; pending
//! asks are answered together after one check, so the rebuild never runs twice at once
//! and need not be `Send`.

use std::io::{self, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use gd_host::Problem;

use crate::http::{self, Method, Refusal, Target};
use crate::watch::Watch;

/// How long the listener sleeps when there is nothing to accept or refresh.
const POLL_INTERVAL: Duration = Duration::from_millis(25);

/// How a preview server listens and answers. Fill with struct-update syntax:
/// `ServeOptions { site_dir: "_build/html".into(), port: 8080, ..Default::default() }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServeOptions {
    /// The built site to serve.
    pub site_dir: PathBuf,
    /// The address to listen on; loopback by default, so the preview stays private.
    pub address: IpAddr,
    /// The port; 0 lets the system choose (see [`Server::address`]).
    pub port: u16,
    /// Worker threads answering connections (at least one is used).
    pub workers: usize,
    /// The largest request head accepted; larger heads get status 431.
    pub max_request_bytes: usize,
    /// How long a client may take to send its request or receive the answer.
    pub timeout: Duration,
    /// Stop when Ctrl+C sets `gd_host::was_interrupted` (true by default).
    pub stop_on_interrupt: bool,
}

impl Default for ServeOptions {
    fn default() -> ServeOptions {
        ServeOptions {
            site_dir: PathBuf::from("_build/html"),
            address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 8000,
            workers: 4,
            max_request_bytes: 8192,
            timeout: Duration::from_secs(2),
            stop_on_interrupt: true,
        }
    }
}

/// Stops a running [`Server`] from another thread; cloneable.
#[derive(Debug, Clone, Default)]
pub struct StopHandle(Arc<AtomicBool>);

impl StopHandle {
    /// Asks the server to stop; `Server::run` returns within a poll interval or after
    /// the rebuild in progress.
    pub fn stop(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether `stop` was called.
    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// A bound preview server. [`Server::bind`] claims the port, [`Server::run`] answers
/// requests until stopped. Dropping a server closes its port.
#[derive(Debug)]
pub struct Server {
    listener: TcpListener,
    options: ServeOptions,
    stop: StopHandle,
}

/// What the workers share: read-only settings.
struct Site<'a> {
    dir: &'a Path,
    max_request_bytes: usize,
    timeout: Duration,
}

/// A worker's request to refresh, answered on the given channel when done.
type RefreshAsk = SyncSender<()>;

/// The caller's rebuild with the stamp of the sources it last built.
struct Refresher<F> {
    watch: Watch,
    stamp: u64,
    rebuild: F,
}

impl<F: FnMut(&mut Watch) -> bool> Refresher<F> {
    /// Rebuilds when the sources changed. After a successful rebuild the stamp is
    /// taken again, since the rebuild may have changed the watched roots; after a
    /// failed one the changed stamp is kept, so the same failure is not rebuilt for
    /// every page until the sources change again.
    fn refresh(&mut self) {
        let now = self.watch.stamp();
        if now == self.stamp {
            return;
        }
        self.stamp = now;
        if (self.rebuild)(&mut self.watch) {
            self.stamp = self.watch.stamp();
        }
    }
}

impl Server {
    /// Listens on `options.address:options.port`. Fails with `serve.listen` (exit 4)
    /// when the port is in use or not permitted. The site folder need not exist yet.
    pub fn bind(options: ServeOptions) -> Result<Server, Problem> {
        let listener = TcpListener::bind((options.address, options.port))
            .and_then(|l| l.set_nonblocking(true).map(|()| l))
            .map_err(|e| listen_problem(&options, &e))?;
        Ok(Server { listener, options, stop: StopHandle::default() })
    }

    /// The address the server listens on, with the port the system chose for port 0.
    pub fn address(&self) -> SocketAddr {
        self.listener
            .local_addr()
            .unwrap_or_else(|_| SocketAddr::new(self.options.address, self.options.port))
    }

    /// The preview's address as a URL, such as `http://127.0.0.1:8000/`.
    pub fn url(&self) -> String {
        format!("http://{}/", self.address())
    }

    /// A handle that stops [`Server::run`] from another thread.
    pub fn stop_handle(&self) -> StopHandle {
        self.stop.clone()
    }

    /// Serves the site until stopped (by the handle or Ctrl+C), refreshing before each
    /// HTML page: when `watch` stamps differently than at the last build, `rebuild`
    /// runs on this thread with the watch, which it may change (a reloaded
    /// configuration may watch other roots); it returns false when the build failed.
    /// Prints nothing. Returns `Ok` when stopped; errors only for a broken listener.
    pub fn run(
        self,
        watch: Watch,
        rebuild: impl FnMut(&mut Watch) -> bool,
    ) -> Result<(), Problem> {
        let stamp = watch.stamp();
        let mut refresher = Refresher { watch, stamp, rebuild };
        let site = Site {
            dir: &self.options.site_dir,
            max_request_bytes: self.options.max_request_bytes,
            timeout: self.options.timeout,
        };
        let (connections, queue) = mpsc::channel::<TcpStream>();
        let queue = Mutex::new(queue);
        let (ask, asks) = mpsc::channel::<RefreshAsk>();
        thread::scope(|scope| {
            for _ in 0..self.options.workers.max(1) {
                let ask = ask.clone();
                let (site, queue) = (&site, &queue);
                scope.spawn(move || work(site, queue, &ask));
            }
            drop(ask);
            let result = self.accept_loop(&connections, &asks, &mut refresher);
            drop(connections);
            // Release workers still waiting for a refresh; they end when the queue does.
            while let Ok(reply) = asks.recv() {
                let _ = reply.send(());
            }
            result
        })
    }

    fn is_stopping(&self) -> bool {
        self.stop.is_stopped() || (self.options.stop_on_interrupt && gd_host::was_interrupted())
    }

    fn accept_loop<F: FnMut(&mut Watch) -> bool>(
        &self,
        connections: &Sender<TcpStream>,
        asks: &Receiver<RefreshAsk>,
        refresher: &mut Refresher<F>,
    ) -> Result<(), Problem> {
        while !self.is_stopping() {
            let answered = answer_refreshes(asks, refresher);
            match self.listener.accept() {
                Ok((stream, _)) => {
                    // Accepted sockets must block; the timeouts bound each worker.
                    if stream.set_nonblocking(false).is_ok() && connections.send(stream).is_err() {
                        return Ok(());
                    }
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    if !answered {
                        thread::sleep(POLL_INTERVAL);
                    }
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                // Running out of descriptors and similar passing failures: wait a little.
                Err(_) => thread::sleep(POLL_INTERVAL),
            }
        }
        Ok(())
    }
}

/// Answers every pending refresh ask after one check; false when none was pending.
fn answer_refreshes<F: FnMut(&mut Watch) -> bool>(
    asks: &Receiver<RefreshAsk>,
    refresher: &mut Refresher<F>,
) -> bool {
    let pending: Vec<RefreshAsk> = asks.try_iter().collect();
    if pending.is_empty() {
        return false;
    }
    refresher.refresh();
    for reply in pending {
        let _ = reply.send(());
    }
    true
}

fn listen_problem(options: &ServeOptions, error: &io::Error) -> Problem {
    Problem::new(
        "serve.listen",
        "CANNOT START THE PREVIEW SERVER",
        &format!("{}:{}", options.address, options.port),
        format!("The preview cannot listen on this port: {error}."),
        "The port is in use or not permitted. Choose another port with --port.",
        4,
    )
}

/// A worker: answers connections from the queue until it closes.
fn work(site: &Site<'_>, queue: &Mutex<Receiver<TcpStream>>, ask: &Sender<RefreshAsk>) {
    let mut head = Vec::with_capacity(site.max_request_bytes.min(8192));
    loop {
        let next = queue.lock().unwrap_or_else(PoisonError::into_inner).recv();
        let Ok(mut stream) = next else { return };
        // A client that disconnects or stalls only loses its own answer.
        let _ = answer(site, &mut stream, ask, &mut head);
    }
}

fn answer(
    site: &Site<'_>,
    stream: &mut TcpStream,
    ask: &Sender<RefreshAsk>,
    head: &mut Vec<u8>,
) -> io::Result<()> {
    stream.set_read_timeout(Some(site.timeout))?;
    stream.set_write_timeout(Some(site.timeout))?;
    let request = match http::read_head(stream, site.max_request_bytes, head)? {
        Ok(()) => http::parse_request(head),
        Err(refusal) => Err(refusal),
    };
    let request = match request {
        Ok(request) => request,
        Err(refusal) => return refuse(stream, refusal),
    };
    if http::relative_path(&request.path)
        .is_ok_and(|p| http::content_type(&p).starts_with("text/html"))
    {
        wait_for_refresh(ask);
    }
    serve_target(site.dir, stream, &request)
}

fn refuse(stream: &mut TcpStream, refusal: Refusal) -> io::Result<()> {
    let (status, body) = refusal.status_and_body();
    let extra = if refusal == Refusal::Method { "Allow: GET, HEAD\r\n" } else { "" };
    let kind = "text/plain; charset=utf-8";
    http::send_text(stream, Method::Get, status, kind, body, extra)
}

/// Asks the listener thread to refresh and waits until it has.
fn wait_for_refresh(ask: &Sender<RefreshAsk>) {
    let (reply, done) = mpsc::sync_channel(1);
    if ask.send(reply).is_ok() {
        let _ = done.recv();
    }
}

fn serve_target(dir: &Path, stream: &mut TcpStream, request: &http::Request) -> io::Result<()> {
    let target = match dir.canonicalize() {
        Ok(root) => http::resolve(&root, &request.path),
        Err(_) => Ok(Target::Missing),
    };
    match target {
        Ok(Target::File(path)) => match http::send_file(stream, request.method, &path) {
            Err(e) if e.kind() == ErrorKind::NotFound => not_found(stream, request.method),
            other => other,
        },
        Ok(Target::Directory) => {
            let location = format!("Location: {}/\r\n", request.raw_path);
            let kind = "text/plain; charset=utf-8";
            let body = "This folder's page is at its address with a trailing slash.\n";
            http::send_text(stream, request.method, "301 Moved Permanently", kind, body, &location)
        }
        Ok(Target::Missing) => not_found(stream, request.method),
        Err(refusal) => refuse(stream, refusal),
    }
}

fn not_found(stream: &mut TcpStream, method: Method) -> io::Result<()> {
    let kind = "text/html; charset=utf-8";
    http::send_text(stream, method, "404 Not Found", kind, http::NOT_FOUND_PAGE, "")
}
