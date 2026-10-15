//! The settings digest of the pages the HTML builders write (html_builder.odin's
//! `html_settings_digest`), on top of gd-resolve's `document_settings` and
//! `page_inputs`. Every input is listed in docs/manual/internals/incremental.rst; a new
//! input changes that page too.

use gd_env::{Digest, Digester};
use gd_resolve::{BuildView, document_settings};

use crate::templates::Layout;

/// What every page of the html and dirhtml builders shares: the document settings,
/// MathJax's configuration file, the built-in layout (used unless `templates_path` has
/// one), and every template file used. Files are hashed in chunks, never held.
pub(crate) fn html_settings(view: &BuildView<'_>, layout: &Layout) -> Digest {
    let mut d = Digester::new();
    d.digest(&document_settings(view));
    let config_path = &view.config.mathjax.config_path;
    if !config_path.is_empty() {
        let sum = gd_env::file_digest(&view.config.source_dir.join(config_path));
        d.digest(&sum.unwrap_or_default());
    }
    d.text(gd_theme::LAYOUT_HTML);
    for path in layout.files() {
        let sum = gd_env::file_digest(path).unwrap_or_default();
        d.text(&path.display().to_string()).digest(&sum);
    }
    d.finish()
}
