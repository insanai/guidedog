//! What extensions do to autodoc's docstrings and members: `autodoc-process-docstring`
//! and `autodoc-skip-member`, as `sphinx.ext.napoleon` handles them.

use gd_pyscan::{ClassRef, DefKind, Object, ObjectKind};

use crate::napoleon::{NapoleonContext, includes_member, napoleon_docstring};
use crate::props::Props;
use crate::{Generator, ObjType, Severity};

/// One docstring as extensions leave it: with napoleon on, its Google and NumPy
/// sections become fields; what napoleon cannot read is a warning.
pub(crate) fn process_docstring(
    g: &mut Generator<'_>,
    p: &Props,
    lines: Vec<String>,
) -> Vec<String> {
    let Some(settings) = g.config.napoleon.as_ref() else { return lines };
    let annotations = match p.obj_type {
        ObjType::Module => Some(module_annotations(g, p)),
        ObjType::Class | ObjType::Exception => Some(class_annotations(g, p)),
        _ => None,
    };
    let full_name = p.full_name();
    let ctx = NapoleonContext {
        what: p.obj_type.name(),
        name: &full_name,
        annotations: annotations.as_deref(),
        no_index: g.options.no_index,
        strip_signature_backslash: g.config.strip_signature_backslash,
    };
    let out = napoleon_docstring(&lines, settings, &ctx);
    for note in out.notes {
        let title = match note.code {
            "napoleon.section" => "SECTION READ AS TEXT",
            "napoleon.field" => "FIELD NOT READ",
            _ => "MALFORMED TYPE",
        };
        let hint = match note.code {
            "napoleon.section" => "Indent the section's body below its header.",
            "napoleon.field" => {
                "Write the field as `name (type): description`, or `name : type` \
                                 with the description below it in NumPy style."
            }
            _ => "Close the brace or quote in the type specification.",
        };
        let text = format!(
            "Line {} of the docstring of {full_name}: {} napoleon keeps the docstring as \
             written from that line on.",
            note.line + 1,
            note.text
        );
        g.report(Severity::Warning, note.code, title, text, hint);
    }
    out.lines
}

/// The annotations of a module's own names.
fn module_annotations(g: &Generator<'_>, p: &Props) -> Vec<(String, String)> {
    let Some(id) = p.module else { return Vec::new() };
    let m = g.project.module(id);
    scope_annotations(m, &m.scope.defs)
}

/// The annotations of a class and of its bases in the source roots, the class's own
/// first, as `typing.get_type_hints` merges them.
fn class_annotations(g: &mut Generator<'_>, p: &Props) -> Vec<(String, String)> {
    let Some(c) = p.obj_class() else { return Vec::new() };
    let (order, _) = g.project.mro(c);
    let mut out: Vec<(String, String)> = Vec::new();
    for class in order.iter().filter_map(|e| e.class) {
        for (name, text) in own_class_annotations(g, class) {
            if !out.iter().any(|(n, _)| *n == name) {
                out.push((name, text));
            }
        }
    }
    out
}

fn own_class_annotations(g: &Generator<'_>, c: ClassRef) -> Vec<(String, String)> {
    let m = g.project.module(c.module);
    scope_annotations(m, &m.class(c.class).scope.defs)
}

/// `name: type` bindings of a scope, as written.
fn scope_annotations(m: &gd_pyscan::Module, defs: &[gd_pyscan::Def]) -> Vec<(String, String)> {
    let annotated = defs.iter().filter(|d| {
        matches!(d.kind, DefKind::Assign | DefKind::Annotation) && d.annotation.is_some()
    });
    annotated.map(|d| (d.name.to_string(), m.text(d.annotation).to_owned())).collect()
}

/// `autodoc-skip-member` as napoleon answers it: whether `name` (bound to `obj`, whose
/// own docstring is `doc`) is documented although autodoc would skip it.
pub(crate) fn napoleon_keeps(
    g: &Generator<'_>,
    p: &Props,
    name: &str,
    obj: &Object,
    doc: bool,
) -> bool {
    let Some(settings) = g.config.napoleon.as_ref() else { return false };
    let has_doc = match obj.kind {
        ObjectKind::Function | ObjectKind::Class => doc,
        // A value's `__doc__` is its type's, which nearly every type has.
        ObjectKind::Value | ObjectKind::Builtin | ObjectKind::External => {
            p.obj_type == ObjType::Module
        }
        _ => false,
    };
    // In a class, only what has a `__qualname__` (functions and classes) qualifies.
    let owned = p.obj_type == ObjType::Module
        || matches!(obj.kind, ObjectKind::Function | ObjectKind::Class);
    owned && includes_member(settings, p.obj_type.name(), name, has_doc)
}
