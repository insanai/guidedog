//! Convert many documents with one workspace, so later conversions reuse its memory.
//!
//!     cargo run -p guidedoc-examples --example reuse_workspace
//!
//! The Odin engine showed bounded memory with an example that ran on fixed static
//! arrays and a panicking allocator. Rust needs no caller-provided storage: a
//! `Workspace` owns its node, text, and output tables as `Vec`s and `String`s and only
//! clears them between conversions, which keeps their capacity. The first conversion
//! grows the tables; the next ones of a similar size find room already there. The
//! engine's own test `steady_state_conversion_does_not_allocate`
//! (crates/guidedoc/tests/services.rs) counts allocations with a counting global
//! allocator and checks that a repeated conversion makes none. Readers may still
//! allocate small vectors while parsing (crates/guidedoc-rst/README.md says where).
//!
//! This program shows the same reuse without an allocator hook: the output buffer, which
//! every conversion returns a borrow of, stays at one address once it is large enough.
//! Bounds come from `Limits`, and running out is a `Capacity` or `Limit` result, never
//! an abort.

use std::process::ExitCode;
use std::time::Instant;

use guidedoc::{Fail, Workspace, convert, default_request};

const SOURCE: &str = "Guide
=====

Guidedoc reads *reStructuredText* and writes Typst.

.. note:: The workspace keeps its buffers between conversions.
";

/// How many times the document is converted.
const ROUNDS: usize = 1000;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(fail) => {
            eprintln!("{fail}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Fail> {
    let registry = guidedoc_examples::registry();
    let mut ws = Workspace::new();
    // The body only: a standalone document would add Typst's helper definitions.
    let mut request = default_request("guide.rst", SOURCE, "rst", "typst");
    request.render.standalone = false;

    let first = convert(&mut ws, &registry, &request)?;
    println!("{first}");
    // Only the address is kept: the borrow of `ws` ends here.
    let first_address = first.as_ptr() as usize;

    let mut moved = 0;
    let started = Instant::now();
    for _ in 0..ROUNDS {
        let output = convert(&mut ws, &registry, &request)?;
        if output.as_ptr() as usize != first_address {
            moved += 1;
        }
    }
    let each = started.elapsed() / ROUNDS as u32;
    println!("{ROUNDS} more conversions with one workspace, {each:?} each.");
    println!("The output buffer moved {moved} times: its capacity was reused.");
    Ok(())
}
