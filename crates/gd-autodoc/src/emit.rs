//! Emission follows autodoc's `_generate_directives`, `_directive_header_lines`, and
//! `_body_alias_lines`, and does at the level of text what
//! `sphinx.ext.autodoc.typehints`' `_merge_typehints` does to the parsed field lists for
//! `autodoc_typehints = "description"`.

use gd_pyscan::{ModuleId, ModuleKind, ObjectKind, UnparseMode, unparse};

use crate::docstrings::class_variable_comment;
use crate::hints::{note_approximate, render_mode};
use crate::members::{checks_module, gather_members, short_name};
use crate::props::{Props, Sentinel};
use crate::typehints::merge_typehints;
use crate::types::{Eval, TypeValue, external_type, restify};
use crate::typevars::AliasKind;
use crate::{Generator, MAX_GENERATED, ObjType, Severity, Typehints};

/// How deep members of members are documented.
const MAX_NESTING: usize = 24;

impl Generator<'_> {
    /// Adds a line; a blank one loses its indentation.
    fn emit(&mut self, indent: &str, line: &str) {
        if line.trim().is_empty() {
            self.out.lines.push(String::new());
        } else {
            self.out.lines.push(format!("{indent}{line}"));
        }
    }

    /// Adds a directive option line.
    fn option(&mut self, indent: &str, text: &str) {
        self.out.lines.push(format!("{indent}   {text}"));
    }
}

/// `_generate_directives`: the directive of one object, then its members'.
pub(crate) fn generate_directives(
    g: &mut Generator<'_>,
    p: &mut Props,
    more_content: &[String],
    parent_modname: &str,
    check_module: bool,
    all_members: bool,
    indent: &str,
) {
    if g.depth >= MAX_NESTING {
        return;
    }
    g.generated += 1;
    if g.generated > MAX_GENERATED {
        g.report(
            Severity::Warning,
            "autodoc.limit",
            "TOO MANY MEMBERS",
            format!(
                "autodoc: {} generates more than {MAX_GENERATED} directives; the rest are \
                     left out. Classes that contain themselves under their own name are \
                     documented again at every level.",
                g.request.argument
            ),
            "Exclude the repeated members with :exclude-members:.",
        );
        return;
    }
    g.depth += 1;
    generate(g, p, more_content, parent_modname, check_module, all_members, indent);
    g.depth -= 1;
}

fn generate(
    g: &mut Generator<'_>,
    p: &mut Props,
    more_content: &[String],
    parent_modname: &str,
    check_module: bool,
    all_members: bool,
    indent: &str,
) {
    let real_modname = if parent_modname.is_empty() || p.obj_type.is_class() {
        p.canonical_module_name().to_string()
    } else {
        parent_modname.to_string()
    };
    let analyzer = g
        .project
        .load_module(&real_modname)
        .filter(|&id| g.project.module(id).kind != ModuleKind::Native);
    if check_module && !g.options.imported_members {
        let defining = defining_module(g, &p.obj);
        if !defining.is_empty() && defining != p.module_name {
            return;
        }
    }
    let is_final =
        analyzer.is_some_and(|a| g.project.module(a).info.finals.contains(&p.dotted_parts()));
    let mut body = std::mem::take(&mut p.docstring_lines);
    body.extend_from_slice(more_content);
    alias_lines(g, p, &mut body);
    if let Some(hints) = p.hints.as_ref().filter(|h| g.config.typehint_fields && !h.is_empty()) {
        let target = g.config.autodoc_typehints_description_target;
        merge_typehints(hints, target, p.obj_type == ObjType::Class, &mut body);
    }
    g.emit("", "");
    header_lines(g, p, is_final, indent);
    g.emit("", "");
    let inner = format!("{indent}   ");
    for l in &body {
        g.emit(&inner, l);
    }
    document_members(g, p, all_members, &real_modname, analyzer, indent);
}

/// `__module__` of a class or function: where its source defines it.
fn defining_module(g: &Generator<'_>, o: &gd_pyscan::Object) -> String {
    match (o.kind, o.class, o.function) {
        (ObjectKind::Class, Some(c), _) => g.project.module(c.module).name.clone(),
        (ObjectKind::Function, _, Some(f)) => g.project.module(f.module).name.clone(),
        (ObjectKind::External, _, _) => {
            o.path.rsplit_once('.').map_or("", |(module, _)| module).to_string()
        }
        (ObjectKind::Builtin, _, _) => "builtins".into(),
        _ => String::new(),
    }
}

fn directive_name(p: &Props) -> String {
    if p.obj_type.is_class() && p.doc_as_attr() {
        return "py:attribute".into();
    }
    format!("py:{}", p.obj_type.name())
}

/// `_directive_header_lines`.
fn header_lines(g: &mut Generator<'_>, p: &Props, is_final: bool, indent: &str) {
    let name = if p.parts.is_empty() { p.module_name.clone() } else { p.dotted_parts() };
    let prefix = format!(".. {}:: ", directive_name(p));
    let first = p.signatures.first().map_or("", String::as_str);
    g.emit(indent, &format!("{prefix}{name}{first}"));
    let pad = " ".repeat(prefix.len());
    for s in p.signatures.iter().skip(1) {
        g.emit(indent, &format!("{pad}{name}{s}"));
    }
    let o = g.options.clone();
    if o.no_index || o.noindex {
        g.option(indent, ":no-index:");
    }
    if o.no_index_entry {
        g.option(indent, ":no-index-entry:");
    }
    if !p.parts.is_empty() {
        g.option(indent, &format!(":module: {}", p.module_name));
    }
    match p.obj_type {
        ObjType::Module => {
            if let Some(synopsis) = o.synopsis.as_ref().filter(|s| !s.is_empty()) {
                g.option(indent, &format!(":synopsis: {synopsis}"));
            }
            if let Some(platform) = o.platform.as_ref().filter(|s| !s.is_empty()) {
                g.option(indent, &format!(":platform: {platform}"));
            }
            if o.deprecated {
                g.option(indent, ":deprecated:");
            }
        }
        ObjType::Class | ObjType::Exception => class_options(g, p, is_final, indent),
        ObjType::Function | ObjType::Decorator => {
            if p.is_async {
                g.option(indent, ":async:");
            }
        }
        ObjType::Method => {
            let flags = [
                (p.is_abstract, ":abstractmethod:"),
                (p.is_async, ":async:"),
                (p.is_classmethod, ":classmethod:"),
                (p.is_staticmethod, ":staticmethod:"),
                (is_final, ":final:"),
            ];
            for (_, flag) in flags.iter().filter(|(on, _)| *on) {
                g.option(indent, flag);
            }
        }
        ObjType::Property => property_options(g, p, indent),
        ObjType::Data | ObjType::Attribute => assignment_options(g, p, indent),
        ObjType::Type => {
            if let (false, false, ObjectKind::Value, Some(m)) =
                (o.no_value, p.hide_value, p.obj.kind, p.obj.module)
            {
                let value = unparse(g.project.module(m).node(p.obj.value), UnparseMode::Python);
                g.option(indent, &format!(":canonical: {value}"));
            }
        }
    }
}

fn property_options(g: &mut Generator<'_>, p: &Props, indent: &str) {
    if p.is_abstract {
        g.option(indent, ":abstractmethod:");
    }
    if p.is_classmethod {
        g.option(indent, ":classmethod:");
    }
    if let Some(t) =
        p.property_type.as_ref().filter(|_| g.config.autodoc_typehints != Typehints::Hidden)
    {
        g.option(indent, &format!(":type: {t}"));
    }
}

fn class_options(g: &mut Generator<'_>, p: &Props, is_final: bool, indent: &str) {
    if p.alias_kind != AliasKind::None {
        return;
    }
    if is_final {
        g.option(indent, ":final:");
    }
    if let Some(canonical) = p.canonical_full_name()
        && !p.doc_as_attr()
        && p.full_name() != canonical
    {
        g.option(indent, &format!(":canonical: {canonical}"));
    }
    if !p.doc_as_attr() && g.options.show_inheritance {
        g.emit(indent, "");
        g.option(indent, &format!("Bases: {}", p.bases.join(", ")));
    }
}

fn assignment_options(g: &mut Generator<'_>, p: &Props, indent: &str) {
    let o = g.options.clone();
    let suppressed = o.has_annotation && o.annotation.is_none();
    if p.obj_type == ObjType::Data {
        if suppressed || p.generic_alias {
            return;
        }
        if let Some(annotation) = o.annotation.as_ref().filter(|a| !a.is_empty()) {
            g.option(indent, &format!(":annotation: {annotation}"));
            return;
        }
    } else if let Some(annotation) = o.annotation.as_ref().filter(|_| !p.generic_alias) {
        g.option(indent, &format!(":annotation: {annotation}"));
        return;
    }
    if let Some(t) =
        p.type_annotation.as_ref().filter(|_| g.config.autodoc_typehints != Typehints::Hidden)
    {
        g.option(indent, &format!(":type: {t}"));
    }
    let shows_value = !o.no_value && p.sentinel == Sentinel::None && !p.hide_value;
    if let Some(value) = p.value_repr.as_ref().filter(|_| shows_value && !p.generic_alias) {
        g.option(indent, &format!(":value: {value}"));
    }
}

/// `_body_alias_lines`.
fn alias_lines(g: &mut Generator<'_>, p: &Props, body: &mut Vec<String>) {
    match p.obj_type {
        ObjType::Data | ObjType::Attribute if p.generic_alias => {
            body.push(format!("alias of {}", p.alias_text));
            body.push(String::new());
        }
        ObjType::Class | ObjType::Exception => {
            if p.alias_kind != AliasKind::None {
                body.push(format!("alias of {}", p.alias_text));
                body.push(String::new());
                return;
            }
            if !p.doc_as_attr() || class_variable_comment(g, p) {
                return;
            }
            let v = match (p.obj.kind, p.obj.class) {
                (ObjectKind::Class, Some(c)) => TypeValue::Class {
                    module: g.project.module(c.module).name.clone(),
                    name: g.project.class(c).qualname.to_string(),
                },
                (ObjectKind::Builtin, _) => TypeValue::Class {
                    module: "builtins".into(),
                    name: short_name(&p.obj.path).into(),
                },
                (ObjectKind::External, _) => {
                    let mut e = Eval::default();
                    let v = external_type(&mut e, &p.obj.path);
                    note_approximate(g, &e);
                    v
                }
                // A class documented as an attribute is only an alias of a class; for
                // any other object Sphinx adds no "alias of" line either.
                _ => return,
            };
            body.push(format!("alias of {}", restify(&v, render_mode(g))));
        }
        _ => {}
    }
}

/// `_document_members`.
fn document_members(
    g: &mut Generator<'_>,
    p: &Props,
    all_members: bool,
    real_modname: &str,
    analyzer: Option<ModuleId>,
    indent: &str,
) {
    let has_members = p.obj_type == ObjType::Module
        || (p.obj_type.is_class() && !p.doc_as_attr() && p.obj.kind == ObjectKind::Class);
    if !has_members {
        return;
    }
    let o = &g.options;
    let want_all = all_members || o.inherited_members.given || o.members.all;
    let members = gather_members(g, p, want_all, indent, real_modname, analyzer);
    let check = checks_module(g, p, want_all);
    for mut m in members {
        let check_module = check && !m.is_attr;
        generate_directives(g, &mut m.props, &[], real_modname, check_module, true, &m.indent);
    }
}
