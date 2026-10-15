//! The C++ domain's symbol tree (cpp_symbols.odin, cpp_lookup.odin).
//!
//! Every declaration's path becomes a chain of symbols from the root; template and
//! function parameters are symbols below it; an enumerator of an unscoped enum is
//! declared in the enum's scope too when that name is free there. objects.inv lists
//! every symbol with a declaration, and C++ references are looked up in the tree as
//! Sphinx looks them up (`find_name`, `find_declaration`).

use std::collections::HashSet;

use gd_env::{CppDeclInfo, Digest, Digester, DocId, Environment};
use gd_report::Report;

use crate::tables::doc_warning;

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

/// One element of a declaration's encoded path (`CppDeclInfo::path`): fields apart by
/// U+001F, elements one per line, as guidedoc-cpp encodes them.
#[derive(Debug, Clone, Default)]
pub(crate) struct PathElement<'a> {
    ident: &'a str,
    args: &'a str,
    params: &'a str,
    display: &'a str,
    id4: &'a str,
    decl: &'a str,
}

pub(crate) fn decode_path(path: &str) -> Vec<PathElement<'_>> {
    path.lines()
        .map(|line| {
            let f: Vec<&str> = line.split('\x1f').collect();
            let field = |i: usize| f.get(i).copied().unwrap_or("");
            PathElement {
                ident: field(0),
                args: field(1),
                params: field(2),
                display: field(3),
                id4: field(7),
                decl: field(8),
            }
        })
        .collect()
}

/// How a name is matched against the symbols of a scope.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Matching {
    /// Template parameters need not be written (a reference's shorthand).
    pub(crate) shorthand: bool,
    /// The scope itself is a candidate.
    pub(crate) match_self: bool,
    /// The children of anonymous children are candidates.
    pub(crate) recurse_anon: bool,
}

const EXACT: Matching = Matching { shorthand: false, match_self: false, recurse_anon: false };

impl CppTree {
    /// The symbol `id` names. Panics for an id of another tree, an internal invariant.
    pub fn symbol(&self, id: CppSymbolId) -> &CppSymbol {
        &self.symbols[id.0 as usize]
    }

    fn symbol_mut(&mut self, id: CppSymbolId) -> &mut CppSymbol {
        &mut self.symbols[id.0 as usize]
    }

    /// Builds the tree of every document's C++ declarations, read in the order of
    /// their docnames as Sphinx reads documents; redeclarations are warned about in
    /// `reports`.
    pub(crate) fn build(env: &Environment, reports: &mut Vec<Report>) -> CppTree {
        let mut t = CppTree { symbols: vec![CppSymbol::default()], ..CppTree::default() };
        let mut digest = Digester::new();
        // The environment is sorted by docname already.
        for (id, d) in env.iter().filter(|(_, d)| !d.failed && !d.cpp.is_empty()) {
            let (mut group, mut last) = (None, None);
            for (k, decl) in d.cpp.iter().enumerate() {
                let added = t.add_declaration(decl, id);
                match added {
                    Some((s, true)) => {
                        t.redeclared.insert((id, k as u32));
                        let message = format!(
                            "The C++ declaration {} is also declared in {}.",
                            t.full_name(s, false),
                            env.doc(t.symbol(s).declaration.as_ref().map_or(id, |x| x.doc))
                                .docname
                        );
                        reports.push(doc_warning(
                            d,
                            decl.line,
                            [
                                "build.cpp.duplicate",
                                "DUPLICATE C++ DECLARATION",
                                &message,
                                "Declare it once; links go to the first declaration.",
                                "duplicate_declaration.cpp",
                            ],
                        ));
                    }
                    Some((s, false)) if group == Some(decl.group) && last.is_some() => {
                        t.symbol_mut(s).sibling_above = last; // signatures of one directive.
                    }
                    _ => {}
                }
                group = Some(decl.group);
                last = added.map(|(s, _)| s);
                digest.text(&d.docname).text(&decl.path).text(&decl.id);
            }
        }
        t.digest = digest.finish();
        t
    }

    fn new_symbol(&mut self, parent: CppSymbolId, e: &PathElement<'_>) -> CppSymbolId {
        let id = CppSymbolId(self.symbols.len() as u32);
        self.symbols.push(CppSymbol {
            parent: Some(parent),
            ident: e.ident.into(),
            args: e.args.into(),
            params: e.params.into(),
            display: e.display.into(),
            id4: e.id4.into(),
            ..CppSymbol::default()
        });
        self.symbol_mut(parent).children.push(id);
        id
    }

    /// Adds a declaration's symbol (`_add_symbols`): its scopes are found or created;
    /// its own symbol fills an empty one, is a new overload of a function, or is a
    /// redeclaration (kept, marked, and not the one later names find). Returns the
    /// symbol later declarations of the directive are declared in, and whether it was a
    /// redeclaration; `None` for an empty path.
    fn add_declaration(&mut self, decl: &CppDeclInfo, doc: DocId) -> Option<(CppSymbolId, bool)> {
        let path = decode_path(&decl.path);
        let (last, scopes) = path.split_last()?;
        let mut parent = CppSymbolId(0);
        for e in scopes {
            parent = match self.first_named(parent, e.ident, e.params, e.args, EXACT) {
                Some(found) => found,
                None => self.new_symbol(parent, e),
            };
        }
        let (mut empty, mut existing) = (None, None);
        for c in self.named(parent, last.ident, last.params, last.args, EXACT) {
            match &self.symbol(c).declaration {
                None => {
                    empty.get_or_insert(c);
                }
                Some(x) if x.redeclaration => {}
                Some(x) => {
                    if decl.objtype != "function" || x.objtype != "function" || x.id == decl.id {
                        existing.get_or_insert(c);
                    }
                }
            }
        }
        let target = match empty {
            Some(e) if existing.is_none() => e,
            _ => self.new_symbol(parent, last),
        };
        self.declare(target, decl, doc, existing.is_some());
        if let Some(e) = existing {
            return Some((e, true));
        }
        if decl.objtype == "enumerator" {
            self.inject_enumerator(target);
        }
        Some((target, false))
    }

    fn declare(&mut self, s: CppSymbolId, decl: &CppDeclInfo, doc: DocId, again: bool) {
        self.symbol_mut(s).declaration = Some(CppDeclaration {
            objtype: decl.objtype.clone(),
            directive: decl.directive.clone(),
            id: decl.id.clone(),
            doc,
            redeclaration: again,
        });
        let unprefixed = decl.id.strip_prefix("_CPPv4").unwrap_or(&decl.id);
        let id = format!("_CPPv4{unprefixed}");
        for name in &decl.tparams {
            self.param_symbol(s, name, "templateParam", &id, doc);
        }
        if decl.objtype == "function" {
            for name in &decl.fparams {
                self.param_symbol(s, name, "functionParam", &id, doc);
            }
        }
    }

    /// Declares a template or function parameter below its declaration, at the
    /// declaration's id.
    fn param_symbol(
        &mut self,
        parent: CppSymbolId,
        name: &str,
        objtype: &str,
        id: &str,
        doc: DocId,
    ) {
        let declaration =
            CppDeclaration { objtype: objtype.into(), id: id.into(), doc, ..Default::default() };
        let existing = self.symbol(parent).children.iter().copied().find(|&c| {
            let s = self.symbol(c);
            s.ident == name && s.args.is_empty() && s.params.is_empty()
        });
        let s = match existing {
            Some(c) if self.symbol(c).declaration.is_some() => return,
            Some(c) => c,
            None => {
                let id4 = element_id(name);
                let e =
                    PathElement { ident: name, display: name, id4: &id4, ..Default::default() };
                self.new_symbol(parent, &e)
            }
        };
        self.symbol_mut(s).declaration = Some(declaration);
    }

    /// Declares an enumerator of an unscoped enum in the enum's scope too, unless that
    /// scope has a symbol of its name (`_add_enumerator_to_parent`).
    fn inject_enumerator(&mut self, s: CppSymbolId) {
        let Some(enum_id) = self.symbol(s).parent.filter(|p| p.0 > 0) else { return };
        let e = self.symbol(enum_id);
        let is_enum =
            e.declaration.as_ref().is_some_and(|d| d.objtype == "enum" && d.directive == "enum");
        let Some(scope) = e.parent.filter(|_| is_enum) else { return };
        let sym = self.symbol(s).clone();
        let anon = Matching { recurse_anon: true, ..EXACT };
        if self.find_identifier(scope, &sym.ident, anon, false).is_some() {
            return;
        }
        let e = PathElement {
            ident: &sym.ident,
            display: &sym.display,
            id4: &sym.id4,
            ..Default::default()
        };
        let clone = self.new_symbol(scope, &e);
        let mut declaration = sym.declaration;
        if let Some(d) = declaration.as_mut() {
            d.redeclaration = false;
        }
        self.symbol_mut(clone).declaration = declaration;
    }

    /// The symbol at the encoded `path` (a declaration's `CppDeclInfo::path`), as far
    /// as it is in the tree (`direct_lookup`): a declared element by its declaration's
    /// id, another by its name. The root for an empty path.
    pub fn find_path(&self, path: &str) -> Option<CppSymbolId> {
        let mut s = CppSymbolId(0);
        self.symbols.first()?;
        for e in decode_path(path) {
            let by_id = (!e.decl.is_empty())
                .then(|| {
                    self.symbol(s).children.iter().copied().find(|&c| {
                        self.symbol(c).declaration.as_ref().is_some_and(|d| d.id == e.decl)
                    })
                })
                .flatten();
            match by_id.or_else(|| self.first_named(s, e.ident, e.params, e.args, EXACT)) {
                Some(next) => s = next,
                None => return Some(s),
            }
        }
        Some(s)
    }

    /// The symbol's full name, as written (`display`) or as its id is made.
    pub fn full_name(&self, id: CppSymbolId, display: bool) -> String {
        let mut parts = Vec::new();
        let mut at = id;
        while at.0 > 0 {
            let s = self.symbol(at);
            parts.push(if display { s.display.clone() } else { format!("{}{}", s.ident, s.args) });
            let Some(parent) = s.parent else { break };
            at = parent;
        }
        parts.reverse();
        parts.join("::")
    }
}

/// The version 4 id of an identifier element.
fn element_id(name: &str) -> String {
    if name == "std" {
        return "St".into();
    }
    if name.starts_with('~') {
        return "D0".into();
    }
    format!("{}{name}", name.len())
}
