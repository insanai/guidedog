//! The built-in registry and typed read options.
//!
//! The registry is an ordinary value: embedders that want a subset build their own
//! [`Registry`] from the reader and renderer crates directly.

use guidedoc::{ReadOptions, Registry, read_extension};

/// The built-in registry: readers `rst`, `sphinx`, `commonmark` (pure CommonMark 0.31.2)
/// and `myst` (MyST Markdown); renderers `html`, `typst` (Typst source), `text`, and
/// `rst` (reStructuredText).
///
/// Building it allocates two small vectors; keep one per program or thread and reuse it.
pub fn registry() -> Registry {
    Registry {
        readers: vec![
            guidedoc_rst::reader(),
            guidedoc_sphinx::reader(),
            guidedoc_commonmark::READER,
            guidedoc_commonmark::MYST_READER,
        ],
        renderers: vec![
            guidedoc_html::RENDERER,
            guidedoc_typst::RENDERER,
            guidedoc_text::RENDERER,
            guidedoc_rst_writer::RENDERER,
        ],
    }
}

/// Read options for the `rst` reader with `settings` (start from
/// `guidedoc_rst::default_settings()`); the options borrow the settings.
///
/// Settings travel with their type, so another reader refuses them with
/// `core.read.settings_type` instead of misreading them.
pub fn rst_options(settings: &guidedoc_rst::Settings) -> ReadOptions<'_> {
    ReadOptions { extension: Some(read_extension(settings)), ..ReadOptions::default() }
}

/// Read options for the `sphinx` reader with `config` (start from
/// `guidedoc_sphinx::Config::default()`); the options borrow the configuration.
pub fn sphinx_options(config: &guidedoc_sphinx::Config) -> ReadOptions<'_> {
    ReadOptions { extension: Some(read_extension(config)), ..ReadOptions::default() }
}

/// Read options for the `myst` reader with `settings` (from
/// `guidedoc_commonmark::Settings::default()` or `guidedoc_sphinx::myst_settings`); the
/// options borrow the settings. The `commonmark` reader takes no settings.
pub fn myst_options(settings: &guidedoc_commonmark::Settings) -> ReadOptions<'_> {
    ReadOptions { extension: Some(read_extension(settings)), ..ReadOptions::default() }
}

/// The settings of the built-in readers for converting one file, kept together so
/// read options can borrow them: Docutils' settings for `rst`, Sphinx's for `sphinx`,
/// and MyST with the Sphinx layer's directives and roles for `myst`.
#[derive(Debug, Clone)]
pub struct ReaderSettings {
    /// The `rst` reader's settings.
    pub rst: guidedoc_rst::Settings,
    /// The `sphinx` reader's configuration.
    pub sphinx: guidedoc_sphinx::Config,
    /// The `myst` reader's settings, with the Sphinx layer as their extension.
    pub myst: guidedoc_commonmark::Settings,
}

impl Default for ReaderSettings {
    /// The readers' defaults; MyST reads `{doc}`, `{ref}` and directives as Sphinx does.
    fn default() -> Self {
        ReaderSettings::with_config(guidedoc_sphinx::Config::default())
    }
}

impl ReaderSettings {
    /// Settings whose Sphinx layer (for `sphinx` and `myst`) uses `config`.
    pub fn with_config(config: guidedoc_sphinx::Config) -> Self {
        let myst = guidedoc_sphinx::myst_settings(config.clone(), Default::default());
        ReaderSettings { rst: guidedoc_rst::default_settings(), sphinx: config, myst }
    }

    /// Settings for writing source again (the `rst` renderer): no smart quotes, so the
    /// text keeps the quotes and dashes it was written with.
    pub fn for_source() -> Self {
        let mut config = guidedoc_sphinx::Config::default();
        config.settings.smart_quotes = guidedoc_rst::SmartQuotes::Off;
        let mut settings = ReaderSettings::with_config(config);
        settings.rst.smart_quotes = guidedoc_rst::SmartQuotes::Off;
        settings
    }

    /// The read options for reader `reader`, borrowing these settings; other readers
    /// (`commonmark`) get the defaults.
    pub fn options(&self, reader: &str) -> ReadOptions<'_> {
        match reader {
            "rst" => rst_options(&self.rst),
            "sphinx" => sphinx_options(&self.sphinx),
            "myst" => myst_options(&self.myst),
            _ => ReadOptions::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use guidedoc::{Request, Workspace, convert, convert_text, default_request};

    use super::*;

    #[test]
    fn the_builtin_registry_is_valid_and_complete() {
        let r = registry();
        assert!(r.validate().is_ok());
        for id in ["rst", "sphinx", "commonmark", "myst"] {
            assert!(r.find_reader(id).is_some(), "reader {id}");
        }
        for id in ["html", "typst", "text", "rst"] {
            assert!(r.find_renderer(id).is_some(), "renderer {id}");
        }
    }

    #[test]
    fn every_reader_converts_to_every_renderer() {
        let r = registry();
        let mut ws = Workspace::new();
        let sources = [
            ("rst", "Title\n=====\n\nSome *text*.\n"),
            ("sphinx", "Title\n=====\n\nSee :ref:`x` later.\n"),
            ("commonmark", "# Title\n\nSome *text*.\n"),
            ("myst", "# Title\n\nSome *text*.\n"),
        ];
        for (reader, text) in sources {
            for renderer in ["html", "typst", "text", "rst"] {
                let out = convert_text(&mut ws, &r, reader, renderer, text);
                let out = out.unwrap_or_else(|f| panic!("{reader} -> {renderer}: {f:?}"));
                assert!(out.contains("Title"), "{reader} -> {renderer}: {out}");
            }
        }
    }

    // The reader id chooses the Markdown profile: "myst" reads $x$ as math, "commonmark"
    // (pure CommonMark) as text; typed read options reach the reader.
    #[test]
    fn typed_read_options_reach_the_reader() {
        let r = registry();
        let mut ws = Workspace::new();
        let text = "Euler: $e^{i\\pi}$\n";
        let myst = guidedoc_commonmark::Settings::default();
        for reader in ["myst", "commonmark"] {
            let mut request: Request<'_> = default_request("page.md", text, reader, "html");
            if reader == "myst" {
                request.read = myst_options(&myst);
            }
            let html = convert(&mut ws, &r, &request).expect("converts").to_owned();
            assert_eq!(html.contains("class=\"math\""), reader == "myst", "{reader}: {html}");
        }
        let mut settings = guidedoc_rst::default_settings();
        settings.pep_references = true;
        let mut request = default_request("page.rst", "See PEP 8.\n", "rst", "html");
        request.read = rst_options(&settings);
        let html = convert(&mut ws, &r, &request).expect("converts");
        assert!(html.contains("pep-0008"), "{html}");
    }

    // MyST with the Sphinx layer converts to reStructuredText with its roles kept.
    #[test]
    fn myst_converts_to_restructuredtext() {
        let r = registry();
        let mut ws = Workspace::new();
        let settings = ReaderSettings::for_source();
        let text = "# Title\n\nSee {doc}`intro` and {ref}`x <lbl>`.\n";
        let mut request = default_request("page.md", text, "myst", "rst");
        request.read = settings.options("myst");
        let rst = convert(&mut ws, &r, &request).expect("converts");
        assert!(rst.contains(":doc:`intro`") && rst.contains(":ref:`x <lbl>`"), "{rst}");
    }

    // Settings meant for one reader are refused by every other, never reinterpreted.
    #[test]
    fn read_options_of_another_reader_are_refused() {
        let r = registry();
        let mut ws = Workspace::new();
        let rst = guidedoc_rst::default_settings();
        let myst = guidedoc_commonmark::Settings::default();
        let cases = [
            ("myst", rst_options(&rst), "core.read.settings_type"),
            ("sphinx", rst_options(&rst), "core.read.settings_type"),
            ("rst", myst_options(&myst), "core.read.settings_type"),
            ("commonmark", myst_options(&myst), "core.read.settings_none"),
        ];
        for (reader, options, code) in cases {
            let mut request = default_request("page", "Text.\n", reader, "html");
            request.read = options;
            let fail = convert(&mut ws, &r, &request).expect_err("refused");
            assert_eq!(fail.code(), code, "{reader}");
        }
    }
}
