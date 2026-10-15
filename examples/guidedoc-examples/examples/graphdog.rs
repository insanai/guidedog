//! Render a DOT graph to SVG with Graphviz linked in, and show what an error looks like.
//!
//!     cargo run -p guidedoc-examples --example graphdog

use std::process::ExitCode;

use gd_graphdog::{Attribute, AttributeKind, Context, Engine, Options, format_error};

const PIPELINE: &str = "digraph {
    rankdir=LR
    rst -> guidedoc; markdown -> guidedoc
    guidedoc -> html; guidedoc -> typst -> pdf
}
";

const BROKEN: &str = "digraph {\n    a -> b\n    b -> -> c\n}\n";

fn main() -> ExitCode {
    // Loading the Graphviz plugins is the expensive part: make one context and keep it.
    // Dropping it frees the context.
    let ctx = match Context::new() {
        Ok(ctx) => ctx,
        Err(err) => {
            eprint!("{}", format_error(&err, "", "graphviz"));
            return ExitCode::FAILURE;
        }
    };
    println!("Graphviz {}", ctx.version());

    // -N defaults, as on the dot command line: they override the graph's own defaults.
    let attributes = [node_default("shape", "box"), node_default("fontname", "Helvetica")];
    let options = Options { engine: Engine::Dot, attributes: &attributes, ..Default::default() };
    match ctx.render(PIPELINE, options) {
        Ok(rendered) => println!("{} bytes of SVG", rendered.output.len()),
        Err(err) => {
            eprint!("{}", format_error(&err, PIPELINE, "pipeline.dot"));
            return ExitCode::FAILURE;
        }
    }

    // A syntax error comes back as a value with its line, an excerpt, and a hint.
    match ctx.render(BROKEN, Options::default()) {
        Ok(_) => println!("broken.dot rendered, unexpectedly"),
        Err(err) => print!("{}", format_error(&err, BROKEN, "broken.dot")),
    }
    ExitCode::SUCCESS
}

/// A default attribute for every node (`-Nname=value`).
fn node_default(name: &str, value: &str) -> Attribute {
    Attribute { kind: AttributeKind::Node, name: name.to_owned(), value: value.to_owned() }
}
