//! References resolved through other projects' inventories,
//! as Sphinx's `intersphinx/_resolve.py`: a reference this project cannot resolve is
//! looked up in the merged inventory by the object types its role may name, except the
//! disabled ones; then, if its target is `name:target` and `name` is an inventory, in
//! that inventory alone. `:external:` references look only in the inventories.

use gd_inventory::{Inventory, Item};

use crate::rust_xref::RUST_OBJECT_TYPES;
use crate::view::{BuildView, python_format};
use crate::xref::{Resolved, Unresolved, XrefQuery};

/// One of a domain's object types and the roles that name it, in the order the domain
/// declares them (Sphinx's `object_types`).
const OBJECT_TYPES: &[(&str, &str, &str)] = &[
    ("c", "member", "var member data identifier"),
    ("c", "var", "var member data identifier"),
    ("c", "function", "func identifier type"),
    ("c", "macro", "macro identifier"),
    ("c", "struct", "struct identifier type"),
    ("c", "union", "union identifier type"),
    ("c", "enum", "enum identifier type"),
    ("c", "enumerator", "enumerator identifier"),
    ("c", "type", "identifier type"),
    ("c", "functionParam", "identifier var member data"),
    ("cpp", "class", "class struct identifier type"),
    ("cpp", "union", "union identifier type"),
    ("cpp", "function", "func identifier type"),
    ("cpp", "member", "member var identifier"),
    ("cpp", "type", "identifier type"),
    ("cpp", "concept", "concept identifier"),
    ("cpp", "enum", "enum identifier type"),
    ("cpp", "enumerator", "enumerator identifier"),
    ("cpp", "functionParam", "identifier member var"),
    ("cpp", "templateParam", "identifier class struct union member var type"),
    ("js", "function", "func"),
    ("js", "method", "meth"),
    ("js", "class", "class"),
    ("js", "data", "data"),
    ("js", "attribute", "attr"),
    ("js", "module", "mod"),
    ("odin", "package", "pkg obj"),
    ("odin", "procedure", "proc obj"),
    ("odin", "procgroup", "proc obj"),
    ("odin", "struct", "type obj"),
    ("odin", "union", "type obj"),
    ("odin", "enum", "type obj"),
    ("odin", "bitset", "type obj"),
    ("odin", "bitfield", "type obj"),
    ("odin", "type", "type obj"),
    ("odin", "const", "const obj"),
    ("odin", "var", "var obj"),
    ("odin", "field", "field obj"),
    ("odin", "enumerator", "enumerator obj"),
    ("odin", "foreign", "obj"),
    ("py", "function", "func obj"),
    ("py", "data", "data obj"),
    ("py", "class", "class exc obj"),
    ("py", "exception", "exc class obj"),
    ("py", "method", "meth obj"),
    ("py", "classmethod", "meth obj"),
    ("py", "staticmethod", "meth obj"),
    ("py", "attribute", "attr obj"),
    ("py", "property", "attr _prop obj"),
    ("py", "type", "type class obj"),
    ("py", "module", "mod obj"),
    ("rst", "directive", "dir"),
    ("rst", "directive:option", "dir"),
    ("rst", "role", "role"),
    ("std", "term", "term"),
    ("std", "token", "token"),
    ("std", "label", "ref keyword"),
    ("std", "confval", "confval"),
    ("std", "envvar", "envvar"),
    ("std", "cmdoption", "option"),
    ("std", "doc", "doc"),
];

/// The domains in Sphinx's sorted order, for `any` references.
const DOMAINS: [&str; 8] = ["c", "cpp", "js", "odin", "py", "rst", "rust", "std"];

/// Splits a reference type into Sphinx's refdomain and reftype: the standard roles
/// belong to `std`; `download` and `any` to no domain.
pub(crate) fn xref_domain(reftype: &str) -> (&str, &str) {
    if let Some(split) = reftype.split_once(':') {
        return split;
    }
    match reftype {
        "ref" | "numref" | "doc" | "keyword" | "term" | "option" | "envvar" | "token"
        | "confval" => ("std", reftype),
        "eq" => ("math", reftype),
        _ => ("", reftype),
    }
}

/// The `domain:objtype` names a role may resolve to (all of the domain's when `role`
/// is empty), with Sphinx's compatibility additions: `std:option` for options,
/// `py:method` for attributes.
pub(crate) fn object_types(domain: &str, role: &str) -> Vec<String> {
    let rust = RUST_OBJECT_TYPES.iter().map(|(t, roles)| ("rust", *t, *roles));
    let mut out: Vec<String> = OBJECT_TYPES
        .iter()
        .copied()
        .chain(rust)
        .filter(|(d, _, roles)| {
            *d == domain && (role.is_empty() || roles.split(' ').any(|r| r == role))
        })
        .map(|(d, t, _)| format!("{d}:{t}"))
        .collect();
    let has = |out: &[String], t: &str| out.iter().any(|x| x == t);
    if domain == "std" && has(&out, "std:cmdoption") {
        out.push("std:option".into());
    }
    if domain == "py" && has(&out, "py:attribute") && !has(&out, "py:method") {
        out.push("py:method".into());
    }
    out
}

/// Sphinx's `missing_reference` (`external` false) and its `IntersphinxRoleResolver`
/// (`external` true, for `:external:` roles).
pub(crate) fn resolve_external(
    view: &BuildView<'_>,
    q: &XrefQuery,
    external: bool,
) -> Option<Resolved> {
    let it = view.intersphinx;
    if external {
        if !q.inventory.is_empty() {
            let store = it.named.get(&q.inventory)?;
            return resolve_in(view, q, &q.inventory, store, false, &q.target);
        }
        return resolve_in(view, q, "", &it.main, false, &q.target);
    }
    if let Some(found) = resolve_in(view, q, "", &it.main, true, &q.target) {
        return Some(found);
    }
    let (name, rest) = q.target.split_once(':')?;
    let resolve_self = &view.config.intersphinx.resolve_self;
    if !resolve_self.is_empty() && name == resolve_self {
        return None;
    }
    let store = it.named.get(name)?;
    resolve_in(view, q, name, store, false, rest)
}

/// Sphinx's `_resolve_reference`: disabled reference types apply only when no
/// inventory is named. A domain's own `any` role (`rust:any`) searches that domain;
/// `any` searches every domain.
fn resolve_in(
    view: &BuildView<'_>,
    q: &XrefQuery,
    inventory: &str,
    store: &Inventory,
    honor_disabled: bool,
    target: &str,
) -> Option<Resolved> {
    let disabled = &view.config.intersphinx.disabled_reftypes;
    let honor = honor_disabled && inventory.is_empty();
    let is_disabled = |name: &str| honor && disabled.iter().any(|d| d == name);
    if is_disabled("*") {
        return None;
    }
    let (domain, role) = xref_domain(&q.reftype);
    let domains: Vec<&str> = match (domain, role) {
        ("rust", "any") => vec!["rust"],
        (_, "any") => DOMAINS.to_vec(),
        ("", _) => return None,
        _ => vec![domain],
    };
    for d in domains {
        if is_disabled(&format!("{d}:*")) {
            continue;
        }
        let types = object_types(d, if role == "any" { "" } else { role });
        let wanted: Vec<&str> =
            types.iter().map(String::as_str).filter(|t| !is_disabled(t)).collect();
        if let Some(found) = resolve_in_domain(view, q, inventory, store, d, &wanted, target) {
            return Some(found);
        }
    }
    None
}

/// Tries the target as written, then qualified by the reference's scope (the
/// domain's `get_full_qualified_name`).
fn resolve_in_domain(
    view: &BuildView<'_>,
    q: &XrefQuery,
    inventory: &str,
    store: &Inventory,
    domain: &str,
    types: &[&str],
    target: &str,
) -> Option<Resolved> {
    let name = target.trim_start_matches('~');
    let link = |item: &Item| external_link(view, q, inventory, domain, item);
    if let Some(item) = find_in(store, types, name) {
        return Some(link(item));
    }
    let full = qualified_name(q, domain, name)?;
    find_in(store, types, &full).map(link)
}

/// The domain's `get_full_qualified_name`: Python and JavaScript join module, class
/// or object, and target; Rust the module path; C++ the parent scope; options their
/// program.
fn qualified_name(q: &XrefQuery, domain: &str, target: &str) -> Option<String> {
    match domain {
        "py" | "js" => {
            let keys = if domain == "py" {
                ["py:module", "py:class"]
            } else {
                ["js:module", "js:object"]
            };
            let mut parts: Vec<&str> =
                keys.iter().map(|k| q.context(k)).filter(|v| !v.is_empty()).collect();
            parts.push(target);
            Some(parts.join("."))
        }
        "rust" => {
            let module = q.context("rust:module");
            (!module.is_empty()).then(|| format!("{module}::{target}"))
        }
        "odin" => {
            let package = q.context("odin:package");
            (!package.is_empty()).then(|| format!("{package}.{target}"))
        }
        "cpp" => {
            let parent = q.context("cpp:parent");
            (!parent.is_empty()).then(|| format!("{parent}::{target}"))
        }
        "std" if xref_domain(&q.reftype).1 == "option" => {
            let program = q.context("std:program");
            let mut command: Vec<&str> = Vec::new();
            if !program.is_empty() {
                command.push(program);
            }
            command.extend(target.split_whitespace());
            if command.len() < 2 {
                return None;
            }
            let option = command.pop()?;
            Some(format!("{}.{option}", command.join("-")))
        }
        _ => None,
    }
}

/// Sphinx's `_resolve_reference_in_domain_by_target`: the first object type that has
/// the target; labels and terms also match ignoring case.
fn find_in<'s>(store: &'s Inventory, types: &[&str], target: &str) -> Option<&'s Item> {
    types.iter().find_map(|t| store.find(t, target).or_else(|| store.find_folded(t, target)))
}

/// Sphinx's `_create_element_from_result`.
fn external_link(
    view: &BuildView<'_>,
    q: &XrefQuery,
    inventory: &str,
    domain: &str,
    item: &Item,
) -> Resolved {
    let mut uri = item.uri.clone();
    if !uri.contains("://") && !uri.starts_with('/') {
        let depth = view.doc(q.from).docname.matches('/').count();
        uri = format!("{}{uri}", "../".repeat(depth));
    }
    let project = item_project(view, inventory, item);
    let tooltip = match project.1 {
        "" => python_format(&view.ui.gettext("(in %s)"), &[project.0]),
        version => {
            let v = if version.starts_with(|c: char| c.is_ascii_digit()) {
                format!("v{version}")
            } else {
                version.to_string()
            };
            python_format(&view.ui.gettext("(in %s %s)"), &[project.0, &v])
        }
    };
    let keep = Resolved {
        url: uri.clone(),
        external: tooltip.clone(),
        keep_content: true,
        ..Resolved::default()
    };
    if q.explicit {
        return keep;
    }
    let (_, role) = xref_domain(&q.reftype);
    if item.display == "-" || (domain == "std" && role == "keyword") {
        let prefix = format!("{inventory}:");
        if let Some(shown) = q.content.strip_prefix(&prefix).filter(|_| !inventory.is_empty()) {
            return Resolved { title: shown.to_string(), keep_content: false, ..keep };
        }
        return keep;
    }
    Resolved { title: item.display.clone(), keep_content: false, ..keep }
}

/// The project and version an item comes from: its inventory's.
fn item_project<'v>(view: &'v BuildView<'_>, inventory: &str, item: &Item) -> (&'v str, &'v str) {
    let it = view.intersphinx;
    let store = if inventory.is_empty() { it.origin(item) } else { it.named.get(inventory) };
    store.map_or(("", ""), |s| (s.project.as_str(), s.version.as_str()))
}

/// The warning for an `:external:` reference no inventory resolves, as Sphinx's.
pub(crate) fn external_missing(view: &BuildView<'_>, q: &XrefQuery) -> Box<Unresolved> {
    let (domain, role) = xref_domain(&q.reftype);
    if !q.inventory.is_empty() && !view.intersphinx.named.contains_key(&q.inventory) {
        let known = view.intersphinx.named.keys().map(String::as_str);
        let suggestion = gd_report::suggest(&q.inventory, known).map(str::to_string);
        let mut u = Unresolved::warning([
            "build.xref.external",
            "UNKNOWN INVENTORY",
            "intersphinx.external",
            &format!("inventory for external cross-reference not found: `{}`", q.inventory),
            "Name a project of intersphinx_mapping in conf.toml.",
        ]);
        u.suggestion = suggestion;
        return u;
    }
    Unresolved::warning([
        "build.xref.external",
        "EXTERNAL TARGET NOT FOUND",
        &format!("ref.{role}"),
        &format!("external {domain}:{role} reference target not found: {}", q.target),
        "Check the target against the other project's inventory.",
    ])
}
