//! The abstract syntax tree of a C++ declaration, as Sphinx's C++ domain builds it
//! (`sphinx/domains/cpp/_ast.py`). The tree is flat: every node lives in one `Vec` and
//! refers to others by `NodeId`. A list of nodes (parameters, name elements, operands) is
//! linked through the nodes themselves; `Ast::list` walks it from its head.

use std::borrow::Cow;

/// The index of a node in its `Ast`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub(crate) u32);

impl NodeId {
    /// Returns the node's index in the tree's node table.
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// The head of a linked list of nodes; `None` for an empty list.
pub type List = Option<NodeId>;

/// The declaration a directive declares: what Sphinx calls the object type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectType {
    /// `cpp:class` and `cpp:struct`.
    Class,
    /// `cpp:union`.
    Union,
    /// `cpp:function`.
    Function,
    /// `cpp:member` and `cpp:var`.
    Member,
    /// `cpp:type`.
    Type,
    /// `cpp:concept`.
    Concept,
    /// `cpp:enum`, `cpp:enum-struct`, and `cpp:enum-class`.
    Enum,
    /// `cpp:enumerator`.
    Enumerator,
}

impl ObjectType {
    /// Returns the object type's name as Sphinx spells it (`class`, `function`, ...).
    pub fn name(self) -> &'static str {
        match self {
            ObjectType::Class => "class",
            ObjectType::Union => "union",
            ObjectType::Function => "function",
            ObjectType::Member => "member",
            ObjectType::Type => "type",
            ObjectType::Concept => "concept",
            ObjectType::Enum => "enum",
            ObjectType::Enumerator => "enumerator",
        }
    }
}

/// The C++ directive a declaration was written with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Directive {
    /// `cpp:class`.
    Class,
    /// `cpp:struct`.
    Struct,
    /// `cpp:union`.
    Union,
    /// `cpp:function`.
    Function,
    /// `cpp:member`.
    Member,
    /// `cpp:var`.
    Var,
    /// `cpp:type`.
    Type,
    /// `cpp:concept`.
    Concept,
    /// `cpp:enum`.
    Enum,
    /// `cpp:enum-struct`.
    EnumStruct,
    /// `cpp:enum-class`.
    EnumClass,
    /// `cpp:enumerator`.
    Enumerator,
}

impl Directive {
    /// Returns the directive named `name` (without the `cpp:` domain), or `None` when
    /// it declares nothing (`namespace`, `alias`, ...).
    pub fn from_name(name: &str) -> Option<Directive> {
        Some(match name {
            "class" => Directive::Class,
            "struct" => Directive::Struct,
            "union" => Directive::Union,
            "function" => Directive::Function,
            "member" => Directive::Member,
            "var" => Directive::Var,
            "type" => Directive::Type,
            "concept" => Directive::Concept,
            "enum" => Directive::Enum,
            "enum-struct" => Directive::EnumStruct,
            "enum-class" => Directive::EnumClass,
            "enumerator" => Directive::Enumerator,
            _ => return None,
        })
    }

    /// Returns the directive's name without the domain (`struct`, `enum-class`, ...).
    pub fn name(self) -> &'static str {
        match self {
            Directive::Class => "class",
            Directive::Struct => "struct",
            Directive::Union => "union",
            Directive::Function => "function",
            Directive::Member => "member",
            Directive::Var => "var",
            Directive::Type => "type",
            Directive::Concept => "concept",
            Directive::Enum => "enum",
            Directive::EnumStruct => "enum-struct",
            Directive::EnumClass => "enum-class",
            Directive::Enumerator => "enumerator",
        }
    }

    /// Returns the object type the directive declares (Sphinx's `objtype`).
    pub fn object_type(self) -> ObjectType {
        match self {
            Directive::Class | Directive::Struct => ObjectType::Class,
            Directive::Union => ObjectType::Union,
            Directive::Function => ObjectType::Function,
            Directive::Member | Directive::Var => ObjectType::Member,
            Directive::Type => ObjectType::Type,
            Directive::Concept => ObjectType::Concept,
            Directive::Enum | Directive::EnumStruct | Directive::EnumClass => ObjectType::Enum,
            Directive::Enumerator => ObjectType::Enumerator,
        }
    }
}

/// Storage classes, function specifiers, cv-qualifiers, and attributes before or after
/// a type (`ASTDeclSpecsSimple`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SimpleSpecs<'a> {
    /// `static`, `extern`, `mutable`, `register`, or `""`.
    pub storage: &'a str,
    /// `thread_local`.
    pub is_thread_local: bool,
    /// `inline`.
    pub is_inline: bool,
    /// `virtual`.
    pub is_virtual: bool,
    /// `consteval`.
    pub is_consteval: bool,
    /// `constexpr`.
    pub is_constexpr: bool,
    /// `constinit`.
    pub is_constinit: bool,
    /// `volatile`.
    pub is_volatile: bool,
    /// `const`.
    pub is_const: bool,
    /// `friend`.
    pub is_friend: bool,
    /// The `ExplicitSpec`, when `explicit` is written.
    pub explicit: Option<NodeId>,
    /// The attributes.
    pub attrs: List,
}

/// A function declarator's parameters and what follows them
/// (`ASTParametersQualifiers`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParamQual<'a> {
    /// The `FunctionParam`s.
    pub params: List,
    /// `const` after the parameters.
    pub is_const: bool,
    /// `volatile` after the parameters.
    pub is_volatile: bool,
    /// `override`.
    pub is_override: bool,
    /// `final`.
    pub is_final: bool,
    /// The ref-qualifier: `&`, `&&`, or `""`.
    pub ref_qualifier: &'a str,
    /// The initializer after `=`: `0`, `delete`, `default`, or `""`.
    pub initializer: &'a str,
    /// The `NoexceptSpec`.
    pub noexcept: Option<NodeId>,
    /// The trailing return `Type`.
    pub trailing_return: Option<NodeId>,
    /// The attributes.
    pub attrs: List,
}

/// A node of a C++ declaration's tree. The variants follow Sphinx's AST classes; a
/// field holding a `NodeId` names the kind of node it refers to in its comment.
#[derive(Debug, Clone, PartialEq)]
pub enum Node<'a> {
    // Names.
    /// An identifier; `@...` for an anonymous entity, `~X` for a destructor.
    Identifier(&'a str),
    /// A built-in operator after `operator` (`+`, `()`, `new[]`, `and`).
    OpBuiltin(&'a str),
    /// `operator""` and its suffix `Identifier`.
    OpLiteral(NodeId),
    /// A conversion operator and its `Type`.
    OpType(NodeId),
    /// One element of a nested name.
    NameElement {
        /// The `Identifier` or operator.
        name: NodeId,
        /// The `TemplateArgs`.
        args: Option<NodeId>,
        /// Written with `template` before it.
        has_template_keyword: bool,
    },
    /// A possibly qualified name.
    NestedName {
        /// Written with `::` before it.
        is_rooted: bool,
        /// The `NameElement`s.
        elements: List,
    },
    /// `<...>` after a name.
    TemplateArgs {
        /// Each a `Type` or an `ArgConstant`.
        args: List,
        /// The last argument is followed by `...`.
        is_pack: bool,
    },
    /// A template argument that is a constant expression.
    ArgConstant(NodeId),

    // Expressions.
    /// `nullptr`.
    LitPointer,
    /// `true` or `false`.
    LitBool(bool),
    /// A number as written.
    LitNumber(&'a str),
    /// A string literal with its quotes.
    LitString(&'a str),
    /// A character literal.
    LitChar {
        /// `u8`, `u`, `U`, `L`, or `""`.
        prefix: &'a str,
        /// The text between the quotes.
        data: &'a str,
        /// The character's code.
        value: u32,
    },
    /// A literal and its user-defined suffix `Identifier`.
    LitUser {
        /// The literal.
        literal: NodeId,
        /// The suffix `Identifier`.
        suffix: NodeId,
    },
    /// `this`.
    This,
    /// A fold expression.
    Fold {
        /// The left operand, absent in a left fold.
        left: Option<NodeId>,
        /// The operator.
        op: &'a str,
        /// The right operand, absent in a right fold.
        right: Option<NodeId>,
    },
    /// `(expression)`.
    Paren(NodeId),
    /// A name used as an expression; its `NestedName`.
    IdExpr(NodeId),
    /// A prefix and postfix operations.
    Postfix {
        /// An expression, or a `Type` (a functional cast).
        prefix: NodeId,
        /// The `Pf*` operations.
        ops: List,
    },
    /// `[index]`.
    PfArray(NodeId),
    /// `.name`, its `NestedName`.
    PfMember(NodeId),
    /// `->name`, its `NestedName`.
    PfMemberPtr(NodeId),
    /// `++` after an operand.
    PfInc,
    /// `--` after an operand.
    PfDec,
    /// A call: its `ParenList` or `BracedInit`.
    PfCall(NodeId),
    /// `dynamic_cast<T>(e)` and the other named casts.
    ExplicitCast {
        /// `dynamic_cast`, `static_cast`, `const_cast`, or `reinterpret_cast`.
        cast: &'a str,
        /// The `Type`.
        ty: NodeId,
        /// The expression.
        expr: NodeId,
    },
    /// `typeid(...)`.
    TypeId {
        /// The `Type` or expression.
        operand: NodeId,
        /// The operand is a type.
        is_type: bool,
    },
    /// A unary operator and its operand.
    Unary {
        /// The operator.
        op: &'a str,
        /// The operand.
        operand: NodeId,
    },
    /// `sizeof...(name)`, its `Identifier`.
    SizeofPack(NodeId),
    /// `sizeof(type)`, its `Type`.
    SizeofType(NodeId),
    /// `sizeof expression`.
    SizeofExpr(NodeId),
    /// `alignof(type)`, its `Type`.
    Alignof(NodeId),
    /// `noexcept(expression)`.
    NoexceptExpr(NodeId),
    /// A new-expression.
    New {
        /// Written `::new`.
        is_rooted: bool,
        /// The `Type`.
        ty: NodeId,
        /// The `ParenList` or `BracedInit`.
        init: Option<NodeId>,
    },
    /// A delete-expression.
    Delete {
        /// Written `::delete`.
        is_rooted: bool,
        /// Written `delete[]`.
        is_array: bool,
        /// The operand.
        operand: NodeId,
    },
    /// `(type) operand`.
    Cast {
        /// The `Type`.
        ty: NodeId,
        /// The operand.
        operand: NodeId,
    },
    /// Operands joined by binary operators of one precedence level.
    BinOp {
        /// The first operand.
        first: NodeId,
        /// The `BinOperand`s after it.
        rest: List,
    },
    /// An operand after a binary operator, within a `BinOp`.
    BinOperand {
        /// The operator before the operand.
        op: &'a str,
        /// The operand.
        expr: NodeId,
    },
    /// `cond ? then : other`.
    Conditional {
        /// The condition.
        cond: NodeId,
        /// The value when true.
        then: NodeId,
        /// The value when false.
        other: NodeId,
    },
    /// `{...}`.
    BracedInit {
        /// The elements.
        items: List,
        /// A comma before the `}`.
        has_trailing_comma: bool,
    },
    /// `left op right`.
    Assignment {
        /// The left side.
        left: NodeId,
        /// The operator.
        op: &'a str,
        /// The right side.
        right: NodeId,
    },
    /// Expressions joined by commas.
    Comma(List),
    /// An expression Sphinx could not parse, as written.
    Fallback(&'a str),
    /// `expression...`.
    PackExpansion(NodeId),
    /// `(...)`.
    ParenList(List),

    // Types.
    /// Fundamental type words.
    Fundamental {
        /// The words as written, single-spaced.
        written: Cow<'a, str>,
        /// The words in canonical order (modifier, signedness, width, type).
        canonical: Cow<'a, str>,
    },
    /// `decltype(auto)`.
    DecltypeAuto,
    /// `decltype(expression)`.
    Decltype(NodeId),
    /// A type named by a nested name.
    TypeName {
        /// `class`, `struct`, `enum`, `union`, `typename`, or `""`.
        key: &'a str,
        /// The `NestedName`.
        name: NodeId,
        /// The placeholder after it: `auto`, `decltype(auto)`, or `""`.
        placeholder: &'a str,
    },
    /// Simple specifiers before or after the type.
    SpecsSimple(SimpleSpecs<'a>),
    /// `explicit` and its condition.
    ExplicitSpec(Option<NodeId>),
    /// Declaration specifiers.
    DeclSpecs {
        /// The `SpecsSimple` before the type.
        left: NodeId,
        /// The `SpecsSimple` after the type.
        right: Option<NodeId>,
        /// The type specifier (`Fundamental`, `Decltype*`, `TypeName`).
        trailing: Option<NodeId>,
    },
    /// `[size]`.
    Array(Option<NodeId>),
    /// A function's parameters and qualifiers.
    ParamQual(ParamQual<'a>),
    /// `noexcept` and its condition.
    NoexceptSpec(Option<NodeId>),
    /// A function parameter: its `TypeWithInit`, or `None` for `...`.
    FunctionParam(Option<NodeId>),
    /// The declarator-id and its suffixes.
    DName {
        /// The `NestedName`.
        name: Option<NodeId>,
        /// The `Array`s.
        arrays: List,
        /// The `ParamQual`.
        params: Option<NodeId>,
    },
    /// A bit-field.
    DBitfield {
        /// The `NestedName`.
        name: Option<NodeId>,
        /// The width.
        size: NodeId,
    },
    /// `*` and the declarator after it.
    DPtr {
        /// The next declarator.
        next: NodeId,
        /// `const` after the `*`.
        is_const: bool,
        /// `volatile` after the `*`.
        is_volatile: bool,
        /// The attributes.
        attrs: List,
    },
    /// `&` and the declarator after it.
    DRef {
        /// The next declarator.
        next: NodeId,
        /// The attributes.
        attrs: List,
    },
    /// `...` and the declarator after it.
    DPack(NodeId),
    /// A pointer to member.
    DMemPtr {
        /// The class `NestedName`.
        class: NodeId,
        /// The next declarator.
        next: NodeId,
        /// `const` after the `::*`.
        is_const: bool,
        /// `volatile` after the `::*`.
        is_volatile: bool,
    },
    /// `(inner) next`.
    DParen {
        /// The declarator in parentheses.
        inner: NodeId,
        /// The declarator after them.
        next: NodeId,
    },
    /// A type: declaration specifiers and a declarator.
    Type {
        /// The `DeclSpecs`.
        specs: NodeId,
        /// The declarator.
        decl: NodeId,
    },
    /// A type and its initializer.
    TypeWithInit {
        /// The `Type`.
        ty: NodeId,
        /// The `Initializer`.
        init: Option<NodeId>,
    },
    /// An initializer.
    Initializer {
        /// The value.
        value: NodeId,
        /// Written with `=`.
        has_assign: bool,
    },
    /// A constrained template parameter with a default type.
    ConstrainedInit {
        /// The `Type`.
        ty: NodeId,
        /// The default `Type`.
        default: NodeId,
    },
    /// A type alias (`using`) or a declared type name.
    TypeUsing {
        /// The `NestedName`.
        name: NodeId,
        /// The aliased `Type`.
        ty: Option<NodeId>,
    },
    /// A concept.
    Concept {
        /// The `NestedName`.
        name: NodeId,
        /// The `Initializer`.
        init: Option<NodeId>,
    },
    /// A base class.
    BaseClass {
        /// The `NestedName`.
        name: NodeId,
        /// `public`, `protected`, `private`, or `""`.
        visibility: &'a str,
        /// `virtual`.
        is_virtual: bool,
        /// Followed by `...`.
        is_pack: bool,
    },
    /// A class.
    Class {
        /// The `NestedName`.
        name: NodeId,
        /// The `BaseClass`es.
        bases: List,
        /// `final`.
        is_final: bool,
        /// The attributes.
        attrs: List,
    },
    /// A union.
    Union {
        /// The `NestedName`.
        name: NodeId,
        /// The attributes.
        attrs: List,
    },
    /// An enumeration.
    Enum {
        /// The `NestedName`.
        name: NodeId,
        /// The underlying `Type`.
        underlying: Option<NodeId>,
        /// The attributes.
        attrs: List,
    },
    /// An enumerator.
    Enumerator {
        /// The `NestedName`.
        name: NodeId,
        /// The `Initializer`.
        init: Option<NodeId>,
        /// The attributes.
        attrs: List,
    },

    // Templates.
    /// `typename` or `class`, a name, and a default.
    TpKey {
        /// `typename` or `class`.
        key: &'a str,
        /// The `Identifier`.
        ident: Option<NodeId>,
        /// Written with `...`.
        is_pack: bool,
        /// The default `Type`.
        default: Option<NodeId>,
    },
    /// A type parameter, its `TpKey`.
    TpType(NodeId),
    /// A template template parameter.
    TpTemplate {
        /// The nested `TemplateParams`.
        params: NodeId,
        /// The `TpKey`.
        key: NodeId,
    },
    /// A non-type parameter.
    TpNonType {
        /// The `TypeWithInit` or `ConstrainedInit`.
        param: NodeId,
        /// Followed by `...`.
        is_pack: bool,
    },
    /// `template<...>` and a requires clause.
    TemplateParams {
        /// The parameters.
        params: List,
        /// The `Requires` clause.
        requires: Option<NodeId>,
    },
    /// A parameter of a template introduction.
    IntroParam {
        /// The `Identifier`.
        ident: NodeId,
        /// Written with `...`.
        is_pack: bool,
    },
    /// `Concept{params}`.
    TemplateIntro {
        /// The concept's `NestedName`.
        concept: NodeId,
        /// The `IntroParam`s.
        params: List,
    },
    /// The template parameter lists before a declaration.
    TemplatePrefix {
        /// The `TemplateParams` and `TemplateIntro`s.
        lists: List,
        /// A member's `template` without a list: an explicit instantiation.
        is_instantiation: bool,
    },
    /// `requires expression`.
    Requires(NodeId),

    // Attributes.
    /// `[[argument]]`, its argument.
    AttrCpp(&'a str),
    /// One attribute of `__attribute__((...))`.
    AttrGnu {
        /// The name.
        name: &'a str,
        /// The `ParenList`.
        args: Option<NodeId>,
    },
    /// `__attribute__((...))`, its `AttrGnu`s.
    AttrGnuList(List),

    // Declarations.
    /// A whole declaration.
    Declaration {
        /// What the directive declares.
        object: ObjectType,
        /// The directive it was written with.
        directive: Directive,
        /// `public`, `protected`, `private`, or `""`.
        visibility: &'a str,
        /// The `TemplatePrefix`.
        prefix: Option<NodeId>,
        /// What is declared (`Type`, `TypeWithInit`, `Class`, ...).
        decl: NodeId,
        /// A function's trailing `Requires` clause.
        requires: Option<NodeId>,
        /// Followed by `;`.
        has_semicolon: bool,
    },
    /// A namespace directive's argument, or a cross-reference's name.
    Namespace {
        /// The `NestedName`.
        name: NodeId,
        /// The `TemplatePrefix`.
        prefix: Option<NodeId>,
    },
}

/// A node and the next node of the list it is in.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Slot<'a> {
    pub node: Node<'a>,
    pub next: Option<NodeId>,
}

/// The tree of one parsed definition. It borrows the definition's text.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ast<'a> {
    pub(crate) def: &'a str,
    pub(crate) slots: Vec<Slot<'a>>,
    pub(crate) used_fallback: bool,
}

impl<'a> Ast<'a> {
    /// Returns the node `id` refers to. Panics when `id` is not of this tree, which is a
    /// caller's bug.
    pub fn node(&self, id: NodeId) -> &Node<'a> {
        &self.slots[id.index()].node
    }

    /// Returns the node after `id` in the list it is in.
    pub fn next(&self, id: NodeId) -> Option<NodeId> {
        self.slots[id.index()].next
    }

    /// Returns an iterator over a list from its head.
    pub fn list(&self, head: List) -> ListIter<'_, 'a> {
        ListIter { ast: self, at: head }
    }

    /// Returns the number of items in a list.
    pub fn count(&self, head: List) -> usize {
        self.list(head).count()
    }

    /// Returns the last item of a list.
    pub fn last(&self, head: List) -> Option<NodeId> {
        self.list(head).last()
    }

    /// Returns the item at `index` of a list.
    pub fn nth(&self, head: List, index: usize) -> Option<NodeId> {
        self.list(head).nth(index)
    }

    /// Returns the definition the tree was parsed from.
    pub fn definition(&self) -> &'a str {
        self.def
    }

    /// Returns the number of nodes in the tree.
    pub fn node_count(&self) -> usize {
        self.slots.len()
    }

    /// Tells whether an expression was scanned by the fallback scanner because it did
    /// not parse (Sphinx warns then; expression roles refuse it).
    pub fn is_fallback_used(&self) -> bool {
        self.used_fallback
    }
}

/// Walks a list of nodes (`Ast::list`).
#[derive(Debug, Clone)]
pub struct ListIter<'t, 'a> {
    ast: &'t Ast<'a>,
    at: Option<NodeId>,
}

impl Iterator for ListIter<'_, '_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        let id = self.at?;
        self.at = self.ast.next(id);
        Some(id)
    }
}
