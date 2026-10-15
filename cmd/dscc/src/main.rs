//! dscc is dsc with Guidedog built in: it manages discussion catalogs and builds them
//! with the Guidedog inside it, so it needs no guidedog installed.
//!
//! TODO(gd-project): the in-process builder needs gd-project, which is not ported yet.
//! Until then dscc builds with the installed guidedog, as dsc does; when gd-project lands,
//! `make_builder` becomes a `gd_discussion::Builder` over the project builder and the
//! tool option becomes `typst`.

use gd_discussion::command::{self, Program};

fn main() {
    let version = format!(
        "dscc {}\nGuidedog built in: not yet; builds with an installed guidedog until \
         gd-project lands\n",
        command::VERSION
    );
    let program = Program { name: "dscc", version: &version, ..Program::default() };
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(command::run(&program, &args));
}
