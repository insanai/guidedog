//! Write a reStructuredText or Markdown file as plain text with the `text` renderer.
//!
//!     cargo run -p guidedoc-examples --example plain_text -- guide.rst
//!     cargo run -p guidedoc-examples --example plain_text        (a built-in sample)
//!
//! A renderer is a plain `Renderer` value whose `render` function walks the document and
//! writes through an emitter; `guidedoc-text` is a complete one. Any caller can put it
//! in its own registry next to the readers it wants.

use std::process::ExitCode;

use guidedoc::{Registry, Workspace, convert_text, validate_registry};

const SAMPLE: &str = "Plain text
==========

A *renderer* is one function, registered by value.

- It walks the document.
- It writes through an emitter.

::

    cargo run -p guidedoc-examples --example plain_text
";

fn main() -> ExitCode {
    let path = std::env::args().nth(1);
    let text = match &path {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => {
                eprintln!("Cannot read {path}: {err}.");
                eprintln!("Hint: pass a readable .rst or .md file, or nothing for a sample.");
                return ExitCode::from(2);
            }
        },
        None => SAMPLE.to_owned(),
    };
    let reader = guidedoc_examples::reader_for(path.as_deref().unwrap_or("sample.rst"));

    // Only the readers and the one renderer this program needs.
    let full = guidedoc_examples::registry();
    let registry = Registry { readers: full.readers, renderers: vec![guidedoc_text::RENDERER] };
    // convert checks the registry on every call; checking once where it is built reports
    // a mistake (a duplicate id, an empty version) before any input is read.
    if let Err(fail) = validate_registry(&registry) {
        eprintln!("{fail}");
        return ExitCode::FAILURE;
    }

    let mut ws = Workspace::new();
    match convert_text(&mut ws, &registry, reader, "text", &text) {
        Ok(plain) => {
            print!("{plain}");
            ExitCode::SUCCESS
        }
        Err(fail) => {
            eprintln!("{fail}");
            ExitCode::FAILURE
        }
    }
}
