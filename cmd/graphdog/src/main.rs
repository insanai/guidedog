//! graphdog renders Graphviz DOT to SVG with the Graphviz library linked in: a
//! dot-compatible subset of the dot command.
//!
//! ```sh
//! echo 'digraph { a -> b }' | graphdog -Tsvg -o g.svg
//! ```

mod args;

use std::io::{IsTerminal, Read, Write};
use std::process::ExitCode;

use gd_graphdog as gv;
use gd_report::Report;

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
            report(&usage);
            return EXIT_USAGE;
        }
    };
    if cmd.help {
        print!("{HELP}");
        return 0;
    }
    let ctx = match gv::Context::new() {
        Ok(ctx) => ctx,
        Err(err) => {
            report(&err.report("", "graphdog"));
            return EXIT_GRAPH;
        }
    };
    if cmd.version {
        let linkage = if gv::STATIC { "static" } else { "shared" };
        let version = ctx.version();
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
    for input in &cmd.inputs {
        let name = if input == "-" { "<stdin>" } else { input.as_str() };
        let Some(source) = read_input(input) else { return EXIT_FILE };
        let options = gv::Options {
            engine: cmd.engine,
            format: cmd.format,
            attributes: &cmd.attributes,
            ..gv::Options::default()
        };
        match ctx.render(&source, options) {
            Ok(rendered) => {
                if !cmd.quiet {
                    for warning in &rendered.warnings {
                        eprintln!("Warning: {name}: {warning}");
                    }
                }
                out.extend_from_slice(&rendered.output);
            }
            Err(err) => {
                report(&err.report(&source, name));
                return EXIT_GRAPH;
            }
        }
    }
    write_output(cmd.output.as_deref(), &out)
}

fn read_input(path: &str) -> Option<String> {
    if path == "-" {
        let mut data = Vec::new();
        if std::io::stdin().read_to_end(&mut data).is_err() {
            report(&file_problem(
                "graphdog.read",
                "CANNOT READ INPUT",
                "<stdin>",
                "graphdog could not read standard input.",
                "Pass the graph as a file instead: graphdog graph.dot",
            ));
            return None;
        }
        return Some(String::from_utf8_lossy(&data).into_owned());
    }
    match std::fs::read(path) {
        Ok(data) => Some(String::from_utf8_lossy(&data).into_owned()),
        Err(err) => {
            report(&file_problem(
                "graphdog.read",
                "CANNOT READ FILE",
                path,
                &format!("graphdog cannot read {path}: {err}."),
                "Check the path. To read standard input, pass - or no file at all.",
            ));
            None
        }
    }
}

fn write_output(path: Option<&str>, data: &[u8]) -> u8 {
    let Some(path) = path.filter(|&p| p != "-") else {
        let mut stdout = std::io::stdout().lock();
        let written = stdout.write_all(data).and_then(|()| stdout.flush());
        return if written.is_ok() { 0 } else { EXIT_FILE };
    };
    if let Err(err) = std::fs::write(path, data) {
        report(&file_problem(
            "graphdog.write",
            "CANNOT WRITE FILE",
            path,
            &format!("graphdog cannot write {path}: {err}."),
            "Check that the directory exists and that you may write to it.",
        ));
        return EXIT_FILE;
    }
    0
}

/// A report about the file `path`, which has no source excerpt.
fn file_problem(code: &str, title: &str, path: &str, message: &str, hint: &str) -> Report {
    let mut report = Report::error(code, title, message).hint(hint);
    report.path = path.to_string();
    report
}

/// Prints an Elm-style report to standard error, in colour on a terminal.
fn report(report: &Report) {
    let style = gd_report::Style { color: std::io::stderr().is_terminal(), width: 80 };
    eprint!("{}", gd_report::render_text(report, &style));
}
