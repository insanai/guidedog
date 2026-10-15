//! Ctrl+C: the flag cancels conversions through the session's probe. In a test binary of
//! its own, since the flag is process-wide.

mod common;

use gd_host::*;
use guidedoc::{Status, default_request};

#[test]
fn an_interrupt_cancels_the_next_conversion() {
    assert!(catch_interrupts());
    assert!(!was_interrupted());
    request_interrupt();
    assert!(was_interrupted() && cancel_probe()());
    let mut session = Session::new();
    let text = "line\n".repeat(40_000);
    let request = default_request("a.rst", &text, "rst", "html");
    let status = session.run(&common::toy_registry(), &request).map(|_| ()).map_err(|f| f.status);
    assert_eq!(status, Err(Status::Cancelled));
    clear_interrupt();
    assert!(session.run(&common::toy_registry(), &request).is_ok());
}
