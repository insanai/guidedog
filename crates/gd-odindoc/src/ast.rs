//! The syntax odindoc reads from an Odin file: file-scope statements, and expressions as
//! flat nodes (`NodeId` indices into `File::nodes`) with their source spans, which is all
//! a signature needs. Procedure bodies are skipped, not kept.

use crate::tokenizer::Tk;

/// An expression node's index in `File::nodes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NodeId(pub u32);

/// A comment group's index in `File::groups`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GroupId(pub u32);

/// A byte span of the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Span {
    pub start: u32,
    pub end: u32,
}

/// What an expression node is, with the children odindoc looks at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Expr {
    Bad,
    /// A name; `blank` when the parser made one up for a missing name ("_").
    Ident {
        blank: bool,
    },
    BasicLit,
    Undef,
    Implicit,
    BasicDirective,
    Paren(NodeId),
    Unary {
        op: Tk,
        expr: Option<NodeId>,
    },
    Binary {
        op: Tk,
        left: NodeId,
        right: NodeId,
    },
    Ternary,
    OrElse,
    ImplicitSelector {
        field: NodeId,
    },
    Selector {
        expr: NodeId,
        field: NodeId,
    },
    Call {
        callee: NodeId,
    },
    Index,
    Slice,
    Deref,
    TypeAssertion,
    OrReturn,
    OrBranch,
    CompLit,
    FieldValue {
        field: NodeId,
        value: NodeId,
    },
    /// A procedure literal; `body` is where its body's `{` starts, if it has one.
    ProcLit {
        body: Option<u32>,
    },
    ProcGroup,
    ProcType,
    /// A struct type; its fields are `File::structs[i]`.
    Struct(u32),
    Union,
    /// An enum type; its fields are `File::enums[i]`.
    Enum(u32),
    BitSet,
    /// A bit_field type; its fields are `File::bit_fields[i]`.
    BitField(u32),
    Distinct,
    Helper,
    Pointer,
    MultiPointer,
    Array,
    DynamicArray,
    FixedCapacityDynamicArray,
    Map,
    Matrix,
    Typeid,
    Poly,
    Relative,
    Tag,
    Ellipsis,
    AutoCast,
    TypeCast,
    Asm,
}

/// An expression node and its span.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Node {
    pub kind: Expr,
    pub span: Span,
}

/// A run of comments on adjacent lines.
#[derive(Debug, Clone, Default)]
pub(crate) struct CommentGroup {
    pub list: Vec<Span>,
    pub line: u32,
    pub end_line: u32,
    pub start: u32,
}

/// A struct field (`using` and `#subtype` in `flags`).
#[derive(Debug, Clone, Default)]
pub(crate) struct Field {
    pub names: Vec<NodeId>,
    pub ty: Option<NodeId>,
    pub flags: u16,
    pub tag: Option<Span>,
    pub docs: Option<GroupId>,
    pub comment: Option<GroupId>,
}

/// Field flags, as Odin's parser names them.
pub(crate) mod flag {
    pub const USING: u16 = 1;
    pub const SUBTYPE: u16 = 1 << 1;
    pub const NO_ALIAS: u16 = 1 << 2;
    pub const C_VARARG: u16 = 1 << 3;
    pub const CONST: u16 = 1 << 4;
    pub const ANY_INT: u16 = 1 << 5;
    pub const BY_PTR: u16 = 1 << 6;
    pub const NO_BROADCAST: u16 = 1 << 7;
    pub const NO_CAPTURE: u16 = 1 << 8;
    pub const ELLIPSIS: u16 = 1 << 9;
    pub const TAGS: u16 = 1 << 10;
    pub const DEFAULT_PARAMETERS: u16 = 1 << 11;
    pub const TYPEID_TOKEN: u16 = 1 << 12;
    pub const RESULTS: u16 = 1 << 13;
    /// What a struct's fields allow.
    pub const STRUCT: u16 = USING | TAGS | SUBTYPE;
    /// What a polymorphic record's parameters allow.
    pub const POLY_PARAMS: u16 = TYPEID_TOKEN | DEFAULT_PARAMETERS;
    /// What a procedure's results allow.
    pub const RESULTS_LIST: u16 = ELLIPSIS
        | USING
        | NO_ALIAS
        | C_VARARG
        | CONST
        | ANY_INT
        | BY_PTR
        | NO_BROADCAST
        | DEFAULT_PARAMETERS;
    /// What a procedure's parameters allow.
    pub const PARAMS: u16 = RESULTS_LIST | TYPEID_TOKEN;
}

/// An enum's fields and the line of its `{`.
#[derive(Debug, Clone, Default)]
pub(crate) struct EnumType {
    pub open_line: u32,
    pub fields: Vec<NodeId>,
}

/// A bit_field's field: `name: type | bits`.
#[derive(Debug, Clone)]
pub(crate) struct BitFieldField {
    pub name: NodeId,
    pub ty: NodeId,
    pub bit_size: NodeId,
    pub tag: Option<Span>,
    pub docs: Option<GroupId>,
    pub comment: Option<GroupId>,
}

/// `names : type = values` or `names : type : values`.
#[derive(Debug, Clone, Default)]
pub(crate) struct ValueDecl {
    pub names: Vec<NodeId>,
    pub ty: Option<NodeId>,
    pub values: Vec<NodeId>,
    pub mutable: bool,
    pub attributes: Vec<Span>,
    pub docs: Option<GroupId>,
    pub comment: Option<GroupId>,
}

/// `import name "path"`.
#[derive(Debug, Clone, Default)]
pub(crate) struct ImportDecl {
    pub name: Option<Span>,
    pub path: Span,
    pub attributes: Vec<Span>,
}

/// `foreign import name "path"` or `foreign import name {"a", "b"}`.
#[derive(Debug, Clone, Default)]
pub(crate) struct ForeignImport {
    pub name: Option<NodeId>,
    pub paths: Vec<NodeId>,
    pub attributes: Vec<Span>,
    pub docs: Option<GroupId>,
    pub comment: Option<GroupId>,
    pub line: u32,
}

/// `foreign lib { decls }`; `lib` is the library's span, or the `foreign` keyword's
/// when the block names none, as Odin's parser records it.
#[derive(Debug, Clone, Default)]
pub(crate) struct ForeignBlock {
    pub lib: Span,
    pub attributes: Vec<Span>,
    pub body: Vec<Stmt>,
}

/// `when cond { body } else ...`; `body` is `None` for a `do` statement.
#[derive(Debug, Clone)]
pub(crate) struct When {
    pub cond: NodeId,
    pub body: Option<Vec<Stmt>>,
    pub else_stmt: Option<Box<Else>>,
}

/// The else branch of a `when`.
#[derive(Debug, Clone)]
pub(crate) enum Else {
    Block(Vec<Stmt>),
    When(When),
    Other,
}

/// A file-scope statement.
#[derive(Debug, Clone)]
pub(crate) enum Stmt {
    Value(ValueDecl),
    ForeignBlock(ForeignBlock),
    ForeignImport(ForeignImport),
    Import(ImportDecl),
    When(When),
    Other,
}

/// A parsed file.
#[derive(Debug, Default)]
pub(crate) struct File {
    pub nodes: Vec<Node>,
    pub groups: Vec<CommentGroup>,
    pub structs: Vec<Vec<Field>>,
    pub enums: Vec<EnumType>,
    pub bit_fields: Vec<Vec<BitFieldField>>,
    pub tags: Vec<Span>,
    pub package: Option<Span>,
    pub package_docs: Option<GroupId>,
    pub decls: Vec<Stmt>,
    pub errors: u32,
    pub too_deep: bool,
    /// The offset each line starts at.
    pub line_starts: Vec<u32>,
}

impl File {
    /// The node `id` names.
    pub(crate) fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0 as usize]
    }

    /// The comment group `id` names.
    pub(crate) fn group(&self, id: GroupId) -> &CommentGroup {
        &self.groups[id.0 as usize]
    }

    /// The line (from 1) that `offset` is on.
    pub(crate) fn line_of(&self, offset: u32) -> u32 {
        self.line_starts.partition_point(|&s| s <= offset) as u32
    }

    /// The line a node ends on: the line of its last character.
    pub(crate) fn end_line(&self, id: NodeId) -> u32 {
        let s = self.node(id).span;
        self.line_of(s.end.saturating_sub(1).max(s.start))
    }
}

/// The offsets lines of `src` start at.
pub(crate) fn line_starts(src: &str) -> Vec<u32> {
    let mut out = vec![0];
    out.extend(src.bytes().enumerate().filter(|&(_, b)| b == b'\n').map(|(i, _)| i as u32 + 1));
    out
}
