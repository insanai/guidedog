# Guidedog API reference

This project generates the reference of Guidedog's Rust API from the source of the workspace's main crates, listed in `rust_autoapi_crates` in `conf.toml`.

The crates are read statically with tree-sitter by `gd-rustdoc`; nothing is compiled or run. Each public module gets a page in the `rust` domain, laid out as rustdoc lays it out, and doc comments (CommonMark) are translated to reStructuredText.

## Commands

Build Guidedog (`cargo build --release -p guidedog-cmd`), then, from the repository root:

```sh
target/release/guidedog build html docs/api -j auto -W
target/release/guidedog build pdf docs/api -W
target/release/guidedog serve docs/api
```

Outputs go to `docs/api/_build/html` and `docs/api/_build/pdf`. The same pages form the appendix of the manual's PDF book (`docs/manual/conf.toml` lists the crates too).

See the [Rust chapter](../manual/rust.rst) for the domain and the `rust_autoapi_*` settings, and the [manual](../manual/index.rst).
