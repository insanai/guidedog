use std::path::{Path, PathBuf};

use super::*;

/// A temporary site folder, removed when dropped.
struct Site(PathBuf);

impl Site {
    fn new(name: &str) -> Site {
        let dir = std::env::temp_dir().join(format!("linkcheck-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a temporary folder");
        Site(dir)
    }

    fn put(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap_or(Path::new("."))).expect("a folder");
        fs::write(path, text).expect("a page");
    }
}

impl Drop for Site {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn broken_files_and_anchors_are_found() {
    let site = Site::new("broken");
    site.put(
        "index.html",
        concat!(
            r##"<a href="sub/a.html#x">a</a> <a href="https://e.org/">e</a>"##,
            r##"<a href="sub/a.html#nope">b</a> <a href="gone.html">c</a> <a href="#top">t</a>"##,
            r##"<a href="sub/">d</a> <a href="mailto:x@y">m</a><p id="top"></p>"##,
        ),
    );
    site.put("sub/a.html", r##"<h1 id="x">A</h1><a href="../index.html?q=1#top">up</a>"##);
    site.put("sub/index.html", r#"<a name="n"></a><a href="a.html">a</a>"#);
    site.put("sub/_downloads/example/layout.html", r#"<a href="{{ pathto('guide') }}">s</a>"#);
    site.put(
        "sub/downloads.html",
        concat!(
            r#"<a href="_downloads/example/layout.html">template</a>"#,
            r#"<a href="_downloads/example/missing.html">missing attachment</a>"#,
        ),
    );
    let outcome = check_site(&site.0);
    assert_eq!(outcome.pages, 4);
    assert_eq!(outcome.links, 9);
    assert_eq!(
        outcome.missing_files,
        ["index.html -> gone.html", "sub/downloads.html -> _downloads/example/missing.html"]
    );
    assert_eq!(outcome.missing_anchors, ["index.html -> sub/a.html#nope"]);
}

#[test]
fn a_base_element_starts_links_at_the_root() {
    let site = Site::new("base");
    site.put("index.html", "<p>home</p>");
    site.put("manual/index.html", "<p>manual</p>");
    site.put(
        "404.html",
        r#"<base href="/site/"><a href="manual/index.html">m</a><a href="index.html">h</a>"#,
    );
    let outcome = check_site(&site.0);
    assert_eq!(outcome.links, 2);
    assert!(outcome.missing_files.is_empty());
}

#[test]
fn hrefs_resolve_from_their_page() {
    assert_eq!(
        resolve_href("a/b/c.html", "../d.html#f"),
        Some(("a/d.html".to_owned(), "f".to_owned()))
    );
    assert_eq!(resolve_href("a.html", "//cdn.example/x.js"), None);
    assert_eq!(resolve_href("a/b.html", "/c.html"), Some(("c.html".to_owned(), String::new())));
}
