//! Roles: the standard roles, the document's roles, and how interpreted text becomes
//! nodes.

use guidedoc::{
    Arg, AttributeKey, Fail, LinkInfo, LinkKind, Message, NodeKind, Text, arg_span,
    normalize_label,
};

use crate::emit::BreakMode;
use crate::extension::RoleCall;
use crate::messages::{ROLE_PEP, ROLE_RAW_DIRECT, ROLE_RFC, ROLE_UNKNOWN};
use crate::scan::Scanner;

/// What a role produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RoleBase {
    /// Emphasis.
    Emphasis,
    /// Strong.
    Strong,
    /// Code, escapes processed.
    Literal,
    /// Code, backslashes kept; a `language` adds the Language attribute.
    Code,
    /// Math.
    Math,
    /// Subscript.
    Subscript,
    /// Superscript.
    Superscript,
    /// TitleReference.
    TitleReference,
    /// A Link to a PEP.
    PepReference,
    /// A Link to an RFC.
    RfcReference,
    /// Abbreviation.
    Abbreviation,
    /// Abbreviation (acronym).
    Acronym,
    /// RawInline in the role's `format`.
    Raw,
    /// A Role node carrying the role name and classes.
    #[default]
    Custom,
}

/// A role the role directive defined, or a standard role.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Role {
    /// The normalized (lowercase) role name.
    pub name: String,
    /// What it produces.
    pub base: RoleBase,
    /// Space-separated classes, from `:class:`; "" gives a custom role its name.
    pub classes: String,
    /// Raw roles: the output format, e.g. "html".
    pub format: String,
    /// Code-derived roles: the language.
    pub language: String,
}

/// The standard interpreted text roles and their aliases.
const STANDARD_ROLES: &[(&str, RoleBase)] = &[
    ("emphasis", RoleBase::Emphasis),
    ("strong", RoleBase::Strong),
    ("literal", RoleBase::Literal),
    ("code", RoleBase::Code),
    ("math", RoleBase::Math),
    ("subscript", RoleBase::Subscript),
    ("sub", RoleBase::Subscript),
    ("superscript", RoleBase::Superscript),
    ("sup", RoleBase::Superscript),
    ("title-reference", RoleBase::TitleReference),
    ("title", RoleBase::TitleReference),
    ("t", RoleBase::TitleReference),
    ("pep-reference", RoleBase::PepReference),
    ("pep", RoleBase::PepReference),
    ("rfc-reference", RoleBase::RfcReference),
    ("rfc", RoleBase::RfcReference),
    ("abbreviation", RoleBase::Abbreviation),
    ("ab", RoleBase::Abbreviation),
    ("acronym", RoleBase::Acronym),
    ("ac", RoleBase::Acronym),
    ("raw", RoleBase::Raw),
    // The specialized roles: resolution turns them into the construct they name.
    ("named-reference", RoleBase::Custom),
    ("anonymous-reference", RoleBase::Custom),
    ("footnote-reference", RoleBase::Custom),
    ("citation-reference", RoleBase::Custom),
    ("substitution-reference", RoleBase::Custom),
    ("target", RoleBase::Custom),
    ("uri-reference", RoleBase::Custom),
];

/// The default PEP base URL.
pub(crate) const PEP_URL: &str = "https://peps.python.org/pep-";
/// The default RFC base URL.
pub(crate) const RFC_URL: &str = "https://datatracker.ietf.org/doc/html/rfc";

/// Returns what the standard role (or alias) `name` produces, compared
/// case-insensitively; `None` for another name. The role directive uses it to resolve
/// a base role.
pub fn standard_role(name: &str) -> Option<RoleBase> {
    let key = fold_key(name);
    STANDARD_ROLES.iter().find(|r| r.0 == key).map(|r| r.1)
}

/// Case-folds a name as role and reference names compare.
pub(crate) fn fold_key(name: &str) -> String {
    let mut key = String::with_capacity(name.len());
    normalize_label(&mut key, name);
    key
}

/// Tells whether two role names are the same, compared case-insensitively.
pub(crate) fn same_name(a: &str, b: &str) -> bool {
    a == b || fold_key(a) == fold_key(b)
}

/// The node kind each base produces.
fn role_kind(base: RoleBase) -> NodeKind {
    match base {
        RoleBase::Emphasis => NodeKind::Emphasis,
        RoleBase::Strong => NodeKind::Strong,
        RoleBase::Literal | RoleBase::Code => NodeKind::Code,
        RoleBase::Math => NodeKind::Math,
        RoleBase::Subscript => NodeKind::Subscript,
        RoleBase::Superscript => NodeKind::Superscript,
        RoleBase::TitleReference => NodeKind::TitleReference,
        RoleBase::PepReference | RoleBase::RfcReference => NodeKind::Link,
        RoleBase::Abbreviation | RoleBase::Acronym => NodeKind::Abbreviation,
        RoleBase::Raw => NodeKind::RawInline,
        RoleBase::Custom => NodeKind::Role,
    }
}

/// A role found by name: the document's own or a standard one.
struct Found<'r> {
    name: &'r str,
    base: RoleBase,
    classes: &'r str,
    format: &'r str,
    language: &'r str,
    custom: bool,
}

impl<'x> Scanner<'x, '_> {
    /// Finds a role defined in the document, then a standard role. Role names are
    /// case-insensitive.
    fn lookup_role(&self, name: &str) -> Option<Found<'x>> {
        let state: &'x crate::InlineState = self.state;
        let key = fold_key(name);
        if let Some(r) = state.roles.iter().find(|r| fold_key(&r.name) == key) {
            return Some(Found {
                name: &r.name,
                base: r.base,
                classes: &r.classes,
                format: &r.format,
                language: &r.language,
                custom: true,
            });
        }
        let &(name, base) = STANDARD_ROLES.iter().find(|r| r.0 == key)?;
        Some(Found { name, base, classes: "", format: "", language: "", custom: false })
    }

    /// Offers interpreted text `[rs, e)` with content `[q, j)` to the dialect's hook.
    fn hook_role(&mut self, name: &str, rs: usize, e: usize, q: usize, j: usize) -> bool {
        self.cook_temp(q, j);
        let call = RoleCall {
            name,
            marked: self.text.get(q..j).unwrap_or(""),
            text: &self.temp,
            span: self.span(rs, e),
            content: self.span(q, j),
        };
        self.hook.role(self.ctx, &call)
    }

    /// Emits interpreted text `[rs, e)` with content `[q, j)` for the role `name`; ""
    /// means the default role.
    pub fn apply_role(
        &mut self,
        name: &str,
        rs: usize,
        e: usize,
        q: usize,
        j: usize,
    ) -> Result<(), Fail> {
        let state: &'x crate::InlineState = self.state;
        let mut role_name = if name.is_empty() { state.default_role.as_str() } else { name };
        if role_name.is_empty() {
            role_name = "title-reference";
        }
        if self.hook_role(role_name, rs, e, q, j) {
            return crate::emit::check(&mut self.ctx.b);
        }
        let Some(role) = self.lookup_role(role_name) else {
            return self.role_problem(ROLE_UNKNOWN, rs, e);
        };
        let classes =
            if role.custom && role.classes.is_empty() { role.name } else { role.classes };
        match role.base {
            RoleBase::PepReference => self.pep_reference(classes, rs, e, q, j),
            RoleBase::RfcReference => self.rfc_reference(classes, rs, e, q, j),
            RoleBase::Literal | RoleBase::Code | RoleBase::Math | RoleBase::Raw => {
                self.leaf_role(&role, classes, rs, e, q, j)
            }
            _ => {
                let id = self.ctx.b.begin_node(role_kind(role.base), self.span(rs, e))?;
                if role.base == RoleBase::Custom {
                    let t = self.ctx.b.add_text(role.name)?;
                    self.ctx.b.set_text(id, t);
                }
                if !classes.is_empty() {
                    self.ctx.b.add_attribute_string(id, AttributeKey::Class, classes)?;
                }
                self.emit_cooked(q, j, BreakMode::Spaces)?;
                self.ctx.b.end_node(id)
            }
        }
    }

    /// Emits a role whose content is one text leaf: literal, code, math, or raw.
    fn leaf_role(
        &mut self,
        role: &Found<'_>,
        classes: &str,
        rs: usize,
        e: usize,
        q: usize,
        j: usize,
    ) -> Result<(), Fail> {
        if role.base == RoleBase::Raw && (!role.custom || role.format.is_empty()) {
            return self.role_problem(ROLE_RAW_DIRECT, rs, e);
        }
        let start = self.ctx.b.text_open();
        if role.base == RoleBase::Literal {
            self.push_cooked(q, j)?;
        } else {
            self.push_raw(q, j)?;
        }
        let t = self.ctx.b.text_close(start);
        let id = self.ctx.b.add_leaf(role_kind(role.base), self.span(rs, e), t)?;
        if role.base == RoleBase::Code && !role.language.is_empty() {
            self.ctx.b.add_attribute_string(id, AttributeKey::Language, role.language)?;
        }
        if role.base == RoleBase::Raw {
            self.ctx.b.add_attribute_string(id, AttributeKey::Format, role.format)?;
        }
        if !classes.is_empty() {
            self.ctx.b.add_attribute_string(id, AttributeKey::Class, classes)?;
        }
        Ok(())
    }

    /// Reports an error and keeps the interpreted text's source as text.
    fn role_problem(&mut self, m: Message, rs: usize, e: usize) -> Result<(), Fail> {
        let span = self.span(rs, e);
        self.ctx.report(m, span, arg_span(span), Arg::None);
        let start = self.ctx.b.text_open();
        self.push_raw(rs, e)?;
        self.text_leaf(start, rs, e)
    }

    /// Emits a Link with an external destination and one Text child.
    fn link_role(
        &mut self,
        classes: &str,
        rs: usize,
        e: usize,
        dest: Text,
        label: Text,
    ) -> Result<(), Fail> {
        let span = self.span(rs, e);
        let id = self.ctx.b.begin_node(NodeKind::Link, span)?;
        let link =
            LinkInfo { destination: dest, kind: LinkKind::External, ..LinkInfo::UNRESOLVED };
        self.ctx.b.set_link(id, link)?;
        if !classes.is_empty() {
            self.ctx.b.add_attribute_string(id, AttributeKey::Class, classes)?;
        }
        self.ctx.b.add_leaf(NodeKind::Text, span, label)?;
        self.ctx.b.end_node(id)
    }

    /// `:PEP:`8`` links to PEP 8 with the text "PEP 8".
    fn pep_reference(
        &mut self,
        classes: &str,
        rs: usize,
        e: usize,
        q: usize,
        j: usize,
    ) -> Result<(), Fail> {
        let start = self.ctx.b.text_open();
        self.ctx.b.text_push("PEP ")?;
        self.push_cooked(q, j)?;
        let label = self.ctx.b.text_close(start);
        let written = self.ctx.b.pool_string(label);
        let Some(n) = parse_number(written.get(4..).unwrap_or(""), 9999) else {
            self.ctx.b.text_rewind(start);
            return self.role_problem(ROLE_PEP, rs, e);
        };
        let dest_start = self.ctx.b.text_open();
        self.push_pep_url(n)?;
        let dest = self.ctx.b.text_close(dest_start);
        self.link_role(classes, rs, e, dest, label)
    }

    /// `:RFC:`2822#section-3`` links to that section of RFC 2822 with the text
    /// "RFC 2822".
    fn rfc_reference(
        &mut self,
        classes: &str,
        rs: usize,
        e: usize,
        q: usize,
        j: usize,
    ) -> Result<(), Fail> {
        self.cook_temp(q, j);
        let content = std::mem::take(&mut self.temp);
        let hash = content.find('#').unwrap_or(content.len());
        let result = match parse_number(&content[..hash], 1 << 40) {
            Some(n) if n >= 1 => self.rfc_link(classes, rs, e, n, &content[hash..]),
            _ => self.role_problem(ROLE_RFC, rs, e),
        };
        self.temp = content;
        result
    }

    fn rfc_link(
        &mut self,
        classes: &str,
        rs: usize,
        e: usize,
        n: i64,
        fragment: &str,
    ) -> Result<(), Fail> {
        let dest_start = self.ctx.b.text_open();
        self.push_rfc_url(n, fragment)?;
        let dest = self.ctx.b.text_close(dest_start);
        let start = self.ctx.b.text_open();
        self.ctx.b.text_push("RFC ")?;
        push_int(&mut self.ctx.b, n, 0)?;
        let label = self.ctx.b.text_close(start);
        self.link_role(classes, rs, e, dest, label)
    }
}

/// Reads decimal digits surrounded by optional white space, at most `limit`.
pub(crate) fn parse_number(s: &str, limit: i64) -> Option<i64> {
    let s = s.trim_matches(|c: char| c.is_ascii() && guidedoc::is_ascii_space(c as u8));
    if s.is_empty() || s.len() > 12 || !s.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let n = s.bytes().fold(0i64, |n, c| n * 10 + i64::from(c - b'0'));
    (n <= limit).then_some(n)
}

/// Pushes `n` in decimal, zero-padded to `width` digits.
pub(crate) fn push_int(b: &mut guidedoc::Builder<'_>, n: i64, width: usize) -> Result<(), Fail> {
    b.text_push(&format!("{n:0width$}"))
}
