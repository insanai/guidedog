//! The crate model: flat vectors of modules and items addressed by ids, the way the
//! rest of Guidedog stores trees. Everything is plain data with public fields; the
//! loader fills it and [`document`](crate::document) reads it.

use crate::diag::Diagnostic;
use std::path::PathBuf;

/// The index of a module in [`Crate::modules`]. The crate root is `ModuleId(0)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ModuleId(pub u32);

/// The index of an item in [`Crate::items`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ItemId(pub u32);

/// The index of a file in [`Crate::files`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FileId(pub u32);

/// A Rust crate read from source: its module tree, every item, and what went wrong.
#[derive(Debug, Clone, Default)]
pub struct Crate {
    /// The crate's name as code writes it (`gd_rustdoc`, never `gd-rustdoc`).
    pub name: String,
    /// The folder files may be read from; nothing outside it is read.
    pub root_dir: PathBuf,
    /// Every source file read, in reading order.
    pub files: Vec<SourceFile>,
    /// Every module; `modules[0]` is the crate root.
    pub modules: Vec<Module>,
    /// Every item of every module, members included.
    pub items: Vec<Item>,
    /// Problems met while loading: missing module files, syntax errors, limits.
    pub warnings: Vec<Diagnostic>,
}

/// One source file read for the crate.
#[derive(Debug, Clone, Default)]
pub struct SourceFile {
    /// The path it was read from.
    pub path: PathBuf,
    /// Its text, kept for warning excerpts.
    pub text: String,
}

/// A module: a file module, an inline `mod name { ... }`, or the crate root.
#[derive(Debug, Clone, Default)]
pub struct Module {
    /// Its own name (the crate name for the root).
    pub name: String,
    /// The enclosing module, `None` for the root.
    pub parent: Option<ModuleId>,
    /// The `mod` item that declares it in its parent, `None` for the root.
    pub item: Option<ItemId>,
    /// Its documentation: the `mod` item's outer docs, then its inner `//!` docs.
    pub docs: String,
    /// `cfg` predicates from inner `#![cfg(...)]` attributes.
    pub cfg: Vec<String>,
    /// Its items in source order (members of types are under their parent item).
    pub items: Vec<ItemId>,
    /// The file it is written in.
    pub file: FileId,
}

/// What kind of thing an [`Item`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ItemKind {
    /// A free function, `fn`.
    #[default]
    Function,
    /// A function in an `impl` or `trait` block (with or without `self`).
    Method,
    /// A `struct`.
    Struct,
    /// An `enum`.
    Enum,
    /// One variant of an enum.
    Variant,
    /// A `union`.
    Union,
    /// A `trait`.
    Trait,
    /// An `impl` block; its members are its children.
    Impl,
    /// A `type` alias.
    TypeAlias,
    /// A `type` in a trait or impl.
    AssociatedType,
    /// A `const`.
    Const,
    /// A `const` in a trait or impl.
    AssociatedConst,
    /// A `static`.
    Static,
    /// A `macro_rules!` macro.
    Macro,
    /// A field of a struct, union or variant (tuple fields are named `0`, `1`, ...).
    Field,
    /// One name a `use` declaration brings in (or `*` for a glob).
    Use,
    /// A `mod` declaration; [`Item::module_target`] is the module.
    Module,
}

impl ItemKind {
    /// The `rust` domain directive that describes this kind (`function`, `struct`, ...).
    pub fn directive(self) -> &'static str {
        match self {
            ItemKind::Function => "function",
            ItemKind::Method => "method",
            ItemKind::Struct => "struct",
            ItemKind::Enum => "enum",
            ItemKind::Variant => "variant",
            ItemKind::Union => "union",
            ItemKind::Trait => "trait",
            ItemKind::Impl => "impl",
            ItemKind::TypeAlias => "type",
            ItemKind::AssociatedType => "associatedtype",
            ItemKind::Const => "const",
            ItemKind::AssociatedConst => "associatedconst",
            ItemKind::Static => "static",
            ItemKind::Macro => "macro",
            ItemKind::Field => "field",
            ItemKind::Use => "use",
            ItemKind::Module => "module",
        }
    }

    /// The `rust` domain role that links to this kind (`fn`, `struct`, ...).
    pub fn role(self) -> &'static str {
        match self {
            ItemKind::Function | ItemKind::Method => "fn",
            ItemKind::Struct => "struct",
            ItemKind::Enum => "enum",
            ItemKind::Variant => "variant",
            ItemKind::Union => "union",
            ItemKind::Trait => "trait",
            ItemKind::TypeAlias | ItemKind::AssociatedType => "type",
            ItemKind::Const | ItemKind::AssociatedConst => "const",
            ItemKind::Static => "static",
            ItemKind::Macro => "macro",
            ItemKind::Field => "field",
            ItemKind::Module => "mod",
            ItemKind::Impl | ItemKind::Use => "any",
        }
    }

    /// Whether the kind is a type that `impl` blocks attach to.
    pub fn is_type(self) -> bool {
        matches!(
            self,
            ItemKind::Struct
                | ItemKind::Enum
                | ItemKind::Union
                | ItemKind::TypeAlias
                | ItemKind::Trait
        )
    }
}

/// Who may name an item, as its visibility modifier says.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Visibility {
    /// `pub`.
    Public,
    /// `pub(crate)` (or the old `crate`).
    Crate,
    /// `pub(super)`.
    Super,
    /// `pub(in path)`, kept as written.
    Restricted(String),
    /// No modifier (or `pub(self)`): private to its module.
    #[default]
    Private,
    /// Enum variants, trait members and trait-impl members: as visible as their parent.
    Inherited,
}

impl Visibility {
    /// The modifier as Rust writes it; empty for private and inherited items.
    pub fn as_str(&self) -> &str {
        match self {
            Visibility::Public => "pub",
            Visibility::Crate => "pub(crate)",
            Visibility::Super => "pub(super)",
            Visibility::Restricted(text) => text,
            Visibility::Private | Visibility::Inherited => "",
        }
    }

    /// Whether code outside the crate may name the item (given a public parent).
    pub fn is_public(&self) -> bool {
        matches!(self, Visibility::Public | Visibility::Inherited)
    }
}

/// The qualifiers of functions and statics that the domain shows as options.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Qualifiers {
    /// `unsafe fn`, `unsafe trait`, `unsafe impl`, or a foreign function.
    pub is_unsafe: bool,
    /// `async fn`.
    pub is_async: bool,
    /// `const fn`.
    pub is_const: bool,
    /// `static mut`.
    pub is_mut: bool,
    /// A trait method or associated item with a default.
    pub is_provided: bool,
    /// `#[macro_export]`: the macro lives at the crate root.
    pub is_exported: bool,
    /// The ABI of `extern "C" fn` (empty when there is none).
    pub abi: String,
}

/// One declaration, or a member of one (field, variant, associated item).
#[derive(Debug, Clone, Default)]
pub struct Item {
    /// What it is.
    pub kind: ItemKind,
    /// Its name; empty for impls, `*` for glob uses, `0`, `1`, ... for tuple fields.
    pub name: String,
    /// The declaration as rustdoc shows it: one line, whitespace collapsed, no body,
    /// comments and attributes removed.
    pub signature: String,
    /// The doc comment text (Markdown), unindented; empty when undocumented.
    pub docs: String,
    /// Its visibility.
    pub visibility: Visibility,
    /// `cfg` predicates from `#[cfg(...)]` and `#[doc(cfg(...))]`, as written.
    pub cfg: Vec<String>,
    /// Attributes rustdoc shows, such as `#[non_exhaustive]` and `#[repr(C)]`.
    pub attributes: Vec<String>,
    /// `Some(note)` when `#[deprecated]` (the note may be empty).
    pub deprecated: Option<String>,
    /// `#[doc(hidden)]`.
    pub is_hidden: bool,
    /// `#[cfg(test)]` or `#[test]`: never documented.
    pub is_test: bool,
    /// `Some(true)` for `#[doc(inline)]`, `Some(false)` for `#[doc(no_inline)]`.
    pub inline: Option<bool>,
    /// Function qualifiers and flags.
    pub qualifiers: Qualifiers,
    /// The module it is declared in.
    pub module: ModuleId,
    /// The item it is a member of (a type, variant, trait or impl).
    pub parent: Option<ItemId>,
    /// Its members in source order.
    pub children: Vec<ItemId>,
    /// For a `use`: the path it names, `a::b::c` or `a::b::*`.
    pub use_path: String,
    /// For an impl: the self type as written (`Point<T>`).
    pub impl_type: String,
    /// For a trait impl: the trait as written (`fmt::Display`).
    pub impl_trait: Option<String>,
    /// For a type: the impl blocks attached to it.
    pub impls: Vec<ItemId>,
    /// For a macro: its arms' patterns, `($x:expr)`.
    pub arms: Vec<String>,
    /// For a `mod` item: the module it declares.
    pub module_target: Option<ModuleId>,
    /// The file it is written in.
    pub file: FileId,
    /// The byte offset of its start in that file.
    pub offset: usize,
}

impl Crate {
    /// The module with id `id`. Panics on an id from another crate.
    pub fn module(&self, id: ModuleId) -> &Module {
        &self.modules[id.0 as usize]
    }

    /// The item with id `id`. Panics on an id from another crate.
    pub fn item(&self, id: ItemId) -> &Item {
        &self.items[id.0 as usize]
    }

    /// The number of modules.
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// The number of items, members included.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// A module's path as declared: `my_crate::parser::lexer`.
    pub fn module_path(&self, id: ModuleId) -> String {
        let mut names = Vec::new();
        let mut current = Some(id);
        while let Some(m) = current {
            names.push(self.module(m).name.as_str());
            current = self.module(m).parent;
        }
        names.reverse();
        names.join("::")
    }

    /// The display path of a file, for diagnostics.
    pub fn file_name(&self, id: FileId) -> String {
        self.files.get(id.0 as usize).map(|f| f.path.display().to_string()).unwrap_or_default()
    }

    /// The text of a file (empty for an unknown id).
    pub fn file_text(&self, id: FileId) -> &str {
        self.files.get(id.0 as usize).map_or("", |f| f.text.as_str())
    }

    pub(crate) fn add_item(&mut self, item: Item) -> ItemId {
        let id = ItemId(self.items.len() as u32);
        self.items.push(item);
        id
    }

    pub(crate) fn add_module(&mut self, module: Module) -> ModuleId {
        let id = ModuleId(self.modules.len() as u32);
        self.modules.push(module);
        id
    }
}
