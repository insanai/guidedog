//! Directives, roles and extensions, from the Sphinx layer's catalog and gd-conf's
//! lists, with what enables each in a project.

use guidedoc_rst::{ArgRule, ContentRule, OptType};
use guidedoc_sphinx::{Origin, Requirement};

use crate::{
    DirectiveCapability, EnabledBy, ExtensionCapability, OptionCapability, RoleCapability,
};

/// The Sphinx layer's configuration of `config`'s project, as its builds read.
fn sphinx(config: &gd_conf::Config) -> guidedoc_sphinx::Config {
    gd_read::sphinx_config(config, &guidedoc::Labels::default(), "")
}

pub(crate) fn directives(config: &gd_conf::Config) -> Vec<DirectiveCapability> {
    guidedoc_sphinx::directives(&sphinx(config))
        .into_iter()
        .map(|d| {
            let (enabled_by, extension, enabled) = availability(config, d.requirement);
            let formats =
                if d.origin == Origin::Myst { vec!["markdown"] } else { vec!["rst", "markdown"] };
            DirectiveCapability {
                name: d.name,
                origin: origin(d.origin),
                enabled_by,
                extension,
                enabled,
                formats,
                argument: argument(d.arg),
                content: content(d.content),
                options: d
                    .options
                    .iter()
                    .map(|o| OptionCapability {
                        name: o.name,
                        kind: option_kind(o.kind),
                        choices: o.choices.split_whitespace().collect(),
                    })
                    .collect(),
            }
        })
        .collect()
}

pub(crate) fn roles(config: &gd_conf::Config) -> Vec<RoleCapability> {
    guidedoc_sphinx::roles(&sphinx(config))
        .into_iter()
        .map(|r| {
            let (enabled_by, extension, enabled) = availability(config, r.requirement);
            RoleCapability {
                name: r.name,
                origin: origin(r.origin),
                enabled_by,
                extension,
                enabled,
            }
        })
        .collect()
}

/// What enables an entry with `requirement`, the setting, and whether `config` has it.
/// The autodoc providers are those the project reader attaches (gd-read): Python's when
/// autodoc is on, the class diagrams' with sphinx.ext.inheritance_diagram, Odin's always.
fn availability(
    config: &gd_conf::Config,
    requirement: Requirement,
) -> (EnabledBy, &'static str, bool) {
    match requirement {
        Requirement::Nothing => (EnabledBy::Default, "", true),
        Requirement::Declaration => (EnabledBy::Config, "", true),
        Requirement::Intersphinx => {
            let name = "sphinx.ext.intersphinx";
            (EnabledBy::Config, name, config.has_extension(name))
        }
        Requirement::Autodoc(guidedoc_sphinx::INHERITANCE_DIAGRAM) => {
            let name = "sphinx.ext.inheritance_diagram";
            (EnabledBy::Config, name, config.has_extension(name))
        }
        Requirement::Autodoc("odin") => (EnabledBy::Default, "", true),
        Requirement::Autodoc(_) => {
            (EnabledBy::Config, "sphinx.ext.autodoc", gd_read::uses_autodoc(config))
        }
    }
}

fn origin(origin: Origin) -> String {
    match origin {
        Origin::Docutils => "docutils".to_string(),
        Origin::Sphinx => "sphinx".to_string(),
        Origin::Domain(name) => format!("domain:{name}"),
        Origin::Myst => "myst".to_string(),
        Origin::Project => "project".to_string(),
    }
}

fn argument(rule: ArgRule) -> &'static str {
    match rule {
        ArgRule::None => "none",
        ArgRule::Optional => "optional",
        ArgRule::Required => "required",
    }
}

fn content(rule: ContentRule) -> &'static str {
    match rule {
        ContentRule::None => "none",
        ContentRule::Optional => "optional",
        ContentRule::Required => "required",
    }
}

/// The stable name of an option's value type.
fn option_kind(kind: OptType) -> &'static str {
    match kind {
        OptType::Flag => "flag",
        OptType::Text => "text",
        OptType::TextRequired => "text-required",
        OptType::Int => "int",
        OptType::NonnegInt => "nonnegative-int",
        OptType::PosInt => "positive-int",
        OptType::OptInt => "optional-int",
        OptType::Length => "length",
        OptType::LengthPercent => "length-or-percentage",
        OptType::Figwidth => "figure-width",
        OptType::Percentage => "percentage",
        OptType::Class => "class",
        OptType::Name => "name",
        OptType::Choice => "choice",
        OptType::Encoding => "encoding",
        OptType::Char => "character",
        OptType::Widths => "widths",
        OptType::Path => "path",
    }
}

/// The MyST syntax extensions, enabled as gd-read enables them: the names in
/// `myst_enable_extensions`, and dollarmath also by `myst_dmath_enable`.
pub(crate) fn markdown_extensions(
    config: &gd_conf::Config,
    defaults: &gd_conf::Config,
) -> Vec<ExtensionCapability> {
    let on = |c: &gd_conf::Config, name: &str| {
        c.myst.enable_extensions.iter().any(|e| e == name)
            || (name == "dollarmath" && c.myst.dmath_enable)
    };
    guidedoc_commonmark::SyntaxExtension::ALL
        .iter()
        .map(|e| ExtensionCapability {
            name: e.name(),
            default: on(defaults, e.name()),
            enabled: on(config, e.name()),
        })
        .collect()
}

/// The Sphinx extensions Guidedog implements natively (GDS 0011).
pub(crate) fn sphinx_extensions(config: &gd_conf::Config) -> Vec<ExtensionCapability> {
    gd_conf::NATIVE_EXTENSIONS
        .iter()
        .map(|&name| ExtensionCapability {
            name,
            default: false,
            enabled: config.has_extension(name),
        })
        .collect()
}
