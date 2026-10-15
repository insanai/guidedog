//! The C++ domain's symbol tree (cpp_symbols.odin, cpp_lookup.odin).
//!
//! Every declaration's path becomes a chain of symbols from the root; template and
//! function parameters are symbols below it; an enumerator of an unscoped enum is
//! declared in the enum's scope too when that name is free there. objects.inv lists
//! every symbol with a declaration, and C++ references are looked up in the tree as
//! Sphinx looks them up (`find_name`, `find_declaration`).

use std::collections::HashSet;

use gd_env::{Digest, DocId};

/// A symbol's index in its tree; the root is `CppSymbolId(0)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct CppSymbolId(pub u32);

/// A declaration a symbol carries.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CppDeclaration {
    /// The object type, such as `function`.
    pub objtype: String,
    /// The directive that declared it.
    pub directive: String,
    /// The declaration's newest id: its anchor.
    pub id: String,
    /// The document it is in.
    pub doc: DocId,
    /// Declared again with the same id: no index entry, no duplicate warning.
    pub redeclaration: bool,
}

/// One symbol of the tree.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CppSymbol {
    /// The enclosing symbol; `None` for the root.
    pub parent: Option<CppSymbolId>,
    /// The identifier or operator.
    pub ident: String,
    /// Its template arguments.
    pub args: String,
    /// Its template parameter list; empty for none.
    pub params: String,
    /// The element's display text.
    pub display: String,
    /// The element's version 4 id.
    pub id4: String,
    /// The symbols below it, in declaration order.
    pub children: Vec<CppSymbolId>,
    /// The symbol of the signature above in the same directive.
    pub sibling_above: Option<CppSymbolId>,
    /// Its declaration, when it has one.
    pub declaration: Option<CppDeclaration>,
}

/// The tree of every C++ symbol the project declares.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CppTree {
    /// The symbols; the root first.
    pub symbols: Vec<CppSymbol>,
    /// The digest of every declaration: the answer to a `Cpp` lookup, so a page that
    /// looked a C++ name up is written again when any declaration changes.
    pub digest: Digest,
    /// (document, declaration index) pairs that redeclare a symbol.
    pub redeclared: HashSet<(DocId, u32)>,
}

impl CppTree {
    /// The symbol `id` names. Panics for an id of another tree, an internal invariant.
    pub fn symbol(&self, id: CppSymbolId) -> &CppSymbol {
        &self.symbols[id.0 as usize]
    }

    /// The symbol at the encoded `path` (a declaration's `CppDeclInfo::path`), when it
    /// is in the tree.
    pub fn find_path(&self, path: &str) -> Option<CppSymbolId> {
        let _ = path;
        todo!("wave 1: port cpp_direct_lookup")
    }

    /// The symbol's full name, as written (`display`) or as its id is made.
    pub fn full_name(&self, id: CppSymbolId, display: bool) -> String {
        let _ = (id, display);
        todo!("wave 1: port cpp_full_name")
    }
}
