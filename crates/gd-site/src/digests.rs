//! Common HTML inputs, extended by gd-resolve's `page_inputs` and per-page traces.
//! Every input is listed in
//! docs/manual/internals/incremental.rst; a new input changes that page too.

use gd_env::{Digest, Digester};
use gd_resolve::BuildView;

/// Shared configuration, environment, inventories and MathJax input.
/// Templates and navigation are per-page lookup dependencies.
pub(crate) fn html_settings(view: &BuildView<'_>) -> Digest {
    let mut d = Digester::new();
    d.digest(&view.settings).digest(&view.env.settings).digest(&view.intersphinx.digest);
    let config_path = &view.config.mathjax.config_path;
    if !config_path.is_empty() {
        let sum = gd_env::file_digest(&view.config.source_dir.join(config_path));
        d.digest(&sum.unwrap_or_default());
    }
    d.finish()
}
