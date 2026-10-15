//! guidedog-pages publishes Guidedog sites to GitHub Pages from this machine, and is the
//! pages publisher `guidedog publish` runs. It builds projects with an installed
//! guidedog and links no document engine.

// signal(2) for Ctrl-C: FFI.
#[allow(unsafe_code)]
mod interrupt;

use gd_pages::command::{self, Program};
use gd_plugins::Cancel;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // In protocol mode the host stops the program; elsewhere Ctrl+C cancels transfers
    // and waits, so a publication is settled or reported, never cut off mid-way.
    if args.first().map(String::as_str) != Some(gd_plugins::PROTOCOL_FLAG) {
        interrupt::catch_interrupts();
    }
    let program =
        Program { cancel: Cancel::from_fn(interrupt::was_interrupted), ..Program::default() };
    std::process::exit(command::run(&program, &args));
}
