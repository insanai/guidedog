//! Prints the tree a reader builds for a file: `dump READER FILE`.

use guidedoc::{Workspace, default_request, dump_to_string, read_document, trusted_policy};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (reader_id, path) = (&args[1], &args[2]);
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let settings = guidedoc_sphinx::myst_settings(
        guidedoc_sphinx::Config::default(),
        guidedoc_commonmark::Settings::default(),
    );
    let mut request = default_request(path, &text, reader_id, "html");
    request.policy = trusted_policy();
    request.policy.keep_documents_with_errors = true;
    if reader_id == "myst" {
        request.read.extension = Some(guidedoc::read_extension(&settings));
    }
    let reader = match reader_id.as_str() {
        "rst" => guidedoc_rst::reader(),
        "sphinx" => guidedoc_sphinx::reader(),
        _ => guidedoc_commonmark::MYST_READER,
    };
    let mut ws = Workspace::new();
    match read_document(&mut ws, &reader, &request) {
        Ok(doc) => print!("{}", dump_to_string(doc)),
        Err(f) => eprintln!("failed: {f:?}"),
    }
    for d in ws.reports().items() {
        eprintln!("{:?} {}", d.severity, d.code);
    }
}
