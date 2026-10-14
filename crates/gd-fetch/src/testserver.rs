//! A small HTTP/1.1 server on the loopback interface, for tests of code that sends HTTP
//! requests (this crate, and the clients built on it).
//!
//! It answers one request per connection, one connection at a time, with what a handler
//! returns. It is a test fixture: it speaks enough HTTP for curl, and nothing more.
//!
//! ```no_run
//! use gd_fetch::testserver::{self, Reply};
//! let server = testserver::start(|_| Reply { status: 200, body: "hello".into(),
//!     ..Reply::default() }).expect("a loopback port");
//! let url = server.url("/path"); // http://127.0.0.1:PORT/path
//! ```

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};

/// The largest request the server reads; a larger one is answered with 413.
pub const MAX_REQUEST: usize = 64 * 1024 * 1024;

/// One request the server received.
#[derive(Debug, Clone, Default)]
pub struct Request {
    /// The method, such as `POST`.
    pub method: String,
    /// The request target, such as `/echo?x=1`.
    pub path: String,
    /// The headers, names in lowercase.
    pub headers: HashMap<String, String>,
    /// The body (as long as `Content-Length` says).
    pub body: Vec<u8>,
}

/// A handler's answer.
#[derive(Debug, Clone, Default)]
pub struct Reply {
    /// The status code.
    pub status: u16,
    /// Whole header lines, such as `"Location: /elsewhere"`.
    pub headers: Vec<String>,
    /// The body.
    pub body: Vec<u8>,
    /// Close the connection without answering, as a lost answer.
    pub close: bool,
}

/// A running server; dropping it (or [`Server::stop`]) ends it and waits for its thread.
#[derive(Debug)]
pub struct Server {
    port: u16,
    stopping: Arc<AtomicBool>,
    served: Arc<AtomicUsize>,
    worker: Option<JoinHandle<()>>,
}

/// Listens on a free loopback port and serves requests with `handler`, called on the
/// server's thread. Fails when no port could be opened.
pub fn start<F>(handler: F) -> io::Result<Server>
where
    F: Fn(&Request) -> Reply + Send + 'static,
{
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let port = listener.local_addr()?.port();
    let stopping = Arc::new(AtomicBool::new(false));
    let served = Arc::new(AtomicUsize::new(0));
    let (stop, count) = (stopping.clone(), served.clone());
    let worker = thread::spawn(move || {
        for client in listener.incoming() {
            if stop.load(Ordering::Acquire) {
                break;
            }
            let Ok(mut client) = client else { continue };
            if let Some(request) = receive(&mut client) {
                let reply = handler(&request);
                count.fetch_add(1, Ordering::AcqRel);
                if !reply.close {
                    respond(&mut client, &reply);
                }
            }
        }
    });
    Ok(Server { port, stopping, served, worker: Some(worker) })
}

impl Server {
    /// The server's URL for `path`, such as `http://127.0.0.1:PORT/path`.
    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    /// The number of requests answered so far.
    pub fn served_count(&self) -> usize {
        self.served.load(Ordering::Acquire)
    }

    /// Ends the server and waits for its thread.
    pub fn stop(self) {}
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        // Wake the accept with a connection of our own.
        let _ = TcpStream::connect(SocketAddr::from((Ipv4Addr::LOCALHOST, self.port)));
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn receive(client: &mut TcpStream) -> Option<Request> {
    let mut data = Vec::new();
    let mut chunk = vec![0u8; 16 * 1024];
    let (mut head_end, mut length) = (None, 0);
    loop {
        let n = client.read(&mut chunk).ok().filter(|&n| n > 0)?;
        data.extend_from_slice(&chunk[..n]);
        if head_end.is_none() {
            head_end = data.windows(4).position(|w| w == b"\r\n\r\n");
            if let Some(end) = head_end {
                length = content_length(&String::from_utf8_lossy(&data[..end]));
            }
        }
        if let Some(end) = head_end.filter(|&end| data.len() >= end + 4 + length) {
            let mut r = parse(&String::from_utf8_lossy(&data[..end]));
            r.body = data[end + 4..end + 4 + length].to_vec();
            return Some(r);
        }
        if data.len() > MAX_REQUEST {
            respond(client, &Reply { status: 413, ..Reply::default() });
            return None;
        }
    }
}

fn content_length(head: &str) -> usize {
    for line in head.lines() {
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            return value.trim().parse().ok().filter(|&n| n <= MAX_REQUEST).unwrap_or(0);
        }
    }
    0
}

fn parse(head: &str) -> Request {
    let mut r = Request::default();
    let mut lines = head.lines();
    if let Some(first) = lines.next() {
        let mut parts = first.split(' ');
        r.method = parts.next().unwrap_or("").to_owned();
        r.path = parts.next().unwrap_or("").to_owned();
    }
    for line in lines {
        let (name, value) = line.split_once(':').unwrap_or((line, ""));
        r.headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
    }
    r
}

fn respond(client: &mut TcpStream, reply: &Reply) {
    let mut out = format!(
        "HTTP/1.1 {} Status\r\nContent-Length: {}\r\nConnection: close\r\n",
        reply.status,
        reply.body.len()
    );
    for h in &reply.headers {
        out.push_str(h);
        out.push_str("\r\n");
    }
    out.push_str("\r\n");
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(&reply.body);
    let _ = client.write_all(&bytes);
}
