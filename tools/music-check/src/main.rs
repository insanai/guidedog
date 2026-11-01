//! Explicit corpus updates and deterministic native music qualification.

#![forbid(unsafe_code)]

mod audit;
mod corpus;
mod fonts;
mod fuzz;
mod png;
mod reference;

fn main() -> std::process::ExitCode {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.as_slice() == ["--help"] {
        println!(
            "Usage: cargo run -p music-check -- [--update | --fuzz COUNT | --reference LILYPOND]\n\
            The default compares native SVG and tolerant PNG pixels with the corpus."
        );
        return std::process::ExitCode::SUCCESS;
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let result = match arguments.as_slice() {
        [] => corpus::check(&root, false),
        [update] if update == "--update" => corpus::check(&root, true),
        [flag, program] if flag == "--reference" => reference::check(&root, program),
        [flag, count] if flag == "--fuzz" => count
            .parse::<usize>()
            .map_err(|_| "A fuzz count must be a positive integer.".to_string())
            .and_then(|count| fuzz::check(&root, count)),
        _ => Err("Use --help for the music qualification commands.".into()),
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            let report = gd_report::Report::error(
                "music.qualification",
                "MUSIC CHECK FAILED",
                &message,
            )
            .hint("Fix the notation or engraving regression; update goldens only after review.");
            let mut text = String::new();
            gd_report::write_text(&mut text, &report, &gd_report::Style::default());
            eprint!("{text}");
            std::process::ExitCode::FAILURE
        }
    }
}
