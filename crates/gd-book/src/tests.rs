use std::path::PathBuf;

use crate::compose::{language_file, typst_markup, typst_string};
use crate::typst_root::{make_typst_root, typst_folders};

/// A fresh folder for one test, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("gd-book {name} {}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir.canonicalize().unwrap())
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// Each language's book has its own file, named after the language (moved from intl).
#[test]
fn books_are_named_after_their_language() {
    assert_eq!(language_file("demo.pdf", "de"), "demo-de.pdf");
    assert_eq!(language_file("demo.pdf", ""), "demo-en.pdf");
    assert_eq!(language_file("Manual.pdf", "pt_BR"), "Manual-pt_BR.pdf");
    assert_eq!(language_file("manual-fr.pdf", "fr"), "manual-fr.pdf");
    assert_eq!(language_file("book", "ja"), "book-ja.pdf");
    assert_eq!(language_file("book.typ", "en"), "book-en.typ");
    assert_eq!(language_file("v1.2/book", "de"), "v1.2/book-de.pdf");
}

#[test]
fn typst_strings_and_markup_are_escaped() {
    assert_eq!(typst_string("a \"b\" \\ c\nd\te"), "\"a \\\"b\\\" \\\\ c\\nd e\"");
    assert_eq!(typst_markup("Index #1 [a]"), "Index \\#1 \\[a\\]");
    assert_eq!(typst_markup("Größe"), "Größe");
}

#[test]
fn book_root_failure_never_widens_readable_access() {
    let dir = TempDir::new("root refusal");
    let source = dir.0.join("docs");
    std::fs::create_dir(&source).unwrap();
    // The second link has a taken destination: a refusal after a first successful
    // step, without changing the environment or privileges.
    let problem = make_typst_root(&dir.0, &[source.clone(), source.clone()]).unwrap_err();
    assert_eq!(problem.code, "typst.root");
    assert_eq!(problem.exit, 4);
    assert!(problem.message.contains("Typst was not run"), "{}", problem.message);
    assert!(source.is_dir()); // cleanup never removes an approved folder.
}

#[cfg(unix)]
#[test]
fn the_private_root_links_only_readable_folders_and_goes_away() {
    let dir = TempDir::new("root links");
    let source = dir.0.join("project/docs");
    let shared = dir.0.join("shared");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::write(source.join("a.txt"), "a").unwrap();
    std::fs::write(dir.0.join("secret.txt"), "s").unwrap();
    let root = make_typst_root(&dir.0, &[source.clone(), shared.clone()]).unwrap();
    let made = root.dir().to_path_buf();
    assert!(made.join("project/docs/a.txt").is_file());
    assert!(made.join("shared").is_dir());
    assert!(!made.join("secret.txt").exists());
    drop(root);
    assert!(!made.exists());
    assert!(source.join("a.txt").is_file(), "removing the root never follows a link");
}

#[test]
fn typst_folders_keep_the_outermost() {
    let dir = TempDir::new("folders");
    let mut config = gd_conf::Config::new(dir.0.join("docs"), dir.0.join("docs/_build"));
    config.include_roots = vec!["..".into(), "sub".into()];
    let graphs = gd_env::doctrees_dir(&config).join("graphs");
    std::fs::create_dir_all(&graphs).unwrap();
    assert_eq!(typst_folders(&config), std::slice::from_ref(&dir.0));
    config.include_roots.clear();
    assert_eq!(typst_folders(&config), [dir.0.join("docs")]);
    let outside = gd_conf::Config::new(dir.0.join("src"), dir.0.join("build"));
    std::fs::create_dir_all(gd_env::doctrees_dir(&outside).join("graphs")).unwrap();
    let folders = typst_folders(&outside);
    assert_eq!(folders.len(), 2, "{folders:?}");
}
