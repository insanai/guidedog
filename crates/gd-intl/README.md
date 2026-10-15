# gd-intl cheat sheet

A Guidedog build's translations: the catalogs documents are translated with, Guidedog's
own words in the build's language, the `gettext` builder, and `guidedog intl update` /
`intl stat`. Ported from `guidedog_old/lib/project` (intl.odin, intl_gettext.odin,
intl_test.odin). The `Translating` reading hook moved to gd-read and the template
functions (`_()`, `gettext()`, `ngettext()`) to gd-site; both use what this crate loads.

```rust
let mut reports = Vec::new();
let intl = gd_intl::load_intl(&config, kind, &budget, &mut reports); // never fails
if kind != BuilderKind::Gettext { gd_intl::localize_config(&mut config, &intl.ui); }
let dates = gd_intl::build_dates(&config, &intl.ui, now);           // |today|, footer
let settings = gd_env::build_settings(reading, intl.digest, untrusted, &limits, plugins);
// gd-read, per document:
let catalog = intl.catalog_of(docname);    // Option<&Catalog>, for gd_i18n::Translation
let deps = intl.files_of(docname);         // &[Dependency]: the document depends on them
// gd-project, for `build gettext`:
GettextBuilder { reread: Some(&reread) }.write(&view, &mut generation, &mut reports)?;
// gd-cli:
let mut files = Vec::new();
gd_intl::intl_update(&config, &IntlOptions { languages: vec!["de".into()],
    ..IntlOptions::default() }, &mut files)?;
```

| Item | Does |
|---|---|
| `Intl { translate, domains, files, ui, labels, targets, progress, compact, digest }` | the build's translation state; `domain_of`, `catalog_of`, `files_of`, `source_name` |
| `load_intl(&config, kind, &budget, &mut reports)`, `locale_paths` | loads it; a bad catalog is an `i18n.catalog` warning |
| `UiTranslator` (`load`, `english`, `labels`, `translator`) | gd-env's `UiText` over the `guidedog` domain |
| `localize_config`, `build_dates` | the English defaults Sphinx translates; the build's dates |
| `compact_mode`, `progress_classes`, `additional_targets`, `catalog_report` | intl settings as gd-i18n types; catalog errors as reports |
| `GettextBuilder { reread }`, `Reread` | gd-resolve's `OutputBuilder` for `gettext` |
| `intl_update`, `intl_stat`, `IntlOptions`, `IntlFile`, `IntlAction` | sphinx-intl's update and stat |

## Redesign notes

- **No `Build` god object.** `load_intl` takes the configuration and returns plain data;
  the build keeps it. `localize_config` (numfig captions, derived `html_title`) is a
  separate call on the build's own copy of the configuration instead of a side effect
  of preparing the catalogs; `stamp_dates` became `build_dates`, which returns
  gd-resolve's `Dates`.
- **Typed settings.** `FlagOrName` becomes gd-i18n's `Compact` and `Progress`;
  `IntlOptions` takes `Option<PathBuf>` folders (was `""` for the default) and defaults
  to keeping obsolete messages, sphinx-intl's default.
- **Catalogs are owned values** in `BTreeMap`s (sorted for the digest), merged in
  place; their index is built once so reading threads only read them.
- **The UI translator is a trait object** (`&dyn UiText`), so renderers and templates
  do not depend on gd-gettext.
- **Commands borrow a configuration, not a project**: `intl_update(&config, ..)`. What
  was done is appended to a caller's `Vec<IntlFile>`, so a failure still shows the
  catalogs written before it (Odin returned both).
- **Templates go through a gd-outputs `Generation`**: one site unit; a template that
  differs only in `POT-Creation-Date` is kept (`Unit::keep_file`), others are emitted.
  A document whose object cannot be loaded fails the unit (`build.output`); the
  `reread` hook lets gd-read read a stale document again, since gd-intl cannot depend
  on gd-read.
- **Budget**: catalog files and the template in place are read within the build's
  budget and released once parsed or compared. The per-allocation charges of the Odin
  build (kept catalogs, extracted messages, scratch arenas) are dropped: Rust aborts on
  allocation failure, and these sizes follow the files already charged.
- Moved: `check_intl_settings` lives in gd-conf (`check_config`); `language_file` and
  its test moved to gd-book with builders.odin.
