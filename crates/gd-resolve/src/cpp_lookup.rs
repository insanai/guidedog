//! Looking C++ names up in the symbol tree, as Sphinx's `Symbol` does:
//! candidates are a symbol itself (`match_self`), its children (and those of anonymous
//! children), and those of the signatures above it in its directive; a name matches by
//! its identifier, its template parameters (only when asked for, unless shorthand), and
//! its arguments.

use gd_env::LookupKind;

use crate::cpp::{CppSymbol, CppSymbolId, CppTree, Matching};
use crate::lookups::Look;
use crate::xref::{Resolved, Unresolved};

/// One element of a reference's name.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CppName<'a> {
    pub(crate) ident: &'a str,
    pub(crate) args: &'a str,
}

/// A C++ name lookup, as `_resolve_xref_inner` makes it.
#[derive(Debug, Clone, Default)]
pub(crate) struct CppQuery<'a> {
    pub(crate) names: Vec<CppName<'a>>,
    pub(crate) rooted: bool,
    /// The template parameter lists written before the name.
    pub(crate) tdecls: Vec<&'a str>,
    /// The role: `identifier`, `class`, `func`, `any`, ...
    pub(crate) role: &'a str,
    pub(crate) matching: Matching,
    pub(crate) search_siblings: bool,
}

/// Why a lookup found nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CppFail {
    Missing,
    /// A qualification names a template parameter.
    TemplateParam,
}

fn matches(s: &CppSymbol, ident: &str, params: &str, args: &str, shorthand: bool) -> bool {
    if s.ident != ident {
        return false;
    }
    if s.params.is_empty() != params.is_empty() && (!params.is_empty() || !shorthand) {
        return false;
    }
    if !params.is_empty() && s.params != params {
        return false;
    }
    if s.args.is_empty() != args.is_empty() {
        return false;
    }
    s.args.is_empty() || s.args == args
}

fn is_template_param(s: &CppSymbol) -> bool {
    s.declaration.as_ref().is_some_and(|d| d.objtype == "templateParam")
}

impl CppTree {
    /// The symbols a lookup from `start` considers, in order.
    fn candidates(&self, start: CppSymbolId, m: Matching) -> Vec<CppSymbolId> {
        let mut out = Vec::new();
        let mut at = Some(start);
        while let Some(s) = at {
            if m.match_self {
                out.push(s);
            }
            self.add_children(s, m.recurse_anon, &mut out);
            at = self.symbol(s).sibling_above;
        }
        out
    }

    fn add_children(&self, s: CppSymbolId, recurse_anon: bool, out: &mut Vec<CppSymbolId>) {
        // An explicit stack: anonymous entities may nest deeply.
        let mut pending: Vec<CppSymbolId> =
            self.symbol(s).children.iter().rev().copied().collect();
        while let Some(c) = pending.pop() {
            out.push(c);
            if recurse_anon && self.symbol(c).ident.starts_with('@') {
                pending.extend(self.symbol(c).children.iter().rev().copied());
            }
        }
    }

    /// The symbols matching a name among the candidates (`_find_named_symbols`).
    pub(crate) fn named(
        &self,
        start: CppSymbolId,
        ident: &str,
        params: &str,
        args: &str,
        m: Matching,
    ) -> Vec<CppSymbolId> {
        let candidates = self.candidates(start, m);
        candidates
            .into_iter()
            .filter(|&s| matches(self.symbol(s), ident, params, args, m.shorthand))
            .collect()
    }

    pub(crate) fn first_named(
        &self,
        start: CppSymbolId,
        ident: &str,
        params: &str,
        args: &str,
        m: Matching,
    ) -> Option<CppSymbolId> {
        self.named(start, ident, params, args, m).first().copied()
    }

    /// A symbol of an identifier in a scope (and, with `search_siblings`, in the scopes
    /// of the signatures above it).
    pub(crate) fn find_identifier(
        &self,
        start: CppSymbolId,
        ident: &str,
        m: Matching,
        search_siblings: bool,
    ) -> Option<CppSymbolId> {
        let mut at = Some(start);
        while let Some(s) = at {
            if m.match_self && self.symbol(s).ident == ident {
                return Some(s);
            }
            let mut children = Vec::new();
            self.add_children(s, m.recurse_anon, &mut children);
            if let Some(c) = children.into_iter().find(|&c| self.symbol(c).ident == ident) {
                return Some(c);
            }
            if !search_siblings {
                break;
            }
            at = self.symbol(s).sibling_above;
        }
        None
    }

    /// Walks up from `start` to the first scope that knows the first name, follows the
    /// qualification, and finds the last name (`_symbol_lookup`). Returns the scope the
    /// lookup ended in and the symbols found there.
    fn symbol_lookup(
        &self,
        start: CppSymbolId,
        q: &CppQuery<'_>,
        raise: bool,
    ) -> Result<(CppSymbolId, Vec<CppSymbolId>), CppFail> {
        let first = q.names.first().ok_or(CppFail::Missing)?;
        let mut parent = if q.rooted { CppSymbolId(0) } else { start };
        let mut m = q.matching;
        while let Some(up) = self.symbol(parent).parent {
            if self.find_identifier(parent, first.ident, m, q.search_siblings).is_some() {
                let constructor = q.names.len() == 1
                    && q.role == "class"
                    && m.match_self
                    && self.symbol(up).ident == first.ident;
                if !constructor {
                    break;
                }
            }
            parent = up;
        }
        let mut next_decl = 0;
        let (last, scopes) = q.names.split_last().ok_or(CppFail::Missing)?;
        for name in scopes {
            let mut params = "";
            if !name.args.is_empty() && next_decl < q.tdecls.len() {
                params = q.tdecls[next_decl];
                next_decl += 1;
            }
            let Some(s) = self.first_named(parent, name.ident, params, name.args, m) else {
                let param = raise && is_template_param(self.symbol(parent));
                return Err(if param { CppFail::TemplateParam } else { CppFail::Missing });
            };
            m.match_self = false;
            parent = s;
        }
        let params = q.tdecls.get(next_decl).copied().unwrap_or("");
        Ok((parent, self.named(parent, last.ident, params, last.args, m)))
    }

    /// The symbols a shorthand reference names (`find_name`): without a match, the last
    /// name again without template parameters and arguments.
    pub(crate) fn find_name(
        &self,
        start: CppSymbolId,
        q: &CppQuery<'_>,
    ) -> Result<CppSymbolId, CppFail> {
        let (parent, symbols) = self.symbol_lookup(start, q, true)?;
        if let Some(&s) = symbols.first() {
            return Ok(s);
        }
        if is_template_param(self.symbol(parent)) {
            return Err(CppFail::TemplateParam);
        }
        let last = q.names.last().ok_or(CppFail::Missing)?;
        self.first_named(parent, last.ident, "", "", q.matching).ok_or(CppFail::Missing)
    }

    /// The symbol a whole declaration names (`find_declaration`): among those of its
    /// name, the one with the declaration's id, made as if it were declared where the
    /// lookup ends.
    pub(crate) fn find_declaration(
        &self,
        start: CppSymbolId,
        q: &CppQuery<'_>,
        query: &str,
        own: &str,
    ) -> Option<CppSymbolId> {
        let (parent, symbols) = self.symbol_lookup(start, q, false).ok()?;
        let id = self.query_id(parent, query, own);
        symbols
            .into_iter()
            .find(|&s| self.symbol(s).declaration.as_ref().is_some_and(|d| d.id == id))
    }

    /// Fills the reader's query id: its full name (between U+001D, the modifiers) is
    /// `N`, the modifiers, the ids of the symbols from the root, and `E` (without `N`
    /// and `E` for a single name and no modifiers).
    fn query_id(&self, parent: CppSymbolId, query: &str, own: &str) -> String {
        let (before, rest) = query.split_once('\x1d').unwrap_or((query, ""));
        let (modifiers, after) = rest.split_once('\x1d').unwrap_or((rest, ""));
        let mut chain = Vec::new();
        let mut at = parent;
        while at.0 > 0 {
            chain.push(self.symbol(at).id4.as_str());
            let Some(p) = self.symbol(at).parent else { break };
            at = p;
        }
        chain.reverse();
        chain.push(own);
        let wrap = chain.len() > 1 || !modifiers.is_empty();
        let mut out = String::from(before);
        if wrap {
            out.push('N');
        }
        out.push_str(modifiers);
        chain.iter().for_each(|c| out.push_str(c));
        if wrap {
            out.push('E');
        }
        out.push_str(after);
        out
    }
}

/// Sphinx's `CPPDomain._check_type`: whether a role may name a declaration of an
/// object type, as the C++ domain's `object_types` list their roles.
pub(crate) fn cpp_role_takes(role: &str, objtype: &str) -> bool {
    match role {
        "any" | "identifier" => true,
        "class" | "struct" => matches!(objtype, "class" | "templateParam"),
        "union" => matches!(objtype, "union" | "templateParam"),
        "func" => objtype == "function",
        "member" | "var" => matches!(objtype, "member" | "functionParam" | "templateParam"),
        "type" => {
            matches!(objtype, "class" | "union" | "function" | "type" | "enum" | "templateParam")
        }
        "concept" | "enum" | "enumerator" => objtype == role,
        _ => true,
    }
}

/// What a C++ reference found.
enum CppOutcome {
    Found(Resolved),
    /// A name Sphinx leaves unlinked without a warning (in the std namespace, or
    /// qualified by a template parameter).
    NoUri,
    Missing,
}

impl Look<'_, '_> {
    /// Resolves a C++ reference of `role` (`class`, `func`, ...); a name Sphinx leaves
    /// unlinked resolves to no link, so it shows as written without a warning.
    pub(crate) fn resolve_cpp(&mut self, role: &str) -> Option<Resolved> {
        match self.lookup_cpp(role) {
            CppOutcome::Found(r) => Some(r),
            CppOutcome::NoUri => Some(Resolved { no_link: true, ..Resolved::default() }),
            CppOutcome::Missing => None,
        }
    }

    /// The C++ part of an `any` reference.
    pub(crate) fn resolve_cpp_any(&mut self) -> Option<Resolved> {
        match self.lookup_cpp("any") {
            CppOutcome::Found(r) => Some(r),
            CppOutcome::NoUri | CppOutcome::Missing => None,
        }
    }

    /// As the C++ domain's `_resolve_xref_inner`, from the context the reader parsed
    /// the target into (`cpp:name`, `cpp:rooted`, `cpp:tdecls`, `cpp:parent`,
    /// `cpp:query`, `cpp:own`, `cpp:unparsed`).
    fn lookup_cpp(&mut self, role: &str) -> CppOutcome {
        self.trace.note_key(LookupKind::Cpp, "");
        let q = self.query;
        let t = &self.view.tables.cpp;
        let text = q.context("cpp:name");
        if t.symbols.is_empty() || q.flag("cpp:unparsed") || text.is_empty() {
            return CppOutcome::Missing;
        }
        let names: Vec<CppName<'_>> = text
            .lines()
            .map(|line| {
                let (ident, args) = line.split_once('\x1f').unwrap_or((line, ""));
                CppName { ident, args }
            })
            .collect();
        let tdecls = q.context("cpp:tdecls");
        let mut query = CppQuery {
            rooted: q.flag("cpp:rooted"),
            tdecls: if tdecls.is_empty() { Vec::new() } else { tdecls.lines().collect() },
            role,
            matching: Matching { shorthand: true, match_self: true, recurse_anon: true },
            names,
            search_siblings: false,
        };
        let start = t.find_path(q.context("cpp:parent")).unwrap_or(CppSymbolId(0));
        let found = if q.context("cpp:query").is_empty() {
            query.search_siblings = !query.rooted && query.names.len() == 1;
            match t.find_name(start, &query) {
                Err(CppFail::TemplateParam) if role == "identifier" => return CppOutcome::NoUri,
                result => result.ok(),
            }
        } else {
            t.find_declaration(start, &query, q.context("cpp:query"), q.context("cpp:own"))
        };
        let declared = found.and_then(|s| t.symbol(s).declaration.as_ref().map(|d| (s, d)));
        let Some((s, declaration)) = declared else {
            let mut name = if query.rooted { "::".to_string() } else { String::new() };
            let parts: Vec<String> =
                query.names.iter().map(|n| format!("{}{}", n.ident, n.args)).collect();
            name.push_str(&parts.join("::"));
            let std = name == "std" || name.starts_with("std::");
            return if std { CppOutcome::NoUri } else { CppOutcome::Missing };
        };
        if !cpp_role_takes(role, &declaration.objtype) {
            let message = format!(
                ":cpp:{role}: names {}, a {}.",
                t.full_name(s, false),
                declaration.objtype
            );
            self.warning = Some(Unresolved::warning([
                "build.cpp.role",
                "ROLE FOR ANOTHER KIND OF DECLARATION",
                "ref.cpp",
                &message,
                "Use the role for that kind of declaration, or :cpp:any:.",
            ]));
        }
        let anchor = declaration.id.clone();
        CppOutcome::Found(Resolved { doc: Some(declaration.doc), anchor, ..Resolved::default() })
    }
}
