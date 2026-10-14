//! The fake crate under `tests/data/shapes` documented end to end: every item kind,
//! docs with code, intra-doc links, `cfg`, re-exports, private items, and modules both
//! inline and in files.

use gd_rustdoc::{Crate, Options, Page, document, load_crate};
use std::path::Path;

const SHAPES: &str = "tests/data/shapes";

fn load(options: &Options) -> Crate {
    match load_crate(Path::new(SHAPES), options) {
        Ok(krate) => krate,
        Err(error) => panic!("the fixture loads:\n{error}"),
    }
}

fn page<'p>(pages: &'p [Page], path: &str) -> &'p Page {
    match pages.iter().find(|p| p.path == path) {
        Some(page) => page,
        None => panic!(
            "no page {path}; pages: {:?}",
            pages.iter().map(|p| &p.path).collect::<Vec<_>>()
        ),
    }
}

fn default_pages() -> Vec<Page> {
    let options = Options::default();
    document(&load(&options), &options)
}

#[test]
fn loads_name_modules_and_no_warnings() {
    let krate = load(&Options::default());
    assert_eq!(krate.name, "fake_shapes");
    assert!(krate.warnings.is_empty(), "{:?}", krate.warnings);
    let paths: Vec<String> = (0..krate.module_count())
        .map(|i| krate.module_path(gd_rustdoc::ModuleId(i as u32)))
        .collect();
    for expected in [
        "fake_shapes::geometry::point",
        "fake_shapes::storage::pixel",
        "fake_shapes::inline",
        "fake_shapes::tests",
    ] {
        assert!(paths.iter().any(|p| p == expected), "{expected} in {paths:?}");
    }
}

#[test]
fn pages_follow_public_modules() {
    let pages = default_pages();
    let names: Vec<&str> = pages.iter().map(|p| p.path.as_str()).collect();
    assert_eq!(
        names,
        [
            "index.rst",
            "fake_shapes.rst",
            "fake_shapes/geometry.rst",
            "fake_shapes/inline.rst",
            "fake_shapes/geometry/point.rst"
        ]
    );
    let index = &page(&pages, "index.rst").text;
    assert!(index.contains("   fake_shapes/geometry/point\n"), "{index}");
    assert!(
        index.contains("- :rust:mod:`fake_shapes::geometry` -- Geometry: points and areas.\n")
    );
}

#[test]
fn crate_page_has_docs_and_groups() {
    let pages = default_pages();
    let text = &page(&pages, "fake_shapes.rst").text;
    let expected_start = "fake_shapes\n===========\n\n.. rust:crate:: fake_shapes\n   \
        :synopsis: Shapes and the Canvas they are drawn on.\n\nShapes and the \
        :rust:struct:`Canvas <fake_shapes::Canvas>` they are drawn on.\n\n\
        .. rubric:: Examples\n\n.. code-block:: rust\n\n   let canvas = Canvas::new(4, 4);\n";
    assert!(text.starts_with(expected_start), "{text}");
    assert!(text.contains("Shapes from the README.\n"), "include_str! docs");
    let order = [
        "Re-exports",
        "Modules",
        "Macros",
        "Structs",
        "Enums",
        "Unions",
        "Traits",
        "Functions",
        "Type Aliases",
        "Constants",
        "Statics",
    ];
    let positions: Vec<usize> = order
        .iter()
        .map(|g| text.find(&format!(".. rubric:: {g}\n")).unwrap_or(usize::MAX))
        .collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "{positions:?}\n{text}");
}

#[test]
fn private_module_items_are_inlined_where_re_exported() {
    let pages = default_pages();
    let text = &page(&pages, "fake_shapes.rst").text;
    assert!(text.contains(".. rust:struct:: pub struct Canvas\n\n   A grid of pixels.\n"));
    assert!(text.contains(".. rust:struct:: pub struct Dot(pub u32)\n\n   One pixel.\n"));
    assert!(
        text.contains(
            "Links to :rust:struct:`Pixel <fake_shapes::Dot>` and \
        :rust:fn:`Canvas::width <fake_shapes::Canvas::width>`."
        ),
        "{text}"
    );
    assert!(text.contains(
        ".. rust:use:: pub use geometry::point::Point\n   \
        :target: fake_shapes::geometry::point::Point\n"
    ));
    assert!(text.contains(".. rust:use:: pub use std::collections::HashMap\n\n"));
    assert!(!text.contains("hidden_helper") && !text.contains("private_helper"));
    assert!(!text.contains("works"), "cfg(test) items are left out");
}

#[test]
fn every_item_kind_has_its_directive() {
    let pages = default_pages();
    let text = &page(&pages, "fake_shapes.rst").text;
    for line in [
        ".. rust:macro:: macro_rules! shape\n",
        "      macro_rules! shape {\n          ($kind:ident) => { ... };\n",
        ".. rust:enum:: pub enum Kind\n   :attributes: #[non_exhaustive]\n",
        "   .. rust:variant:: Polygon(u8)\n",
        "      .. rust:field:: 0: u8\n\n         The side count.\n",
        "   .. rust:variant:: Rect { width: f32, height: f32 }\n",
        "   .. rust:variant:: Unknown = 9\n",
        ".. rust:union:: pub union Bits\n   :attributes: #[repr(C)]\n",
        "   .. rust:field:: pub bytes: [u8; 4]\n",
        ".. rust:trait:: pub trait Draw: Clone + Send\n",
        "   .. rust:associatedtype:: type Output: Default\n",
        "   .. rust:associatedconst:: const PASSES: usize = 1\n      :provided:\n",
        "   .. rust:method:: fn draw(&self, canvas: &mut Canvas) \
            -> Result<Self::Output, String>\n\n",
        "   .. rust:method:: fn name(&self) -> &str\n      :provided:\n",
        ".. rust:function:: pub unsafe fn draw_all<T: Draw + 'static>(ptr: *const T, \
            count: usize) -> Result<()> where T: Sync\n   :unsafe:\n",
        ".. rust:function:: pub async fn draw_later()\n   :async:\n   :deprecated: use draw_all\n",
        ".. rust:function:: pub unsafe extern \"C\" fn c_draw(x: i32) -> i32\n   :unsafe:\n",
        ".. rust:type:: pub type Result<T> = std::result::Result<T, String>\n",
        ".. rust:const:: pub const MAX_SIDE: usize = 4_096\n",
        ".. rust:static:: pub static mut DRAWN: u64\n",
    ] {
        assert!(text.contains(line), "missing {line:?} in\n{text}");
    }
}

#[test]
fn links_resolve_to_documented_paths() {
    let pages = default_pages();
    let text = &page(&pages, "fake_shapes.rst").text;
    assert!(
        text.contains(
            "See :rust:variant:`kinds <fake_shapes::Kind::Circle>` and \
        :rust:fn:`crate::geometry::area <fake_shapes::geometry::area>`."
        ),
        "{text}"
    );
    assert!(
        text.contains(
            ":rust:fn:`Draw::draw <fake_shapes::Draw::draw>` and \
        :rust:macro:`shape! <fake_shapes::shape>`."
        ),
        "{text}"
    );
    assert!(page(&pages, "fake_shapes.rst").warnings.is_empty());
}

#[test]
fn impls_attach_to_their_types() {
    let pages = default_pages();
    let text = &page(&pages, "fake_shapes/geometry/point.rst").text;
    let expected = "   .. rust:impl:: impl<T: Copy> Point<T>\n      :noindex:\n      \
        :for: Point<T>\n\n      .. rust:method:: pub fn new(x: T, y: T) -> Self\n\n         \
        Makes a point at ``(x, y)``; see \
        :rust:field:`Self::x <fake_shapes::geometry::point::Point::x>`.\n";
    assert!(text.contains(expected), "{text}");
    assert!(
        text.contains(
            "   .. rust:impl:: impl<T: fmt::Display> fmt::Display for Point<T>\n\
        \x20     :noindex:\n      :for: Point<T>\n      :trait: fmt::Display\n"
        ),
        "{text}"
    );
    assert!(text.contains("         :noindex:\n"), "trait impl methods are not indexed");
    assert!(!text.contains("fn private"), "{text}");
    assert!(!text.contains("hidden: ()"), "{text}");
    assert!(text.contains(".. rust:struct:: pub struct Pair(pub i32, _)\n"), "{text}");
}

#[test]
fn markdown_blocks_become_rest() {
    let pages = default_pages();
    let text = &page(&pages, "fake_shapes/geometry.rst").text;
    let expected = "   - one\n   - two, with *emphasis* continued\n\n   1. first\n   \
        2. second\n\n   \
        .. list-table::\n      :header-rows: 1\n\n      * - a\n        - b\n      * - 1\n        \
        - ``2``\n\n   .. code-block:: text\n\n      not rust\n\n   ..\n\n      quoted\n";
    assert!(text.contains(expected), "{text}");
}

#[test]
fn broken_and_private_links_warn() {
    let pages = default_pages();
    let geometry = page(&pages, "fake_shapes/geometry.rst");
    assert!(
        geometry.text.contains(
            "Broken link to :rust:any:`crate::nowhere` and private \
        ``helper``."
        ),
        "{}",
        geometry.text
    );
    let codes: Vec<&str> = geometry.warnings.iter().map(|w| w.code).collect();
    assert_eq!(codes, ["rust.autodoc.link", "rust.autodoc.private-link"]);
    let shown = geometry.warnings[0].to_string();
    assert!(shown.contains("geometry.rs:28:1"), "{shown}");
    assert!(shown.contains("Hint: "), "{shown}");
}

#[test]
fn cfg_is_an_option() {
    let pages = default_pages();
    let text = &page(&pages, "fake_shapes/inline.rst").text;
    assert!(
        text.contains(
            ".. rust:module:: fake_shapes::inline\n   :synopsis: An inline \
        module.\n   :cfg: feature = \"inline\"\n"
        ),
        "{text}"
    );
    assert!(text.contains(".. rust:function:: pub const fn double(x: u32) -> u32\n   :const:\n"));
}

#[test]
fn private_items_on_request() {
    let options = Options { include_private: true, document_hidden: true, ..Options::default() };
    let pages = document(&load(&options), &options);
    let names: Vec<&str> = pages.iter().map(|p| p.path.as_str()).collect();
    assert!(names.contains(&"fake_shapes/storage.rst"), "{names:?}");
    assert!(!names.contains(&"fake_shapes/tests.rst"), "{names:?}");
    let root = &page(&pages, "fake_shapes.rst").text;
    assert!(root.contains(".. rust:function:: fn private_helper()\n   :visibility: private\n"));
    assert!(root.contains(".. rust:function:: pub fn hidden_helper()\n"));
    let point = &page(&pages, "fake_shapes/geometry/point.rst").text;
    assert!(point.contains(".. rust:method:: fn private(&mut self)\n"), "{point}");
}

#[test]
fn alphabetical_order_and_noindex() {
    let options = Options {
        member_order: gd_rustdoc::MemberOrder::Alphabetical,
        noindex: true,
        ..Options::default()
    };
    let pages = document(&load(&options), &options);
    let text = &page(&pages, "fake_shapes.rst").text;
    let c_draw = text.find("fn c_draw").unwrap_or(usize::MAX);
    let draw_all = text.find("fn draw_all").unwrap_or(0);
    assert!(c_draw < draw_all);
    assert!(
        text.contains(
            ".. rust:crate:: fake_shapes\n   :synopsis: Shapes and the Canvas \
        they are drawn on.\n   :noindex:\n"
        ),
        "{text}"
    );
}

#[test]
fn item_limit_warns() {
    let options = Options { max_items: 10, ..Options::default() };
    let krate = load(&options);
    assert!(krate.item_count() <= 10);
    assert!(krate.warnings.iter().any(|w| w.code == "rust.autodoc.limit"));
}

#[test]
fn missing_root_is_an_error() {
    let error = load_crate(Path::new("tests/data/no-such-crate"), &Options::default());
    let Err(error) = error else { panic!("a missing crate loads") };
    assert_eq!(error.diagnostic.code, "rust.autodoc.crate");
    assert!(error.to_string().contains("Hint: "));
}

#[test]
fn problems_below_the_root_are_warnings() {
    let options = Options { include_private: true, ..Options::default() };
    let krate = match load_crate(Path::new("tests/data/broken/lib.rs"), &options) {
        Ok(krate) => krate,
        Err(error) => panic!("{error}"),
    };
    let codes: Vec<&str> = krate.warnings.iter().map(|w| w.code).collect();
    assert_eq!(codes, ["rust.autodoc.parse", "rust.autodoc.module"], "{:?}", krate.warnings);
    let missing = krate.warnings[1].to_string();
    assert!(missing.contains("lib.rs:3:1") && missing.contains("3| mod missing;"), "{missing}");
    let uses: Vec<&str> = krate
        .items
        .iter()
        .filter(|i| i.kind == gd_rustdoc::ItemKind::Use)
        .map(|i| i.use_path.as_str())
        .collect();
    assert_eq!(uses, ["self::a::B"]);
    let pages = document(&krate, &options);
    assert!(page(&pages, "broken.rst").text.contains(".. rust:function:: pub fn fine()\n"));
}

#[test]
fn deep_files_are_left_out() {
    let options = Options { max_depth: 4, ..Options::default() };
    let krate = load(&options);
    assert!(krate.warnings.iter().any(|w| w.code == "rust.autodoc.nesting"));
    assert!(krate.module(gd_rustdoc::ModuleId(0)).items.is_empty());
}
