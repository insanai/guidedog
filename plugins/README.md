# Guidedog plugin catalog

[registry.toml](registry.toml) is the public catalog of [GDS 0009](../docs/gds/records/0009-native-plugin-api.rst):
it describes plugins, and does not install, authorize, or run them. Guidedog reads
nothing from it to decide what runs.

Three released plugins are linked into the single `guidedog` executable, so no separate binary or registration is needed:

- `pages` ([GDS 0008](../docs/gds/records/0008-github-pages-publishing.rst), [gd-pages](../crates/gd-pages)): the GitHub Pages publisher, adding `guidedog publish pages` and `guidedog pages` maintenance commands.
- `dsc` ([GDS 0007](../docs/gds/records/0007-discussion-catalogs.rst), [gd-discussion](../crates/gd-discussion)): discussion catalogs, adding `guidedog dsc`.
- `ssg` ([GDS 0013](../docs/gds/records/0013-static-site-generation.rst), [gd-ssg](../crates/gd-ssg)): static site publishing, presets, pinned theme archives, and multi-project website mounts, adding `guidedog ssg`.

Each entry names its ID, source, license, public maintainer names, description,
service kind, status, the interface and protocol versions it speaks, and its design
records. The versions are the constants of [gd-plugins](../crates/gd-plugins):
`API_VERSION` 1.1 for publishers, `PLUGIN_API_VERSION` 1.0 for build plugins, and 1.0
for both process protocols (`guidedog-publisher` and `guidedog-build-plugin`). A
released entry lists the releases that carry it; its artifacts and SHA-256 checksums
are those of each release's `SHA256SUMS`. Entries contain no email addresses or
credentials.

The `product-badge` entry is the complete build-plugin teaching example of
[GDS 0010](../docs/gds/records/0010-typed-build-plugins.rst). Its `Badge` type supplies
source, HTML and asset hooks, and is an ordinary Rust module: a host links it in
through `badge::plugin()` (for example with `gd_cli::run_with`), and the example's
`main.rs` serves the same plugin over the process protocol as an installed program.
It is built from [crates/gd-plugins/examples/build_plugin](../crates/gd-plugins/examples/build_plugin)
and is not bundled as another program in the release:

```sh
cargo build --release -p gd-plugins --example build_plugin
guidedog plugin register product-badge "$PWD/target/release/examples/build_plugin" \
    --project=docs --config=badge.toml
```

The [tutorial](../docs/manual/plugins.rst) creates its project and configuration, then
checks and registers the program before using it in a build.

The **machine registry** is separate: `plugins.toml` in Guidedog's configuration
folder, written only by `guidedog plugin register` and `unregister`. It records the
programs a user chose to run, pinned by SHA-256. Build plugins pass bounded process
qualification before CLI registration. Registration is interoperability evidence,
not certification; runtime replies are checked and installed programs remain
separate processes from the host. A change to this public catalog never changes the
code a user's machine runs. Automatic installation and third-party entries need a
review policy first.

Read the [plugin chapter](../docs/manual/plugins.rst) to write a plugin, and the
[GitHub Pages guide](../docs/manual/howto/github-pages.rst) to publish.
