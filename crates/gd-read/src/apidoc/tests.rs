//! The pages of Sphinx's apidoc test roots, compared with what Sphinx 9.1's
//! `recurse_tree` and `create_modules_toc_file` write for them
//! (`tests/data/apidoc-*.txt`, one `===== name` header per page).

use super::*;

/// Makes the files of `tree` (paths relative to a fresh folder) and returns the folder.
fn make_tree(tag: &str, tree: &[&str]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gd-read-apidoc-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for file in tree {
        let path = dir.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "").unwrap();
    }
    dir
}

/// The pages Sphinx wrote, by name.
fn expected(file: &str) -> Vec<(String, String)> {
    let path = format!("{}/tests/data/{file}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(path).unwrap();
    let mut pages: Vec<(String, String)> = Vec::new();
    for line in text.split_inclusive('\n') {
        match line.strip_prefix("===== ") {
            Some(name) => pages.push((name.trim().to_owned(), String::new())),
            None => pages.last_mut().unwrap().1.push_str(line),
        }
    }
    pages.sort();
    pages
}

fn settings(path: PathBuf) -> Settings {
    Settings {
        path,
        destination: "api".into(),
        excludes: Vec::new(),
        max_depth: 4,
        follow_links: false,
        separate_modules: false,
        include_private: false,
        no_headings: false,
        module_first: false,
        implicit_namespaces: false,
        automodule_options: ["members", "show-inheritance", "undoc-members"]
            .map(String::from)
            .to_vec(),
    }
}

fn pages_of(s: &Settings) -> Vec<(String, String)> {
    let mut reports = Vec::new();
    let mut pages = walk::package_pages(s, &templates(), &mut reports);
    assert!(reports.is_empty(), "{reports:?}");
    for (_, text) in &mut pages {
        // Sphinx's pages end where the template ends; the data file adds the newline.
        if !text.ends_with('\n') {
            text.push('\n');
        }
    }
    pages.sort();
    pages
}

#[test]
fn a_package_with_subpackages() {
    let dir = make_tree(
        "toc",
        &[
            "mypackage/__init__.py",
            "mypackage/main.py",
            "mypackage/something/__init__.py",
            "mypackage/resource/__init__.py",
            "mypackage/resource/resource.txt",
            "mypackage/no_init/foo.py",
        ],
    );
    assert_eq!(pages_of(&settings(dir.join("mypackage"))), expected("apidoc-toc.txt"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn implicit_namespace_packages() {
    let dir = make_tree(
        "pep420",
        &["a/b/x/y.py", "a/b/e/f.py", "a/b/e/__init__.py", "a/b/c/d.py", "a/b/c/__init__.py"],
    );
    let s =
        Settings { implicit_namespaces: true, separate_modules: true, ..settings(dir.join("a")) };
    assert_eq!(pages_of(&s), expected("apidoc-pep420.txt"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_folder_of_modules_with_excludes() {
    let dir = make_tree("ext", &["src/my_package.py", "src/exclude_package.py"]);
    let s = Settings {
        include_private: true,
        separate_modules: true,
        module_first: true,
        max_depth: 3,
        automodule_options: vec!["members".into(), "undoc-members".into()],
        excludes: vec![dir.join("src/exclude_package.py").to_string_lossy().into_owned()],
        ..settings(dir.join("src"))
    };
    assert_eq!(pages_of(&s), expected("apidoc-ext.txt"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn package_roots_are_the_folders_above_packages() {
    let dir = make_tree("roots", &["src/pkg/__init__.py", "mods/m.py"]);
    let mut c = Config::new(dir.join("docs"), dir.join("docs/_build"));
    c.config_dir = dir.join("docs");
    c.extensions = vec!["sphinx.ext.apidoc".into()];
    for path in ["../src/pkg", "../mods"] {
        c.ext.apidoc.modules.push(ApidocModule {
            path: path.into(),
            destination: "api".into(),
            ..ApidocModule::default()
        });
    }
    assert_eq!(package_roots(&c), [dir.join("src"), dir.join("mods")]);
    std::fs::remove_dir_all(dir).unwrap();
}
