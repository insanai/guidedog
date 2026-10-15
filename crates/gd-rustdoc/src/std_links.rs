//! Links into the Rust standard library's documentation at doc.rust-lang.org, for
//! intra-doc links the crate cannot resolve: primitives (`prim@u8`, `[str]`,
//! `[u8::MAX]`), `std::`, `core::` and `alloc::` paths, and the prelude's names
//! (`Option`, `Vec::new`).
//!
//! The standard library publishes no inventory a Sphinx reader could use, so these
//! links are external hyperlinks to stable URLs. A page needs the item's kind
//! (`struct.Vec.html`); when neither a disambiguator nor the table of common items
//! below gives it, the link opens rustdoc's search for the path, which is just as
//! stable and always finds it.

use crate::model::ItemKind;
use crate::resolve::Namespace;

/// The root of the standard library's documentation.
const DOCS: &str = "https://doc.rust-lang.org";

/// Rust's primitive types, which have pages of their own (`primitive.u8.html`).
const PRIMITIVES: [&str; 27] = [
    "bool",
    "char",
    "str",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "usize",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "isize",
    "f16",
    "f32",
    "f64",
    "f128",
    "slice",
    "array",
    "tuple",
    "unit",
    "never",
    "pointer",
    "reference",
    "fn",
];

/// The kinds of page rustdoc writes, by the file name prefix it uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Struct,
    Enum,
    Trait,
    Macro,
    Module,
}

impl Page {
    fn prefix(self) -> &'static str {
        match self {
            Page::Struct => "struct",
            Page::Enum => "enum",
            Page::Trait => "trait",
            Page::Macro => "macro",
            Page::Module => "index",
        }
    }
}

/// Common standard library items with their pages, so the usual links go straight to
/// the item. The prelude's names also stand alone (`Vec`), as rustdoc resolves them.
const KNOWN: [(&str, Page); 44] = [
    ("std::option::Option", Page::Enum),
    ("std::result::Result", Page::Enum),
    ("std::vec::Vec", Page::Struct),
    ("std::string::String", Page::Struct),
    ("std::boxed::Box", Page::Struct),
    ("std::borrow::Cow", Page::Enum),
    ("std::borrow::ToOwned", Page::Trait),
    ("std::string::ToString", Page::Trait),
    ("std::clone::Clone", Page::Trait),
    ("std::marker::Copy", Page::Trait),
    ("std::marker::Send", Page::Trait),
    ("std::marker::Sync", Page::Trait),
    ("std::marker::Sized", Page::Trait),
    ("std::marker::PhantomData", Page::Struct),
    ("std::default::Default", Page::Trait),
    ("std::ops::Drop", Page::Trait),
    ("std::ops::Fn", Page::Trait),
    ("std::ops::FnMut", Page::Trait),
    ("std::ops::FnOnce", Page::Trait),
    ("std::ops::Deref", Page::Trait),
    ("std::ops::Range", Page::Struct),
    ("std::iter::Iterator", Page::Trait),
    ("std::iter::IntoIterator", Page::Trait),
    ("std::iter::Extend", Page::Trait),
    ("std::convert::From", Page::Trait),
    ("std::convert::Into", Page::Trait),
    ("std::convert::TryFrom", Page::Trait),
    ("std::convert::TryInto", Page::Trait),
    ("std::convert::AsRef", Page::Trait),
    ("std::convert::AsMut", Page::Trait),
    ("std::cmp::PartialEq", Page::Trait),
    ("std::cmp::Eq", Page::Trait),
    ("std::cmp::PartialOrd", Page::Trait),
    ("std::cmp::Ord", Page::Trait),
    ("std::collections::HashMap", Page::Struct),
    ("std::collections::BTreeMap", Page::Struct),
    ("std::collections::HashSet", Page::Struct),
    ("std::rc::Rc", Page::Struct),
    ("std::sync::Arc", Page::Struct),
    ("std::fmt::Display", Page::Trait),
    ("std::fmt::Debug", Page::Trait),
    ("std::error::Error", Page::Trait),
    ("std::path::Path", Page::Struct),
    ("std::path::PathBuf", Page::Struct),
];

/// Names of the prelude that are not items of their own page: the variants of `Option`
/// and `Result`.
const PRELUDE_VARIANTS: [(&str, &str); 4] = [
    ("Some", "std::option::Option"),
    ("None", "std::option::Option"),
    ("Ok", "std::result::Result"),
    ("Err", "std::result::Result"),
];

/// The names the prelude brings into every module (rustdoc resolves `[Vec]` to std).
const PRELUDE: [&str; 31] = [
    "Option",
    "Result",
    "Vec",
    "String",
    "Box",
    "ToOwned",
    "ToString",
    "Clone",
    "Copy",
    "Send",
    "Sync",
    "Sized",
    "Default",
    "Drop",
    "Fn",
    "FnMut",
    "FnOnce",
    "Iterator",
    "IntoIterator",
    "Extend",
    "From",
    "Into",
    "TryFrom",
    "TryInto",
    "AsRef",
    "AsMut",
    "PartialEq",
    "Eq",
    "PartialOrd",
    "Ord",
    "Unpin",
];

/// Whether `name` is one of Rust's primitive types.
pub(crate) fn is_primitive(name: &str) -> bool {
    PRIMITIVES.contains(&name)
}

/// The doc.rust-lang.org page of a primitive (`u8`), with an anchor for a member
/// (`u8::MAX`, `str::len`). `None` when `path` does not start with a primitive.
pub(crate) fn primitive_url(path: &str) -> Option<String> {
    let path = strip_primitive_module(path.trim_start_matches("::"));
    let mut segments = path.split("::");
    let primitive = segments.next().filter(|p| is_primitive(p))?;
    let member = segments.next();
    if segments.next().is_some() {
        return None;
    }
    let mut url = format!("{DOCS}/std/primitive.{primitive}.html");
    if let Some(member) = member {
        url.push_str(&member_anchor(member, Page::Struct));
    }
    Some(url)
}

/// `std::primitive::u8` and `core::primitive::u8` name the primitive `u8`.
fn strip_primitive_module(path: &str) -> &str {
    ["std::primitive::", "core::primitive::"]
        .iter()
        .find_map(|prefix| path.strip_prefix(prefix))
        .unwrap_or(path)
}

/// The doc.rust-lang.org URL of a standard library path the crate could not resolve:
/// a primitive, a `std::`/`core::`/`alloc::` path, or a prelude name. `None` for any
/// other path (another crate's, or a broken one).
pub(crate) fn std_url(path: &str, namespace: Namespace) -> Option<String> {
    let path = path.trim_start_matches("::");
    if let Some(url) = primitive_url(path) {
        return Some(url);
    }
    let segments: Vec<&str> = path.split("::").collect();
    let first = *segments.first()?;
    if matches!(first, "std" | "core" | "alloc") {
        return Some(library_url(&segments, namespace));
    }
    prelude_url(&segments)
}

/// A prelude name, or a member of one: `Vec`, `Vec::new`, `Some`, `Option::Some`.
fn prelude_url(segments: &[&str]) -> Option<String> {
    let first = segments[0];
    if let Some((_, owner)) = PRELUDE_VARIANTS.iter().find(|(name, _)| *name == first) {
        let base = known_url(owner)?;
        return (segments.len() == 1).then(|| format!("{base}#variant.{first}"));
    }
    if !PRELUDE.contains(&first) || segments.len() > 2 {
        return None;
    }
    let full = KNOWN.iter().find(|(p, _)| p.rsplit("::").next() == Some(first));
    let Some((full, page)) = full else {
        return Some(search_url("std", first));
    };
    let mut url = known_url(full)?;
    if let Some(member) = segments.get(1) {
        url.push_str(&member_anchor(member, *page));
    }
    Some(url)
}

/// A path into `std`, `core` or `alloc`.
fn library_url(segments: &[&str], namespace: Namespace) -> String {
    let library = segments[0];
    let path = segments.join("::");
    if segments.len() == 1 {
        return format!("{DOCS}/{library}/index.html");
    }
    // Paths into `core` and `alloc` have the same pages under `std` in the table.
    let as_std = format!("std::{}", segments[1..].join("::"));
    if let Some(url) = known_url(&as_std) {
        return url.replacen("/std/", &format!("/{library}/"), 1);
    }
    // A member of a known type: `std::vec::Vec::new`.
    let (owner, member) = (&segments[..segments.len() - 1], segments[segments.len() - 1]);
    let owner_std = format!("std::{}", owner[1..].join("::"));
    if let Some((_, page)) = KNOWN.iter().find(|(p, _)| *p == owner_std)
        && let Some(url) = known_url(&owner_std)
    {
        let url = url.replacen("/std/", &format!("/{library}/"), 1);
        return format!("{url}{}", member_anchor(member, *page));
    }
    match page_of(namespace) {
        Some(page) => page_url(segments, page),
        None => search_url(library, &path),
    }
}

/// The page kind a disambiguator names, when it names exactly one.
fn page_of(namespace: Namespace) -> Option<Page> {
    match namespace {
        Namespace::Kind(ItemKind::Struct) => Some(Page::Struct),
        Namespace::Kind(ItemKind::Enum) => Some(Page::Enum),
        Namespace::Kind(ItemKind::Trait) => Some(Page::Trait),
        Namespace::Macro => Some(Page::Macro),
        Namespace::Module => Some(Page::Module),
        _ => None,
    }
}

/// The URL of a table entry.
fn known_url(path: &str) -> Option<String> {
    let (_, page) = KNOWN.iter().find(|(p, _)| *p == path)?;
    let segments: Vec<&str> = path.split("::").collect();
    Some(page_url(&segments, *page))
}

/// The URL of the page of `segments` (library first) of kind `page`. Macros live at
/// the library's root, as `#[macro_export]` puts them.
fn page_url(segments: &[&str], page: Page) -> String {
    let library = segments[0];
    let last = segments[segments.len() - 1];
    match page {
        Page::Module => format!("{DOCS}/{}/index.html", segments.join("/")),
        Page::Macro => format!("{DOCS}/{library}/macro.{last}.html"),
        _ => {
            let dirs = segments[..segments.len() - 1].join("/");
            format!("{DOCS}/{dirs}/{}.{last}.html", page.prefix())
        }
    }
}

/// The anchor of a member on its owner's page: variants of enums, associated
/// constants (upper case) and methods (lower case) of the rest.
fn member_anchor(member: &str, owner: Page) -> String {
    let upper = member.chars().next().is_some_and(char::is_uppercase);
    let kind = match (owner, upper) {
        (Page::Enum, true) => "variant",
        (_, true) => "associatedconstant",
        _ => "method",
    };
    format!("#{kind}.{member}")
}

/// Rustdoc's search for `query` in `library`: a stable URL that finds any item.
fn search_url(library: &str, query: &str) -> String {
    format!("{DOCS}/{library}/index.html?search={query}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(path: &str) -> String {
        std_url(path, Namespace::Any).unwrap_or_default()
    }

    #[test]
    fn primitives_have_their_own_pages() {
        assert_eq!(url("u8"), "https://doc.rust-lang.org/std/primitive.u8.html");
        assert_eq!(url("str::len"), "https://doc.rust-lang.org/std/primitive.str.html#method.len");
        assert_eq!(
            url("std::primitive::u32::MAX"),
            "https://doc.rust-lang.org/std/primitive.u32.html#associatedconstant.MAX"
        );
    }

    #[test]
    fn known_items_and_prelude_names_link_to_their_pages() {
        assert_eq!(url("std::vec::Vec"), "https://doc.rust-lang.org/std/vec/struct.Vec.html");
        assert_eq!(
            url("core::fmt::Display"),
            "https://doc.rust-lang.org/core/fmt/trait.Display.html"
        );
        assert_eq!(
            url("Vec::new"),
            "https://doc.rust-lang.org/std/vec/struct.Vec.html#method.new"
        );
        assert_eq!(
            url("Some"),
            "https://doc.rust-lang.org/std/option/enum.Option.html#variant.Some"
        );
        assert_eq!(
            url("std::option::Option::None"),
            "https://doc.rust-lang.org/std/option/enum.Option.html#variant.None"
        );
    }

    #[test]
    fn other_library_paths_use_the_disambiguator_or_search() {
        let mac = std_url("std::println", Namespace::Macro).unwrap_or_default();
        assert_eq!(mac, "https://doc.rust-lang.org/std/macro.println.html");
        let module = std_url("std::io", Namespace::Module).unwrap_or_default();
        assert_eq!(module, "https://doc.rust-lang.org/std/io/index.html");
        assert_eq!(
            url("std::mem::swap"),
            "https://doc.rust-lang.org/std/index.html?search=std::mem::swap"
        );
        assert_eq!(url("std"), "https://doc.rust-lang.org/std/index.html");
    }

    #[test]
    fn other_paths_are_not_the_standard_library() {
        assert!(std_url("serde::Serialize", Namespace::Any).is_none());
        assert!(std_url("Canvas", Namespace::Any).is_none());
        assert!(std_url("u8::a::b", Namespace::Any).is_none());
    }
}
