use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use super::*;
use crate::testserver::{self, Reply};

/// A fresh directory under the system's temporary directory, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("gd-fetch-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a temporary directory");
        TempDir(dir)
    }

    fn file(&self, name: &str, data: &[u8]) -> String {
        let path = self.0.join(name);
        std::fs::write(&path, data).expect("a temporary file");
        path.to_string_lossy().into_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn kind_of<T: std::fmt::Debug>(r: Result<T, Error>) -> ErrorKind {
    r.expect_err("a failure").kind
}

#[test]
fn files_and_limits() {
    let dir = TempDir::new();
    let path = dir.file("objects.inv", b"hello");
    let options = Options::default();
    let r = get(&path, &options).expect("the file");
    assert_eq!((r.data.as_slice(), r.url.as_str(), r.status), (&b"hello"[..], &*path, None));
    assert!(get(&format!("file://{path}"), &options).is_ok());
    let small = Options { max_bytes: 3, ..Options::default() };
    assert_eq!(kind_of(get(&path, &small)), ErrorKind::TooLarge);
    let none = dir.0.join("none").to_string_lossy().into_owned();
    assert_eq!(kind_of(get(&none, &options)), ErrorKind::Missing);
    assert_eq!(kind_of(get("ftp://example.com/x", &options)), ErrorKind::Unsupported);
}

fn answer(r: &testserver::Request) -> Reply {
    let text = |status: u16, body: String| Reply { status, body: body.into(), ..Reply::default() };
    match r.path.as_str() {
        "/echo" => {
            let auth = r.headers.get("authorization").map_or("", String::as_str);
            let body = format!("{} {auth} {}", r.method, String::from_utf8_lossy(&r.body));
            Reply {
                headers: vec!["X-Kind: echo".into(), "Retry-After: 7".into()],
                ..text(201, body)
            }
        }
        "/moved" => {
            let headers = vec!["Location: http://elsewhere.test/".into()];
            Reply { headers, ..text(302, "".into()) }
        }
        "/hop" => Reply { headers: vec!["Location: /plain".into()], ..text(301, "".into()) },
        "/plain" => text(200, "plain".into()),
        "/missing" => text(404, r#"{"message": "Not Found"}"#.into()),
        "/big" => text(200, "x".repeat(4096)),
        "/agent" => text(200, r.headers.get("user-agent").cloned().unwrap_or_default()),
        "/slow" => {
            std::thread::sleep(Duration::from_millis(1500));
            text(200, "late".into())
        }
        _ => text(500, String::new()),
    }
}

/// send sends methods, headers and bodies (also from a file), returns statuses other than
/// 2xx with their bodies, never follows a redirect, and refuses a header that would split.
#[test]
fn requests_are_sent_as_given() {
    let s = testserver::start(answer).expect("a loopback port");
    let o = Options::default();
    let auth = vec![Header::new("Authorization", "Bearer secret")];
    let post = Request {
        method: Method::Post,
        url: s.url("/echo"),
        headers: auth.clone(),
        body: Body::Bytes(b"hello".to_vec()),
    };
    let r = send(&post, &o);
    assert!(r.is_ok(), "{:?}", r.error);
    assert_eq!(r.status, Some(201));
    assert_eq!(r.body, b"POST Bearer secret hello");
    assert_eq!(r.header("retry-after"), Some("7"));
    let dir = TempDir::new();
    let path = dir.file("body", &vec![b'y'; 300_000]);
    let put = Request {
        method: Method::Put,
        url: s.url("/echo"),
        body: Body::File(path.into()),
        ..Request::default()
    };
    assert_eq!(send(&put, &o).body.len(), "PUT  ".len() + 300_000);
    let moved = Request { url: s.url("/moved"), headers: auth, ..Request::default() };
    let r = send(&moved, &o);
    assert_eq!(r.error.as_ref().map(|e| e.kind), Some(ErrorKind::Redirect));
    assert_eq!(r.header("location"), Some("http://elsewhere.test/"));
    let r = send(&Request { url: s.url("/missing"), ..Request::default() }, &o);
    assert_eq!(r.error.as_ref().map(|e| (e.kind, e.status)), Some((ErrorKind::Status, Some(404))));
    assert_eq!(r.body, br#"{"message": "Not Found"}"#);
    let small = Options { max_bytes: 100, ..Options::default() };
    let r = send(&Request { url: s.url("/big"), ..Request::default() }, &small);
    assert_eq!(r.error.map(|e| e.kind), Some(ErrorKind::TooLarge));
    let bad = vec![Header::new("X-Injected", "a\r\nAuthorization: stolen")];
    let r = send(&Request { url: s.url("/echo"), headers: bad, ..Request::default() }, &o);
    assert_eq!(r.error.map(|e| e.kind), Some(ErrorKind::Unsupported));
    let r = send(&Request { url: "ftp://example.com/".into(), ..Request::default() }, &o);
    assert_eq!(r.error.map(|e| e.kind), Some(ErrorKind::Unsupported));
    let bodied = Request { url: s.url("/echo"), body: Body::Bytes(vec![1]), ..Request::default() };
    assert_eq!(send(&bodied, &o).error.map(|e| e.kind), Some(ErrorKind::Unsupported));
    assert_eq!(s.served_count(), 5);
}

#[test]
fn gets_follow_redirects_and_report_statuses() {
    let s = testserver::start(answer).expect("a loopback port");
    let o = Options { user_agent: "Guidedog test".into(), ..Options::default() };
    let r = get(&s.url("/hop"), &o).expect("the redirected body");
    assert_eq!((r.data.as_slice(), r.status), (&b"plain"[..], Some(200)));
    assert_eq!(r.url, s.url("/plain"));
    assert_eq!(get(&s.url("/agent"), &o).map(|r| r.data), Ok(b"Guidedog test".to_vec()));
    let e = get(&s.url("/missing"), &o).expect_err("a 404");
    assert_eq!((e.kind, e.status), (ErrorKind::Status, Some(404)));
    let small = Options { max_bytes: 100, ..Options::default() };
    assert_eq!(kind_of(get(&s.url("/big"), &small)), ErrorKind::TooLarge);
}

#[test]
fn timeouts_and_cancellation_stop_a_transfer() {
    let s = testserver::start(answer).expect("a loopback port");
    let quick = Options { timeout: Some(Duration::from_millis(200)), ..Options::default() };
    assert_eq!(kind_of(get(&s.url("/slow"), &quick)), ErrorKind::Timeout);
    let cancel = Arc::new(AtomicBool::new(false));
    let o = Options { cancel: Some(cancel.clone()), ..Options::default() };
    let flag = cancel.clone();
    let setter = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        flag.store(true, Ordering::Release);
    });
    assert_eq!(kind_of(get(&s.url("/slow"), &o)), ErrorKind::Cancelled);
    let _ = setter.join();
}

#[test]
fn network_failures_carry_the_clients_message() {
    // Port 1 on the loopback interface refuses connections.
    let e = get("http://127.0.0.1:1/", &Options::default()).expect_err("refused");
    assert_eq!(e.kind, ErrorKind::Network);
    assert!(e.detail.contains("Connection"), "{}", e.detail);
    assert!(e.to_string().contains("Hint: Check the network"), "{e}");
}

#[test]
fn tokens_and_lines_are_checked() {
    assert!(is_token("X-GitHub-Api-Version") && !is_token("") && !is_token("a b"));
    assert!(is_one_line("value") && !is_one_line("a\nb") && !is_one_line("a\0"));
    let report = Error { status: Some(404), ..Error::new(ErrorKind::Status, "https://x/y", "") };
    let text = report.to_string();
    assert!(text.contains("HTTP status 404") && text.contains("Hint: Check the URL"), "{text}");
}

/// A real HTTPS read: rustls with the built-in root certificates. It needs the network,
/// so it runs only on request: `cargo test -p gd-fetch -- --ignored`.
#[test]
#[ignore = "reaches docs.python.org over the network"]
fn https_reads_a_real_inventory() {
    let r =
        get("https://docs.python.org/3/objects.inv", &Options::default()).expect("the inventory");
    assert_eq!(r.status, Some(200));
    assert!(r.data.starts_with(b"# Sphinx inventory version 2"), "{:?}", &r.data[..32]);
    println!("{} bytes from {}", r.data.len(), r.url);
}
