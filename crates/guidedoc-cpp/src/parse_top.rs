//! Templates and whole declarations, as `DefinitionParser` parses them.

use crate::ast::{Directive, Node, NodeId, ObjectType};
use crate::lex;
use crate::parse_decl::{Named, Outer};
use crate::parser::{Fail, ListBuilder, PResult, Parser};

/// What a template prefix is read for: a concept allows one list and no requires
/// clause; a member may be an explicit instantiation (`template` without a list).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrefixFor {
    Concept,
    Member,
    Other,
}

impl<'a> Parser<'a> {
    /// A (template) type parameter, or a non-type or constrained parameter.
    fn template_parameter(&mut self) -> PResult<NodeId> {
        self.skip_ws();
        let mut nested = None;
        if self.skip_word("template") {
            nested = Some(self.template_parameter_list()?);
        }
        let m = self.mark();
        if let Ok(res) = self.type_parameter(nested) {
            return Ok(res);
        }
        if nested.is_some() {
            return Err(Fail);
        }
        self.reset(m);
        let param = self.type_with_init(Named::Maybe, Outer::TemplateParam)?;
        self.skip_ws();
        let is_pack = self.skip_string("...");
        self.add(Node::TpNonType { param, is_pack })
    }

    fn type_parameter(&mut self, nested: Option<NodeId>) -> PResult<NodeId> {
        self.skip_ws();
        let key = if self.skip_word_ws("typename") {
            "typename"
        } else if self.skip_word_ws("class") {
            "class"
        } else {
            return Err(Fail);
        };
        self.skip_ws();
        let is_pack = self.skip_string("...");
        self.skip_ws();
        let ident = if self.match_identifier() { Some(self.add_identifier()?) } else { None };
        self.skip_ws();
        let mut default = None;
        if !is_pack && self.skip_string("=") {
            default = Some(self.parse_type(Named::False, Outer::None)?);
        } else if !matches!(self.current(), b',' | b'>') {
            return Err(Fail);
        }
        let key = self.add(Node::TpKey { key, ident, is_pack, default })?;
        match nested {
            Some(params) => self.add(Node::TpTemplate { params, key }),
            None => self.add(Node::TpType(key)),
        }
    }

    /// `<` parameters `>` and a requires clause.
    fn template_parameter_list(&mut self) -> PResult<NodeId> {
        self.guarded(Parser::template_parameter_list_inner)
    }

    fn template_parameter_list_inner(&mut self) -> PResult<NodeId> {
        self.skip_ws();
        if !self.skip_string("<") {
            return Err(Fail);
        }
        let mut params = ListBuilder::default();
        loop {
            let m = self.mark();
            match self.template_parameter() {
                Ok(param) => params.push(self, param),
                Err(Fail) => self.reset(m),
            }
            self.skip_ws();
            if self.skip_string(">") {
                let requires = self.requires_clause()?;
                return self.add(Node::TemplateParams { params: params.head(), requires });
            }
            if !self.skip_string(",") {
                return Err(Fail);
            }
        }
    }

    /// `Concept{params}`; `None` when the text is not one.
    fn template_introduction(&mut self) -> PResult<Option<NodeId>> {
        let m = self.mark();
        let Ok(concept) = self.nested_name(false) else {
            self.reset(m);
            return Ok(None);
        };
        self.skip_ws();
        if !self.skip_string("{") {
            self.reset(m);
            return Ok(None);
        }
        let mut params = ListBuilder::default();
        loop {
            self.skip_ws();
            let is_pack = self.skip_string("...");
            self.skip_ws();
            if !self.match_identifier() || lex::is_keyword(self.last) {
                return Err(Fail);
            }
            let ident = self.add_identifier()?;
            let param = self.add(Node::IntroParam { ident, is_pack })?;
            params.push(self, param);
            self.skip_ws();
            if self.skip_string("}") {
                break;
            }
            if !self.skip_string(",") {
                return Err(Fail);
            }
        }
        self.add(Node::TemplateIntro { concept, params: params.head() }).map(Some)
    }

    /// `requires` and primary expressions joined by `&&` and `||`; `None` for none.
    fn requires_clause(&mut self) -> PResult<Option<NodeId>> {
        self.skip_ws();
        if !self.skip_word("requires") {
            return Ok(None);
        }
        let expr = self.joined(true)?;
        self.add(Node::Requires(expr)).map(Some)
    }

    /// Primary expressions joined by `&&`/`and` (or, for the outer level, conjunctions
    /// joined by `||`/`or`).
    fn joined(&mut self, disjunction: bool) -> PResult<NodeId> {
        let operand = |p: &mut Self| {
            if disjunction { p.joined(false) } else { p.primary_expression() }
        };
        let (symbol, word) = if disjunction { ("||", "or") } else { ("&&", "and") };
        let first = operand(self)?;
        let mut rest = ListBuilder::default();
        loop {
            self.skip_ws();
            let op = if self.skip_string(symbol) {
                symbol
            } else if self.skip_word(word) {
                word
            } else {
                break;
            };
            let expr = operand(self)?;
            let item = self.add(Node::BinOperand { op, expr })?;
            rest.push(self, item);
        }
        if rest.is_empty() {
            return Ok(first);
        }
        self.add(Node::BinOp { first, rest: rest.head() })
    }

    /// Template parameter lists and introductions; `None` for none. A member's failing
    /// first list makes the explicit instantiation marker.
    pub(crate) fn template_prefix(&mut self, what: PrefixFor) -> PResult<Option<NodeId>> {
        let mut lists = ListBuilder::default();
        let mut count = 0;
        loop {
            self.skip_ws();
            let params = if self.skip_word("template") {
                let m = self.mark();
                let Ok(params) = self.template_parameter_list() else {
                    if what != PrefixFor::Member || count > 0 {
                        return Err(Fail);
                    }
                    self.reset(m);
                    let marker = Node::TemplatePrefix { lists: None, is_instantiation: true };
                    return self.add(marker).map(Some);
                };
                let has_requires =
                    matches!(self.node(params), Node::TemplateParams { requires: Some(_), .. });
                if what == PrefixFor::Concept && has_requires {
                    return Err(Fail);
                }
                params
            } else {
                match self.template_introduction()? {
                    Some(intro) => intro,
                    None => break,
                }
            };
            if what == PrefixFor::Concept && count > 0 {
                return Err(Fail);
            }
            lists.push(self, params);
            count += 1;
        }
        if count == 0 {
            return if what == PrefixFor::Concept { Err(Fail) } else { Ok(None) };
        }
        let prefix = Node::TemplatePrefix { lists: lists.head(), is_instantiation: false };
        self.add(prefix).map(Some)
    }

    /// Prepends empty parameter lists when the name has more template argument lists
    /// than the prefix has parameter lists (Sphinx warns unless it is a full
    /// specialization shorthand or a member instantiation).
    fn check_template_consistency(
        &mut self,
        name: Option<NodeId>,
        prefix: Option<NodeId>,
        member: bool,
    ) -> PResult<Option<NodeId>> {
        let num_args = name.map_or(0, |n| self.ast.template_arg_lists(n));
        let mut num_params = 0;
        let mut existing = None;
        if let Some(prefix) = prefix
            && let Node::TemplatePrefix { lists, is_instantiation } = *self.node(prefix)
            && !(member && is_instantiation)
        {
            num_params = self.ast.count(lists);
            existing = lists;
        }
        if num_args + 1 < num_params {
            return Err(Fail);
        }
        if num_args <= num_params {
            return Ok(prefix);
        }
        let mut lists = ListBuilder::default();
        for _ in 0..num_args - num_params {
            let empty = self.add(Node::TemplateParams { params: None, requires: None })?;
            lists.push(self, empty);
        }
        if let Some(existing) = existing {
            lists.push(self, existing);
        }
        let prefix = Node::TemplatePrefix { lists: lists.head(), is_instantiation: false };
        self.add(prefix).map(Some)
    }

    /// A declaration of the object type the directive declares.
    pub(crate) fn declaration(&mut self, directive: Directive) -> PResult<NodeId> {
        let object = directive.object_type();
        self.skip_ws();
        let visibility = if self.match_visibility() { self.last } else { "" };
        let what = match object {
            ObjectType::Concept => PrefixFor::Concept,
            ObjectType::Member => PrefixFor::Member,
            _ => PrefixFor::Other,
        };
        let mut prefix = match object {
            ObjectType::Enum | ObjectType::Enumerator => None,
            _ => self.template_prefix(what)?,
        };
        let mut requires = None;
        let decl = match object {
            ObjectType::Type => self.type_declaration(prefix)?,
            ObjectType::Concept => {
                let name = self.nested_name(false)?;
                self.skip_ws();
                let init = self.initializer(Outer::Member, true)?;
                self.add(Node::Concept { name, init })?
            }
            ObjectType::Member => self.type_with_init(Named::True, Outer::Member)?,
            ObjectType::Function => {
                let decl = self.parse_type(Named::True, Outer::Function)?;
                requires = self.requires_clause()?;
                decl
            }
            ObjectType::Class => self.class_declaration()?,
            ObjectType::Union => {
                let attrs = self.attribute_list()?;
                let name = self.nested_name(false)?;
                self.add(Node::Union { name, attrs })?
            }
            ObjectType::Enum => self.enum_declaration()?,
            ObjectType::Enumerator => self.enumerator_declaration()?,
        };
        let name = self.ast.declared_name(decl);
        prefix = self.check_template_consistency(name, prefix, object == ObjectType::Member)?;
        self.skip_ws();
        let has_semicolon = self.skip_string(";");
        self.add(Node::Declaration {
            object,
            directive,
            visibility,
            prefix,
            decl,
            requires,
            has_semicolon,
        })
    }

    /// A typedef-like declaration, or a type alias.
    fn type_declaration(&mut self, prefix: Option<NodeId>) -> PResult<NodeId> {
        let m = self.mark();
        if prefix.is_none() {
            if let Ok(decl) = self.parse_type(Named::True, Outer::Type) {
                return Ok(decl);
            }
            self.reset(m);
        }
        let name = self.nested_name(false)?;
        self.skip_ws();
        let mut ty = None;
        if self.skip_string("=") {
            ty = Some(self.parse_type(Named::False, Outer::None)?);
        }
        self.add(Node::TypeUsing { name, ty })
    }

    fn class_declaration(&mut self) -> PResult<NodeId> {
        let attrs = self.attribute_list()?;
        let name = self.nested_name(false)?;
        self.skip_ws();
        let is_final = self.skip_word_ws("final");
        self.skip_ws();
        let mut bases = ListBuilder::default();
        if self.skip_string(":") {
            loop {
                self.skip_ws();
                let mut is_virtual = self.skip_word_ws("virtual");
                let mut visibility = "";
                if self.match_visibility() {
                    visibility = self.last;
                    self.skip_ws();
                }
                if !is_virtual {
                    is_virtual = self.skip_word_ws("virtual");
                }
                let name = self.nested_name(false)?;
                self.skip_ws();
                let is_pack = self.skip_string("...");
                let base = self.add(Node::BaseClass { name, visibility, is_virtual, is_pack })?;
                bases.push(self, base);
                self.skip_ws();
                if !self.skip_string(",") {
                    break;
                }
            }
        }
        self.add(Node::Class { name, bases: bases.head(), is_final, attrs })
    }

    fn enum_declaration(&mut self) -> PResult<NodeId> {
        let attrs = self.attribute_list()?;
        let name = self.nested_name(false)?;
        self.skip_ws();
        let mut underlying = None;
        if self.skip_string(":") {
            underlying = Some(self.parse_type(Named::False, Outer::None)?);
        }
        self.add(Node::Enum { name, underlying, attrs })
    }

    fn enumerator_declaration(&mut self) -> PResult<NodeId> {
        let name = self.nested_name(false)?;
        let attrs = self.attribute_list()?;
        self.skip_ws();
        let mut init = None;
        if self.skip_string("=") {
            self.skip_ws();
            let value = self.expression_fallback("", true, Parser::constant_plain)?;
            init = Some(self.add(Node::Initializer { value, has_assign: true })?);
        }
        self.add(Node::Enumerator { name, init, attrs })
    }

    /// A namespace directive's argument.
    pub(crate) fn namespace(&mut self) -> PResult<NodeId> {
        let prefix = self.template_prefix(PrefixFor::Other)?;
        let name = self.nested_name(false)?;
        let prefix = self.check_template_consistency(Some(name), prefix, false)?;
        self.add(Node::Namespace { name, prefix })
    }

    /// A cross-reference target: a (template-prefixed) name, `()` allowed after it
    /// (shorthand, returned as a `Namespace`), or else a whole function declaration.
    pub(crate) fn xref(&mut self) -> PResult<(NodeId, bool)> {
        let m = self.mark();
        if let Some(short) = self.xref_shorthand() {
            return Ok((short, true));
        }
        self.reset(m);
        let decl = self.declaration(Directive::Function)?;
        self.skip_ws();
        self.skip_string("()");
        self.skip_ws();
        if !self.is_eof() {
            return Err(Fail);
        }
        Ok((decl, false))
    }

    fn xref_shorthand(&mut self) -> Option<NodeId> {
        let prefix = self.template_prefix(PrefixFor::Other).ok()?;
        let name = self.nested_name(false).ok()?;
        self.skip_ws();
        self.skip_string("()");
        self.skip_ws();
        if !self.is_eof() {
            return None;
        }
        let prefix = self.check_template_consistency(Some(name), prefix, false).ok()?;
        self.add(Node::Namespace { name, prefix }).ok()
    }

    /// The text of an `expr` or `texpr` role: an expression, or else a type.
    pub(crate) fn expression_or_type(&mut self) -> PResult<NodeId> {
        let m = self.mark();
        if let Ok(expr) = self.expression() {
            self.skip_ws();
            if self.is_eof() {
                return Ok(expr);
            }
        }
        self.reset(m);
        let ty = self.parse_type(Named::False, Outer::None)?;
        self.skip_ws();
        if !self.is_eof() {
            return Err(Fail);
        }
        Ok(ty)
    }
}
