//! Info fields, as Sphinx's DocFieldTransformer reads them (sphinx/util/docfields.py).
//! Once an object's content is read, each field list directly inside it is rebuilt from
//! the field types its domain declares. The uses of a grouped type (:param:, :raises:)
//! join in one field, where the first stood, whose body lists them; a typed field takes
//! its type from a type field (:type x:) or from its own name (:param int x:), and type
//! fields leave the list. A field of no known type, or with an argument its type does
//! not take, stays as written with its name capitalized. Python's :meta: fields are
//! dropped. Types, exceptions, and return types become Xref nodes, as Sphinx's
//! pending_xref, so the project resolves them.

use guidedoc::{Builder, Fail, Label, NodeId, NodeKind, is_ascii_space};

use crate::docfield_write::{replace_nodes, write_group, write_kept, write_plain};
use crate::domains::{Cx, Object, find_domain};
use crate::text::{has_at, trim_space};

pub(crate) use crate::docfield_write::reopen;

/// How a field type gathers its uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldShape {
    /// Sphinx's Field: a field of its own for each use.
    Plain,
    /// GroupedField: every use in one field, as a bullet list.
    Grouped,
    /// TypedField: grouped, each use with its type.
    Typed,
}

/// One field type of a domain. Lists of names are separated by "|".
#[derive(Debug, Clone, Copy)]
pub(crate) struct DocField {
    /// Sphinx's name of the type, kept as Custom "docfield".
    pub name: &'static str,
    /// How uses gather.
    pub shape: FieldShape,
    /// The field's label.
    pub label: Label,
    /// The field names of the type ("param|parameter|arg").
    pub names: &'static str,
    /// The fields take an argument (":param x:").
    pub has_arg: bool,
    /// The role that links the argument; "" for none.
    pub role: &'static str,
    /// Typed: the fields that give a type.
    pub typenames: &'static str,
    /// Typed: the role that links a type.
    pub typerole: &'static str,
    /// Plain: the role that links a body of one text.
    pub bodyrole: &'static str,
    /// One use is written without the list.
    pub collapse: bool,
    /// Python's fields: text is split at delimiters (PyXrefMixin).
    pub python: bool,
}

impl DocField {
    /// A plain field type with no names, for struct-update syntax in tables.
    pub(crate) const DEFAULT: DocField = DocField {
        name: "",
        shape: FieldShape::Plain,
        label: Label::Parameters,
        names: "",
        has_arg: false,
        role: "",
        typenames: "",
        typerole: "",
        bodyrole: "",
        collapse: false,
        python: false,
    };
}

/// One field of the rebuilt list, in order: a field kept as written (`kind` None), or a
/// field type with its uses in `items`.
#[derive(Debug, Clone, Default)]
pub(crate) struct FieldEntry {
    /// The field as written, or the first use of the type.
    pub field: NodeId,
    /// The index in the domain's field types; `None` for a kept field.
    pub kind: Option<usize>,
    /// A kept field's new name.
    pub name: String,
    /// A kept field whose body of one text is a type: its field type and the text.
    pub link: Option<(usize, String)>,
}

/// One use of a field type.
#[derive(Debug, Clone)]
pub(crate) struct FieldItem {
    pub entry: usize,
    pub arg: String,
    pub body: NodeId,
}

/// A type given for an argument: one text, or the inline content of `para`.
#[derive(Debug, Clone)]
pub(crate) struct FieldType {
    pub kind: usize,
    pub arg: String,
    pub text: Option<String>,
    pub para: NodeId,
    pub used: bool,
}

/// The field list being rebuilt.
#[derive(Debug)]
pub(crate) struct DocFields {
    pub fields: &'static [DocField],
    pub domain: &'static str,
    pub entries: Vec<FieldEntry>,
    pub items: Vec<FieldItem>,
    pub types: Vec<FieldType>,
}

impl DocFields {
    /// Returns the type of an argument once, as Sphinx pops it.
    pub(crate) fn take_type(&mut self, kind: usize, arg: &str) -> Option<usize> {
        let k = self.types.iter().position(|t| !t.used && t.kind == kind && t.arg == arg)?;
        self.types[k].used = true;
        Some(k)
    }
}

/// Rebuilds the field lists directly inside an object's content, which is still open,
/// as DocFieldTransformer.transform_all does, while the content's context (module,
/// class) is in effect.
pub(crate) fn doc_fields(
    cx: &mut Cx<'_, '_, '_>,
    o: &Object<'_>,
    content: NodeId,
) -> Result<(), Fail> {
    if cx.b().current() != Some(content) {
        return Ok(());
    }
    let fields = find_domain(o.domain).map_or(&[][..], |(_, d)| d.fields(o.objtype));
    let mut child = content.index() + 1;
    while child < cx.b().snap.nodes.len() {
        let node = cx.b().snap.nodes[child];
        if node.kind == NodeKind::FieldList {
            let mut t = DocFields {
                fields,
                domain: o.domain,
                entries: Vec::new(),
                items: Vec::new(),
                types: Vec::new(),
            };
            child = transform_list(cx, &mut t, NodeId::from_index(child))?;
        } else {
            child = node.subtree_end as usize;
        }
    }
    Ok(())
}

/// Rebuilds one field list and returns the index after it.
fn transform_list(
    cx: &mut Cx<'_, '_, '_>,
    t: &mut DocFields,
    list: NodeId,
) -> Result<usize, Fail> {
    let end = cx.b().snap.nodes[list].subtree_end as usize;
    let mut f = list.index() + 1;
    while f < end {
        collect_field(cx.b(), t, NodeId::from_index(f));
        f = cx.b().snap.nodes[f].subtree_end as usize;
    }
    let start = cx.b().snap.nodes.len();
    let rebuilt = reopen(cx.b(), list)?;
    for i in 0..t.entries.len() {
        match t.entries[i].kind {
            None => write_kept(cx, t, i)?,
            Some(k) if t.fields[k].shape == FieldShape::Plain => write_plain(cx, t, i)?,
            Some(_) => write_group(cx, t, i)?,
        }
    }
    cx.b().end_node(rebuilt)?;
    replace_nodes(cx.b(), list.index(), end, start)
}

/// Step 1 of Sphinx's transform for one field.
fn collect_field(b: &Builder<'_>, t: &mut DocFields, field: NodeId) {
    let (Some(name), Some(body)) = field_parts(b, field) else {
        keep_field(b, t, field, "", "", None);
        return;
    };
    let text = plain_name(b, name);
    if t.domain == "py" && (text == "meta" || has_at(&text, 0, "meta ")) {
        return;
    }
    let (type_name, mut arg) = match text.find(|c: char| c.is_ascii() && is_ascii_space(c as u8)) {
        Some(i) => (&text[..i], trim_space(&text[i..])),
        None => (text.as_str(), ""),
    };
    let found = field_kind(t.fields, type_name);
    let Some((kind, is_type)) = found.filter(|(k, _)| t.fields[*k].has_arg == !arg.is_empty())
    else {
        keep_field(b, t, field, type_name, arg, found);
        return;
    };
    if is_type {
        note_type(b, t, kind, arg, body);
        return;
    }
    if t.fields[kind].shape == FieldShape::Typed {
        let a = arg.as_bytes();
        if let Some(i) = (1..a.len()).rev().find(|&i| is_ascii_space(a[i])) {
            let typ = trim_space(&arg[..i]).to_owned();
            set_type(
                t,
                FieldType {
                    kind,
                    arg: arg[i + 1..].to_owned(),
                    text: Some(typ),
                    para: field,
                    used: false,
                },
            );
            arg = &arg[i + 1..];
        }
    }
    add_item(t, kind, field, arg, body);
}

/// Finds a field name among the types' names and type names; as in Sphinx's map of
/// them, a later type wins. Returns the type and whether the name gives a type.
fn field_kind(fields: &[DocField], name: &str) -> Option<(usize, bool)> {
    let mut found = None;
    for (i, f) in fields.iter().enumerate() {
        if name_listed(f.names, name) {
            found = Some((i, false));
        }
        if f.shape == FieldShape::Typed && name_listed(f.typenames, name) {
            found = Some((i, true));
        }
    }
    found
}

fn name_listed(list: &str, name: &str) -> bool {
    list.split('|').any(|n| n == name)
}

/// Notes a field kept as written, named "Type arg" with the type's first letter
/// capitalized. A known type field with a body of one text links it as a type.
fn keep_field(
    b: &Builder<'_>,
    t: &mut DocFields,
    field: NodeId,
    type_name: &str,
    arg: &str,
    found: Option<(usize, bool)>,
) {
    let mut name = String::with_capacity(type_name.len() + arg.len() + 1);
    let mut chars = type_name.chars();
    if let Some(c) = chars.next() {
        name.push(c.to_ascii_uppercase());
        name.push_str(chars.as_str());
    }
    if !arg.is_empty() {
        name.push(' ');
        name.push_str(arg);
    }
    let mut link = None;
    if let Some((kind, true)) = found {
        let body = field_parts(b, field).1;
        if let Some(para) = body.and_then(|body| single_paragraph(b, body))
            && let Some(text) = one_text(b, para)
        {
            link = Some((kind, text));
        }
    }
    t.entries.push(FieldEntry { field, kind: None, name, link });
}

/// Records a type field's content, when it is inline content.
fn note_type(b: &Builder<'_>, t: &mut DocFields, kind: usize, arg: &str, body: NodeId) {
    let Some(para) = single_paragraph(b, body) else { return };
    if b.snap.nodes[para].subtree_end as usize == para.index() + 1 {
        return;
    }
    let text = one_text(b, para);
    set_type(t, FieldType { kind, arg: arg.to_owned(), text, para, used: false });
}

/// Records a type, replacing an earlier one for the same argument.
fn set_type(t: &mut DocFields, typ: FieldType) {
    match t.types.iter_mut().find(|old| old.kind == typ.kind && old.arg == typ.arg) {
        Some(old) => *old = typ,
        None => t.types.push(typ),
    }
}

/// Adds a use of a field type: to the group of its type, or as a new entry.
fn add_item(t: &mut DocFields, kind: usize, field: NodeId, arg: &str, body: NodeId) {
    let grouped = t.fields[kind].shape != FieldShape::Plain;
    let existing =
        if grouped { t.entries.iter().position(|e| e.kind == Some(kind)) } else { None };
    let entry = existing.unwrap_or_else(|| {
        t.entries.push(FieldEntry { field, kind: Some(kind), ..FieldEntry::default() });
        t.entries.len() - 1
    });
    t.items.push(FieldItem { entry, arg: arg.to_owned(), body });
}

/// The text of a field name, trimmed.
fn plain_name(b: &Builder<'_>, name: NodeId) -> String {
    let mut out = String::new();
    let end = b.snap.nodes[name].subtree_end as usize;
    for n in &b.snap.nodes[name.index() + 1..end] {
        match n.kind {
            NodeKind::Text | NodeKind::Code | NodeKind::Math | NodeKind::RawInline => {
                out.push_str(b.pool_string(n.text));
            }
            NodeKind::SoftBreak | NodeKind::HardBreak => out.push(' '),
            _ => {}
        }
    }
    trim_space(&out).to_owned()
}

/// The text of a paragraph that holds only text, as one Docutils Text node would; line
/// breaks become spaces. `None` for a paragraph with other content or none.
pub(crate) fn one_text(b: &Builder<'_>, para: NodeId) -> Option<String> {
    let end = b.snap.nodes[para].subtree_end as usize;
    let inner = &b.snap.nodes[para.index() + 1..end];
    if inner.is_empty()
        || inner.iter().any(|n| n.kind != NodeKind::Text && n.kind != NodeKind::SoftBreak)
    {
        return None;
    }
    let mut out = String::new();
    for n in inner {
        if n.kind == NodeKind::Text {
            out.push_str(b.pool_string(n.text));
        } else {
            out.push(' ');
        }
    }
    Some(out)
}

/// Returns a field's name and body.
pub(crate) fn field_parts(b: &Builder<'_>, field: NodeId) -> (Option<NodeId>, Option<NodeId>) {
    let (mut name, mut body) = (None, None);
    let end = b.snap.nodes[field].subtree_end as usize;
    let mut c = field.index() + 1;
    while c < end {
        match b.snap.nodes[c].kind {
            NodeKind::FieldName => name = Some(NodeId::from_index(c)),
            NodeKind::FieldBody => body = Some(NodeId::from_index(c)),
            _ => {}
        }
        c = b.snap.nodes[c].subtree_end as usize;
    }
    (name, body)
}

/// Returns the paragraph of a field body that holds one paragraph and nothing else.
pub(crate) fn single_paragraph(b: &Builder<'_>, body: NodeId) -> Option<NodeId> {
    let first = body.index() + 1;
    let end = b.snap.nodes[body].subtree_end as usize;
    if first >= end || b.snap.nodes[first].kind != NodeKind::Paragraph {
        return None;
    }
    (b.snap.nodes[first].subtree_end as usize == end).then(|| NodeId::from_index(first))
}
