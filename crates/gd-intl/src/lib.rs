//! gd-intl: a Guidedog build's translations. The catalogs documents are translated
//! with and the settings of gd-i18n's translation pass (`Intl`, `load_intl`), Guidedog's
//! own words in the build's language (`UiTranslator`, gd-env's `UiText`), what the
//! language changes in the settings and dates (`localize_config`, `build_dates`), the
//! `gettext` builder (`GettextBuilder`, POT templates through a gd-outputs
//! `Generation`), and `guidedog intl update` / `intl stat` (`intl_update`, `intl_stat`).
//!
//! ```no_run
//! # fn build(config: &mut gd_conf::Config) {
//! use gd_intl::{load_intl, localize_config};
//! let budget = gd_host::Budget::new(gd_env::BUILD_BUDGET);
//! let mut reports = Vec::new();
//! let intl = load_intl(config, gd_env::BuilderKind::Html, &budget, &mut reports);
//! localize_config(config, &intl.ui);
//! if let Some(catalog) = intl.catalog_of("guide/usage") {
//!     println!("{} messages for guide/usage", catalog.message_count());
//! }
//! # }
//! ```
//!
//! Loading never fails: a catalog that cannot be read is a warning. The commands and
//! the builder fail with `gd_host::Problem`s that carry a hint. Nothing here prints.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod gettext;
mod intl;
mod localize;
mod ui;
mod update;

pub use gettext::{GettextBuilder, Reread};
pub use intl::{Intl, load_intl, locale_paths};
pub use localize::{
    additional_targets, build_dates, catalog_report, compact_mode, localize_config,
    progress_classes, python_format,
};
pub use ui::UiTranslator;
pub use update::{IntlAction, IntlFile, IntlOptions, intl_stat, intl_update};

#[cfg(test)]
mod tests;
