//! Measures fresh and reused workspaces without process startup or file I/O in the loop.
//!
//! cargo run --release -p guidedoc-examples --example measure_conversion -- \
//!     document.rst rst html 1000 15
//!
//! Each CSV row is one batch. Input loading and the first warm conversion are outside
//! the timer. Fresh batches include workspace construction and destruction. Run without
//! competing builds, alternate compiler versions, and compare distributions, not one row.

use std::error::Error;
use std::hint::black_box;
use std::io;
use std::process::ExitCode;
use std::time::Instant;

use guidedoc::{Registry, Request, Workspace, convert, default_request};

#[derive(Debug)]
struct Options {
    path: String,
    reader: String,
    renderer: String,
    rounds: u32,
    batches: u32,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn options() -> Result<Options, io::Error> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Usage: measure_conversion FILE READER RENDERER ROUNDS BATCHES; \
             rounds and batches must be positive integers.",
        )
    };
    if args.len() != 5 {
        return Err(invalid());
    }
    let positive = |text: &str| text.parse::<u32>().ok().filter(|&n| n > 0).ok_or_else(invalid);
    Ok(Options {
        path: args[0].clone(),
        reader: args[1].clone(),
        renderer: args[2].clone(),
        rounds: positive(&args[3])?,
        batches: positive(&args[4])?,
    })
}

fn run() -> Result<(), Box<dyn Error>> {
    let options = options()?;
    let source = std::fs::read_to_string(&options.path)?;
    let registry = guidedoc_examples::registry();
    let mut request = default_request(&options.path, &source, &options.reader, &options.renderer);
    request.render.standalone = false;
    let mut workspace = Workspace::new();
    let expected = convert(&mut workspace, &registry, &request)?.len();
    println!("mode,batch,rounds,input_bytes,output_bytes,elapsed_ns");
    for batch in 0..options.batches {
        // Reverse each pair so a mode does not always inherit the warmer CPU.
        for reused in if batch % 2 == 0 { [false, true] } else { [true, false] } {
            let start = Instant::now();
            for _ in 0..options.rounds {
                if reused {
                    measure(&mut workspace, &registry, &request, expected)?;
                } else {
                    measure(&mut Workspace::new(), &registry, &request, expected)?;
                }
            }
            let elapsed = start.elapsed().as_nanos();
            let mode = if reused { "reused" } else { "fresh" };
            println!("{mode},{batch},{},{},{expected},{elapsed}", options.rounds, source.len());
        }
    }
    Ok(())
}

fn measure(
    workspace: &mut Workspace,
    registry: &Registry,
    request: &Request<'_>,
    expected: usize,
) -> Result<(), guidedoc::Fail> {
    let output = convert(black_box(workspace), black_box(registry), black_box(request))?;
    assert_eq!(black_box(output).len(), expected, "conversion output length changed");
    Ok(())
}
