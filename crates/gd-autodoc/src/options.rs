//! Directive options: `autodoc_default_options` applied, then each converted as
//! `_process_documenter_options` converts them.

use crate::{ClassContent, Config, DefaultOption, DirectiveOption, Generator, MemberOrder};
use crate::{ObjType, Severity};

/// An option that lists members: absent, ALL (given without a value), or names.
#[derive(Debug, Clone, Default)]
pub(crate) struct Names {
    pub(crate) given: bool,
    pub(crate) all: bool,
    pub(crate) names: Vec<String>,
}

impl Names {
    /// `name in option`: ALL includes everything.
    pub(crate) fn includes(&self, name: &str) -> bool {
        self.given && (self.all || contains(&self.names, name))
    }
}

/// `_AutoDocumenterOptions`: the directive's options after `autodoc_default_options`
/// are applied and each value converted.
#[derive(Debug, Clone, Default)]
pub(crate) struct Options {
    pub(crate) no_index: bool,
    pub(crate) no_index_entry: bool,
    pub(crate) noindex: bool,
    pub(crate) members: Names,
    pub(crate) undoc_members: bool,
    /// Names, "object" by default.
    pub(crate) inherited_members: Names,
    pub(crate) show_inheritance: bool,
    pub(crate) synopsis: Option<String>,
    pub(crate) platform: Option<String>,
    pub(crate) deprecated: bool,
    pub(crate) member_order: Option<MemberOrder>,
    /// Given with no names is EMPTY.
    pub(crate) exclude_members: Names,
    pub(crate) private_members: Names,
    pub(crate) special_members: Names,
    pub(crate) imported_members: bool,
    pub(crate) ignore_module_all: bool,
    pub(crate) no_value: bool,
    pub(crate) class_doc_from: Option<ClassContent>,
    /// `:annotation:` with a value.
    pub(crate) annotation: Option<String>,
    /// `:annotation:` given at all.
    pub(crate) has_annotation: bool,
}

/// The options `autodoc_default_options` may set, in Sphinx's order.
const DEFAULT_OPTIONS: &[&str] = &[
    "members",
    "undoc-members",
    "no-index",
    "no-index-entry",
    "inherited-members",
    "show-inheritance",
    "private-members",
    "special-members",
    "ignore-module-all",
    "exclude-members",
    "member-order",
    "imported-members",
    "class-doc-from",
    "no-value",
];

/// Default options whose value a directive's `+value` extends.
const EXTENDABLE: &[&str] = &["members", "private-members", "special-members", "exclude-members"];

const SPEC_MODULE: &[&str] = &[
    "no-index",
    "no-index-entry",
    "members",
    "exclude-members",
    "undoc-members",
    "private-members",
    "special-members",
    "member-order",
    "ignore-module-all",
    "imported-members",
    "deprecated",
    "platform",
    "synopsis",
    "show-inheritance",
    "inherited-members",
    "no-value",
    "noindex",
];
const SPEC_CLASS: &[&str] = &[
    "no-index",
    "no-index-entry",
    "members",
    "exclude-members",
    "undoc-members",
    "private-members",
    "special-members",
    "member-order",
    "class-doc-from",
    "show-inheritance",
    "inherited-members",
    "noindex",
];
const SPEC_FUNCTION: &[&str] = &["no-index", "no-index-entry", "noindex"];
const SPEC_ASSIGNMENT: &[&str] =
    &["no-index", "no-index-entry", "annotation", "no-value", "noindex"];
const SPEC_TYPE: &[&str] = &["no-index", "no-index-entry", "annotation", "no-value"];

/// The options any autodoc directive may be given, for a reader's option table; each
/// directive then accepts only its own.
pub const ALL_OPTIONS: &[&str] = &[
    "no-index",
    "no-index-entry",
    "noindex",
    "members",
    "exclude-members",
    "undoc-members",
    "private-members",
    "special-members",
    "member-order",
    "ignore-module-all",
    "imported-members",
    "deprecated",
    "platform",
    "synopsis",
    "show-inheritance",
    "inherited-members",
    "no-value",
    "class-doc-from",
    "annotation",
];

/// The options an object type accepts (`_OPTION_SPECS`).
fn option_spec(t: ObjType) -> &'static [&'static str] {
    match t {
        ObjType::Module => SPEC_MODULE,
        ObjType::Class | ObjType::Exception => SPEC_CLASS,
        ObjType::Function | ObjType::Decorator | ObjType::Method | ObjType::Property => {
            SPEC_FUNCTION
        }
        ObjType::Attribute | ObjType::Data => SPEC_ASSIGNMENT,
        ObjType::Type => SPEC_TYPE,
    }
}

/// An option before conversion: `value` is `None` for a flag (Python's None).
#[derive(Debug, Clone)]
struct Raw {
    name: String,
    value: Option<String>,
}

/// `_process_documenter_options`; `None` (with an error reported) when an option is
/// not accepted.
pub(crate) fn process_options(
    g: &mut Generator<'_>,
    t: ObjType,
    given: &[DirectiveOption],
) -> Option<Options> {
    let mut raw: Vec<Raw> = given
        .iter()
        .map(|o| Raw {
            name: o.name.clone(),
            value: Some(o.value.clone()).filter(|v| !v.is_empty()),
        })
        .collect();
    let spec = option_spec(t);
    for name in DEFAULT_OPTIONS.iter().filter(|name| spec.contains(name)) {
        apply_default(&mut raw, g.config, name);
    }
    let mut o = Options::default();
    for r in &raw {
        let why = if spec.contains(&r.name.as_str()) {
            convert_option(&mut o, r)
        } else {
            Some("it is not an option of this directive".to_string())
        };
        if let Some(why) = why {
            report_option(g, &r.name, &why);
            return None;
        }
    }
    Some(o)
}

/// Applies `autodoc_default_options` to one option, unless `no-<name>` negates it.
fn apply_default(raw: &mut Vec<Raw>, config: &Config, name: &str) {
    let mut negated = false;
    if let Some(i) = raw.iter().position(|r| r.name == format!("no-{name}")) {
        negated = raw[i].value.is_none();
        raw.remove(i);
    }
    let default = default_option(config, name);
    let i = raw.iter().position(|r| r.name == name);
    match (default, i) {
        (Some(d), Some(i)) if !negated && !d.flag => {
            let extend = EXTENDABLE.contains(&name);
            if let Some(v) = raw[i].value.as_mut().filter(|v| extend && v.starts_with('+')) {
                *v = format!("{},{}", d.value, &v[1..]);
            }
        }
        (Some(d), i) if !negated => {
            let value = Raw { name: name.to_string(), value: (!d.flag).then(|| d.value.clone()) };
            match i {
                Some(i) => raw[i] = value,
                None => raw.push(value),
            }
        }
        (_, Some(i)) => {
            if let Some(v) = raw[i].value.as_mut().filter(|v| v.starts_with('+')) {
                v.remove(0);
            }
        }
        _ => {}
    }
}

fn report_option(g: &mut Generator<'_>, name: &str, why: &str) {
    let text = format!(
        "An option to {} is either unknown or has an invalid value: {name}: {why}.",
        g.request.directive
    );
    g.report(
        Severity::Error,
        "autodoc.option",
        "AUTODOC OPTION NOT ACCEPTED",
        text,
        "See the options autodoc accepts for this directive in the autodoc guide.",
    );
}

/// Converts one option; `Some(why)` when its value is invalid.
fn convert_option(o: &mut Options, r: &Raw) -> Option<String> {
    let value = r.value.as_deref();
    match r.name.as_str() {
        "no-index" => o.no_index = true,
        "noindex" => o.noindex = true,
        "no-index-entry" => o.no_index_entry = true,
        "members" => o.members = names_option(value),
        "private-members" => o.private_members = names_option(value),
        "special-members" => o.special_members = names_option(value),
        "exclude-members" => {
            let names = value.map(|v| split_names(v, true)).unwrap_or_default();
            o.exclude_members = Names { given: true, all: false, names };
        }
        "inherited-members" => {
            let names = match value {
                None => vec!["object".to_string()],
                Some(v) => v.split(',').map(|p| p.trim().to_string()).collect(),
            };
            o.inherited_members = Names { given: true, all: false, names };
        }
        "undoc-members" => o.undoc_members = true,
        "show-inheritance" => o.show_inheritance = true,
        "deprecated" => o.deprecated = true,
        "imported-members" => o.imported_members = true,
        "ignore-module-all" => o.ignore_module_all = true,
        "no-value" => o.no_value = true,
        "synopsis" => o.synopsis = Some(value.unwrap_or("").to_string()),
        "platform" => o.platform = Some(value.unwrap_or("").to_string()),
        "member-order" => match value {
            None => {}
            Some(v) => match MemberOrder::parse(v) {
                Some(order) => o.member_order = Some(order),
                None => return Some(format!("invalid value for member-order option: {v}")),
            },
        },
        "class-doc-from" => match ClassContent::parse(value.unwrap_or("")) {
            Some(content) => o.class_doc_from = Some(content),
            None => {
                let v = value.unwrap_or("");
                return Some(format!("invalid value for class-doc-from option: {v}"));
            }
        },
        "annotation" => {
            o.has_annotation = true;
            o.annotation = value.map(str::to_string);
        }
        _ => {}
    }
    None
}

/// `members_option`: a flag means every member.
fn names_option(value: Option<&str>) -> Names {
    match value {
        None => Names { given: true, all: true, names: Vec::new() },
        Some(v) => Names { given: true, all: false, names: split_names(v, false) },
    }
}

fn split_names(value: &str, unique: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in value.split(',').map(str::trim) {
        if name.is_empty() || (unique && contains(&out, name)) {
            continue;
        }
        out.push(name.to_string());
    }
    out
}

fn default_option<'c>(c: &'c Config, name: &str) -> Option<&'c DefaultOption> {
    c.autodoc_default_options.iter().find(|d| d.name == name)
}

/// Whether a list of names holds `s`.
pub(crate) fn contains<S: AsRef<str>>(list: &[S], s: &str) -> bool {
    list.iter().any(|x| x.as_ref() == s)
}
