//! dsc manages discussion catalogs and builds them with an installed guidedog: catalog
//! commands need nothing else, and build runs guidedog, named by --guidedog,
//! $DSC_GUIDEDOG, or PATH. dscc is the same program with Guidedog built in.

use gd_discussion::command::{self, Program};
use gd_discussion::ENGINE_VARIABLE;

fn main() {
    let version = format!(
        "dsc {}\nbuilds with an installed guidedog (--guidedog, ${ENGINE_VARIABLE}, or PATH)\n",
        command::VERSION
    );
    let program = Program { version: &version, ..Program::default() };
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(command::run(&program, &args));
}
