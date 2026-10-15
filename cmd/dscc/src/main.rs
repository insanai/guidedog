//! dscc is dsc with Guidedog built in: it manages discussion catalogs and builds them
//! with the Guidedog inside it (`gd_cli::discussion_builder`), so it needs no guidedog
//! installed. Older Typst catalogs compile with the Typst compiler built in, or the
//! `typst` program `--typst` names.

#![forbid(unsafe_code)]

use gd_discussion::command::{self, Program};

fn main() {
    let version = format!(
        "dscc {}\nGuidedog {} built in (guidedoc {}, document schema {})\n",
        command::VERSION,
        gd_cli::VERSION,
        guidedoc::VERSION,
        guidedoc::SCHEMA_VERSION
    );
    let program = Program {
        name: "dscc",
        version: &version,
        tool: "typst",
        make_builder: gd_cli::discussion_builder,
        ..Program::default()
    };
    let args: Vec<String> =
        std::env::args_os().map(|a| a.to_string_lossy().into_owned()).collect();
    // A crash, which is always a bug, ends with an explanation and exit status 70.
    gd_host::install_crash_reporter(&args, &version);
    // Builds size their stack budgets for WORKER_STACK_BYTES, as in guidedog.
    let status = gd_host::catch_crash(|| {
        gd_host::run_with_stack(gd_host::WORKER_STACK_BYTES, || {
            command::run(&program, args.get(1..).unwrap_or(&[]))
        })
    });
    std::process::exit(status);
}
