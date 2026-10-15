//! A C++ signature as Sphinx's describe_signature writes it: the declaration's text,
//! with the declared name as Desc_Name (its qualification as Desc_Addname), and every
//! name a type or expression uses as an Xref{reftype "cpp:identifier"} to the name as
//! written from the root of its qualification, looked up from the declared symbol (its
//! path in Custom "cpp:parent"). The host resolves those as Sphinx's C++ domain does
//! and leaves the others as text.

use guidedoc::{Builder, Fail, INVALID_NODE, LinkInfo, LinkKind, NodeKind, SourceSpan};
use guidedoc_cpp::{
    Ast, Declaration, Expression, List, Node, NodeId, ObjectType, ParamQual, is_spaced_operator,
};

use super::{role_limits, xref_target};

/// How a node is described (Sphinx's modes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    MarkType,
    LastIsName,
    NoneIsName,
    Param,
    Udl,
}

/// The describer: the builder, the tree, and text not yet written as a node.
struct Describe<'d, 'b, 't> {
    b: &'d mut Builder<'b>,
    ast: &'d Ast<'t>,
    span: SourceSpan,
    text: String,
    /// The path cross-references look up from.
    key: &'d str,
    /// The declaration's template parameter lists (lastIsName).
    templates: List,
}

/// Writes a declaration's signature, as ASTDeclaration.describe_signature does with
/// mode lastIsName.
pub(super) fn describe_declaration(
    b: &mut Builder<'_>,
    decl: &Declaration<'_>,
    key: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    let ast = &decl.ast;
    let Node::Declaration {
        object,
        directive,
        visibility,
        decl: what,
        requires,
        has_semicolon,
        ..
    } = *ast.node(decl.root)
    else {
        return Ok(());
    };
    let mut d = Describe {
        b,
        ast,
        span,
        text: String::new(),
        key,
        templates: ast.template_lists(decl.root),
    };
    for t in ast.list(d.templates) {
        d.node(t, Mode::LastIsName)?;
    }
    if !visibility.is_empty() && visibility != "public" {
        d.put(visibility);
        d.put(" ");
    }
    d.put(&object_keyword(ast, object, directive.name(), what));
    d.node(what, Mode::LastIsName)?;
    if let Some(r) = requires {
        d.put(" ");
        d.node(r, Mode::MarkType)?;
    }
    if has_semicolon {
        d.put(";");
    }
    d.flush()
}

/// The keyword before what a declaration declares.
fn object_keyword(ast: &Ast<'_>, object: ObjectType, directive: &str, what: NodeId) -> String {
    match object {
        ObjectType::Type => match *ast.node(what) {
            Node::TypeUsing { .. } => "using ".into(),
            Node::Type { specs, .. }
                if matches!(ast.node(specs), Node::DeclSpecs { trailing: Some(_), .. }) =>
            {
                "typedef ".into()
            }
            _ => "type ".into(),
        },
        ObjectType::Concept => "concept ".into(),
        ObjectType::Class => format!("{directive} "),
        ObjectType::Union => "union ".into(),
        ObjectType::Enum => match directive {
            "enum-class" => "enum class ".into(),
            "enum-struct" => "enum struct ".into(),
            _ => "enum ".into(),
        },
        ObjectType::Enumerator => "enumerator ".into(),
        ObjectType::Function | ObjectType::Member => String::new(),
    }
}

/// Writes the expression or type of an expr or texpr role, as Sphinx describes it in
/// markType mode.
pub(super) fn describe_expression(
    b: &mut Builder<'_>,
    expr: &Expression<'_>,
    key: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    let mut d = Describe { b, ast: &expr.ast, span, text: String::new(), key, templates: None };
    d.node(expr.root, Mode::MarkType)?;
    d.flush()
}

impl Describe<'_, '_, '_> {
    fn put(&mut self, s: &str) {
        self.text.push_str(s);
    }

    fn flush(&mut self) -> Result<(), Fail> {
        if !self.text.is_empty() {
            self.b.add_text_leaf(NodeKind::Text, self.span, &self.text)?;
            self.text.clear();
        }
        Ok(())
    }

    fn open(&mut self, kind: NodeKind) -> Result<guidedoc::NodeId, Fail> {
        self.flush()?;
        self.b.begin_node(kind, self.span)
    }

    fn close(&mut self, id: guidedoc::NodeId) -> Result<(), Fail> {
        self.flush()?;
        self.b.end_node(id)
    }

    /// str() of a node.
    fn str_of(&self, id: Option<NodeId>) -> String {
        id.map(|id| self.ast.text(id, false)).unwrap_or_default()
    }

    /// The items of a list as str(), apart by spaces.
    fn str_list(&self, head: List) -> String {
        let mut out = String::new();
        self.ast.write_list(head, " ", false, &mut out);
        out
    }

    /// Writes `shown` as a cross-reference to `target`.
    fn xref(&mut self, target: &str, shown: &str) -> Result<(), Fail> {
        let id = self.open(NodeKind::Xref)?;
        let name = self.b.add_text(target)?;
        self.b.set_link(
            id,
            LinkInfo {
                name,
                kind: LinkKind::Unresolved,
                resolved: INVALID_NODE,
                ..LinkInfo::UNRESOLVED
            },
        )?;
        self.b.add_custom_attribute(id, "reftype", "cpp:identifier")?;
        self.b.add_custom_attribute(id, "unstyled", "1")?;
        if !self.key.is_empty() {
            self.b.add_custom_attribute(id, "cpp:parent", self.key)?;
        }
        let limits = role_limits(self.b);
        xref_target(self.b, id, target, false, limits)?;
        self.put(shown);
        self.close(id)
    }

    fn opt(&mut self, id: Option<NodeId>, mode: Mode) -> Result<(), Fail> {
        match id {
            Some(id) => self.node(id, mode),
            None => Ok(()),
        }
    }

    /// Writes the items of a list apart by `sep`.
    fn list(&mut self, head: List, sep: &str, mode: Mode) -> Result<(), Fail> {
        for (k, item) in self.ast.list(head).enumerate() {
            if k > 0 {
                self.put(sep);
            }
            self.node(item, mode)?;
        }
        Ok(())
    }

    /// Writes a node in a mode.
    fn node(&mut self, id: NodeId, mode: Mode) -> Result<(), Fail> {
        let ast = self.ast;
        match *ast.node(id) {
            Node::NestedName { .. } => self.nested(id, mode),
            Node::NameElement { .. } => self.element(id, mode, ""),
            Node::TemplateArgs { args, is_pack } => {
                self.put("<");
                self.list(args, ", ", Mode::MarkType)?;
                if is_pack {
                    self.put("...");
                }
                self.put(">");
                Ok(())
            }
            Node::ArgConstant(e) => self.node(e, mode),
            _ => {
                if self.type_part(id, mode)?
                    || self.declarator(id, mode)?
                    || self.declaration(id, mode)?
                    || self.template(id, mode)?
                {
                    return Ok(());
                }
                self.expression(id, mode)
            }
        }
    }

    /// Writes a type, its specifiers, or a parameter; false for another node.
    fn type_part(&mut self, id: NodeId, mode: Mode) -> Result<bool, Fail> {
        let ast = self.ast;
        let written = match *ast.node(id) {
            Node::Type { specs, decl } => {
                let shown = self.str_of(Some(specs));
                self.node(specs, Mode::MarkType)?;
                if ast.space_after_specs(decl) && !shown.is_empty() {
                    self.put(" ");
                }
                self.node(decl, if mode == Mode::MarkType { Mode::NoneIsName } else { mode })
            }
            Node::DeclSpecs { left, right, trailing } => {
                let shown = self.str_of(Some(left));
                self.put(&shown);
                let Some(trailing) = trailing else { return Ok(true) };
                if !shown.is_empty() {
                    self.put(" ");
                }
                self.node(trailing, mode)?;
                let after = self.str_of(right);
                if !after.is_empty() {
                    self.put(" ");
                    self.put(&after);
                }
                Ok(())
            }
            Node::TypeName { key, name, placeholder } => {
                if !key.is_empty() {
                    self.put(key);
                    self.put(" ");
                }
                self.node(name, mode)?;
                if !placeholder.is_empty() {
                    self.put(" ");
                    self.put(placeholder);
                }
                Ok(())
            }
            Node::Decltype(e) => {
                self.put("decltype(");
                self.node(e, mode)?;
                self.put(")");
                Ok(())
            }
            Node::TypeWithInit { ty, init } => {
                self.node(ty, mode)?;
                self.opt(init, mode)
            }
            Node::ConstrainedInit { ty, default } => {
                self.node(ty, mode)?;
                self.put(" = ");
                self.node(default, mode)
            }
            Node::Initializer { value, has_assign } => {
                if has_assign {
                    self.put(" = ");
                }
                self.node(value, Mode::MarkType)
            }
            Node::FunctionParam(arg) => match arg {
                Some(arg) => self.node(arg, mode),
                None => {
                    self.put("...");
                    Ok(())
                }
            },
            _ => return Ok(false),
        };
        written.map(|()| true)
    }

    fn element(&mut self, id: NodeId, mode: Mode, prefix: &str) -> Result<(), Fail> {
        let Node::NameElement { name, args, .. } = *self.ast.node(id) else { return Ok(()) };
        let targs = self.str_of(args);
        self.ident_or_op(name, mode, prefix, &targs)?;
        self.opt(args, Mode::MarkType)
    }

    fn ident_or_op(
        &mut self,
        id: NodeId,
        mode: Mode,
        prefix: &str,
        targs: &str,
    ) -> Result<(), Fail> {
        let Node::Identifier(text) = *self.ast.node(id) else {
            return self.operator(id, mode, prefix, targs);
        };
        let shown = if text.starts_with('@') { "[anonymous]" } else { text };
        match mode {
            Mode::MarkType => self.xref(&format!("{prefix}{text}{targs}"), shown),
            Mode::LastIsName => {
                let name = self.open(NodeKind::DescName)?;
                self.put(shown);
                self.close(name)
            }
            Mode::Udl => self.xref(&format!("operator\"\"{text}"), shown),
            _ => {
                self.put(shown);
                Ok(())
            }
        }
    }

    /// The operator's name as the declared name, as a link (with "operator" before it),
    /// or as a qualification.
    fn operator(&mut self, id: NodeId, mode: Mode, prefix: &str, targs: &str) -> Result<(), Fail> {
        match mode {
            Mode::LastIsName => {
                let name = self.open(NodeKind::DescName)?;
                self.operator_text(id)?;
                self.close(name)
            }
            Mode::MarkType => {
                self.put("operator");
                let shown = match *self.ast.node(id) {
                    Node::OpBuiltin(op) => {
                        if is_spaced_operator(op) {
                            self.put(" ");
                        }
                        op.to_owned()
                    }
                    Node::OpType(t) => {
                        self.put(" ");
                        self.str_of(Some(t))
                    }
                    Node::OpLiteral(s) => {
                        self.put("\"\"");
                        self.str_of(Some(s))
                    }
                    _ => String::new(),
                };
                let target = format!("{prefix}{}{targs}", self.str_of(Some(id)));
                self.xref(&target, &shown)
            }
            _ => {
                let add = self.open(NodeKind::DescAddname)?;
                self.operator_text(id)?;
                self.close(add)
            }
        }
    }

    fn operator_text(&mut self, id: NodeId) -> Result<(), Fail> {
        self.put("operator");
        match *self.ast.node(id) {
            Node::OpBuiltin(op) => {
                if is_spaced_operator(op) {
                    self.put(" ");
                }
                self.put(op);
                Ok(())
            }
            Node::OpLiteral(s) => {
                self.put("\"\"");
                self.ident_or_op(s, Mode::MarkType, "", "")
            }
            Node::OpType(t) => {
                self.put(" ");
                self.node(t, Mode::MarkType)
            }
            _ => Ok(()),
        }
    }

    /// ASTNestedName.describe_signature: every element a link to the qualification so
    /// far (lastIsName puts them in Desc_Addname and the last in Desc_Name, with the
    /// declaration's template parameter lists before its targets).
    fn nested(&mut self, id: NodeId, mode: Mode) -> Result<(), Fail> {
        let ast = self.ast;
        let Node::NestedName { is_rooted, elements } = *ast.node(id) else { return Ok(()) };
        match mode {
            Mode::NoneIsName | Mode::Udl => {
                if is_rooted {
                    self.put("::");
                }
                for (k, e) in ast.list(elements).enumerate() {
                    if k > 0 {
                        self.put("::");
                    }
                    self.element(e, Mode::NoneIsName, "")?;
                }
                return Ok(());
            }
            Mode::Param => return self.opt(elements, Mode::Param),
            Mode::MarkType | Mode::LastIsName => {}
        }
        let last = ast.last(elements);
        let count = ast.count(elements);
        let add = if mode == Mode::LastIsName && count > 1 {
            Some(self.open(NodeKind::DescAddname)?)
        } else {
            None
        };
        self.qualification(elements, last, is_rooted, mode)?;
        if mode != Mode::LastIsName {
            return Ok(());
        }
        if let Some(add) = add {
            self.put("::");
            self.close(add)?;
        }
        let Some(last) = last else { return Ok(()) };
        if matches!(ast.node(last), Node::NameElement { has_template_keyword: true, .. }) {
            self.put("template ");
        }
        self.element(last, Mode::LastIsName, "")
    }

    /// Writes the elements of a nested name before the last (all of them unless
    /// `mode` is lastIsName), each a link to the qualification so far.
    fn qualification(
        &mut self,
        elements: List,
        last: Option<NodeId>,
        is_rooted: bool,
        mode: Mode,
    ) -> Result<(), Fail> {
        let ast = self.ast;
        let (mut prefix, mut tp_prefix) = (String::new(), String::new());
        if is_rooted {
            prefix.push_str("::");
            self.put("::");
        }
        let mut tp = self.templates;
        for (k, e) in ast.list(elements).enumerate() {
            if mode == Mode::LastIsName && Some(e) == last {
                break;
            }
            if k > 0 {
                self.put("::");
                prefix.push_str("::");
            }
            let Node::NameElement { args, has_template_keyword, .. } = *ast.node(e) else {
                continue;
            };
            if has_template_keyword {
                self.put("template ");
            }
            let text = self.str_of(Some(e));
            if !text.is_empty() {
                if args.is_some()
                    && mode == Mode::LastIsName
                    && let Some(t) = tp
                {
                    tp_prefix.push_str(&self.str_of(Some(t)));
                    tp = ast.next(t);
                }
                let target = format!("{tp_prefix}{prefix}");
                self.element(e, Mode::MarkType, &target)?;
            }
            prefix.push_str(&text);
        }
        Ok(())
    }
}

impl Describe<'_, '_, '_> {
    /// Writes a declarator; false for another node.
    fn declarator(&mut self, id: NodeId, mode: Mode) -> Result<bool, Fail> {
        let ast = self.ast;
        match *ast.node(id) {
            Node::DName { name, arrays, params } => {
                self.opt(name, mode)?;
                for a in ast.list(arrays) {
                    self.put("[");
                    if let Node::Array(size) = *ast.node(a) {
                        self.opt(size, Mode::MarkType)?;
                    }
                    self.put("]");
                }
                if let Some(q) = params
                    && let Node::ParamQual(pq) = *ast.node(q)
                {
                    self.param_qual(&pq, mode)?;
                }
            }
            Node::DBitfield { name, size } => {
                self.opt(name, mode)?;
                self.put(" : ");
                self.node(size, mode)?;
            }
            Node::DPtr { next, is_const, is_volatile, attrs } => {
                self.pointer(next, (is_const, is_volatile), attrs, mode)?;
            }
            Node::DRef { next, attrs } => {
                self.put("&");
                let a = self.str_list(attrs);
                self.put(&a);
                if attrs.is_some() && ast.space_after_specs(next) {
                    self.put(" ");
                }
                self.node(next, mode)?;
            }
            Node::DPack(next) => {
                self.put("...");
                if ast.declarator_name(next).is_some() {
                    self.put(" ");
                }
                self.node(next, mode)?;
            }
            Node::DMemPtr { class, next, is_const, is_volatile } => {
                self.node(class, Mode::MarkType)?;
                self.put("::*");
                if is_volatile {
                    self.put("volatile");
                }
                if is_const {
                    self.put(if is_volatile { " const" } else { "const" });
                }
                if ast.space_after_specs(next) {
                    self.put(" ");
                }
                self.node(next, mode)?;
            }
            Node::DParen { inner, next } => {
                self.put("(");
                self.node(inner, mode)?;
                self.put(")");
                self.node(next, Mode::NoneIsName)?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// `*`, its attributes and cv-qualifiers, and the declarator after it.
    fn pointer(
        &mut self,
        next: NodeId,
        cv: (bool, bool),
        attrs: List,
        mode: Mode,
    ) -> Result<(), Fail> {
        let ast = self.ast;
        let (is_const, is_volatile) = cv;
        self.put("*");
        let a = self.str_list(attrs);
        self.put(&a);
        let cv = is_const || is_volatile;
        if attrs.is_some() && cv {
            self.put(" ");
        }
        if is_volatile {
            self.put("volatile");
        }
        if is_const {
            self.put(if is_volatile { " const" } else { "const" });
        }
        if (cv || attrs.is_some()) && ast.space_after_specs(next) {
            self.put(" ");
        }
        self.node(next, mode)?;
        Ok(())
    }

    fn param_qual(&mut self, q: &ParamQual<'_>, mode: Mode) -> Result<(), Fail> {
        self.put("(");
        let inner = if mode == Mode::LastIsName { Mode::Param } else { Mode::MarkType };
        self.list(q.params, ", ", inner)?;
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
        if let Some(n) = q.noexcept {
            self.put(" noexcept");
            if let Node::NoexceptSpec(Some(e)) = *self.ast.node(n) {
                self.put("(");
                self.node(e, Mode::MarkType)?;
                self.put(")");
            }
        }
        if let Some(r) = q.trailing_return {
            self.put(" -> ");
            self.node(r, mode)?;
        }
        if q.is_final {
            self.put(" final");
        }
        if q.is_override {
            self.put(" override");
        }
        if q.attrs.is_some() {
            self.put(" ");
            let a = self.str_list(q.attrs);
            self.put(&a);
        }
        if !q.initializer.is_empty() {
            self.put(" = ");
            self.put(q.initializer);
        }
        Ok(())
    }

    /// Writes attributes and a space, when there are any.
    fn attrs(&mut self, attrs: List) {
        if attrs.is_some() {
            let a = self.str_list(attrs);
            self.put(&a);
            self.put(" ");
        }
    }

    /// Writes what a declaration declares, other than a type; false for another node.
    fn declaration(&mut self, id: NodeId, mode: Mode) -> Result<bool, Fail> {
        let ast = self.ast;
        match *ast.node(id) {
            Node::TypeUsing { name, ty } => {
                self.node(name, mode)?;
                if let Some(ty) = ty {
                    self.put(" = ");
                    self.node(ty, Mode::MarkType)?;
                }
            }
            Node::Concept { name, init } => {
                self.node(name, mode)?;
                self.opt(init, mode)?;
            }
            Node::Class { name, bases, is_final, attrs } => {
                self.attrs(attrs);
                self.node(name, mode)?;
                if is_final {
                    self.put(" final");
                }
                if bases.is_some() {
                    self.put(" : ");
                }
                for (k, base) in ast.list(bases).enumerate() {
                    if k > 0 {
                        self.put(", ");
                    }
                    let Node::BaseClass { name, visibility, is_virtual, is_pack } =
                        *ast.node(base)
                    else {
                        continue;
                    };
                    if !visibility.is_empty() {
                        self.put(visibility);
                        self.put(" ");
                    }
                    if is_virtual {
                        self.put("virtual ");
                    }
                    self.node(name, Mode::MarkType)?;
                    if is_pack {
                        self.put("...");
                    }
                }
            }
            Node::Union { name, attrs } => {
                self.attrs(attrs);
                self.node(name, mode)?;
            }
            Node::Enum { name, underlying, attrs } => {
                self.attrs(attrs);
                self.node(name, mode)?;
                if let Some(u) = underlying {
                    self.put(" : ");
                    self.node(u, Mode::NoneIsName)?;
                }
            }
            Node::Enumerator { name, init, attrs } => {
                self.node(name, mode)?;
                if attrs.is_some() {
                    self.put(" ");
                    let a = self.str_list(attrs);
                    self.put(&a);
                }
                self.opt(init, Mode::MarkType)?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Writes template parameters; false for another node.
    fn template(&mut self, id: NodeId, mode: Mode) -> Result<bool, Fail> {
        let ast = self.ast;
        match *ast.node(id) {
            Node::TemplateParams { params, requires } => {
                self.put("template<");
                self.list(params, ", ", mode)?;
                self.put(">");
                if let Some(r) = requires {
                    self.put(" ");
                    self.node(r, Mode::MarkType)?;
                }
                self.put(" ");
            }
            Node::TpKey { key, ident, is_pack, default } => {
                self.put(key);
                if is_pack {
                    if ident.is_some() {
                        self.put(" ");
                    }
                    self.put("...");
                }
                if let Some(ident) = ident {
                    if !is_pack {
                        self.put(" ");
                    }
                    self.ident_or_op(ident, mode, "", "")?;
                }
                if let Some(t) = default {
                    self.put(" = ");
                    self.node(t, Mode::MarkType)?;
                }
            }
            Node::TpType(key) => self.node(key, mode)?,
            Node::TpTemplate { params, key } => {
                self.node(params, Mode::NoneIsName)?;
                self.node(key, mode)?;
            }
            Node::TpNonType { param, is_pack } => {
                self.node(param, mode)?;
                if is_pack {
                    self.put("...");
                }
            }
            Node::TemplateIntro { concept, params } => {
                self.node(concept, Mode::MarkType)?;
                self.put("{");
                for (k, t) in ast.list(params).enumerate() {
                    if k > 0 {
                        self.put(", ");
                    }
                    let Node::IntroParam { ident, is_pack } = *ast.node(t) else { continue };
                    if is_pack {
                        self.put("...");
                    }
                    self.ident_or_op(ident, Mode::LastIsName, "", "")?;
                }
                self.put("} ");
            }
            Node::Requires(e) => {
                self.put("requires ");
                self.node(e, mode)?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Writes an expression as its text, its names as links.
    fn expression(&mut self, id: NodeId, mode: Mode) -> Result<(), Fail> {
        let ast = self.ast;
        match *ast.node(id) {
            Node::IdExpr(name) => self.node(name, mode),
            Node::Paren(e) => self.wrapped("(", e, ")", mode),
            Node::Postfix { prefix, ops } => self.postfix(prefix, ops, mode),
            Node::BinOp { first, rest } => {
                self.node(first, mode)?;
                for item in ast.list(rest) {
                    let Node::BinOperand { op, expr } = *ast.node(item) else { continue };
                    self.put(" ");
                    self.put(op);
                    self.put(" ");
                    self.node(expr, mode)?;
                }
                Ok(())
            }
            Node::Comma(items) => self.list(items, ", ", mode),
            Node::ParenList(items) => {
                self.put("(");
                self.list(items, ", ", mode)?;
                self.put(")");
                Ok(())
            }
            Node::BracedInit { items, has_trailing_comma } => {
                self.put("{");
                self.list(items, ", ", mode)?;
                if has_trailing_comma {
                    self.put(",");
                }
                self.put("}");
                Ok(())
            }
            Node::Unary { op, operand } => {
                self.put(op);
                if op.starts_with('c') || op.starts_with('n') {
                    self.put(" ");
                }
                self.node(operand, mode)
            }
            Node::LitUser { literal, suffix } => {
                self.node(literal, mode)?;
                self.ident_or_op(suffix, Mode::Udl, "", "")
            }
            Node::SizeofPack(ident) => {
                self.put("sizeof...(");
                self.ident_or_op(ident, Mode::MarkType, "", "")?;
                self.put(")");
                Ok(())
            }
            Node::PackExpansion(e) => {
                self.node(e, mode)?;
                self.put("...");
                Ok(())
            }
            _ => self.operand_expression(id, mode),
        }
    }

    /// A prefix and its postfix operations.
    fn postfix(&mut self, prefix: NodeId, ops: List, mode: Mode) -> Result<(), Fail> {
        let ast = self.ast;
        self.node(prefix, mode)?;
        for op in ast.list(ops) {
            match *ast.node(op) {
                Node::PfMember(n) => self.wrapped(".", n, "", Mode::NoneIsName)?,
                Node::PfMemberPtr(n) => self.wrapped("->", n, "", Mode::NoneIsName)?,
                Node::PfArray(e) => self.wrapped("[", e, "]", mode)?,
                Node::PfCall(e) => self.node(e, mode)?,
                _ => {
                    let t = self.str_of(Some(op));
                    self.put(&t);
                }
            }
        }
        Ok(())
    }

    /// Writes `before`, a node, and `after`.
    fn wrapped(&mut self, before: &str, id: NodeId, after: &str, mode: Mode) -> Result<(), Fail> {
        self.put(before);
        self.node(id, mode)?;
        self.put(after);
        Ok(())
    }

    /// The expressions around a type or an operand, as Sphinx's ASTSizeofType,
    /// ASTAlignofExpr, ASTCastExpr, and the like describe them; others as their text.
    fn operand_expression(&mut self, id: NodeId, mode: Mode) -> Result<(), Fail> {
        match *self.ast.node(id) {
            Node::SizeofType(t) => self.wrapped("sizeof(", t, ")", mode),
            Node::Alignof(t) => self.wrapped("alignof(", t, ")", mode),
            Node::NoexceptExpr(e) => self.wrapped("noexcept(", e, ")", mode),
            Node::SizeofExpr(e) => self.wrapped("sizeof ", e, "", mode),
            Node::Cast { ty, operand } => {
                self.wrapped("(", ty, ")", mode)?;
                self.node(operand, mode)
            }
            Node::ExplicitCast { cast, ty, expr } => {
                self.put(cast);
                self.wrapped("<", ty, ">(", mode)?;
                self.wrapped("", expr, ")", mode)
            }
            Node::TypeId { operand, .. } => self.wrapped("typeid(", operand, ")", mode),
            _ => {
                let t = self.str_of(Some(id));
                self.put(&t);
                Ok(())
            }
        }
    }
}
