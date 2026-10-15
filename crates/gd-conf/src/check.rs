//! Checking the settings together, once conf.toml and every `-D` override are applied:
//! `check_config`.

use std::path::{Path, PathBuf};

use gd_host::Problem;
use gd_report::Report;

use crate::config::{Config, FlagOrName};
use crate::paths::{readable_roots, source_path};
use crate::settings::{KNOWN_THEMES, MYST_EXTENSIONS, NATIVE_EXTENSIONS};
use crate::store::{check_plugin_timeout, config_problem, warning};

/// Checks the settings together, as Sphinx checks them when it starts. Some mistakes
/// are fixed with a warning added to `warnings` (a substituted theme or code style, a
/// missing logo or static folder left out, PNG graphs written as SVG); the others fail
/// with `config.invalid` (exit 2), naming `path`, the file or `"-D"` they came from.
pub fn check_config(
    config: &mut Config,
    path: &str,
    warnings: &mut Vec<Report>,
) -> Result<(), Problem> {
    check_plugin_settings(config, path)?;
    check_theme(config, path, warnings)?;
    check_paths(config, path, warnings)?;
    check_myst(config, path)?;
    check_pygments(config, path, warnings);
    check_documents(config, path)?;
    check_graphviz(config, path, warnings)?;
    check_pdf(config, path)?;
    check_toc(config, path)?;
    check_intl(config, path)
}

fn invalid(path: &str, message: String, hint: &str) -> Problem {
    config_problem(path, 0, message, hint)
}

fn check_plugin_settings(config: &Config, path: &str) -> Result<(), Problem> {
    if let Some(e) = config.extensions.iter().find(|e| !NATIVE_EXTENSIONS.contains(&e.as_str())) {
        return Err(invalid(
            path,
            format!("The extension {e} is not built in."),
            "Use built-in names in extensions. Register native plugins with guidedog plugin \
             register and select their IDs in plugins.",
        ));
    }
    check_plugin_timeout(path, 0, config.plugins.timeout)
}

fn check_theme(config: &Config, path: &str, warnings: &mut Vec<Report>) -> Result<(), Problem> {
    let theme = &config.html.theme;
    if !KNOWN_THEMES.contains(&theme.as_str()) {
        return Err(invalid(
            path,
            format!("The theme {theme} is not available."),
            "Use html_theme = \"guidedog\".",
        ));
    }
    if theme != "guidedog" {
        warnings.push(warning(
            path,
            0,
            "config.theme",
            "THEME SUBSTITUTED",
            format!("{theme} is built with Guidedog's own theme."),
            "Set html_theme = \"guidedog\" to silence this note.",
        ));
    }
    Ok(())
}

/// Checks the MathJax settings, warns about missing HTML files, requires every include
/// root to be a folder, and checks the API settings (odin, rust, autodoc).
fn check_paths(
    config: &mut Config,
    path: &str,
    warnings: &mut Vec<Report>,
) -> Result<(), Problem> {
    check_mathjax(config, path)?;
    check_html_files(config, path, warnings);
    let roots = readable_roots(config);
    for (written, root) in config.include_roots.iter().zip(roots.iter().skip(1)) {
        if !root.is_dir() {
            return Err(invalid(
                path,
                format!(
                    "include_roots names {written}, which is not a folder ({}).",
                    root.display()
                ),
                "Give folders relative to the source folder, such as \"..\".",
            ));
        }
    }
    check_odin(config, path, warnings)?;
    check_rust(config, path, warnings)?;
    check_autodoc(config, path, warnings)
}

/// Checks the MathJax settings as Sphinx does when it installs MathJax.
fn check_mathjax(config: &Config, path: &str) -> Result<(), Problem> {
    let m = &config.mathjax;
    if m.inline.len() != 2 || m.display.len() != 2 {
        return Err(invalid(
            path,
            "mathjax_inline and mathjax_display each need an opening and a closing delimiter."
                .into(),
            "Write mathjax_inline = [\"\\\\(\", \"\\\\)\"].",
        ));
    }
    if m.path.is_empty() {
        return Err(invalid(
            path,
            "mathjax_path is empty.".into(),
            "Remove it to use MathJax from its CDN, or name a copy in html_static_path.",
        ));
    }
    if m.config_path.is_empty() {
        return Ok(());
    }
    let file = config.source_dir.join(&m.config_path);
    if !file.is_file() || !m.config_path.ends_with(".js") {
        return Err(invalid(
            path,
            format!("mathjax_config_path: expected a .js file, but got {}.", file.display()),
            "Name a JavaScript file relative to conf.toml.",
        ));
    }
    Ok(())
}

/// Warns, as Sphinx's html builder does, about a logo, a favicon, or a static or extra
/// folder that does not exist, and leaves it out of the build; a logo or favicon
/// outside the readable folders is left out too.
fn check_html_files(config: &mut Config, path: &str, warnings: &mut Vec<Report>) {
    let source = config.source_dir.clone();
    let missing =
        |file: &str| !file.is_empty() && !file.contains("://") && !source.join(file).exists();
    let roots = gd_host::canonical_roots(&readable_roots(config));
    for (what, file) in [("logo", &mut config.html.logo), ("favicon", &mut config.html.favicon)] {
        if missing(file) {
            warnings.push(warning(
                path,
                0,
                "config.html",
                "FILE NOT FOUND",
                format!("{what} file {file} does not exist."),
                "Name a file relative to the source folder, or remove the setting.",
            ));
            file.clear();
        } else if !file.is_empty()
            && !file.contains("://")
            && !gd_host::confined(&roots, &source.join(&*file))
        {
            warnings.push(warning(
                path,
                0,
                "config.html",
                "FILE NOT READ",
                format!(
                    "{what} file {file} leads outside the source folder and the include roots."
                ),
                "Add its folder to include_roots.",
            ));
            file.clear();
        }
    }
    let folders = [
        ("html_static_path", &mut config.html.static_path),
        ("html_extra_path", &mut config.html.extra_path),
    ];
    for (setting, list) in folders {
        list.retain(|f| {
            if !missing(f) {
                return true;
            }
            warnings.push(warning(
                path,
                0,
                "config.html",
                "FOLDER NOT FOUND",
                format!("{setting} entry {f} does not exist."),
                "Create the folder, or remove it from the setting.",
            ));
            false
        });
    }
}

/// A folder an API setting names that does not exist, as a warning.
fn missing_folder(path: &str, code: &str, title: &str, setting: &str, dir: &Path) -> Report {
    warning(
        path,
        0,
        code,
        title,
        format!("{setting} names {}, which is not a folder.", dir.display()),
        "Give folders relative to the source folder, such as \"../src\".",
    )
}

/// An API root folder must be a name inside the source folder.
fn check_api_root(path: &str, setting: &str, root: &str, example: &str) -> Result<(), Problem> {
    if root.is_empty() || root.starts_with('/') || root.contains("..") {
        return Err(invalid(
            path,
            format!("{setting} is {root:?}; it must name a folder inside the source folder."),
            &format!("Use a name such as \"{example}\"."),
        ));
    }
    Ok(())
}

/// Checks the odin_* settings.
fn check_odin(config: &Config, path: &str, warnings: &mut Vec<Report>) -> Result<(), Problem> {
    let o = &config.odin_api;
    if !["source", "alphabetical", "groupwise"].contains(&o.member_order.as_str()) {
        return Err(invalid(
            path,
            format!(
                "odin_autoapi_member_order is {:?}, which is not source, alphabetical, or \
                 groupwise.",
                o.member_order
            ),
            "Use \"source\" to keep the order of the files.",
        ));
    }
    let known = ["members", "undoc-members", "private-members"];
    if let Some(bad) = o.options.iter().find(|x| !known.contains(&x.as_str())) {
        return Err(invalid(
            path,
            format!(
                "odin_autoapi_options lists {bad:?}, which is not members, undoc-members, or \
                 private-members."
            ),
            "Remove it, or use one of the options listed.",
        ));
    }
    check_api_root(path, "odin_autoapi_root", &o.root, "api")?;
    if let Some(name) = o.collections.keys().find(|n| n.is_empty() || n.contains([':', '/', ' ']))
    {
        return Err(invalid(
            path,
            format!("odin_collections has the name {name:?}, which Odin could not import from."),
            "Name each collection as Odin does, such as core = \"/path/to/odin/core\".",
        ));
    }
    let named = o.dirs.iter().map(|d| ("odin_autoapi_dirs", d));
    let collections = o.collections.values().map(|d| ("odin_collections", d));
    for (setting, dir) in named.chain(collections) {
        let dir = source_path(config, dir);
        if !dir.is_dir() {
            let title = "ODIN SOURCE FOLDER MISSING";
            warnings.push(missing_folder(path, "config.odin", title, setting, &dir));
        }
    }
    Ok(())
}

/// Checks the rust_autoapi_* settings, as check_odin checks the odin ones.
fn check_rust(config: &Config, path: &str, warnings: &mut Vec<Report>) -> Result<(), Problem> {
    let r = &config.rust_api;
    if !["source", "alphabetical"].contains(&r.member_order.as_str()) {
        return Err(invalid(
            path,
            format!(
                "rust_autoapi_member_order is {:?}, which is not source or alphabetical.",
                r.member_order
            ),
            "Use \"source\" to keep the order of the files.",
        ));
    }
    let known = ["private-members", "hidden-members", "noindex"];
    if let Some(bad) = r.options.iter().find(|x| !known.contains(&x.as_str())) {
        return Err(invalid(
            path,
            format!(
                "rust_autoapi_options lists {bad:?}, which is not private-members, \
                 hidden-members, or noindex."
            ),
            "Remove it, or use one of the options listed.",
        ));
    }
    check_api_root(path, "rust_autoapi_root", &r.root, "rustapi")?;
    for written in &r.crates {
        let dir = source_path(config, written);
        if !dir.exists() {
            let title = "RUST CRATE FOLDER MISSING";
            warnings.push(missing_folder(path, "config.rust", title, "rust_autoapi_crates", &dir));
        }
    }
    Ok(())
}

/// Checks the autodoc settings that take one of a few values, and warns about missing
/// source folders when autodoc is on.
fn check_autodoc(config: &Config, path: &str, warnings: &mut Vec<Report>) -> Result<(), Problem> {
    let a = &config.autodoc;
    let choices = [
        ("autoclass_content", &a.autoclass_content, "class init both"),
        ("autodoc_class_signature", &a.class_signature, "mixed separated"),
        ("autodoc_member_order", &a.member_order, "alphabetical bysource groupwise"),
        ("autodoc_typehints", &a.typehints, "signature description none both"),
        (
            "autodoc_typehints_description_target",
            &a.typehints_description_target,
            "all documented documented_params",
        ),
        ("autodoc_typehints_format", &a.typehints_format, "short fully-qualified"),
    ];
    for (name, value, allowed) in choices {
        if !allowed.split(' ').any(|w| w == value) {
            return Err(invalid(
                path,
                format!("{name} is {value:?}, which is not one of {allowed}."),
                "Use one of the values listed.",
            ));
        }
    }
    if !config.has_extension("sphinx.ext.autodoc") {
        return Ok(());
    }
    for dir in autodoc_roots(config) {
        if !dir.is_dir() {
            let title = "AUTODOC SOURCE PATH MISSING";
            let setting = "autodoc_source_paths";
            warnings.push(missing_folder(path, "config.autodoc", title, setting, &dir));
        }
    }
    Ok(())
}

/// The folders autodoc reads Python packages from: `autodoc_source_paths`, relative to
/// the source folder, cleaned.
pub fn autodoc_roots(config: &Config) -> Vec<PathBuf> {
    config.autodoc.source_paths.iter().map(|p| source_path(config, p)).collect()
}

fn check_myst(config: &Config, path: &str) -> Result<(), Problem> {
    for name in &config.myst.enable_extensions {
        if MYST_EXTENSIONS.contains(&name.as_str()) {
            continue;
        }
        let best = MYST_EXTENSIONS
            .iter()
            .map(|m| (gd_report::edit_distance(name, m), *m))
            .min()
            .filter(|(d, _)| *d <= 3);
        let hint = match best {
            Some((_, m)) => format!("Did you mean {m}?"),
            None => format!("Use {}.", MYST_EXTENSIONS.join(", ")),
        };
        return Err(invalid(
            path,
            format!(
                "myst_enable_extensions lists {name}, which is not one of myst-parser's \
                 optional syntaxes."
            ),
            &hint,
        ));
    }
    let d = &config.myst.sub_delimiters;
    if d.len() != 2 || d.iter().any(|c| c.chars().count() != 1) {
        return Err(invalid(
            path,
            "myst_sub_delimiters must be two single characters.".into(),
            "Write two characters, each doubled around a name: [\"{\", \"}\"] gives {{ name }}.",
        ));
    }
    Ok(())
}

/// Replaces a code style Guidedog does not have with its fallback, with a warning.
fn check_pygments(config: &mut Config, path: &str, warnings: &mut Vec<Report>) {
    let styles =
        [(&mut config.pygments_style, "sphinx"), (&mut config.pygments_dark_style, "github-dark")];
    for (name, fallback) in styles {
        if name.is_empty() {
            continue;
        }
        let Some(problem) = gd_highlight::check_style(name) else { continue };
        let guess = problem.suggestion.map(|s| format!("Did you mean {s}? ")).unwrap_or_default();
        warnings.push(warning(
            path,
            0,
            "config.pygments",
            "CODE STYLE REPLACED",
            format!("{name} is not one of Guidedog's code styles, so {fallback} is used instead."),
            &format!(
                "{guess}Use one of Pygments' built-in styles, such as default, friendly, \
                 monokai, github-dark, or solarized-dark, or Sphinx's sphinx or none."
            ),
        ));
        *name = fallback.to_string();
    }
}

fn check_documents(config: &Config, path: &str) -> Result<(), Problem> {
    if config.source_suffix.iter().any(|s| s.eq_ignore_ascii_case(".typ")) {
        return Err(invalid(
            path,
            "source_suffix lists .typ, but Typst files are not documents: documents are \
             reStructuredText and Markdown."
                .into(),
            "Typst shapes the PDF: edit _templates/book.typ, set pdf_preamble, or put Typst \
             in a document with .. raw:: typst.",
        ));
    }
    Ok(())
}

fn check_graphviz(
    config: &mut Config,
    path: &str,
    warnings: &mut Vec<Report>,
) -> Result<(), Problem> {
    match config.graphviz.output_format.as_str() {
        "svg" => Ok(()),
        "png" => {
            warnings.push(warning(
                path,
                0,
                "config.graphviz",
                "GRAPHS ARE WRITTEN AS SVG",
                "graphviz_output_format = \"png\" builds SVG graphs, which stay sharp at any \
                 size and embed in the PDF book."
                    .into(),
                "Set graphviz_output_format = \"svg\".",
            ));
            config.graphviz.output_format = "svg".into();
            Ok(())
        }
        _ => Err(invalid(
            path,
            "graphviz_output_format must be svg.".into(),
            "Remove the setting; graphs are written as SVG.",
        )),
    }
}

fn check_pdf(config: &Config, path: &str) -> Result<(), Problem> {
    let p = &config.pdf;
    if p.packages != "download" && p.packages != "offline" {
        return Err(invalid(
            path,
            "pdf_packages must be download or offline.".into(),
            "Use pdf_packages = \"offline\" to build only with packages already on disk.",
        ));
    }
    if p.fonts != "system" && p.fonts != "embedded" {
        return Err(invalid(
            path,
            "pdf_fonts must be system or embedded.".into(),
            "Use pdf_fonts = \"embedded\" for byte-reproducible PDFs.",
        ));
    }
    Ok(())
}

fn check_toc(config: &Config, path: &str) -> Result<(), Problem> {
    match config.toc_object_entries_show_parents.as_str() {
        "domain" | "hide" | "all" => Ok(()),
        _ => Err(invalid(
            path,
            "toc_object_entries_show_parents must be domain, hide, or all.".into(),
            "Use \"hide\" to show objects by their own names only.",
        )),
    }
}

/// The names `gettext_additional_targets` may list.
pub const GETTEXT_TARGETS: &[&str] = &["index", "literal-block", "doctest-block", "raw", "image"];

/// The fields `figure_language_filename` may use.
pub const FIGURE_FIELDS: &[&str] = &["root", "path", "docpath", "basename", "ext", "language"];

/// Refuses internationalization settings Guidedog cannot follow.
fn check_intl(config: &Config, path: &str) -> Result<(), Problem> {
    let i = &config.intl;
    if let Some(name) =
        i.gettext_additional_targets.iter().find(|n| !GETTEXT_TARGETS.contains(&n.as_str()))
    {
        return Err(invalid(
            path,
            format!("gettext_additional_targets lists {name}, which is not a target."),
            "Use index, literal-block, doctest-block, raw, or image.",
        ));
    }
    if let FlagOrName::Name(name) = &i.translation_progress_classes
        && name != "translated"
        && name != "untranslated"
    {
        return Err(invalid(
            path,
            format!("translation_progress_classes is {name}."),
            "Use true, false, \"translated\", or \"untranslated\".",
        ));
    }
    if let Err(why) = check_figure_format(&i.figure_language_filename) {
        return Err(invalid(
            path,
            format!("figure_language_filename: {why}."),
            "Use the fields {root}, {path}, {docpath}, {basename}, {ext}, and {language}.",
        ));
    }
    Ok(())
}

/// Checks a `figure_language_filename` format: `{{` and `}}` are braces, and every
/// `{field}` is one of [`FIGURE_FIELDS`]. Fails with why it is not.
pub fn check_figure_format(format: &str) -> Result<(), String> {
    let mut rest = format;
    while let Some(i) = rest.find(['{', '}']) {
        let doubled = rest[i + 1..].starts_with(&rest[i..=i]);
        if doubled {
            rest = &rest[i + 2..];
            continue;
        }
        if rest.as_bytes()[i] == b'}' {
            rest = &rest[i + 1..];
            continue;
        }
        let Some(close) = rest[i..].find('}') else {
            return Err("a \"{\" is not closed".to_string());
        };
        let field = &rest[i + 1..i + close];
        if !FIGURE_FIELDS.contains(&field) {
            return Err(format!("{{{field}}} is not a field of figure_language_filename"));
        }
        rest = &rest[i + close + 1..];
    }
    Ok(())
}
