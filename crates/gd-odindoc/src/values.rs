//! Declarations' signatures and members, as the odin domain's directives take them:
//! one line, without attributes ("clone :: proc(s: string) -> string").

use crate::ast::{Expr, File, GroupId, NodeId};
use crate::text::{comment_text, normalize, skip_block_comment, string_end};
use crate::{Kind, Member, MemberKind};

/// The longest constant or variable value a signature shows.
const MAX_VALUE: usize = 100;

/// A parsed file and its source, which the signature builders read together.
pub(crate) struct Source<'f> {
    pub file: &'f File,
    pub src: &'f str,
}

impl<'f> Source<'f> {
    /// The source text of a node.
    pub(crate) fn span(&self, id: NodeId) -> &'f str {
        let s = self.file.node(id).span;
        self.slice(s.start, s.end)
    }

    pub(crate) fn slice(&self, start: u32, end: u32) -> &'f str {
        let end = (end as usize).min(self.src.len());
        self.src.get(start as usize..end).unwrap_or("")
    }

    /// The name a node holds: an identifier's text ("_" for one the parser made up).
    pub(crate) fn name(&self, id: NodeId) -> &'f str {
        match self.file.node(id).kind {
            Expr::Ident { blank: true } => "_",
            _ => self.span(id),
        }
    }

    pub(crate) fn normalized(&self, id: NodeId) -> String {
        normalize(self.span(id))
    }

    /// A comment group's text without its markers.
    pub(crate) fn comment(&self, group: Option<GroupId>) -> String {
        let Some(g) = group else {
            return String::new();
        };
        let list = &self.file.group(g).list;
        comment_text(list.iter().map(|s| self.slice(s.start, s.end)))
    }

    /// A declaration's documentation: the comment above it, then the one after it on its
    /// line.
    pub(crate) fn docs(&self, lead: Option<GroupId>, line: Option<GroupId>) -> String {
        let lead = self.comment(lead);
        let line = self.comment(line);
        match (lead.is_empty(), line.is_empty()) {
            (true, _) => line,
            (_, true) => lead,
            _ => format!("{lead}\n\n{line}"),
        }
    }

    /// `name: type = value`, `name: type`, `name := value`, or `name` for a variable.
    pub(crate) fn var_signature(&self, name: &str, ty: &str, value: Option<NodeId>) -> String {
        let mut v = value.map(|id| self.normalized(id)).unwrap_or_default();
        if v.len() > MAX_VALUE {
            v.clear();
        }
        match (ty.is_empty(), v.is_empty()) {
            (false, false) => format!("{name}: {ty} = {v}"),
            (false, true) => format!("{name}: {ty}"),
            (true, false) => format!("{name} := {v}"),
            (true, true) => name.to_string(),
        }
    }

    /// `name :: value`, `name : T : value`, or for a type or procedure its header: a
    /// struct, enum, or bit_field without its body (its members go to `members`), a
    /// procedure without its body.
    pub(crate) fn const_signature(
        &self,
        name: &str,
        kind: Kind,
        ty: &str,
        value: Option<NodeId>,
        members: &mut Vec<Member>,
    ) -> String {
        // The parser keeps a declaration whose value is missing (`A, B :: 1`, or a
        // syntax error): it is documented by its name and type.
        let Some(value) = value else {
            return if ty.is_empty() { name.to_string() } else { format!("{name} : {ty}") };
        };
        let mut v = match self.file.node(value).kind {
            Expr::ProcLit { body } => self.proc_header(value, body),
            Expr::Struct(i) => {
                *members = self.struct_members(i);
                self.header(value)
            }
            Expr::Enum(i) => {
                *members = self.enum_members(i);
                self.header(value)
            }
            Expr::BitField(i) => {
                *members = self.bit_field_members(i);
                self.header(value)
            }
            _ => {
                let text = self.normalized(value);
                if kind == Kind::Const && text.len() > MAX_VALUE { String::new() } else { text }
            }
        };
        let start = self.file.node(value).span.start;
        v.insert_str(0, directive_before(self.src, start as usize));
        match (ty.is_empty(), v.is_empty()) {
            (false, true) => format!("{name} : {ty}"),
            (false, false) => format!("{name} : {ty} : {v}"),
            (true, true) => name.to_string(),
            (true, false) => format!("{name} :: {v}"),
        }
    }

    /// A type's text up to its body's opening brace: the first `{` outside parentheses,
    /// brackets, strings, and comments.
    fn header(&self, value: NodeId) -> String {
        let text = self.span(value);
        let b = text.as_bytes();
        let mut depth = 0i32;
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'"' | b'\'' | b'`' => {
                    i = string_end(b, i);
                    continue;
                }
                b'/' if b.get(i + 1) == Some(&b'*') => {
                    i = skip_block_comment(b, i);
                    continue;
                }
                b'/' if b.get(i + 1) == Some(&b'/') => {
                    i = text[i..].find('\n').map_or(b.len(), |k| i + k);
                    continue;
                }
                b'(' | b'[' => depth += 1,
                b')' | b']' => depth -= 1,
                b'{' if depth == 0 => return normalize(&text[..i]),
                _ => {}
            }
            i += 1;
        }
        normalize(text)
    }

    /// A procedure literal without its body, or without the `---` of a foreign one.
    fn proc_header(&self, value: NodeId, body: Option<u32>) -> String {
        let span = self.file.node(value).span;
        let end = body.unwrap_or(span.end);
        let text = normalize(self.slice(span.start, end));
        let text = text.trim();
        text.strip_suffix("---").unwrap_or(text).trim().to_string()
    }

    fn struct_members(&self, index: u32) -> Vec<Member> {
        let mut out = Vec::new();
        for f in &self.file.structs[index as usize] {
            let prefix = if f.flags & crate::ast::flag::SUBTYPE != 0 {
                "#subtype "
            } else if f.flags & crate::ast::flag::USING != 0 {
                "using "
            } else {
                ""
            };
            let ty = f.ty.map(|t| self.normalized(t)).unwrap_or_default();
            let tag =
                f.tag.map(|t| format!(" {}", self.slice(t.start, t.end))).unwrap_or_default();
            let docs = self.docs(f.docs, f.comment);
            for &n in &f.names {
                let name = self.normalized(n);
                let signature = format!("{prefix}{name}: {ty}{tag}");
                let docs = docs.clone();
                out.push(Member { name, kind: MemberKind::Field, signature, docs });
            }
        }
        out
    }

    fn bit_field_members(&self, index: u32) -> Vec<Member> {
        let fields = &self.file.bit_fields[index as usize];
        fields
            .iter()
            .map(|f| {
                let name = self.normalized(f.name);
                let ty = self.normalized(f.ty);
                let bits = self.normalized(f.bit_size);
                let mut signature = format!("{name}: {ty} | {bits}");
                if let Some(t) = f.tag {
                    signature.push(' ');
                    signature.push_str(self.slice(t.start, t.end));
                }
                let docs = self.docs(f.docs, f.comment);
                Member { name, kind: MemberKind::BitField, signature, docs }
            })
            .collect()
    }

    /// An enum's names with their values and comments: the comment group ending on the
    /// line above a name (after the previous name's line), and the one starting on its
    /// line after it.
    fn enum_members(&self, index: u32) -> Vec<Member> {
        let e = &self.file.enums[index as usize];
        let mut out = Vec::with_capacity(e.fields.len());
        let mut previous = e.open_line;
        for &f in &e.fields {
            let (name, value) = match self.file.node(f).kind {
                Expr::FieldValue { field, value } => (field, Some(value)),
                _ => (f, None),
            };
            let text = self.normalized(name);
            let signature = match value {
                Some(v) => format!("{text} = {}", self.normalized(v)),
                None => text.clone(),
            };
            let (lead, line) = self.comments_around(previous, f);
            let docs = self.docs(lead, line);
            out.push(Member { name: text, kind: MemberKind::Enumerator, signature, docs });
            previous = self.file.end_line(f);
        }
        out
    }

    fn comments_around(&self, previous: u32, f: NodeId) -> (Option<GroupId>, Option<GroupId>) {
        let start_line = self.file.line_of(self.file.node(f).span.start);
        let end_line = self.file.end_line(f);
        let end = self.file.node(f).span.end;
        let (mut lead, mut line) = (None, None);
        for (i, g) in self.file.groups.iter().enumerate() {
            let id = Some(GroupId(i as u32));
            if g.end_line + 1 == start_line && g.line > previous {
                lead = id;
            }
            if g.line == end_line && g.start >= end {
                line = id;
            }
        }
        (lead, line)
    }
}

/// A `#force_inline` or `#force_no_inline` written before a value (the parser starts the
/// value after it), with a space.
fn directive_before(src: &str, at: usize) -> &'static str {
    let b = src.as_bytes();
    let mut k = at.min(b.len());
    while k > 0 && matches!(b[k - 1], b' ' | b'\t') {
        k -= 1;
    }
    let mut start = k;
    while start > 0 && is_word(b[start - 1]) {
        start -= 1;
    }
    if start == 0 || b[start - 1] != b'#' || start == k {
        return "";
    }
    match &src[start..k] {
        "force_inline" => "#force_inline ",
        "force_no_inline" => "#force_no_inline ",
        _ => "",
    }
}

fn is_word(c: u8) -> bool {
    c == b'_' || c.is_ascii_alphanumeric()
}

/// What a constant declares. Procedure literals, groups, and type literals say so; a
/// bare name or selector (`Handle :: other.Handle`), a parametric type (`Vec3 ::
/// Vec(3)`), or a built-in type name is a type when it looks like one: a capitalized
/// name not in capitals throughout, or a built-in type. Any other value is a constant.
pub(crate) fn kind_of(source: &Source<'_>, value: Option<NodeId>) -> Kind {
    let Some(value) = value else {
        return Kind::Const;
    };
    let file = source.file;
    match file.node(value).kind {
        Expr::ProcLit { .. } => Kind::Proc,
        Expr::ProcGroup => Kind::ProcGroup,
        Expr::Struct(_) => Kind::Struct,
        Expr::Union => Kind::Union,
        Expr::Enum(_) => Kind::Enum,
        Expr::BitSet => Kind::BitSet,
        Expr::BitField(_) => Kind::BitField,
        Expr::Distinct
        | Expr::Helper
        | Expr::ProcType
        | Expr::Pointer
        | Expr::MultiPointer
        | Expr::Array
        | Expr::DynamicArray
        | Expr::Map
        | Expr::Matrix
        | Expr::Typeid
        | Expr::Poly
        | Expr::Relative
        | Expr::FixedCapacityDynamicArray => Kind::Type,
        Expr::Ident { .. } => type_or_const(type_like(source.name(value))),
        Expr::Selector { expr, field } => {
            // pkg.Type names a type; pkg.Enum.Member, a deeper selector, is a value.
            let package_name = matches!(file.node(expr).kind, Expr::Ident { .. });
            type_or_const(package_name && type_like(source.name(field)))
        }
        Expr::Call { callee } => match file.node(callee).kind {
            Expr::Ident { .. } => type_or_const(type_like(source.name(callee))),
            Expr::Selector { field, .. } => type_or_const(type_like(source.name(field))),
            _ => Kind::Const,
        },
        _ => Kind::Const,
    }
}

fn type_or_const(is_type: bool) -> Kind {
    if is_type { Kind::Type } else { Kind::Const }
}

const BUILTIN_TYPES: &[&str] = &[
    "bool",
    "b8",
    "b16",
    "b32",
    "b64",
    "int",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "uint",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "uintptr",
    "i16le",
    "i32le",
    "i64le",
    "i128le",
    "u16le",
    "u32le",
    "u64le",
    "u128le",
    "i16be",
    "i32be",
    "i64be",
    "i128be",
    "u16be",
    "u32be",
    "u64be",
    "u128be",
    "f16",
    "f32",
    "f64",
    "f16le",
    "f32le",
    "f64le",
    "f16be",
    "f32be",
    "f64be",
    "complex32",
    "complex64",
    "complex128",
    "quaternion64",
    "quaternion128",
    "quaternion256",
    "rune",
    "string",
    "cstring",
    "string16",
    "cstring16",
    "rawptr",
    "typeid",
    "any",
    "byte",
];

fn type_like(name: &str) -> bool {
    if BUILTIN_TYPES.contains(&name) {
        return true;
    }
    let b = name.as_bytes();
    if b.first().is_none_or(|c| !c.is_ascii_uppercase()) {
        return false;
    }
    b.iter().any(u8::is_ascii_lowercase)
}
