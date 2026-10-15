//! sphinx.ext.mathjax's scripts: math is rendered for MathJax by gd-link
//! (`MathStyle::Sphinx` with `mathjax_inline` and `mathjax_display`), and a page with
//! math loads MathJax as Sphinx's `install_mathjax` adds it: `mathjax3_config` and
//! `mathjax4_config` as `window.MathJax`, the file `mathjax_config_path` names, then
//! `mathjax_path`, deferred unless `mathjax_options` says otherwise.

use gd_conf::Config;
use gd_jinja::Value;

use crate::escape::push_escaped;

/// The class the renderer gives math written for MathJax.
const MATH_CLASS: &str = "class=\"math notranslate nohighlight\"";

/// Appends MathJax's scripts to a page's `js_files` when its `body` has math. A local
/// `mathjax_path` is relative to the output root, reached from the page by `root`
/// (`"../"`); a URL or an absolute path is kept as written. The configuration file is
/// read when the page is laid out; one that cannot be read is left out (gd-conf checked
/// it when conf.toml loaded).
pub(crate) fn mathjax_scripts(config: &Config, files: &Value, body: &str, root: &str) {
    if !body.contains(MATH_CLASS) {
        return;
    }
    let m = &config.mathjax;
    for json in [&m.v3_config, &m.v4_config] {
        if json.is_empty() {
            continue;
        }
        let entry = Value::dict();
        entry.set("body", Value::from(format!("window.MathJax = {json}")));
        files.push(entry);
    }
    if !m.config_path.is_empty()
        && let Ok(text) = std::fs::read_to_string(config.source_dir.join(&m.config_path))
    {
        let entry = Value::dict();
        entry.set("body", Value::from(text));
        files.push(entry);
    }
    let mut url = m.path.clone();
    if !url.contains("://") && !url.starts_with('/') {
        url = format!("{root}{url}");
    }
    let attributes = Value::dict();
    for (name, value) in &m.options {
        attributes.set(name, Value::from(value.as_str()));
    }
    if !m.options.contains_key("async") && !m.options.contains_key("defer") {
        attributes.set("defer", Value::from("defer")); // MathJax 3 and later load deferred.
    }
    let entry = Value::dict();
    entry.set("url", Value::from(url));
    entry.set("attributes", attributes);
    files.push(entry);
}

/// A `js_files` entry as Sphinx's `js_tag` writes it: its attributes in name order,
/// then `src`, and an inline body. An entry without attributes that loads a file loads
/// it deferred.
pub(crate) fn script_tag(entry: &Value, url: &str) -> String {
    let mut out = String::from("<script");
    let attributes = entry.get("attributes");
    if attributes.is_undefined() {
        if !url.is_empty() {
            out.push_str(" defer");
        }
    } else {
        let mut names = attributes.keys();
        names.sort();
        for name in names {
            out.push(' ');
            push_escaped(&mut out, &name);
            let value = gd_jinja::to_str(&attributes.get(&name)).into_owned();
            if value.is_empty() {
                continue;
            }
            out.push_str("=\"");
            push_escaped(&mut out, &value);
            out.push('"');
        }
    }
    if !url.is_empty() {
        out.push_str(" src=\"");
        push_escaped(&mut out, url);
        out.push('"');
    }
    out.push('>');
    let body = entry.get("body");
    if !body.is_undefined() {
        out.push_str(&gd_jinja::to_str(&body));
    }
    out.push_str("</script>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A local path follows the page's depth; a URL and an
    /// absolute path stay as written.
    #[test]
    fn local_mathjax_paths_follow_nested_pages_and_external_urls_stay_absolute() {
        let mut config = Config::new("/src", "/src/_build");
        let body = "<span class=\"math notranslate nohighlight\">x</span>";
        for path in ["_static/math.js", "https://example.org/math.js", "/math.js"] {
            config.mathjax.path = path.to_string();
            let files = Value::list([]);
            mathjax_scripts(&config, &files, body, "../../");
            assert_eq!(files.len(), 1);
            let Value::List(list) = &files else { panic!("not a list") };
            let url = list.item(0).map(|e| e.get("url")).unwrap_or_default();
            let expected =
                if path.starts_with("_static/") { format!("../../{path}") } else { path.into() };
            assert_eq!(gd_jinja::to_str(&url), expected);
        }
    }

    #[test]
    fn pages_without_math_load_no_mathjax() {
        let config = Config::new("/src", "/src/_build");
        let files = Value::list([]);
        mathjax_scripts(&config, &files, "<p>no math</p>", "");
        assert!(files.is_empty());
    }

    #[test]
    fn script_tags_sort_attributes_and_defer_plain_files() {
        let entry = Value::dict();
        entry.set("url", Value::from("a.js"));
        assert_eq!(script_tag(&entry, "a.js"), "<script defer src=\"a.js\"></script>");
        let attributes = Value::dict();
        attributes.set("defer", Value::from("defer"));
        attributes.set("async", Value::from(""));
        entry.set("attributes", attributes);
        assert_eq!(
            script_tag(&entry, "a.js"),
            "<script async defer=\"defer\" src=\"a.js\"></script>"
        );
        let inline = Value::dict();
        inline.set("body", Value::from("x = 1"));
        assert_eq!(script_tag(&inline, ""), "<script>x = 1</script>");
    }
}
