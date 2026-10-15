//! The rust domain: the pages gd-rustdoc writes for its `shapes` fixture read without
//! a report, and hand-written directives and roles give the node shapes of
//! docs/manual/internals/rust-domain.rst.

use std::path::{Path, PathBuf};

use gd_rustdoc::{Options, Page, document, load_crate};
use guidedoc::{
    Workspace, default_request, dump_to_string, read_document, read_extension, trusted_policy,
};
use guidedoc_sphinx::{Config, TocParents, reader};

/// Reads `text` with `config`: the dump (empty when the read failed) and the report
/// codes, space-separated.
fn read_with(name: &str, text: &str, config: &Config) -> (String, String) {
    let mut request = default_request(name, text, "sphinx", "html");
    request.read.promote_title = false;
    request.policy = trusted_policy();
    request.read.extension = Some(read_extension(config));
    let mut ws = Workspace::new();
    let dump = read_document(&mut ws, &reader(), &request).map(dump_to_string).unwrap_or_default();
    let codes: Vec<&str> = ws.reports().items().iter().map(|d| d.code).collect();
    (dump, codes.join(" "))
}

fn read_text(text: &str) -> String {
    read_with("test.rst", text, &Config::default()).0
}

/// Checks that `got` holds every fragment, in order.
#[track_caller]
fn expect_in_order(got: &str, fragments: &[&str]) {
    let mut at = 0;
    for f in fragments {
        match got[at..].find(f) {
            Some(k) => at += k + f.len(),
            None => panic!("missing {f:?} (in order); got:\n{got}"),
        }
    }
}

#[track_caller]
fn expect_contains(text: &str, fragments: &[&str]) {
    let (got, codes) = read_with("test.rst", text, &Config::default());
    assert!(!got.is_empty(), "read failed: {codes}");
    expect_in_order(&got, fragments);
}

fn shapes_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../gd-rustdoc/tests/data/shapes")
}

fn shapes_pages(options: &Options) -> Vec<Page> {
    match load_crate(&shapes_dir(), options) {
        Ok(krate) => document(&krate, options),
        Err(error) => panic!("the shapes fixture loads:\n{error}"),
    }
}

/// Reads one generated page, which must give no report.
fn read_page(pages: &[Page], path: &str) -> String {
    let Some(page) = pages.iter().find(|p| p.path == path) else {
        panic!("no page {path}");
    };
    let (dump, codes) = read_with(&page.path, &page.text, &Config::default());
    assert!(!dump.is_empty() && codes.is_empty(), "{path}: {codes}\n{}", page.text);
    dump
}

#[test]
fn every_generated_page_reads_without_reports() {
    for options in [Options::default(), Options { include_private: true, ..Options::default() }] {
        let pages = shapes_pages(&options);
        assert!(pages.len() >= 5, "{} pages", pages.len());
        for page in &pages {
            let dump = read_page(&pages, &page.path);
            assert!(!dump.contains("system-message"), "{}:\n{dump}", page.path);
        }
    }
}

#[test]
fn generated_types_nest_fields_and_impls() {
    let pages = shapes_pages(&Options::default());
    let got = read_page(&pages, "fake_shapes/geometry/point.rst");
    expect_in_order(
        &got,
        &[
            "index @id=\"rust-module-fake_shapes.geometry.point\" \
             @entries=\"single: fake_shapes::geometry::point (module)\" \
             @module=\"fake_shapes::geometry::point\" @domain=\"rust\" @synopsis=\"Points.\"",
            "desc @domain=\"rust\" @objtype=\"struct\"",
            "desc-signature @id=\"rust-fake_shapes.geometry.point.Point\" @fullname=\"Point\" \
             @module=\"fake_shapes::geometry::point\" @toc=\"Point\"",
            "desc-annotation\n          text \"pub struct \"",
            "desc-addname\n          text \"point::\"",
            "desc-name\n          text \"Point\"",
            "text \"<T = f32>\"",
            "index @entries=\"single: Point (struct in fake_shapes::geometry::point)\" \
             @targets=\"rust-fake_shapes.geometry.point.Point\"",
            "@id=\"rust-fake_shapes.geometry.point.Point.x\" @fullname=\"Point::x\"",
            "single: Point::x (field in fake_shapes::geometry::point)",
            "desc @domain=\"rust\" @objtype=\"impl\" @noindex=\"1\"\n          desc-signature\n",
            "text \"impl\"",
            "xref link=unresolved name=\"Copy\" @reftype=\"rust:any\" \
             @rust:module=\"fake_shapes::geometry::point\" @rust:type=\"Point\"",
            "@id=\"rust-fake_shapes.geometry.point.Point.new\" @fullname=\"Point::new\" \
             @module=\"fake_shapes::geometry::point\" @toc=\"Point::new()\"",
            "desc-parameter\n                    text \"x\"\n                    text \": \"\n\
             \x20                   text \"T\"",
            "text \" -> \"\n                text \"Self\"",
            "single: Point::new (method in fake_shapes::geometry::point)",
            "xref link=unresolved name=\"fake_shapes::geometry::point::Point::x\" \
             @reftype=\"rust:field\" @refexplicit=\"1\"",
            "text \" for \"",
            "desc @domain=\"rust\" @objtype=\"method\" @noindex=\"1\"\n\
             \x20             desc-signature @fullname=\"Point::fmt\"",
            "xref link=unresolved name=\"fmt::Formatter\" @reftype=\"rust:any\"",
            "text \"<'_>\"",
            "@id=\"rust-fake_shapes.geometry.point.Pair\"",
            "text \"(pub i32, _)\"",
        ],
    );
    // A trait impl's members are documented on the trait: no id, no entry.
    assert!(!got.contains("Point.fmt") && !got.contains("Point::fmt (method"), "{got}");
}

#[test]
fn generated_items_link_their_types() {
    let pages = shapes_pages(&Options::default());
    let got = read_page(&pages, "fake_shapes.rst");
    expect_in_order(
        &got,
        &[
            "index @id=\"rust-module-fake_shapes\" @entries=\"single: fake_shapes (crate)\"",
            "desc @domain=\"rust\" @objtype=\"use\" @noindex=\"1\"",
            "text \"pub use \"",
            "xref link=unresolved name=\"fake_shapes::geometry::point::Point\" \
             @reftype=\"rust:any\" @rust:module=\"fake_shapes\"\n          \
             text \"geometry::point::Point\"",
            "xref link=unresolved name=\"std::collections::HashMap\"",
            "text \"macro_rules! \"",
            "@id=\"rust-fake_shapes.Canvas.new\" @fullname=\"Canvas::new\"",
            "text \"#[non_exhaustive] pub enum \"",
            "@id=\"rust-fake_shapes.Kind.Polygon.0\" @fullname=\"Kind::Polygon::0\"",
            "@id=\"rust-fake_shapes.Kind.Rect.width\" @fullname=\"Kind::Rect::width\"",
            "text \" = \"\n            text \"9\"",
            "text \"; 4]\"",
            "xref link=unresolved name=\"Send\" @reftype=\"rust:any\"",
            "single: Draw::Output (associated type in fake_shapes)",
            "single: Draw::PASSES (associated constant in fake_shapes)",
            "xref link=unresolved name=\"fake_shapes::Draw::Output\" @reftype=\"rust:any\" \
             @rust:module=\"fake_shapes\" @rust:type=\"Draw\"\n              \
             text \"Self::Output\"",
            "text \"pub unsafe fn \"",
            "text \" + 'static>\"",
            "text \"*const T\"",
            "text \"where T: \"",
            "single: draw_all (unsafe function in fake_shapes)",
            "single: draw_later (async function in fake_shapes)",
            "strong +generated\n            text +generated \"Deprecated\"",
            "text \"use draw_all\"",
            "text \"pub unsafe extern \\\"C\\\" fn \"",
            "xref link=unresolved name=\"std::result::Result\"",
            "single: Result (type alias in fake_shapes)",
            "text \"4_096\"",
            "text \"pub static mut \"",
        ],
    );
    let inline = read_page(&pages, "fake_shapes/inline.rst");
    expect_in_order(
        &inline,
        &[
            "text +generated \"Available on \"\n      code \"feature = \\\"inline\\\"\"\n      \
             text +generated \" only.\"",
            "single: double (const function in fake_shapes::inline)",
        ],
    );
}

#[test]
fn roles_take_python_forms_and_resolve_relative_paths() {
    expect_contains(
        ".. rust:module:: my_crate::parser\n\n\
         .. rust:struct:: pub struct Lexer\n\n\
         \x20  :rust:field:`Self::pos` :rust:fn:`next_token`\n\n\
         :rust:mod:`crate::lexer` :rust:struct:`.Lexer` :rust:fn:`super::run` \
         :rust:any:`self::Token` :rust:struct:`~my_crate::parser::Lexer`\n",
        &[
            "xref link=unresolved name=\"my_crate::parser::Lexer::pos\" @reftype=\"rust:field\" \
             @rust:module=\"my_crate::parser\" @rust:type=\"Lexer\"",
            "xref link=unresolved name=\"next_token\" @reftype=\"rust:fn\"",
            "code \"next_token()\"",
            "xref link=unresolved name=\"my_crate::lexer\" @reftype=\"rust:mod\"",
            "xref link=unresolved name=\"Lexer\" @reftype=\"rust:struct\" @refspecific=\"1\"",
            "xref link=unresolved name=\"my_crate::run\" @reftype=\"rust:fn\"",
            "xref link=unresolved name=\"my_crate::parser::Token\" @reftype=\"rust:any\"",
            "xref link=unresolved name=\"my_crate::parser::Lexer\" @reftype=\"rust:struct\"",
        ],
    );
}

#[test]
fn hand_written_forms() {
    expect_contains(
        ".. rust:crate:: my_crate\n\n\
         .. rust:function:: f(x: u8) -> Option<u8>\n\n\
         .. rust:impl:: impl Point\n\n   .. rust:method:: fn norm(&self) -> f32\n\n\
         .. rust:struct:: Other\n   :module: my_crate::geo\n\n\
         .. rust:function:: g()\n",
        &[
            "desc-annotation\n        text \"fn \"",
            "desc-name\n        text \"f\"",
            "xref link=unresolved name=\"Option\"",
            "single: f (function in my_crate)",
            "desc @domain=\"rust\" @objtype=\"impl\" @noindex=\"1\"",
            "@id=\"rust-my_crate.Point.norm\" @fullname=\"Point::norm\"",
            "single: Point::norm (method in my_crate)",
            "@id=\"rust-my_crate.geo.Other\" @fullname=\"Other\" @module=\"my_crate::geo\"",
            "desc-addname\n        text \"geo::\"",
            "@id=\"rust-my_crate.g\" @fullname=\"g\" @module=\"my_crate\"",
        ],
    );
    // A standalone trait impl's members are never targets; :for: names the type.
    let got = read_text(
        ".. rust:crate:: c\n\n.. rust:impl:: impl<T> From<T> for Wrapper<T>\n\n\
         \x20  .. rust:method:: fn from(value: T) -> Self\n\n\
         .. rust:impl:: impl X\n   :for: Y\n\n   .. rust:const:: const N: u8 = 1\n",
    );
    assert!(!got.contains("Wrapper.from") && got.contains("@fullname=\"Wrapper::from\""), "{got}");
    assert!(got.contains("@id=\"rust-c.Y.N\" @fullname=\"Y::N\""), "{got}");
    assert!(!got.contains("name=\"T\""), "generic names are not links:\n{got}");
}

#[test]
fn no_index_variants_and_unreadable_signatures() {
    let got = read_text(
        ".. rust:module:: m\n   :noindex:\n\n.. rust:function:: fn f()\n   :no-index:\n",
    );
    assert!(!got.contains("@id=") && !got.contains("index "), "no-index:\n{got}");
    // A second signature is the same object: no second id or entry.
    let got = read_text(".. rust:function:: fn f(a: u8)\n                   fn f(a: u16)\n");
    assert_eq!(got.matches("@id=").count(), 1, "{got}");
    assert_eq!(got.matches("single:").count(), 1, "{got}");
    // Text with no name is shown as written, without a target.
    let got = read_text(".. rust:function:: fn (x)\n.. rust:impl:: Point\n");
    assert!(got.contains("desc-name\n        text \"fn (x)\"") && !got.contains("@id="), "{got}");
    assert!(got.contains("desc-name\n        text \"Point\""), "{got}");
}

#[test]
fn toc_entries_follow_the_setting() {
    let text =
        ".. rust:module:: a::b\n\n.. rust:struct:: struct S\n\n   .. rust:method:: fn m()\n";
    let (got, _) = read_with("t.rst", text, &Config::default());
    expect_in_order(&got, &["@toc=\"S\"", "@toc=\"S::m()\""]);
    let all = Config { toc_object_entries_show_parents: TocParents::All, ..Config::default() };
    let (got, _) = read_with("t.rst", text, &all);
    expect_in_order(&got, &["@toc=\"a::b::S\"", "@toc=\"a::b::S::m()\""]);
    let hide = Config {
        toc_object_entries_show_parents: TocParents::Hide,
        add_module_names: false,
        add_function_parentheses: false,
        ..Config::default()
    };
    let (got, _) = read_with("t.rst", text, &hide);
    expect_in_order(&got, &["@toc=\"S\"", "@toc=\"m\""]);
    assert!(!got.contains("desc-addname"), "{got}");
}
