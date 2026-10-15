//! Device authorization (OAuth's device flow) signs in without a browser on this
//! machine: the command shows a code, the user enters it at github.com/login/device on
//! any device, and the command receives a token. It needs the client ID of a GitHub App
//! or OAuth App with the device flow enabled; the ID is public, and no client secret is
//! used.

use std::time::{Duration, Instant};

use gd_fetch::{Body, ErrorKind, Header, Method, Request};
use gd_json::Value;
use gd_plugins::Problem;

use crate::client::{Client, parse_json, percent_encode, status_problem, transfer_problem};
use crate::credential::{Secret, forget_bytes, is_valid_token};
use crate::problem::{problem, token_problem};

/// The client ID a release is built with (`GUIDEDOG_PAGES_CLIENT_ID` at build time);
/// empty when it has none, and `auth login --client-id` names another.
pub const CLIENT_ID: &str = match option_env!("GUIDEDOG_PAGES_CLIENT_ID") {
    Some(id) => id,
    None => "",
};

/// What GitHub gives to start a device authorization.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeviceCode {
    /// The code this program polls with; private to it.
    pub device_code: String,
    /// The code the user enters.
    pub user_code: String,
    /// Where the user enters it (always on `https://github.com/`).
    pub verification_uri: String,
    /// How long the code is valid, in seconds (60 to 1800).
    pub expires_in: u64,
    /// How long to wait between polls, in seconds (5 to 60).
    pub interval: u64,
}

/// Asks GitHub for a device code for `client_id`, with `scope` (an OAuth App's scopes,
/// such as `public_repo`; a GitHub App ignores it). Fails without a client ID, and when
/// the answer has no code or sends the user anywhere but GitHub.
pub fn start_device(c: &Client, client_id: &str, scope: &str) -> Result<DeviceCode, Problem> {
    if client_id.is_empty() {
        return Err(problem(
            "pages.client-id",
            "This build has no GitHub app",
            "auth login",
            "Device login needs the client ID of a GitHub App or OAuth App with the device \
             flow enabled, and this build was made without one.",
            "Pass --client-id=ID of an app you registered, or store a token with \
             guidedog-pages auth import.",
        )
        .exit(2));
    }
    let body = format!("client_id={}&scope={}", percent_encode(client_id), percent_encode(scope));
    let v = form(c, "/login/device/code", body, "start the device login")?;
    let text = |key: &str| v.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
    let number = |key: &str| v.get(key).and_then(Value::as_u64).unwrap_or(0);
    let d = DeviceCode {
        device_code: text("device_code"),
        user_code: text("user_code"),
        verification_uri: text("verification_uri"),
        expires_in: number("expires_in").clamp(60, 1800),
        interval: number("interval").clamp(5, 60),
    };
    // The address is shown to the user: only GitHub's own is.
    if d.device_code.is_empty()
        || d.user_code.is_empty()
        || !d.verification_uri.starts_with("https://github.com/")
    {
        return Err(problem(
            "pages.device",
            "GitHub's device answer is not as expected",
            "/login/device/code",
            "The answer has no code, or sends the user elsewhere.",
            "Run the command again; check the client ID if it persists.",
        ));
    }
    Ok(d)
}

/// Polls GitHub until the user authorizes the device code `d`, at the interval GitHub
/// asks for (longer after `slow_down`), and returns the token. It stops at the code's
/// expiry, when the user denies it, or when the client is cancelled.
pub fn wait_device(c: &Client, client_id: &str, d: &DeviceCode) -> Result<Secret, Problem> {
    let deadline = Instant::now() + Duration::from_secs(d.expires_in);
    let mut interval = d.interval;
    let body = format!(
        "client_id={}&device_code={}&grant_type=urn:ietf:params:oauth:grant-type:device_code",
        percent_encode(client_id),
        percent_encode(&d.device_code)
    );
    loop {
        for _ in 0..interval * 10 {
            if c.is_cancelled() {
                return Err(problem(
                    "pages.cancelled",
                    "Cancelled",
                    "auth login",
                    "The login was cancelled.",
                    "Run the auth login command again when you are ready.",
                )
                .exit(130));
            }
            std::thread::sleep(c.poll_unit);
        }
        if Instant::now() >= deadline {
            return Err(expired());
        }
        let v = form(c, "/login/oauth/access_token", body.clone(), "finish the device login")?;
        if let Some(token) = v.get("access_token").and_then(Value::as_str)
            && !token.is_empty()
        {
            let token = Secret::new(token);
            return if is_valid_token(token.expose()) { Ok(token) } else { Err(token_problem()) };
        }
        let error = v.get("error").and_then(Value::as_str).unwrap_or_default();
        match error {
            "authorization_pending" => {}
            "slow_down" => {
                interval += 5;
                let asked = v.get("interval").and_then(Value::as_u64).unwrap_or(0);
                interval = interval.max(asked);
            }
            "expired_token" => return Err(expired()),
            "access_denied" => return Err(denied()),
            other => return Err(refused(other)),
        }
    }
}

fn expired() -> Problem {
    problem(
        "pages.expired",
        "The login code expired",
        "auth login",
        "The code was not entered in time.",
        "Run auth login again, and enter the new code.",
    )
    .exit(2)
}

fn denied() -> Problem {
    problem(
        "pages.denied",
        "The login was denied",
        "auth login",
        "The authorization was denied on GitHub.",
        "Run auth login again and approve the request, or use auth import with a token.",
    )
    .exit(2)
}

fn refused(error: &str) -> Problem {
    problem(
        "pages.device",
        "GitHub refused the device login",
        "auth login",
        &format!("GitHub answered {error:?}."),
        "Check that the app's device flow is enabled and the client ID is right, then run \
         auth login again.",
    )
    .exit(2)
}

/// Posts a form to GitHub's web root and returns the JSON object it answers. The
/// answer, which may hold a token, is overwritten once read.
fn form(c: &Client, path: &str, body: String, doing: &str) -> Result<Value, Problem> {
    let request = Request {
        method: Method::Post,
        url: format!("{}{path}", c.roots.web),
        headers: vec![
            Header::new("Accept", "application/json"),
            Header::new("Content-Type", "application/x-www-form-urlencoded"),
        ],
        body: Body::Bytes(body.into_bytes()),
    };
    let mut r = c.send(request, Duration::from_secs(60), 64 * 1024);
    let x = crate::client::Call {
        path,
        doing,
        needs: "no permission",
        ..crate::client::Call::default()
    };
    let kind = r.error.as_ref().map(|e| e.kind);
    if kind.is_some_and(|k| k != ErrorKind::Status) {
        return Err(transfer_problem(&r, path, doing));
    }
    let parsed = parse_json(&r.body, 4);
    forget_bytes(std::mem::take(&mut r.body));
    if kind == Some(ErrorKind::Status) {
        return Err(status_problem(&r, parsed.as_ref().ok(), &x));
    }
    match parsed {
        Ok(v) if v.is_object() => Ok(v),
        _ => Err(problem(
            "pages.github",
            "GitHub's answer is not a JSON object",
            path,
            "GitHub answered the device login with something unexpected.",
            "Try the command again in a few minutes.",
        )),
    }
}
