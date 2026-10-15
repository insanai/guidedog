//! The connection to GitHub: the roots it reaches, the token it sends, and the problems
//! of a call that failed. Requests go through `gd-fetch`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gd_fetch::{ErrorKind, Header, Method, Request, Response};
use gd_json::Value;
use gd_plugins::{Cancel, Problem};

use crate::credential::{Secret, forget_string};
use crate::problem::problem;

/// GitHub's REST API.
pub const API_ROOT: &str = "https://api.github.com";
/// GitHub's web root: Git's smart HTTP and the device flow.
pub const WEB_ROOT: &str = "https://github.com";

const API_VERSION_HEADER: &str = "2022-11-28";
const DEFAULT_MAX_BYTES: usize = 4 * 1024 * 1024;

/// The roots publications reach. Only GitHub.com is supported (an Enterprise Server
/// needs its own qualification); tests point a client at a local server instead. The
/// token goes to these roots, so only code, never a configuration, chooses them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roots {
    /// The REST API root, such as [`API_ROOT`].
    pub api: String,
    /// The web root, such as [`WEB_ROOT`].
    pub web: String,
}

impl Default for Roots {
    fn default() -> Roots {
        Roots { api: API_ROOT.to_string(), web: WEB_ROOT.to_string() }
    }
}

impl Roots {
    /// Both roots at one address, as a test server serves them.
    pub fn at(base: &str) -> Roots {
        let base = base.trim_end_matches('/');
        Roots { api: base.to_string(), web: base.to_string() }
    }
}

/// A connection's settings: the token it sends (none for the device flow), the roots it
/// reaches, and the probe that cancels its transfers.
#[derive(Debug, Clone, Default)]
pub struct Client {
    token: Secret,
    /// Where requests go.
    pub roots: Roots,
    /// Stops transfers and waits when it reports cancellation.
    pub cancel: Cancel,
    /// A tenth of the second the device flow waits by; tests shorten it.
    pub poll_unit: Duration,
}

/// What a call to GitHub's API needs: the method, the path below the API root, a JSON
/// body, what it is for (in problems), the permission it needs (in hints), and the
/// largest answer it reads (0: 4 MiB).
#[derive(Debug, Clone, Default)]
pub(crate) struct Call<'a> {
    pub method: Method,
    pub path: &'a str,
    pub body: Option<String>,
    pub doing: &'a str,
    pub needs: &'a str,
    pub max_bytes: usize,
}

impl Client {
    /// A client that sends `token` (an empty secret sends none) to `roots`.
    pub fn new(token: Secret, roots: Roots, cancel: Cancel) -> Client {
        Client { token, roots, cancel, poll_unit: Duration::from_millis(100) }
    }

    /// The user agent requests name.
    pub(crate) fn agent() -> String {
        format!("guidedog-pages/{}", crate::VERSION)
    }

    /// Whether the client's transfers and waits should stop.
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// Makes a call to GitHub's REST API; returns the status and the JSON answer
    /// (`Null` for 204).
    pub(crate) fn call(&self, x: &Call<'_>) -> Result<(u16, Value), Problem> {
        let mut headers = vec![
            Header::new("Accept", "application/vnd.github+json"),
            Header::new("X-GitHub-Api-Version", API_VERSION_HEADER),
        ];
        if !self.token.is_empty() {
            headers.push(Header::new("Authorization", &format!("Bearer {}", self.token.expose())));
        }
        if x.body.is_some() {
            headers.push(Header::new("Content-Type", "application/json"));
        }
        let body =
            x.body.clone().map_or(gd_fetch::Body::None, |b| gd_fetch::Body::Bytes(b.into()));
        let request = Request {
            method: x.method,
            url: format!("{}{}", self.roots.api, x.path),
            headers,
            body,
        };
        let max = if x.max_bytes > 0 { x.max_bytes } else { DEFAULT_MAX_BYTES };
        let r = self.send(request, Duration::from_secs(60), max);
        let status = r.status.unwrap_or(0);
        let error = r.error.as_ref().map(|e| e.kind);
        if error.is_some_and(|k| k != ErrorKind::Status) {
            return Err(transfer_problem(&r, x.path, x.doing));
        }
        if status == 204 {
            return Ok((status, Value::Null));
        }
        let parsed = parse_json(&r.body, 32);
        if error == Some(ErrorKind::Status) {
            return Err(status_problem(&r, parsed.as_ref().ok(), x));
        }
        let value = parsed.map_err(|_| {
            problem(
                "pages.github",
                "GitHub's answer is not JSON",
                x.path,
                "GitHub answered with something other than JSON.",
                "Try again later; check https://www.githubstatus.com if it persists.",
            )
        })?;
        Ok((status, value))
    }

    /// Sends `request`, stopping it when the client is cancelled; the request's headers,
    /// which may hold the token, are overwritten once it is sent.
    pub(crate) fn send(&self, request: Request, timeout: Duration, max_bytes: usize) -> Response {
        let flag = Arc::new(AtomicBool::new(false));
        let options = gd_fetch::Options {
            timeout: Some(timeout),
            max_bytes,
            user_agent: Client::agent().into(),
            cancel: Some(flag.clone()),
            curl: None,
        };
        let done = AtomicBool::new(false);
        let response = std::thread::scope(|s| {
            // gd-fetch polls a flag; the plugin SDK's probe is a function: a watcher joins
            // them for the length of the transfer.
            s.spawn(|| {
                while !done.load(Ordering::SeqCst) {
                    if self.cancel.is_cancelled() {
                        flag.store(true, Ordering::SeqCst);
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            });
            let response = gd_fetch::send(&request, &options);
            done.store(true, Ordering::SeqCst);
            response
        });
        for h in request.headers {
            forget_string(h.value);
        }
        response
    }

    /// The `Authorization` header Git's smart HTTP takes the token in.
    pub(crate) fn basic_auth(&self) -> Header {
        let credentials = Secret::from_string(format!("x-access-token:{}", self.token.expose()));
        let encoded = Secret::from_string(base64(credentials.expose().as_bytes()));
        Header::new("Authorization", &format!("Basic {}", encoded.expose()))
    }
}

/// Parses a JSON answer, nested at most `depth` deep.
pub(crate) fn parse_json(body: &[u8], depth: usize) -> Result<Value, ()> {
    let text = std::str::from_utf8(body).map_err(|_| ())?;
    let limits = gd_json::Limits { max_depth: depth, ..gd_json::Limits::default() };
    gd_json::parse_with(text, &limits).map_err(|_| ())
}

/// The problem of a call that got no answer, or a redirect.
pub(crate) fn transfer_problem(r: &Response, path: &str, doing: &str) -> Problem {
    let Some(e) = &r.error else {
        let hint = "Run the same guidedog-pages command again.";
        return problem("pages.github", "GitHub had a problem", path, "No error.", hint);
    };
    match e.kind {
        ErrorKind::Redirect => problem(
            "pages.moved",
            "The repository moved",
            path,
            &format!(
                "GitHub redirected the request to {}: the repository was renamed or \
                 transferred.",
                r.header("location").unwrap_or("another address")
            ),
            "Check the repository on GitHub, update pages.toml, and run guidedog-pages bind \
             to authorize the new target.",
        )
        .exit(2),
        ErrorKind::Cancelled => problem(
            "pages.cancelled",
            "Cancelled",
            path,
            &format!("The request to {doing} was cancelled."),
            "Run the command again when you are ready.",
        )
        .exit(130),
        ErrorKind::TooLarge => problem(
            "pages.github",
            "GitHub's answer is too large",
            path,
            &format!("The answer to {doing} is larger than this publisher reads."),
            "Publish a smaller site, or report this to Guidedog's authors.",
        ),
        ErrorKind::Memory => Problem::memory(),
        ErrorKind::CurlMissing => problem(
            "pages.curl",
            "curl was not found",
            "curl",
            "guidedog-pages reaches GitHub through the curl program, and none could be run.",
            "Install curl and put it on PATH, then run the command again.",
        )
        .exit(4),
        _ => problem(
            "pages.network",
            "GitHub cannot be reached",
            path,
            &format!("The request to {doing} failed: {}.", e.detail.trim_end_matches('.')),
            "Check the network connection, then run the command again.",
        )
        .exit(4),
    }
}

/// The problem of a call GitHub refused with a status.
pub(crate) fn status_problem(r: &Response, value: Option<&Value>, x: &Call<'_>) -> Problem {
    let said = value.and_then(|v| v.get("message")).and_then(Value::as_str).unwrap_or("");
    let detail = if said.is_empty() { String::new() } else { format!(" GitHub said: {said}") };
    let (doing, needs) = (x.doing, x.needs);
    let status = r.status.unwrap_or(0);
    let limited = r.header("x-ratelimit-remaining") == Some("0");
    match status {
        401 => problem(
            "pages.credential",
            "GitHub refused the token",
            x.path,
            &format!(
                "The token was refused when trying to {doing}: it expired, was revoked, or \
                 is not a token.{detail}"
            ),
            "Run guidedog-pages auth login (or auth import) for this profile again.",
        )
        .exit(2),
        429 => rate_limited(r, x),
        403 if limited => rate_limited(r, x),
        403 => problem(
            "pages.permission",
            "The token may not do this",
            x.path,
            &format!("GitHub refused to {doing}.{detail}"),
            &format!(
                "Give the token {needs} for this repository (a fine-grained token's \
                 repository permissions), or approve it for the organization's single \
                 sign-on."
            ),
        )
        .exit(2),
        404 => problem(
            "pages.not-found",
            "GitHub found nothing here",
            x.path,
            &format!(
                "GitHub found nothing when trying to {doing}: the repository does not exist, \
                 or the token may not see it.{detail}"
            ),
            &format!("Check repository in pages.toml, and that the token can read it ({needs})."),
        )
        .exit(2),
        409 | 422 => problem(
            "pages.refused",
            "GitHub refused the request",
            x.path,
            &format!("GitHub refused to {doing}.{detail}"),
            "Check the repository's settings on GitHub, then run the command again.",
        ),
        _ => problem(
            "pages.github",
            "GitHub had a problem",
            x.path,
            &format!("GitHub answered {status} when trying to {doing}.{detail}"),
            "Try again later; check https://www.githubstatus.com if it persists.",
        ),
    }
}

fn rate_limited(r: &Response, x: &Call<'_>) -> Problem {
    problem(
        "pages.rate-limit",
        "GitHub's rate limit was reached",
        x.path,
        &format!("GitHub asks to wait before trying to {} again{}.", x.doing, wait_text(r)),
        "Run the command again after that time.",
    )
}

fn wait_text(r: &Response) -> String {
    if let Some(after) = r.header("retry-after").and_then(|v| v.parse::<u64>().ok())
        && after > 0
    {
        return format!(" ({after} seconds)");
    }
    if let Some(reset) = r.header("x-ratelimit-reset").and_then(|v| v.parse::<u64>().ok()) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        if reset > now {
            return format!(" ({} minutes)", (reset - now).div_ceil(60));
        }
    }
    String::new()
}

/// `bytes` in standard base64, with padding.
pub(crate) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[(n >> (18 - 6 * i) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// `text` percent-encoded for a form: everything but unreserved characters.
pub(crate) fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc_4648() {
        let cases =
            [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg==")];
        for (plain, encoded) in cases {
            assert_eq!(base64(plain.as_bytes()), encoded);
        }
        assert_eq!(percent_encode("a b/c~"), "a%20b%2Fc~");
    }

    #[test]
    fn refusals_name_their_cause() {
        let response = |status: u16, headers: Vec<Header>| Response {
            status: Some(status),
            headers,
            error: Some(gd_fetch::Error::new(ErrorKind::Status, "x", "status")),
            ..Response::default()
        };
        let x = Call {
            path: "/repos/o/r",
            doing: "read it",
            needs: "Contents: read",
            ..Call::default()
        };
        let said = gd_json::parse(r#"{"message":"Bad credentials"}"#).unwrap();
        let p = status_problem(&response(401, vec![]), Some(&said), &x);
        assert_eq!(p.code, "pages.credential");
        assert!(p.message.ends_with("GitHub said: Bad credentials"));
        let limited = vec![Header::new("x-ratelimit-remaining", "0")];
        assert_eq!(status_problem(&response(403, limited), None, &x).code, "pages.rate-limit");
        let wait = vec![Header::new("retry-after", "30")];
        assert!(status_problem(&response(429, wait), None, &x).message.contains("30 seconds"));
        assert_eq!(status_problem(&response(403, vec![]), None, &x).code, "pages.permission");
        assert_eq!(status_problem(&response(404, vec![]), None, &x).code, "pages.not-found");
        assert_eq!(status_problem(&response(422, vec![]), None, &x).code, "pages.refused");
        assert_eq!(status_problem(&response(502, vec![]), None, &x).code, "pages.github");
    }
}
