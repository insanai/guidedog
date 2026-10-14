//! Graphviz's error log. Messages arrive through a C callback while the Graphviz lock is
//! held, so one process-wide buffer is enough.

use std::ffi::{CStr, c_char, c_int};
use std::marker::PhantomData;
use std::sync::{Mutex, MutexGuard};

use crate::{Locked, ffi};

/// Bytes kept from one render's messages; the rest is dropped.
const CAPACITY: usize = 16384;

struct Log {
    data: Vec<u8>,
    previous: ffi::Agusererrf,
    previous_level: ffi::Agerrlevel,
}

static LOG: Mutex<Log> =
    Mutex::new(Log { data: Vec::new(), previous: None, previous_level: ffi::AGWARN });

fn log() -> MutexGuard<'static, Log> {
    LOG.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The handler Graphviz calls with each formatted message. It is installed only while a
/// render holds the Graphviz lock, so it never runs concurrently with `begin` or the
/// session's drop, which take the log's own lock without calling into Graphviz's error
/// reporting.
unsafe extern "C" fn capture(message: *mut c_char) -> c_int {
    if message.is_null() {
        return 0;
    }
    // SAFETY: Graphviz passes a NUL-terminated string valid for the duration of the call.
    let bytes = unsafe { CStr::from_ptr(message) }.to_bytes();
    let mut log = log();
    let room = CAPACITY.saturating_sub(log.data.len());
    let take = bytes.len().min(room);
    log.data.extend_from_slice(&bytes[..take]);
    0
}

/// Starts capturing Graphviz messages; the returned session restores the previous
/// handler and level when dropped, before the borrowed lock is released.
pub(crate) fn begin(_lock: &Locked) -> Session<'_> {
    let mut log = log();
    log.data.clear();
    // SAFETY: the Graphviz lock is held, so no other thread is inside Graphviz;
    // `capture` matches `agusererrf` and lives for the whole program.
    unsafe {
        log.previous = ffi::agseterrf(Some(capture));
        log.previous_level = ffi::agseterr(ffi::AGWARN);
        ffi::agreseterrors();
    }
    Session { _lock: PhantomData }
}

/// A capture in progress: dropping it restores the previous handler and level.
pub(crate) struct Session<'l> {
    _lock: PhantomData<&'l Locked>,
}

impl Drop for Session<'_> {
    fn drop(&mut self) {
        let log = log();
        // SAFETY: the session borrows the Graphviz lock, which is still held, and the
        // previous handler came from Graphviz itself.
        unsafe {
            ffi::agseterrf(log.previous);
            ffi::agseterr(log.previous_level);
        }
    }
}

/// The messages captured so far, as text.
fn text() -> String {
    String::from_utf8_lossy(&log().data).into_owned()
}

/// One message from the log: an error or a warning, without its prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Message<'a> {
    pub error: bool,
    pub text: &'a str,
}

/// Splits a log into messages. Graphviz sends each message as "Error: ..." or
/// "Warning: ...", and continuation lines separately and unprefixed, so the log is split
/// at the prefixes.
pub(crate) fn messages(log: &str) -> impl Iterator<Item = Message<'_>> {
    let mut rest = log;
    // Some Graphviz messages repeat their own prefix ("Warning: Warning: ..."), which
    // leaves an empty message between them.
    std::iter::from_fn(move || next_raw_message(&mut rest)).filter(|m| !m.text.is_empty())
}

fn next_raw_message<'a>(rest: &mut &'a str) -> Option<Message<'a>> {
    let text = *rest;
    let error_at = text.find("Error: ");
    let warning_at = text.find("Warning: ");
    let (start, error) = match (error_at, warning_at) {
        (Some(e), Some(w)) if e < w => (e, true),
        (_, Some(w)) => (w, false),
        (Some(e), None) => (e, true),
        (None, None) => return None,
    };
    let prefix = if error { "Error: ".len() } else { "Warning: ".len() };
    let text = &text[start + prefix..];
    let mut end = text.len();
    if let Some(at) = text.find("Error: ") {
        end = at;
    }
    if let Some(at) = text[..end].find("Warning: ") {
        end = at;
    }
    *rest = &text[end..];
    let body = text[..end].trim();
    let body = body.strip_prefix("<stdin>: ").unwrap_or(body).trim();
    // A Graphviz built without GTS cannot remove node overlaps for sfdp or overlap=prism.
    // The layout is still complete, so graphdog treats it as a warning.
    let error = error && !body.contains("not built with triangulation library");
    Some(Message { error, text: body })
}

/// The text of the first error message in the current log, if there is one.
pub(crate) fn first_error() -> Option<String> {
    let text = text();
    messages(&text).find(|m| m.error).map(|m| m.text.to_string())
}

/// The text of every warning in the current log.
pub(crate) fn warnings() -> Vec<String> {
    let text = text();
    messages(&text).filter(|m| !m.error).map(|m| m.text.to_string()).collect()
}
