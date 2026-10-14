//! gd-rustdoc over real crates of this workspace: they load without errors and every
//! module page is written.

use gd_rustdoc::{Options, document, load_crate};
use std::path::Path;

fn check(path: &str, expected_page: &str) {
    let options = Options::default();
    let krate = match load_crate(Path::new(path), &options) {
        Ok(krate) => krate,
        Err(error) => panic!("{path} loads:\n{error}"),
    };
    let parse_errors: Vec<_> =
        krate.warnings.iter().filter(|w| w.code == "rust.autodoc.parse").collect();
    assert!(parse_errors.is_empty(), "{parse_errors:?}");
    let pages = document(&krate, &options);
    assert!(pages.iter().any(|p| p.path == expected_page), "{expected_page} in {path}");
    for page in &pages {
        assert!(page.text.ends_with('\n'), "{}", page.path);
    }
}

#[test]
fn documents_itself() {
    check(".", "gd_rustdoc.rst");
    let pages = gd_rustdoc::document_crate(Path::new(".")).unwrap_or_default();
    let root = pages.iter().find(|p| p.path == "gd_rustdoc.rst").map(|p| p.text.as_str());
    let root = root.unwrap_or_default();
    assert!(
        root.contains(
            ".. rust:function:: pub fn load_crate(path: &Path, options: &Options) \
        -> Result<Crate, Error>\n"
        ),
        "{root}"
    );
    assert!(root.contains(".. rust:struct:: pub struct Options\n"));
    assert!(
        pages.iter().all(|p| p.warnings.is_empty()),
        "{:?}",
        pages.iter().flat_map(|p| &p.warnings).collect::<Vec<_>>()
    );
}

#[test]
fn documents_the_check_tool() {
    check("../../tools/check", "check.rst");
}

#[test]
fn documents_a_root_file_and_a_loose_folder() {
    check("tests/data/shapes/src/lib.rs", "src.rst");
    let options = Options::default();
    let krate = load_crate(Path::new("tests/data/shapes/src/geometry"), &options);
    let Ok(krate) = krate else { panic!("a folder of .rs files loads") };
    assert_eq!(krate.name, "geometry");
    assert_eq!(krate.module_path(gd_rustdoc::ModuleId(1)), "geometry::point");
}
