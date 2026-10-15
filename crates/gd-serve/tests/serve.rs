//! Real TCP requests against a preview server on an ephemeral port.

use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, SystemTime};

use gd_serve::{ServeOptions, Server, StopHandle, Watch};

/// A temporary folder removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let nanos =
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let path =
            std::env::temp_dir().join(format!("gd-serve-{name}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp dir");
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn put(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, text).expect("write fixture");
}

/// A server running on its own thread, stopped and joined on drop.
struct Running {
    address: SocketAddr,
    stop: StopHandle,
    thread: Option<thread::JoinHandle<()>>,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop.stop();
        if let Some(thread) = self.thread.take() {
            thread.join().expect("server thread");
        }
    }
}

fn start(site: &Path, watch: Watch, rebuilds: &'static AtomicUsize) -> Running {
    let options = ServeOptions {
        site_dir: site.to_path_buf(),
        port: 0,
        max_request_bytes: 1024,
        stop_on_interrupt: false,
        ..Default::default()
    };
    let server = Server::bind(options).expect("bind");
    let address = server.address();
    let stop = server.stop_handle();
    let thread = thread::spawn(move || {
        server
            .run(watch, |_| {
                rebuilds.fetch_add(1, Ordering::SeqCst);
                true
            })
            .expect("run");
    });
    Running { address, stop, thread: Some(thread) }
}

fn exchange(address: SocketAddr, request: &[u8]) -> String {
    let mut stream = TcpStream::connect(address).expect("connect");
    stream.set_read_timeout(Some(Duration::from_secs(5))).expect("timeout");
    stream.write_all(request).expect("send");
    let mut answer = Vec::new();
    let _ = stream.read_to_end(&mut answer);
    String::from_utf8_lossy(&answer).into_owned()
}

fn get(address: SocketAddr, target: &str) -> String {
    exchange(address, format!("GET {target} HTTP/1.1\r\nHost: preview\r\n\r\n").as_bytes())
}

fn site() -> TempDir {
    let temp = TempDir::new("site");
    put(&temp.0.join("html/index.html"), "<p>Home</p>");
    put(&temp.0.join("html/guide/index.html"), "<p>Guide</p>");
    put(&temp.0.join("html/_static/style.css"), "p {}");
    put(&temp.0.join("html/my page.html"), "<p>Spaced</p>");
    put(&temp.0.join("secret.txt"), "secret");
    temp
}

#[test]
fn serves_files_with_content_types() {
    static REBUILDS: AtomicUsize = AtomicUsize::new(0);
    let temp = site();
    let server = start(&temp.0.join("html"), Watch::default(), &REBUILDS);
    let home = get(server.address, "/");
    assert!(home.starts_with("HTTP/1.1 200 OK\r\n"), "{home}");
    assert!(home.contains("Content-Type: text/html; charset=utf-8\r\n"));
    assert!(home.contains("Content-Length: 11\r\n"));
    assert!(home.contains("Cache-Control: no-store\r\n"));
    assert!(home.ends_with("\r\n\r\n<p>Home</p>"));
    let css = get(server.address, "/_static/style.css?v=3#top");
    assert!(css.contains("Content-Type: text/css; charset=utf-8\r\n"), "{css}");
    assert!(css.ends_with("p {}"));
    assert!(get(server.address, "/guide/").ends_with("<p>Guide</p>"));
    assert!(get(server.address, "/my%20page.html").ends_with("<p>Spaced</p>"));
    let folder = get(server.address, "/guide?x=1");
    assert!(folder.starts_with("HTTP/1.1 301"), "{folder}");
    assert!(folder.contains("Location: /guide/\r\n"));
}

#[test]
fn head_sends_only_the_head() {
    static REBUILDS: AtomicUsize = AtomicUsize::new(0);
    let temp = site();
    let server = start(&temp.0.join("html"), Watch::default(), &REBUILDS);
    let head = exchange(server.address, b"HEAD /index.html HTTP/1.1\r\n\r\n");
    assert!(head.starts_with("HTTP/1.1 200 OK\r\n"), "{head}");
    assert!(head.contains("Content-Length: 11\r\n"));
    assert!(head.ends_with("\r\n\r\n"));
}

#[test]
fn refuses_unsafe_paths_methods_and_large_requests() {
    static REBUILDS: AtomicUsize = AtomicUsize::new(0);
    let temp = site();
    let server = start(&temp.0.join("html"), Watch::default(), &REBUILDS);
    for target in ["/../secret.txt", "/%2e%2e/secret.txt", "/a%5c..%5csecret.txt", "/a%00"] {
        let answer = get(server.address, target);
        assert!(answer.starts_with("HTTP/1.1 400"), "{target}: {answer}");
    }
    let missing = get(server.address, "/nope.html");
    assert!(missing.starts_with("HTTP/1.1 404"), "{missing}");
    assert!(missing.contains("This page does not exist."));
    assert!(get(server.address, "//etc/passwd").starts_with("HTTP/1.1 404"));
    assert!(get(server.address, "http://x/").starts_with("HTTP/1.1 400"));
    let post = exchange(server.address, b"POST / HTTP/1.1\r\n\r\n");
    assert!(post.starts_with("HTTP/1.1 405"), "{post}");
    assert!(post.contains("Allow: GET, HEAD\r\n"));
    let long = format!("GET / HTTP/1.1\r\nX: {}\r\n\r\n", "a".repeat(4000));
    let large = exchange(server.address, long.as_bytes());
    assert!(large.is_empty() || large.starts_with("HTTP/1.1 431"), "{large}");
}

#[cfg(unix)]
#[test]
fn symbolic_links_cannot_leave_the_site() {
    static REBUILDS: AtomicUsize = AtomicUsize::new(0);
    let temp = site();
    let link = temp.0.join("html/leak.txt");
    std::os::unix::fs::symlink(temp.0.join("secret.txt"), &link).expect("symlink");
    let server = start(&temp.0.join("html"), Watch::default(), &REBUILDS);
    assert!(get(server.address, "/leak.txt").starts_with("HTTP/1.1 404"));
}

#[test]
fn rebuilds_before_a_page_when_sources_changed() {
    static REBUILDS: AtomicUsize = AtomicUsize::new(0);
    let temp = site();
    let source = temp.0.join("src");
    put(&source.join("index.rst"), "Before.\n");
    let watch = Watch { roots: vec![source.clone()], skip: vec![] };
    let server = start(&temp.0.join("html"), watch, &REBUILDS);
    get(server.address, "/");
    assert_eq!(REBUILDS.load(Ordering::SeqCst), 0);
    put(&source.join("index.rst"), "After a longer edit.\n");
    get(server.address, "/_static/style.css");
    assert_eq!(REBUILDS.load(Ordering::SeqCst), 0, "assets do not rebuild");
    get(server.address, "/");
    assert_eq!(REBUILDS.load(Ordering::SeqCst), 1);
    get(server.address, "/guide/");
    assert_eq!(REBUILDS.load(Ordering::SeqCst), 1, "unchanged sources do not rebuild");
}

#[test]
fn a_used_port_is_a_problem() {
    let first = Server::bind(ServeOptions { port: 0, ..Default::default() }).expect("bind");
    let taken = ServeOptions { port: first.address().port(), ..Default::default() };
    let problem = Server::bind(taken).expect_err("port in use");
    assert_eq!(problem.code, "serve.listen");
    assert_eq!(problem.exit, 4);
    assert!(problem.hint.contains("--port"));
}
