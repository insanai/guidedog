//! The product badge served as an installed program. The same `Badge` type is linked
//! into a host through `badge::plugin()`; this binary only adds the protocol loop.
//!
//! ```text
//! cargo build --release -p gd-plugins --example build_plugin
//! guidedog plugin register product-badge "$PWD/target/release/examples/build_plugin" \
//!     --project=docs --config=badge.toml
//! ```
//!
//! Then select it in the project's `conf.toml`:
//!
//! ```toml
//! plugins = ["product-badge"]
//! [plugin_configs]
//! product-badge = "badge.toml"
//! ```

mod badge;

fn main() -> std::process::ExitCode {
    gd_plugins::serve_plugin(&badge::plugin())
}
