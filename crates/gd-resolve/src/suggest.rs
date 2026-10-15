//! The warning for a reference nothing resolves: whether it warns (Sphinx's
//! `warn_dangling`, `nitpicky`, `nitpick_ignore`), the hint, and the closest name of the
//! right kind for "Did you mean ...?".

use crate::external::xref_domain;
use crate::lookups::role_takes;
use crate::view::{BuildView, dir_of, doc_target};
use crate::xref::{Unresolved, XrefQuery};
use gd_env::normalize_name;

/// The failure of a reference nothing resolves.
pub(crate) fn unresolved(view: &BuildView<'_>, q: &XrefQuery) -> Box<Unresolved> {
    let role = q.reftype.rsplit(':').next().unwrap_or(&q.reftype);
    let warn = must_resolve(view, q);
    Box::new(Unresolved {
        warn,
        code: "build.xref.unresolved".into(),
        // Sphinx's type "ref", with the role's name as the subtype: "func", not "py:func".
        category: format!("ref.{role}"),
        title: "REFERENCE TARGET NOT FOUND".into(),
        message: format!(
            "The :{}: reference to `{}` has no target in this project.",
            q.reftype, q.target
        ),
        hint: hint_for(&q.reftype).into(),
        // Only a warning shows the suggestion, so only a warning pays for the search.
        suggestion: if warn { suggestion(view, q) } else { None },
    })
}

/// Which references warn when unresolved, as Sphinx's roles declare `warn_dangling`:
/// ref, numref, doc, keyword, term, option, any, and the math and download roles
/// always; the others when nitpicky. In nitpicky mode every unresolved reference
/// warns, except the (type, target) pairs of `nitpick_ignore` and
/// `nitpick_ignore_regex`; a standard role's type may be written with or without `std:`.
pub(crate) fn must_resolve(view: &BuildView<'_>, q: &XrefQuery) -> bool {
    let c = view.config;
    let reftype = q.reftype.as_str();
    let target = q.target.as_str();
    if !view.options.nitpicky && !c.nitpicky {
        return matches!(
            reftype,
            "ref"
                | "numref"
                | "doc"
                | "keyword"
                | "term"
                | "option"
                | "any"
                | "download"
                | "eq"
                | "math:numref"
        );
    }
    let standard = !domain_role(reftype);
    let typed = if standard { format!("std:{reftype}") } else { reftype.to_string() };
    let ignored = c
        .nitpick_ignore
        .iter()
        .any(|(kind, t)| t == target && (*kind == typed || (standard && kind == reftype)));
    let matched = c.nitpick_ignore_regex.iter().any(|(kind, t)| {
        t.full_match(target) && (kind.full_match(&typed) || (standard && kind.full_match(reftype)))
    });
    !ignored && !matched
}

/// Whether a role belongs to a domain (`py:func`), as opposed to the standard roles.
pub(crate) fn domain_role(reftype: &str) -> bool {
    reftype.contains(':') && reftype != "math:numref"
}

fn hint_for(reftype: &str) -> &'static str {
    match reftype {
        "ref" | "numref" => "Define the label with .. _name: before a section, figure, or table.",
        "doc" => "Name the document relative to this one, or from the source root with /.",
        "term" => "Add the term to a glossary directive.",
        "eq" | "math:numref" => "Give the equation a :label: in its math directive.",
        "download" => "Check the file's path: it is relative to the document.",
        r if r.starts_with("rust:") => {
            "Write the item's full path (my_crate::module::Item), or one relative to the \
             current rust:module; start it with . to search by its last part."
        }
        _ => "Check the spelling, or describe the object with a domain directive.",
    }
}

/// The closest name a reference of this kind could have meant.
fn suggestion(view: &BuildView<'_>, q: &XrefQuery) -> Option<String> {
    let t = view.tables;
    let target = q.target.trim();
    match q.reftype.as_str() {
        "ref" | "numref" | "keyword" | "eq" | "math:numref" => {
            closest(&normalize_name(target), t.labels.keys().map(String::as_str))
        }
        "term" => closest(&normalize_name(target), t.terms.keys().map(String::as_str)),
        "doc" => {
            let base = dir_of(&view.doc(q.from).docname);
            closest(&doc_target(base, target), t.by_name.keys().map(String::as_str))
        }
        "any" => {
            let names = t.labels.keys().chain(t.by_name.keys()).chain(t.terms.keys());
            closest(&normalize_name(target), names.map(String::as_str))
        }
        "download" => None,
        reftype => object_suggestion(view, q, reftype),
    }
}

/// The closest object of the role's domain and types, by full name or by the name
/// relative to the reference's module.
fn object_suggestion(view: &BuildView<'_>, q: &XrefQuery, reftype: &str) -> Option<String> {
    let (domain, _) = xref_domain(reftype);
    if domain.is_empty() || domain == "cpp" {
        return None;
    }
    let declared = view.config.declared_object_type(reftype);
    let prefix = match (domain, declared) {
        (_, Some(t)) => format!("std:{}:", t.directive),
        ("std", None) if reftype == "option" => "std:cmdoption:".to_string(),
        _ => format!("{domain}:"),
    };
    let name = q.target.trim().trim_start_matches(['~', '.', '!']);
    let module = q.context(&format!("{domain}:module"));
    let separator = if domain == "rust" { "::" } else { "." };
    let mut names: Vec<&str> = Vec::new();
    for (key, o) in &view.tables.objects {
        let Some(full) = key.strip_prefix(&prefix) else { continue };
        if !role_takes(reftype, &o.objtype) {
            continue;
        }
        names.push(full);
        if let Some(relative) = full.strip_prefix(module).and_then(|r| r.strip_prefix(separator)) {
            names.push(relative);
        }
    }
    names.sort_unstable(); // the first of equally close names, every build.
    closest(name, names.into_iter())
}

fn closest<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut sorted: Vec<&str> = candidates.collect();
    sorted.sort_unstable();
    gd_report::suggest(name, sorted).map(str::to_string)
}
