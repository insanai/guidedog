//! graphdog renders Graphviz DOT to SVG with the Graphviz library linked in: a
//! dot-compatible subset of the dot command.
//!
//! ```sh
//! echo 'digraph { a -> b }' | graphdog -Tsvg -o g.svg
//! ```

mod args;

use std::io::{Read, Write};
use std::process::ExitCode;

use gd_graphdog as gv;

/// The graph has a problem.
const EXIT_GRAPH: u8 = 1;
/// The command line has a problem.
const EXIT_USAGE: u8 = 2;
/// A file cannot be read or written.
const EXIT_FILE: u8 = 4;

const HELP: &str = "graphdog renders Graphviz DOT graphs, with Graphviz linked in.

Usage: graphdog [options] [FILE...]

Reads each FILE, or standard input when there is none (or FILE is -), and renders the
first graph in it. Outputs are written one after another, like dot.

Options:
  -Tsvg | -Tpng | -Tdot   output format (default svg)
  -Kengine                layout engine: dot (default), neato, fdp, sfdp, circo,
                          twopi, osage, patchwork
  -o FILE                 write to FILE instead of standard output (also -oFILE)
  -Gname=value            default graph attribute; overrides the graph's own default
  -Nname=value            default node attribute
  -Ename=value            default edge attribute
  -q                      do not print warnings
  -V, --version           print the graphdog and Graphviz versions
  -h, --help              print this help

Exit status: 0 ok, 1 the graph has a problem, 2 usage, 4 a file cannot be read or written.
";

fn main() -> ExitCode {
    ExitCode::from(run())
}

fn run() -> u8 {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut cmd = match args::parse_args(&argv) {
        Ok(cmd) => cmd,
        Err(usage) => {
            report(usage.title, "command line", &usage.message, &usage.hint);
            return EXIT_USAGE;
        }
    };
    if cmd.help {
        print!("{HELP}");
        return 0;
    }
    let ctx = match gv::init() {
        Ok(ctx) => ctx,
        Err(err) => {
            eprint!("{}", gv::format_error(&err, "", "graphdog"));
            return EXIT_GRAPH;
        }
    };
    if cmd.version {
        let linkage = if gv::STATIC { "static" } else { "shared" };
        let version = gv::version(&ctx);
        println!("graphdog {} (Graphviz {version}, {linkage})", env!("CARGO_PKG_VERSION"));
        return 0;
    }
    render_all(&ctx, &mut cmd)
}

/// Renders every input before writing anything, so a failure leaves no half-written
/// output file behind.
fn render_all(ctx: &gv::Context, cmd: &mut args::Command) -> u8 {
    if cmd.inputs.is_empty() {
        cmd.inputs.push("-".to_string());
    }
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    for input in &cmd.inputs {
        let name = if input == "-" { "<stdin>" } else { input.as_str() };
        let Some(source) = read_input(input) else { return EXIT_FILE };
        warnings.clear();
        let options = gv::Options {
            engine: cmd.engine,
            format: cmd.format,
            attributes: &cmd.attributes,
            warnings: Some(&mut warnings),
            ..gv::Options::default()
        };
        let rendered = gv::render(ctx, &source, options);
        if !cmd.quiet {
            for warning in &warnings {
                eprintln!("Warning: {name}: {warning}");
            }
        }
        match rendered {
            Ok(bytes) => out.extend_from_slice(&bytes),
            Err(err) => {
                eprint!("{}", gv::format_error(&err, &source, name));
                return EXIT_GRAPH;
            }
        }
    }
    write_output(&cmd.output, &out)
}

fn read_input(path: &str) -> Option<String> {
    if path == "-" {
        let mut data = Vec::new();
        if std::io::stdin().read_to_end(&mut data).is_err() {
            report("CANNOT READ INPUT", "<stdin>", "graphdog could not read standard input.",
                "Pass the graph as a file instead: graphdog graph.dot");
            return None;
        }
        return Some(String::from_utf8_lossy(&data).into_owned());
    }
    match std::fs::read(path) {
        Ok(data) => Some(String::from_utf8_lossy(&data).into_owned()),
        Err(err) => {
            report("CANNOT READ FILE", path, &format!("graphdog cannot read {path}: {err}."),
                "Check the path. To read standard input, pass - or no file at all.");
            None
        }
    }
}

fn write_output(path: &str, data: &[u8]) -> u8 {
    if path.is_empty() || path == "-" {
        let mut stdout = std::io::stdout().lock();
        return if stdout.write_all(data).and_then(|()| stdout.flush()).is_ok() { 0 } else {
            EXIT_FILE
        };
    }
    if let Err(err) = std::fs::write(path, data) {
        report("CANNOT WRITE FILE", path, &format!("graphdog cannot write {path}: {err}."),
            "Check that the directory exists and that you may write to it.");
        return EXIT_FILE;
    }
    0
}

/// Prints an Elm-style error that has no source excerpt.
fn report(title: &str, place: &str, message: &str, hint: &str) {
    eprint!("{}", gv::format_message(title, place, message, hint));
}
