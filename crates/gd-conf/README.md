# gd-conf cheat sheet

conf.toml: a Guidedog project's configuration. Loading, the settings table, object
types, trust (readable roots), and the checks of the mathjax, intersphinx, autodoc,
Odin API, Rust API, plugins, intl and built-in extension settings.

```rust
let loaded = gd_conf::load_config(Path::new("docs"), &gd_conf::LoadOptions {
    overrides: vec!["html_context.edition=beta".into()],
    ..Default::default()
})?;                                   // Err: gd_host::Problem (config.invalid, exit 2)
let c = &loaded.config;               // c.project, c.html.theme, c.myst.heading_anchors
for w in &loaded.warnings { /* gd_report::Report */ }
let roots = gd_conf::readable_roots(c);       // source + include_roots + rust crates
let dropped = gd_conf::confine_untrusted(&mut c.clone()); // --untrusted
```

| Item | Does |
|---|---|
| `Config` (+ `Config::new(source, build)`) | Sphinx's names and defaults; general settings flat, the rest grouped: `mathjax`, `intersphinx`, `autodoc`, `odin_api`, `rust_api`, `plugins`, `html`, `pdf`, `graphviz`, `myst`, `intl`, and `ext` (`ExtensionSettings`: `napoleon`, `autosummary`, `apidoc`, `viewcode`, `linkcode`, `inheritance`, `coverage`, `duration`, `image_converter`; GDS 0011) |
| `load_config(dir, &LoadOptions) -> Result<Loaded, Problem>` | find, read, `-D`, check, finish; `Loaded { config, warnings, settings: Digest }` |
| `find_project`, `apply_table`, `apply_override`, `check_config`, `finish_config`, `merge_table` | the staged path |
| `SETTINGS`, `Setting { name, kind }`, `SettingKind`, `find_setting`, `suggestion`, `is_foreign_setting`, `NATIVE_EXTENSIONS`, `KNOWN_THEMES` | the settings table (gd-migrate checks conf.py values against it) |
| `readable_roots`, `confine_untrusted`, `unread_warning`, `rust_crate_roots` | trust |
| `Digest`, `Digester` | SHA-256 of parts (each followed by a zero byte), hex `Display`; gd-env re-exports them |
| `Pattern` | `nitpick_ignore_regex`: a small bounded regex, `full_match` |
| `json_dumps`, `json_dumps_table` | a TOML value or table as Python's `json.dumps` (mathjax3/4_config) |

## Design notes

- **Typed settings table.** `Store` is an enum of typed accessor functions: a setting
  can only be stored into a field of its kind's type, checked at compile time.
- **Grouped struct, flat keys.** conf.toml keys are unchanged; the struct groups them
  (`html_theme` is `config.html.theme`, `plugin_timeout` is `config.plugins.timeout`).
- **Option and enums instead of sentinels**: `html_last_updated_fmt: Option<String>`,
  `FlagOrName` enum, `Option<PathBuf>` load options, tables as `BTreeMap` (stable order
  for digests, no `sorted_pairs`).
- **Table settings merge into their defaults** (`numfig_format = {figure = ".."}` keeps
  the other formats), as in Sphinx.
- **No regex crate**: `Pattern` covers what conf.py patterns use, with a step budget.
- **Intl checks are local**: `gettext_additional_targets` and `figure_language_filename`
  are validated here (`GETTEXT_TARGETS`, `check_figure_format`) so gd-conf does not
  depend on gd-i18n.
- **The Rust API settings** `rust_autoapi_crates`, `rust_autoapi_root`
  (`rustapi`), `rust_autoapi_options` (`private-members`, `hidden-members`, `noindex`),
  `rust_autoapi_member_order` (`source`, `alphabetical`),
  `rust_autoapi_add_toctree_entry`, `rust_autoapi_generate_api_docs`, checked like the
  `odin_*` ones; crate folders join `readable_roots` and are dropped by `confine_untrusted`
  when outside the source folder.
- `-C` without a project builds the given folder.
- The TOML parser's own tests live in gd-toml.
