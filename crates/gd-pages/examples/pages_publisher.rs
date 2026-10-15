//! GitHub Pages reached through the same SDK as any other publisher program. No
//! document engine is linked here: the host builds the site and passes a checked
//! snapshot. Register it with `guidedog plugin register pages /path/to/pages_publisher`
//! and publish with a reviewed `pages.toml`:
//!
//! ```sh
//! guidedog publish --publisher=pages --config=pages.toml
//! ```

use std::process::ExitCode;

fn main() -> ExitCode {
    // Started without the protocol flag, it says how to use it and returns 2.
    gd_plugins::serve_publisher(gd_pages::Pages::default())
}
