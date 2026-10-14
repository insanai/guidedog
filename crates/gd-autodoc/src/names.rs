//! Directive arguments: autodoc's `py_ext_sig_re`, and `_parse_name` with
//! `_resolve_name`, which find the module an object is in.

use crate::{Generator, ObjType, Severity};

/// A match of autodoc's `py_ext_sig_re`:
///
/// ```text
/// ^ ([\w.]+::)? ([\w.]+\.)? (\w+) \s* (?: \[\s*(.*?)\s*])?
///   (?: \((.*)\) (?:\s* -> \s* (.*))? )? $
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SigMatch<'a> {
    /// The module before `::`.
    pub(crate) explicit_module: Option<&'a str>,
    /// With its trailing dot; `""` when absent.
    pub(crate) path: &'a str,
    pub(crate) base: &'a str,
    pub(crate) type_params: &'a str,
    /// With parentheses.
    pub(crate) args: Option<&'a str>,
    pub(crate) retann: Option<&'a str>,
}

fn is_word(r: char) -> bool {
    r == '_' || r.is_alphanumeric()
}

/// How many bytes at the start of `s` are word characters, or dots too.
fn word_run(s: &str, dots: bool) -> usize {
    s.char_indices()
        .find(|&(_, r)| !(is_word(r) || (dots && r == '.')))
        .map_or(s.len(), |(i, _)| i)
}

/// Matches `s` against `py_ext_sig_re`.
pub(crate) fn match_signature(s: &str) -> Option<SigMatch<'_>> {
    let mut m = SigMatch::default();
    let mut rest = s;
    if let Some(k) = rest.find("::").filter(|&k| k > 0 && word_run(&rest[..k], true) == k) {
        m.explicit_module = Some(&rest[..k]);
        rest = &rest[k + 2..];
    }
    let n = word_run(rest, true);
    let dotted = &rest[..n];
    rest = &rest[n..];
    if dotted.is_empty() || dotted.ends_with('.') {
        return None;
    }
    match dotted.rfind('.') {
        Some(dot) => (m.path, m.base) = (&dotted[..=dot], &dotted[dot + 1..]),
        None => m.base = dotted,
    }
    if word_run(m.base, false) != m.base.len() {
        return None;
    }
    rest = rest.trim_start();
    if let Some(after) = rest.strip_prefix('[') {
        let close = after.find(']')?;
        m.type_params = after[..close].trim();
        rest = &after[close + 1..];
    }
    if rest.is_empty() {
        return Some(m);
    }
    if !rest.starts_with('(') {
        return None;
    }
    // (.*) is greedy: the last ")" after which the rest is empty or "-> annotation".
    for (k, _) in rest.match_indices(')').rev().filter(|&(k, _)| k > 0) {
        let after = rest[k + 1..].trim_start();
        if after.is_empty() {
            m.args = Some(&rest[..=k]);
            return Some(m);
        }
        if let Some(annotation) = after.strip_prefix("->") {
            m.args = Some(&rest[..=k]);
            m.retann = Some(annotation.trim_start());
            return Some(m);
        }
    }
    None
}

/// What `_parse_name` returns: the module to import, the attribute path, and the
/// signature written in the directive's argument, if any.
#[derive(Debug, Clone, Default)]
pub(crate) struct ParsedName {
    pub(crate) module: String,
    pub(crate) parts: Vec<String>,
    pub(crate) args: Option<String>,
    pub(crate) retann: Option<String>,
}

/// `_parse_name` with `_resolve_name`: the module an object is in, from its name as
/// written and the context (the current module and class); `None`, reported, when
/// there is none.
pub(crate) fn parse_name(g: &mut Generator<'_>, name: &str, t: ObjType) -> Option<ParsedName> {
    let Some(m) = match_signature(name) else {
        g.report(
            Severity::Warning,
            "autodoc.signature",
            "INVALID AUTODOC SIGNATURE",
            format!("invalid signature for auto{} ({}).", t.name(), quote(name)),
            "Write a dotted name, optionally with a signature: module.Class.method(args).",
        );
        return None;
    };
    if t == ObjType::Module && (m.args.is_some() || m.retann.is_some()) {
        let what = if m.args.is_some() { "signature arguments" } else { "return annotation" };
        g.report(
            Severity::Warning,
            "autodoc.signature",
            "AUTOMODULE TAKES NO SIGNATURE",
            format!("{what} given for automodule: {}", quote(name)),
            "Name the module alone: .. automodule:: package.module",
        );
        return None;
    }
    let mut out = ParsedName {
        args: m.args.map(str::to_string),
        retann: m.retann.map(str::to_string),
        ..ParsedName::default()
    };
    let resolved = resolve_name(g, t, &mut out, &m);
    if !resolved || (out.module.is_empty() && t != ObjType::Module) {
        g.report(
            Severity::Warning,
            "autodoc.module",
            "NO MODULE FOR AUTODOC",
            format!("I don't know which module to import for autodocumenting {}.", quote(name)),
            "Place a module or currentmodule directive in the document, or give an explicit \
             module name, like module.Name or module::Class.method.",
        );
        return None;
    }
    Some(out)
}

fn resolve_name(g: &Generator<'_>, t: ObjType, out: &mut ParsedName, m: &SigMatch<'_>) -> bool {
    let parents: Vec<String> = match m.explicit_module {
        Some(_) if !m.path.is_empty() => {
            m.path.trim_end_matches('.').split('.').map(str::to_string).collect()
        }
        _ => Vec::new(),
    };
    if let Some(explicit) = m.explicit_module {
        out.module = explicit.to_string();
    }
    match t {
        ObjType::Module => {
            out.module = format!("{}{}", m.path, m.base);
            true
        }
        ObjType::Class
        | ObjType::Exception
        | ObjType::Function
        | ObjType::Decorator
        | ObjType::Data
        | ObjType::Type => {
            out.parts.extend(parents);
            out.parts.push(m.base.to_string());
            if m.explicit_module.is_none() {
                out.module = if !m.path.is_empty() {
                    m.path.trim_end_matches('.').to_string()
                } else {
                    first_set(&g.autodoc_module, &g.request.module).to_string()
                };
            }
            true
        }
        ObjType::Method | ObjType::Property | ObjType::Attribute => {
            if m.explicit_module.is_some() {
                out.parts.extend(parents);
                out.parts.push(m.base.to_string());
                return true;
            }
            resolve_member_name(g, out, m)
        }
    }
}

/// A method or attribute named without `::`: its class is the path, or the current one.
fn resolve_member_name(g: &Generator<'_>, out: &mut ParsedName, m: &SigMatch<'_>) -> bool {
    let mod_cls = if m.path.is_empty() {
        first_set(&g.autodoc_class, &g.request.class)
    } else {
        m.path.trim_end_matches('.')
    };
    if mod_cls.is_empty() {
        return false;
    }
    let (module, cls) = mod_cls.rsplit_once('.').unwrap_or(("", mod_cls));
    let module =
        if module.is_empty() { first_set(&g.autodoc_module, &g.request.module) } else { module };
    out.module = module.to_string();
    out.parts.push(cls.to_string());
    out.parts.push(m.base.to_string());
    true
}

/// `a` unless it is empty, else `b`.
fn first_set<'s>(a: &'s str, b: &'s str) -> &'s str {
    if a.is_empty() { b } else { a }
}

/// Python's `repr` of a name, as Sphinx's messages show it.
pub(crate) fn quote(s: &str) -> String {
    format!("'{s}'")
}
