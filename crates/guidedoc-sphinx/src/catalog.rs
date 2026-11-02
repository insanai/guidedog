//! The catalog of the directives and roles a read with a given configuration knows:
//! Docutils' standard ones, Sphinx's own, every domain's, MyST's `eval-rst`, and the
//! project's declarations. Editors and capability reports list them from here, so they
//! agree with the reader instead of keeping tables of their own.

use guidedoc_rst::{ArgRule, ContentRule, Extension, OptSpec};

use crate::config::Config;
use crate::directives::CORE;
use crate::domains::{DOMAINS, DirectiveKind};
use crate::extension::Sphinx;
use crate::roles::CORE_ROLES;
use crate::text::equal_fold;

/// Where a directive or role is defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Origin {
    /// Docutils' standard reStructuredText.
    Docutils,
    /// Sphinx's core layer (toctree, code-block, ...).
    Sphinx,
    /// A Sphinx domain, by name ("py", "std", ...).
    Domain(&'static str),
    /// MyST Markdown only (`eval-rst`).
    Myst,
    /// The project's configuration: an object type, a directive alias, an extlink.
    Project,
}

/// What a directive or role needs besides the reader itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Requirement {
    /// Nothing: it is always available.
    Nothing,
    /// An autodoc provider for the named domain (or for [`crate::INHERITANCE_DIAGRAM`]),
    /// which the host supplies when the project enables the extension.
    Autodoc(&'static str),
    /// `sphinx.ext.intersphinx` (the `external` roles).
    Intersphinx,
    /// A declaration in the project's configuration, which the configuration has.
    Declaration,
}

/// One directive a read knows, with the rules the reader checks before it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectiveInfo {
    /// The canonical name, such as `note` or `py:function`.
    pub name: String,
    /// Where it is defined.
    pub origin: Origin,
    /// What it needs.
    pub requirement: Requirement,
    /// The configuration meets the requirement, so a read runs the directive.
    pub enabled: bool,
    /// Whether it takes an argument.
    pub arg: ArgRule,
    /// The last argument may contain white space.
    pub final_ws: bool,
    /// Whether it takes content.
    pub content: ContentRule,
    /// The options it accepts.
    pub options: Vec<OptSpec>,
}

/// One interpreted text role a read knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleInfo {
    /// The name, such as `emphasis`, `ref` or `py:func`; `external` stands for every
    /// `external:...` role.
    pub name: String,
    /// Where it is defined.
    pub origin: Origin,
    /// What it needs.
    pub requirement: Requirement,
    /// The configuration meets the requirement.
    pub enabled: bool,
}

/// Lists every directive a read with `config` knows: Sphinx's core directives (which
/// replace Docutils' directives of the same name), each domain's directives under their
/// full names, the Docutils directives Sphinx does not replace, `eval-rst`, then the
/// project's object types and directive aliases. Directives that need a missing
/// autodoc provider are listed with `enabled: false`.
pub fn directives(config: &Config) -> Vec<DirectiveInfo> {
    let mut out: Vec<DirectiveInfo> = Vec::new();
    for c in CORE {
        let requirement = core_requirement(c.name);
        out.push(DirectiveInfo {
            name: c.name.to_string(),
            origin: Origin::Sphinx,
            requirement,
            enabled: is_met(config, requirement),
            arg: c.arg,
            final_ws: c.final_ws,
            content: c.content,
            options: c.options.to_vec(),
        });
    }
    for domain in DOMAINS {
        for d in domain.directives() {
            let requirement = match d.kind {
                DirectiveKind::Autodoc => Requirement::Autodoc(domain.name()),
                _ => Requirement::Nothing,
            };
            out.push(DirectiveInfo {
                name: d.name.to_string(),
                origin: Origin::Domain(domain.name()),
                requirement,
                enabled: is_met(config, requirement),
                arg: d.arg,
                final_ws: d.final_ws,
                content: d.content,
                options: d.options.to_vec(),
            });
        }
    }
    for h in guidedoc_rst::standard_directives() {
        if !out.iter().any(|d| equal_fold(&d.name, h.name)) {
            out.push(info(h, Origin::Docutils));
        }
    }
    out.push(eval_rst());
    out.extend(declared_directives(config));
    out
}

/// Finds the directive `name` as a read with `config` finds it outside any
/// `default-domain`: Sphinx's core, a full domain name, the primary domain, the
/// standard domain, the project's declarations, then Docutils. `None` for a name no
/// read knows (`eval-rst` is found only in Markdown, and so not here).
pub fn find_directive(config: &Config, name: &str) -> Option<DirectiveInfo> {
    let sphinx = Sphinx::new(config);
    if let Some(h) = sphinx.directive(name) {
        let found = directives(config).into_iter().find(|d| equal_fold(&d.name, h.name));
        return Some(found.unwrap_or_else(|| info(h, Origin::Project)));
    }
    guidedoc_rst::find_standard_directive(name).map(|h| info(h, Origin::Docutils))
}

/// Lists every role a read with `config` knows: Docutils' standard roles, Sphinx's
/// generic roles, each domain's roles under their full names, `autolink`, the
/// `external` roles, and the project's object type roles and extlinks.
pub fn roles(config: &Config) -> Vec<RoleInfo> {
    let mut out = Vec::new();
    let mut push = |name: String, origin: Origin, requirement: Requirement| {
        let enabled = is_met(config, requirement);
        out.push(RoleInfo { name, origin, requirement, enabled });
    };
    for (name, _) in guidedoc_rst::standard_roles() {
        push(name.to_string(), Origin::Docutils, Requirement::Nothing);
    }
    for r in CORE_ROLES {
        push(r.name.to_string(), Origin::Sphinx, Requirement::Nothing);
    }
    for domain in DOMAINS {
        for r in domain.roles() {
            let name = format!("{}:{}", domain.name(), r.name);
            push(name, Origin::Domain(domain.name()), Requirement::Nothing);
        }
    }
    push("autolink".to_string(), Origin::Sphinx, Requirement::Autodoc("py"));
    push("external".to_string(), Origin::Sphinx, Requirement::Intersphinx);
    for t in config.object_types.iter().filter(|t| !t.role.is_empty()) {
        push(t.role.clone(), Origin::Project, Requirement::Declaration);
    }
    for e in &config.extlinks {
        push(e.name.clone(), Origin::Project, Requirement::Declaration);
    }
    out
}

fn info(h: guidedoc_rst::DirectiveHandler<'_>, origin: Origin) -> DirectiveInfo {
    let requirement =
        if origin == Origin::Project { Requirement::Declaration } else { Requirement::Nothing };
    DirectiveInfo {
        name: h.name.to_string(),
        origin,
        requirement,
        enabled: true,
        arg: h.arg,
        final_ws: h.final_ws,
        content: h.content,
        options: h.options.to_vec(),
    }
}

/// The core directives that generate their text through an autodoc provider.
fn core_requirement(name: &str) -> Requirement {
    match name {
        "inheritance-diagram" => Requirement::Autodoc(crate::INHERITANCE_DIAGRAM),
        n if n.starts_with("auto") => Requirement::Autodoc("py"),
        _ => Requirement::Nothing,
    }
}

fn is_met(config: &Config, requirement: Requirement) -> bool {
    match requirement {
        Requirement::Nothing | Requirement::Declaration => true,
        Requirement::Autodoc(domain) => config.autodoc_for(domain).is_some(),
        Requirement::Intersphinx => config.intersphinx,
    }
}

fn eval_rst() -> DirectiveInfo {
    DirectiveInfo {
        name: "eval-rst".to_string(),
        origin: Origin::Myst,
        requirement: Requirement::Nothing,
        enabled: true,
        arg: ArgRule::None,
        final_ws: false,
        content: ContentRule::Required,
        options: Vec::new(),
    }
}

/// The project's object types and directive aliases, with the rules of the directive
/// each one runs.
fn declared_directives(config: &Config) -> Vec<DirectiveInfo> {
    let mut out = Vec::new();
    let sphinx = Sphinx::new(config);
    let names = config.object_types.iter().map(|t| &t.directive);
    for name in names.chain(config.directive_aliases.iter().map(|a| &a.name)) {
        if let Some(h) = sphinx.directive(name) {
            out.push(DirectiveInfo { name: name.clone(), ..info(h, Origin::Project) });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DirectiveAlias, Extlink, ObjectType};

    #[test]
    fn sphinx_replaces_docutils_and_lists_domains() {
        let list = directives(&Config::default());
        let code = list.iter().filter(|d| d.name == "code-block").collect::<Vec<_>>();
        assert_eq!(code.len(), 1);
        assert_eq!(code[0].origin, Origin::Sphinx);
        let note = list.iter().find(|d| d.name == "note").unwrap();
        assert_eq!(note.origin, Origin::Docutils);
        assert!(note.options.iter().any(|o| o.name == "class"));
        assert!(list.iter().any(|d| d.name == "py:function"));
        let auto = list.iter().find(|d| d.name == "automodule").unwrap();
        assert_eq!((auto.requirement, auto.enabled), (Requirement::Autodoc("py"), false));
    }

    #[test]
    fn declarations_and_lookup_follow_the_configuration() {
        let config = Config {
            object_types: vec![ObjectType { directive: "setting".into(), ..Default::default() }],
            directive_aliases: vec![DirectiveAlias {
                name: "awaitable".into(),
                target: "py:function".into(),
            }],
            extlinks: vec![Extlink { name: "issue".into(), ..Default::default() }],
            ..Config::default()
        };
        let list = directives(&config);
        assert!(list.iter().any(|d| d.name == "setting" && d.origin == Origin::Project));
        assert!(list.iter().any(|d| d.name == "awaitable" && d.arg == ArgRule::Required));
        assert_eq!(find_directive(&config, "function").unwrap().name, "py:function");
        assert_eq!(find_directive(&config, "IMAGE").unwrap().origin, Origin::Docutils);
        assert!(find_directive(&config, "no-such").is_none());
        let roles = roles(&config);
        assert!(roles.iter().any(|r| r.name == "issue" && r.origin == Origin::Project));
        assert!(roles.iter().any(|r| r.name == "py:func"));
        assert!(roles.iter().any(|r| r.name == "emphasis" && r.origin == Origin::Docutils));
    }
}
