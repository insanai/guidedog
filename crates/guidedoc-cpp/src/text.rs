//! The text of a tree, as Sphinx's `_stringify` gives it: `str()` with `display` false,
//! `get_display_string()` with `display` true (anonymous names as `[anonymous]`).

use crate::ast::{Ast, List, Node, NodeId, ParamQual, SimpleSpecs};

/// Tells whether a built-in operator is shown after a space: `new`, `delete`, and
/// those spelled as words (`and`, `bitor`, ...).
pub fn is_spaced_operator(op: &str) -> bool {
    matches!(op, "new" | "new[]" | "delete" | "delete[]")
        || matches!(op.as_bytes().first(), Some(b'a' | b'b' | b'c' | b'n' | b'o' | b'x'))
}

impl Ast<'_> {
    /// Returns the text of a node (`str()`, or the display string when `display`).
    pub fn text(&self, id: NodeId, display: bool) -> String {
        let mut out = String::new();
        self.write_text(id, display, &mut out);
        out
    }

    /// Appends the text of a node to `out`.
    pub fn write_text(&self, id: NodeId, display: bool, out: &mut String) {
        Writer { ast: self, display, out }.node(id);
    }

    /// Appends the text of an optional node to `out`.
    pub fn write_opt(&self, id: Option<NodeId>, display: bool, out: &mut String) {
        if let Some(id) = id {
            self.write_text(id, display, out);
        }
    }

    /// Appends the text of a list's items to `out`, `sep` between them.
    pub fn write_list(&self, head: List, sep: &str, display: bool, out: &mut String) {
        for (i, item) in self.list(head).enumerate() {
            if i > 0 {
                out.push_str(sep);
            }
            self.write_text(item, display, out);
        }
    }
}

/// Writes nodes into one string.
struct Writer<'t, 'a, 'o> {
    ast: &'t Ast<'a>,
    display: bool,
    out: &'o mut String,
}

impl Writer<'_, '_, '_> {
    fn put(&mut self, s: &str) {
        self.out.push_str(s);
    }

    fn opt(&mut self, id: Option<NodeId>) {
        if let Some(id) = id {
            self.node(id);
        }
    }

    fn list(&mut self, head: List, sep: &str) {
        for (i, item) in self.ast.list(head).enumerate() {
            if i > 0 {
                self.put(sep);
            }
            self.node(item);
        }
    }

    fn wrapped(&mut self, before: &str, id: NodeId, after: &str) {
        self.put(before);
        self.node(id);
        self.put(after);
    }

    /// Writes a node in `str()` form whatever the writer's mode (where Sphinx calls
    /// `str()` inside a display string).
    fn plain(&mut self, id: NodeId) {
        let display = std::mem::replace(&mut self.display, false);
        self.node(id);
        self.display = display;
    }

    fn node(&mut self, id: NodeId) {
        match self.ast.node(id) {
            Node::Identifier(text) => {
                let shown =
                    if self.display && text.starts_with('@') { "[anonymous]" } else { text };
                self.put(shown);
            }
            Node::OpBuiltin(op) => {
                self.put(if is_spaced_operator(op) { "operator " } else { "operator" });
                self.put(op);
            }
            Node::OpLiteral(ident) => self.wrapped("operator\"\"", *ident, ""),
            Node::OpType(ty) => self.wrapped("operator ", *ty, ""),
            Node::NameElement { name, args, .. } => {
                self.node(*name);
                self.opt(*args);
            }
            Node::NestedName { is_rooted, elements } => self.nested(*is_rooted, *elements),
            Node::TemplateArgs { args, is_pack } => {
                self.put("<");
                self.list(*args, ", ");
                if *is_pack {
                    self.put("...");
                }
                self.put(">");
            }
            Node::ArgConstant(expr) => self.node(*expr),
            _ => {
                if !self.expression(id) && !self.ty(id) && !self.declarator(id) {
                    self.declaration(id);
                }
            }
        }
    }

    fn nested(&mut self, is_rooted: bool, elements: List) {
        if is_rooted {
            self.put("::"); // '::'.join(['', ...])
        }
        for (i, e) in self.ast.list(elements).enumerate() {
            if i > 0 {
                self.put("::");
            }
            if matches!(self.ast.node(e), Node::NameElement { has_template_keyword: true, .. }) {
                self.put("template ");
            }
            self.node(e);
        }
    }

    /// Writes an expression; false for other kinds.
    fn expression(&mut self, id: NodeId) -> bool {
        match self.ast.node(id) {
            Node::LitPointer => self.put("nullptr"),
            Node::LitBool(value) => self.put(if *value { "true" } else { "false" }),
            Node::LitNumber(text) | Node::LitString(text) | Node::Fallback(text) => self.put(text),
            Node::AttrCpp(text) => {
                self.put("[[");
                self.put(text);
                self.put("]]");
            }
            Node::LitChar { prefix, data, .. } => {
                self.put(prefix);
                self.put("'");
                self.put(data);
                self.put("'");
            }
            Node::LitUser { literal, suffix } => {
                self.node(*literal);
                self.node(*suffix);
            }
            Node::This => self.put("this"),
            Node::Fold { left, op, right } => self.fold(*left, op, *right),
            Node::Paren(inner) => self.wrapped("(", *inner, ")"),
            Node::IdExpr(name) => self.node(*name),
            Node::PackExpansion(expr) => self.wrapped("", *expr, "..."),
            Node::Postfix { prefix, ops } => {
                self.node(*prefix);
                self.list(*ops, "");
            }
            Node::PfArray(index) => self.wrapped("[", *index, "]"),
            Node::PfMember(name) => self.wrapped(".", *name, ""),
            Node::PfMemberPtr(name) => self.wrapped("->", *name, ""),
            Node::PfInc => self.put("++"),
            Node::PfDec => self.put("--"),
            Node::PfCall(list) => self.node(*list),
            Node::ExplicitCast { cast, ty, expr } => {
                self.put(cast);
                self.wrapped("<", *ty, ">(");
                self.wrapped("", *expr, ")");
            }
            Node::TypeId { operand, .. } => self.wrapped("typeid(", *operand, ")"),
            Node::Unary { op, operand } => {
                self.put(op);
                if op.starts_with(['c', 'n']) {
                    self.put(" ");
                }
                self.node(*operand);
            }
            Node::SizeofPack(ident) => self.wrapped("sizeof...(", *ident, ")"),
            Node::SizeofType(ty) => self.wrapped("sizeof(", *ty, ")"),
            Node::SizeofExpr(expr) => self.wrapped("sizeof ", *expr, ""),
            Node::Alignof(ty) => self.wrapped("alignof(", *ty, ")"),
            Node::NoexceptExpr(expr) => self.wrapped("noexcept(", *expr, ")"),
            _ => return self.expression_more(id),
        }
        true
    }

    fn fold(&mut self, left: Option<NodeId>, op: &str, right: Option<NodeId>) {
        self.put("(");
        if let Some(left) = left {
            self.node(left);
            self.put(" ");
            self.put(op);
            self.put(" ");
        }
        self.put("...");
        if let Some(right) = right {
            self.put(" ");
            self.put(op);
            self.put(" ");
            self.node(right);
        }
        self.put(")");
    }

    fn expression_more(&mut self, id: NodeId) -> bool {
        match self.ast.node(id) {
            Node::New { is_rooted, ty, init } => {
                self.put(if *is_rooted { "::new " } else { "new " });
                self.node(*ty);
                self.opt(*init);
            }
            Node::Delete { is_rooted, is_array, operand } => {
                self.put(if *is_rooted { "::delete " } else { "delete " });
                if *is_array {
                    self.put("[] ");
                }
                self.node(*operand);
            }
            Node::Cast { ty, operand } => {
                self.wrapped("(", *ty, ")");
                self.node(*operand);
            }
            Node::BinOp { first, rest } => {
                self.node(*first);
                self.list(*rest, "");
            }
            Node::BinOperand { op, expr } => {
                self.put(" ");
                self.put(op);
                self.put(" ");
                self.node(*expr);
            }
            Node::Conditional { cond, then, other } => {
                self.node(*cond);
                self.wrapped(" ? ", *then, " : ");
                self.node(*other);
            }
            Node::BracedInit { items, has_trailing_comma } => {
                self.put("{");
                self.list(*items, ", ");
                if *has_trailing_comma {
                    self.put(",");
                }
                self.put("}");
            }
            Node::Assignment { left, op, right } => {
                self.node(*left);
                self.put(" ");
                self.put(op);
                self.put(" ");
                self.node(*right);
            }
            Node::Comma(items) => self.list(*items, ", "),
            Node::ParenList(items) => {
                self.put("(");
                self.list(*items, ", ");
                self.put(")");
            }
            Node::AttrGnu { name, args } => {
                self.put(name);
                self.opt(*args);
            }
            Node::AttrGnuList(items) => {
                self.put("__attribute__((");
                self.list(*items, ", ");
                self.put("))");
            }
            _ => return false,
        }
        true
    }

    /// Writes a type or a part of one; false for other kinds.
    fn ty(&mut self, id: NodeId) -> bool {
        match self.ast.node(id) {
            Node::Fundamental { written, .. } => self.put(written),
            Node::DecltypeAuto => self.put("decltype(auto)"),
            Node::Decltype(expr) => self.wrapped("decltype(", *expr, ")"),
            Node::TypeName { key, name, placeholder } => {
                if !key.is_empty() {
                    self.put(key);
                    self.put(" ");
                }
                self.node(*name);
                if !placeholder.is_empty() {
                    self.put(" ");
                    self.put(placeholder);
                }
            }
            Node::SpecsSimple(specs) => self.simple_specs(specs),
            Node::ExplicitSpec(condition) => {
                self.put("explicit");
                if let Some(condition) = condition {
                    self.wrapped("(", *condition, ")");
                }
            }
            Node::DeclSpecs { left, right, trailing } => self.decl_specs(*left, *right, *trailing),
            Node::Array(size) => {
                self.put("[");
                self.opt(*size);
                self.put("]");
            }
            Node::ParamQual(q) => self.param_qual(q),
            Node::NoexceptSpec(condition) => {
                self.put("noexcept");
                if let Some(condition) = condition {
                    self.wrapped("(", *condition, ")");
                }
            }
            Node::FunctionParam(arg) => match arg {
                Some(arg) => self.node(*arg),
                None => self.put("..."),
            },
            _ => return false,
        }
        true
    }

    fn simple_specs(&mut self, s: &SimpleSpecs<'_>) {
        let mut first = s.attrs.is_none();
        self.list(s.attrs, " ");
        let mut part = |w: &mut Self, word: &str| {
            if !first {
                w.put(" ");
            }
            first = false;
            w.put(word);
        };
        if !s.storage.is_empty() {
            part(self, s.storage);
        }
        let words = [
            (s.is_thread_local, "thread_local"),
            (s.is_inline, "inline"),
            (s.is_friend, "friend"),
            (s.is_virtual, "virtual"),
        ];
        for (_, word) in words.into_iter().filter(|(on, _)| *on) {
            part(self, word);
        }
        if let Some(explicit) = s.explicit {
            part(self, "");
            self.node(explicit);
        }
        let more = [
            (s.is_consteval, "consteval"),
            (s.is_constexpr, "constexpr"),
            (s.is_constinit, "constinit"),
            (s.is_volatile, "volatile"),
            (s.is_const, "const"),
        ];
        for (_, word) in more.into_iter().filter(|(on, _)| *on) {
            part(self, word);
        }
    }

    fn decl_specs(&mut self, left: NodeId, right: Option<NodeId>, trailing: Option<NodeId>) {
        let start = self.out.len();
        self.node(left);
        let Some(trailing) = trailing else { return };
        if self.out.len() > start {
            self.put(" ");
        }
        self.node(trailing);
        let Some(right) = right else { return };
        let mark = self.out.len();
        self.put(" ");
        let after = self.out.len();
        self.plain(right); // str(), as Sphinx writes it.
        if self.out.len() == after {
            self.out.truncate(mark);
        }
    }

    fn param_qual(&mut self, q: &ParamQual<'_>) {
        self.put("(");
        let display = std::mem::replace(&mut self.display, false);
        self.list(q.params, ", "); // str(a), as Sphinx writes the parameters.
        self.display = display;
        self.put(")");
        if q.is_volatile {
            self.put(" volatile");
        }
        if q.is_const {
            self.put(" const");
        }
        if !q.ref_qualifier.is_empty() {
            self.put(" ");
            self.put(q.ref_qualifier);
        }
        if let Some(noexcept) = q.noexcept {
            self.wrapped(" ", noexcept, "");
        }
        if let Some(ret) = q.trailing_return {
            self.wrapped(" -> ", ret, "");
        }
        if q.is_final {
            self.put(" final");
        }
        if q.is_override {
            self.put(" override");
        }
        if q.attrs.is_some() {
            self.put(" ");
            self.list(q.attrs, " ");
        }
        if !q.initializer.is_empty() {
            self.put(" = ");
            self.put(q.initializer);
        }
    }

    /// Writes `const`/`volatile` after a pointer or member pointer.
    fn cv(&mut self, is_const: bool, is_volatile: bool) {
        if is_volatile {
            self.put("volatile");
        }
        if is_const {
            if is_volatile {
                self.put(" ");
            }
            self.put("const");
        }
    }

    /// Writes a declarator or a typed part of a declaration; false for other kinds.
    fn declarator(&mut self, id: NodeId) -> bool {
        let ast = self.ast;
        match ast.node(id) {
            Node::DName { name, arrays, params } => {
                self.opt(*name);
                self.list(*arrays, "");
                self.opt(*params);
            }
            Node::DBitfield { name, size } => {
                self.opt(*name);
                self.wrapped(" : ", *size, "");
            }
            Node::DPtr { next, is_const, is_volatile, attrs } => {
                self.put("*");
                self.list(*attrs, " ");
                let cv = *is_const || *is_volatile;
                if attrs.is_some() && cv {
                    self.put(" ");
                }
                self.cv(*is_const, *is_volatile);
                if (cv || attrs.is_some()) && ast.space_after_specs(*next) {
                    self.put(" ");
                }
                self.node(*next);
            }
            Node::DRef { next, attrs } => {
                self.put("&");
                self.list(*attrs, " ");
                if attrs.is_some() && ast.space_after_specs(*next) {
                    self.put(" ");
                }
                self.node(*next);
            }
            Node::DPack(next) => {
                self.put("...");
                if ast.declarator_name(*next).is_some() {
                    self.put(" ");
                }
                self.node(*next);
            }
            Node::DMemPtr { class, next, is_const, is_volatile } => {
                self.wrapped("", *class, "::*");
                self.cv(*is_const, *is_volatile);
                if ast.space_after_specs(*next) {
                    self.put(" ");
                }
                self.node(*next);
            }
            Node::DParen { inner, next } => {
                self.wrapped("(", *inner, ")");
                self.node(*next);
            }
            _ => return self.typed_part(id),
        }
        true
    }

    fn typed_part(&mut self, id: NodeId) -> bool {
        match self.ast.node(id) {
            Node::Type { specs, decl } => {
                let start = self.out.len();
                self.node(*specs);
                if self.ast.space_after_specs(*decl) && self.out.len() > start {
                    self.put(" ");
                }
                self.node(*decl);
            }
            Node::TypeWithInit { ty, init } => {
                self.node(*ty);
                self.opt(*init);
            }
            Node::Initializer { value, has_assign } => {
                if *has_assign {
                    self.put(" = ");
                }
                self.node(*value);
            }
            Node::ConstrainedInit { ty, default } => {
                self.node(*ty);
                self.wrapped(" = ", *default, "");
            }
            _ => return false,
        }
        true
    }

    /// Writes a declaration's part, a template part, or a whole declaration.
    fn declaration(&mut self, id: NodeId) {
        match self.ast.node(id) {
            Node::TypeUsing { name, ty } => {
                self.node(*name);
                if let Some(ty) = ty {
                    self.wrapped(" = ", *ty, "");
                }
            }
            Node::Concept { name, init } => {
                self.node(*name);
                self.opt(*init);
            }
            Node::BaseClass { name, visibility, is_virtual, is_pack } => {
                if !visibility.is_empty() {
                    self.put(visibility);
                    self.put(" ");
                }
                if *is_virtual {
                    self.put("virtual ");
                }
                self.node(*name);
                if *is_pack {
                    self.put("...");
                }
            }
            Node::Class { name, bases, is_final, attrs } => {
                self.attributes(*attrs);
                self.node(*name);
                if *is_final {
                    self.put(" final");
                }
                if bases.is_some() {
                    self.put(" : ");
                    self.list(*bases, ", ");
                }
            }
            Node::Union { name, attrs } => {
                self.attributes(*attrs);
                self.node(*name);
            }
            Node::Enum { name, underlying, attrs } => {
                self.attributes(*attrs);
                self.node(*name);
                if let Some(underlying) = underlying {
                    self.wrapped(" : ", *underlying, "");
                }
            }
            Node::Enumerator { name, init, attrs } => {
                self.node(*name);
                if attrs.is_some() {
                    self.put(" ");
                    self.list(*attrs, " ");
                }
                self.opt(*init);
            }
            _ => self.template(id),
        }
    }

    /// Writes an attribute list and a space after it, when there is one.
    fn attributes(&mut self, attrs: List) {
        if attrs.is_some() {
            self.list(attrs, " ");
            self.put(" ");
        }
    }

    fn template(&mut self, id: NodeId) {
        match self.ast.node(id) {
            Node::TpKey { key, ident, is_pack, default } => {
                self.put(key);
                if *is_pack {
                    if ident.is_some() {
                        self.put(" ");
                    }
                    self.put("...");
                }
                if let Some(ident) = ident {
                    if !is_pack {
                        self.put(" ");
                    }
                    self.node(*ident);
                }
                if let Some(default) = default {
                    self.wrapped(" = ", *default, "");
                }
            }
            Node::TpType(key) => self.node(*key),
            Node::TpTemplate { params, key } => {
                self.node(*params);
                self.node(*key);
            }
            Node::TpNonType { param, is_pack } => {
                self.node(*param);
                if *is_pack {
                    self.put("...");
                }
            }
            Node::TemplateParams { params, requires } => {
                self.put("template<");
                self.list(*params, ", ");
                self.put("> ");
                if let Some(requires) = requires {
                    self.wrapped("", *requires, " ");
                }
            }
            Node::IntroParam { ident, is_pack } => {
                if *is_pack {
                    self.put("...");
                }
                self.node(*ident);
            }
            Node::TemplateIntro { concept, params } => {
                self.wrapped("", *concept, "{");
                self.list(*params, ", ");
                self.put("} ");
            }
            Node::TemplatePrefix { lists, .. } => self.list(*lists, ""),
            Node::Requires(expr) => self.wrapped("requires ", *expr, ""),
            Node::Declaration { .. } => self.whole_declaration(id),
            Node::Namespace { name, prefix } => {
                self.opt(*prefix);
                self.node(*name);
            }
            // The kinds the other writers handle never reach here.
            _ => {}
        }
    }

    fn whole_declaration(&mut self, id: NodeId) {
        let Node::Declaration { visibility, prefix, decl, requires, has_semicolon, .. } =
            *self.ast.node(id)
        else {
            return;
        };
        if !visibility.is_empty() && visibility != "public" {
            self.put(visibility);
            self.put(" ");
        }
        self.opt(prefix);
        self.node(decl);
        if let Some(requires) = requires {
            self.wrapped(" ", requires, "");
        }
        if has_semicolon {
            self.put(";");
        }
    }
}
