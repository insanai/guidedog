//! Convert Markdown to a standalone HTML page with one call.
//!
//!     cargo run -p guidedoc-examples --example hello

use std::process::ExitCode;

use guidedoc::{Workspace, convert_text};

fn main() -> ExitCode {
    let text = "# Hello\n\nFrom *Guidedoc*: [CommonMark](https://commonmark.org) to HTML.\n";
    let registry = guidedoc_examples::registry();
    // The workspace owns every table a conversion needs; nothing is sized up front.
    let mut ws = Workspace::new();
    match convert_text(&mut ws, &registry, "commonmark", "html", text) {
        // The page borrows the workspace until its next conversion.
        Ok(html) => {
            println!("{html}");
            ExitCode::SUCCESS
        }
        // A failure carries a stable code, a title, a message, and a hint.
        Err(fail) => {
            eprintln!("{fail}");
            ExitCode::FAILURE
        }
    }
}
