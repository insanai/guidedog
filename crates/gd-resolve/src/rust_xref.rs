//! Rust references (new in the Rust port): `:rust:struct:`, `:rust:fn:`, ... resolve
//! objects of the `rust` domain by their full `::` path, as gd-rustdoc writes them
//! (`my_crate::parser::Parser::new`).
//!
//! The search, like the Python domain's: the current module first (`rust:module`, then
//! each enclosing module outward), then the path as written; `crate::`, `self::` and
//! `super::` are read as in Rust code. A leading `.` (or `rust:any`) also searches by
//! suffix: any object whose path ends with `::<name>`. `~` only changes what the
//! reference shows. What none of this finds, `resolve_xref` looks up in the
//! inventories.

use crate::lookups::Look;
use crate::xref::Resolved;

/// The rust domain's object types (gd-rustdoc's directives) and the roles that name
/// them; `any`, `obj` and an empty role take every type.
pub(crate) const RUST_OBJECT_TYPES: &[(&str, &str)] = &[
    ("crate", "crate mod"),
    ("module", "mod"),
    ("function", "fn"),
    ("method", "fn meth"),
    ("struct", "struct type"),
    ("enum", "enum type"),
    ("variant", "variant"),
    ("union", "union type"),
    ("trait", "trait type"),
    ("type", "type"),
    ("associatedtype", "type"),
    ("const", "const"),
    ("associatedconst", "const"),
    ("static", "static"),
    ("macro", "macro"),
    ("field", "field"),
    ("impl", "impl"),
    ("use", "use"),
];

/// Whether a rust role may name an object of a type.
pub(crate) fn rust_role_takes(role: &str, objtype: &str) -> bool {
    if matches!(role, "any" | "obj" | "") {
        return true;
    }
    match RUST_OBJECT_TYPES.iter().find(|(t, _)| *t == objtype) {
        Some((_, roles)) => roles.split(' ').any(|r| r == role),
        None => role == objtype,
    }
}

impl Look<'_, '_> {
    /// Resolves a `rust` domain reference; see the module documentation.
    pub(crate) fn resolve_rust(&mut self, reftype: &str) -> Option<Resolved> {
        let q = self.query;
        let written = q.target.trim_start_matches(['~', '!']);
        let suffix_search = written.starts_with('.') || q.refspecific || reftype == "rust:any";
        let name = written.trim_start_matches('.');
        let module = q.context("rust:module");
        for path in candidate_paths(module, name) {
            // As Sphinx's exact search, a whole path is found whatever its type.
            if let Some(o) = self.object(&format!("rust:{path}")) {
                return Some(Look::at(o));
            }
        }
        if suffix_search {
            return self.suffix("rust:", &format!("::{name}"), reftype).map(Look::at);
        }
        None
    }
}

/// The full paths a name written in `module` may stand for, most specific first:
/// `crate::`, `self::` and `super::` read as in Rust; otherwise the name below the
/// module and each module enclosing it, then the name as written.
pub(crate) fn candidate_paths(module: &str, name: &str) -> Vec<String> {
    let parts: Vec<&str> = module.split("::").filter(|p| !p.is_empty()).collect();
    if let Some(rest) = name.strip_prefix("crate::") {
        let root = parts.first().copied().unwrap_or("");
        return vec![if root.is_empty() { rest.to_string() } else { format!("{root}::{rest}") }];
    }
    let mut depth = parts.len();
    let mut rest = name;
    let mut relative = false;
    if let Some(r) = rest.strip_prefix("self::") {
        rest = r;
        relative = true;
    }
    while let Some(r) = rest.strip_prefix("super::") {
        rest = r;
        depth = depth.saturating_sub(1);
        relative = true;
    }
    if relative {
        let mut path = parts[..depth].join("::");
        if !path.is_empty() {
            path.push_str("::");
        }
        path.push_str(rest);
        return vec![path];
    }
    let mut out: Vec<String> =
        (1..=parts.len()).rev().map(|n| format!("{}::{name}", parts[..n].join("::"))).collect();
    out.push(name.to_string());
    out
}
