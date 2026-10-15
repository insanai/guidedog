//! The site unit (html_site.odin's `write_site_files` and `index_pages`, plus
//! additional_pages.odin and plugins.odin's `write_plugin_assets`): everything besides
//! the document pages. It is written every build; its files are compared byte for
//! byte, so only changed ones are staged.

use std::collections::BTreeMap;
use std::path::PathBuf;

use gd_env::Asset;
use gd_host::Problem;
use gd_outputs::Unit;
use gd_report::{Report, Severity};
use gd_resolve::{BuildView, create_index, has_module_index, module_index, write_inventory};

use crate::context::base_name;
use crate::index_pages::{
    genindex_body, genindex_single_body, genindex_split_body, modindex_body, search_body,
    split_index_page,
};
use crate::site::Site;

/// The guidedog theme's code styles, used when conf.toml sets none, as a Sphinx
/// theme's theme.conf names its `pygments_style` and `pygments_dark_style`.
const THEME_PYGMENTS_STYLE: &str = "default";
const THEME_PYGMENTS_DARK_STYLE: &str = "github-dark";

/// Writes the files every page shares into the site unit: the theme's files and the
/// project's static and extra files, the images and downloads in `assets`, the
/// sources, the plugins' assets, the pages made from the whole project, and the
/// inventory. Warnings go to `reports`.
pub(crate) fn write_site_files(
    site: &Site<'_, '_>,
    unit: &mut Unit<'_>,
    assets: &[Asset],
    reports: &mut Vec<Report>,
) -> Result<(), Problem> {
    let view = site.view;
    write_theme_files(view, unit)?;
    let c = view.config;
    for dir in &c.html.static_path {
        unit.emit_tree(&c.source_dir.join(dir), "_static", view.roots, view.budget, reports)?;
    }
    for dir in &c.html.extra_path {
        unit.emit_tree(&c.source_dir.join(dir), "", view.roots, view.budget, reports)?;
    }
    for file in [&c.html.logo, &c.html.favicon] {
        if file.is_empty() || file.contains("://") {
            continue;
        }
        let target = format!("_static/{}", base_name(file));
        unit.emit_copy(&c.source_dir.join(file), &target, view.budget)?;
    }
    // A page that needs an image or download no longer there was warned about with it.
    let by_target: BTreeMap<&str, &PathBuf> =
        assets.iter().map(|a| (a.target.as_str(), &a.source)).collect();
    for (target, source) in by_target {
        if source.is_file() {
            unit.emit_copy(source, target, view.budget)?;
        }
    }
    if c.html.copy_source {
        write_sources(site, unit)?;
    }
    write_plugin_assets(view, unit)?;
    write_index_pages(site, unit, reports)
}

/// guidedog.css, guidedog.js and pygments.css, unless `html_static_path` has its own.
fn write_theme_files(view: &BuildView<'_>, unit: &mut Unit<'_>) -> Result<(), Problem> {
    let c = view.config;
    let project_has = |name: &str| {
        c.html.static_path.iter().any(|dir| c.source_dir.join(dir).join(name).is_file())
    };
    let pygments = pygments_css(&c.pygments_style, &c.pygments_dark_style);
    let files = [
        ("guidedog.css", gd_theme::GUIDEDOG_CSS),
        ("guidedog.js", gd_theme::GUIDEDOG_JS),
        ("pygments.css", pygments.as_str()),
    ];
    for (name, text) in files {
        if !project_has(name) {
            unit.emit_text(&format!("_static/{name}"), text)?;
        }
    }
    Ok(())
}

/// The stylesheet of highlighted code: `pygments_style`, and `pygments_dark_style`
/// when the page is dark, by the reader's system setting or the theme's switch. An
/// empty name is the theme's style; an unknown one (refused when conf.toml loads) the
/// default style.
pub fn pygments_css(light: &str, dark: &str) -> String {
    let style = |name: &str, theme: &str| {
        let name = if name.is_empty() { theme } else { name };
        gd_highlight::find_style(name).or_else(|| gd_highlight::find_style("default"))
    };
    let mut out = String::new();
    let Some(light) = style(light, THEME_PYGMENTS_STYLE) else { return out };
    let dark = style(dark, THEME_PYGMENTS_DARK_STYLE).unwrap_or(light);
    gd_highlight::write_css(light, ".highlight", &mut out);
    gd_highlight::write_css(dark, ":root[data-theme=\"dark\"] .highlight", &mut out);
    out.push_str("@media (prefers-color-scheme: dark) {\n");
    gd_highlight::write_css(dark, ":root:not([data-theme=\"light\"]) .highlight", &mut out);
    out.push_str("}\n");
    out
}

/// `_sources/<relative><html_sourcelink_suffix>` for every document: a copy of its
/// source, or a generated page's text.
fn write_sources(site: &Site<'_, '_>, unit: &mut Unit<'_>) -> Result<(), Problem> {
    let view = site.view;
    let c = view.config;
    for d in &view.env.docs {
        let target = format!("_sources/{}{}", d.relative, c.html.sourcelink_suffix);
        if d.generated {
            let generated = site.sources.iter().find(|s| s.docname == d.docname);
            if let Some(text) = generated.and_then(|s| s.generated.as_ref()) {
                unit.emit_text(&target, &text.text)?;
            }
            continue;
        }
        unit.emit_copy(&c.source_dir.join(&d.relative), &target, view.budget)?;
    }
    Ok(())
}

/// The plugins' assets, at `_static/plugins/<id>/<path>`. An asset may not take the
/// place of a file the site already has.
fn write_plugin_assets(view: &BuildView<'_>, unit: &mut Unit<'_>) -> Result<(), Problem> {
    for (id, asset) in view.plugins.assets() {
        let rel = format!("_static/plugins/{id}/{}", asset.path);
        if let Some(existing) = unit.files().iter().find(|f| paths_conflict(f, &rel)) {
            return Err(Problem::new(
                "plugins.config",
                "BUILD PLUGIN CONFIGURATION",
                &view.config.config_path().display().to_string(),
                format!("Plugin {id}: Its asset {rel} conflicts with project file {existing}."),
                "Move the project file out of _static/plugins/ID, or rename the plugin asset.",
                2,
            ));
        }
        unit.emit(&rel, &asset.data)?;
    }
    Ok(())
}

/// Whether two output paths name the same file, ignoring case, or one is a folder of
/// the other.
fn paths_conflict(a: &str, b: &str) -> bool {
    let (a, b) = (a.to_lowercase(), b.to_lowercase());
    let (shorter, longer) = if a.len() <= b.len() { (&a, &b) } else { (&b, &a) };
    longer
        .strip_prefix(shorter.as_str())
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// The pages made from the whole project (search, module index, general index, the
/// additional pages), GitHub Pages' files, and the inventory.
fn write_index_pages(
    site: &Site<'_, '_>,
    unit: &mut Unit<'_>,
    reports: &mut Vec<Report>,
) -> Result<(), Problem> {
    let view = site.view;
    let search = view.ui.gettext("Search");
    special(site, unit, "search.html", &search, &search_body(view), reports)?;
    if has_module_index(view) {
        let title = view.ui.gettext("Python Module Index");
        let body = modindex_body(view, &module_index(view));
        special(site, unit, "py-modindex.html", &title, &body, reports)?;
    }
    if view.config.html.use_index {
        write_general_index(site, unit, reports)?;
    }
    write_additional_pages(site, unit, reports)?;
    write_github_pages(view, unit)?;
    let mut inventory = Vec::new();
    write_inventory(view, &mut inventory);
    unit.emit("objects.inv", &inventory)
}

/// Lays out and writes one page at the output root.
fn special(
    site: &Site<'_, '_>,
    unit: &mut Unit<'_>,
    name: &str,
    title: &str,
    body: &str,
    reports: &mut Vec<Report>,
) -> Result<(), Problem> {
    let html = site.special_page(name, title, body, reports)?;
    unit.emit_text(name, &html)
}

/// genindex.html, or with `html_split_index`, as Sphinx does, a page of letters, a
/// page per letter, and genindex-all.html.
fn write_general_index(
    site: &Site<'_, '_>,
    unit: &mut Unit<'_>,
    reports: &mut Vec<Report>,
) -> Result<(), Problem> {
    let view = site.view;
    let groups = create_index(view);
    let index = view.ui.gettext("Index");
    if !view.config.html.split_index {
        return special(
            site,
            unit,
            "genindex.html",
            &index,
            &genindex_body(view, &groups),
            reports,
        );
    }
    let letters = genindex_split_body(view, &groups);
    special(site, unit, "genindex.html", &index, &letters, reports)?;
    for g in &groups {
        let body = genindex_single_body(view, g);
        special(site, unit, &split_index_page(&g.name), &index, &body, reports)?;
    }
    special(site, unit, "genindex-all.html", &index, &genindex_body(view, &groups), reports)
}

/// Each page of `html_additional_pages`, in name order (a `BTreeMap`), written at the
/// output root as `<name><html_file_suffix>` by every HTML builder. A page named as a
/// document replaces that document's page ([`additional_page_replaces`]).
fn write_additional_pages(
    site: &Site<'_, '_>,
    unit: &mut Unit<'_>,
    reports: &mut Vec<Report>,
) -> Result<(), Problem> {
    let c = site.view.config;
    for (name, template) in &c.html.additional_pages {
        let file = format!("{name}{}", c.html.file_suffix);
        if let Some(reason) = additional_page_refusal(unit, name, &file) {
            reports.push(Report {
                severity: Severity::Warning,
                path: c.config_path().display().to_string(),
                ..Report::error(
                    "build.additional_page",
                    "ADDITIONAL PAGE NOT WRITTEN",
                    &format!("html_additional_pages.{name} is not written: {reason}."),
                )
                .hint("Give the page another name.")
            });
            continue;
        }
        let html = site.additional_page(name, &file, template, reports)?;
        unit.emit_text(&file, &html)?;
    }
    Ok(())
}

/// Why a page cannot be written under its name, or `None`. A name stays at the output
/// root, since the page's links are made from there, and a page may not take the place
/// of a file the site already has, such as genindex.html or the single page of
/// singlehtml.
fn additional_page_refusal(unit: &Unit<'_>, name: &str, file: &str) -> Option<String> {
    if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
        return Some(
            "a page's name must be a file name at the output root, such as \"download\""
                .to_string(),
        );
    }
    unit.files().iter().any(|f| f == file).then(|| format!("the site already has {file}"))
}

/// Whether a page of `html_additional_pages` is written in place of a document's page,
/// which is then not written.
pub(crate) fn additional_page_replaces(view: &BuildView<'_>, docname: &str) -> bool {
    let c = view.config;
    if view.kind == gd_env::BuilderKind::Singlehtml || docname.contains(['/', '\\']) {
        return false;
    }
    c.html.additional_pages.contains_key(docname)
        && view.page_name(docname) == format!("{docname}{}", c.html.file_suffix)
}

/// sphinx.ext.githubpages: `.nojekyll`, so GitHub Pages serves the site as it is, and
/// `CNAME` with `html_baseurl`'s host unless that is a github.io one.
fn write_github_pages(view: &BuildView<'_>, unit: &mut Unit<'_>) -> Result<(), Problem> {
    if !view.config.has_extension("sphinx.ext.githubpages") {
        return Ok(());
    }
    unit.emit_text(".nojekyll", "")?;
    let domain = url_host(&view.config.html.baseurl);
    if domain.is_empty() || domain.ends_with(".github.io") {
        return Ok(());
    }
    unit.emit_text("CNAME", &domain)
}

/// A URL's host name, lowercased, as Python's `urlparse` gives it; empty for none.
fn url_host(url: &str) -> String {
    let Some((_, rest)) = url.split_once("://") else { return String::new() };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let authority = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    if let Some(bracketed) = authority.strip_prefix('[') {
        return bracketed.split_once(']').map_or(String::new(), |(host, _)| host.to_lowercase());
    }
    authority.split(':').next().unwrap_or("").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_are_read_as_urlparse_reads_them() {
        assert_eq!(url_host("https://Docs.Example.org/a/"), "docs.example.org");
        assert_eq!(url_host("https://user@x.org:8080/"), "x.org");
        assert_eq!(url_host("http://[::1]:80/"), "::1");
        assert_eq!(url_host("docs.example.org"), "");
    }

    #[test]
    fn plugin_assets_conflict_by_name_or_folder_ignoring_case() {
        assert!(paths_conflict("_static/plugins/a/X.js", "_static/plugins/a/x.js"));
        assert!(paths_conflict("_static/plugins/a", "_static/plugins/a/x.js"));
        assert!(!paths_conflict("_static/plugins/ab", "_static/plugins/a/x.js"));
    }

    #[test]
    fn code_styles_cover_light_and_dark_pages() {
        let css = pygments_css("", "");
        assert!(css.contains(".highlight"));
        assert!(css.contains(":root[data-theme=\"dark\"] .highlight"));
        assert!(css.contains("@media (prefers-color-scheme: dark)"));
    }
}
