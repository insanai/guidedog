//! Questions about a tree that the ids, the text, and a describer ask: what a
//! declaration declares, where a declarator's name and parameters are, and what a
//! template parameter is called.

use crate::ast::{Ast, List, Node, NodeId};

impl Ast<'_> {
    /// Returns the `NestedName` a declaration (or a part of one) declares, or `None`
    /// when it is unnamed.
    pub fn declared_name(&self, id: NodeId) -> Option<NodeId> {
        match *self.node(id) {
            Node::Declaration { decl, .. } => self.declared_name(decl),
            Node::Type { decl, .. } => self.declarator_name(decl),
            Node::TypeWithInit { ty, .. } | Node::ConstrainedInit { ty, .. } => {
                self.declared_name(ty)
            }
            Node::TypeUsing { name, .. }
            | Node::Concept { name, .. }
            | Node::Class { name, .. }
            | Node::Union { name, .. }
            | Node::Enum { name, .. }
            | Node::Enumerator { name, .. }
            | Node::Namespace { name, .. } => Some(name),
            _ => None,
        }
    }

    /// Follows a declarator to its declarator-id (`NestedName`).
    pub fn declarator_name(&self, d: NodeId) -> Option<NodeId> {
        let mut at = d;
        loop {
            match *self.node(at) {
                Node::DName { name, .. } | Node::DBitfield { name, .. } => return name,
                Node::DPtr { next, .. } | Node::DRef { next, .. } | Node::DPack(next) => at = next,
                Node::DMemPtr { next, .. } => at = next,
                Node::DParen { inner, .. } => at = inner,
                _ => return None,
            }
        }
    }

    /// Returns a function declarator's `ParamQual`, following pointers and parentheses
    /// to the declarator-id.
    pub fn function_params(&self, d: NodeId) -> Option<NodeId> {
        let mut at = d;
        loop {
            match *self.node(at) {
                Node::DName { params, .. } => return params,
                Node::DPtr { next, .. } | Node::DRef { next, .. } | Node::DPack(next) => at = next,
                Node::DParen { inner, .. } => at = inner,
                Node::DMemPtr { next, .. } => at = next,
                _ => return None,
            }
        }
    }

    /// Tells whether a declarator has parameters (declares a function type).
    pub fn is_function_type(&self, d: NodeId) -> bool {
        match *self.node(d) {
            Node::DName { params, .. } => params.is_some(),
            Node::DPtr { next, .. } | Node::DRef { next, .. } | Node::DPack(next) => {
                self.is_function_type(next)
            }
            Node::DMemPtr { next, .. } => self.is_function_type(next),
            Node::DParen { inner, .. } => self.is_function_type(inner),
            _ => false,
        }
    }

    /// Returns a function declarator's trailing return `Type`.
    pub fn trailing_return(&self, d: NodeId) -> Option<NodeId> {
        let params = self.function_params(d)?;
        match *self.node(params) {
            Node::ParamQual(q) => q.trailing_return,
            _ => None,
        }
    }

    /// Counts a name's elements with template arguments. (Sphinx means to skip
    /// operators, but its name elements never say they are one.)
    pub fn template_arg_lists(&self, name: NodeId) -> usize {
        let Node::NestedName { elements, .. } = *self.node(name) else { return 0 };
        self.list(elements)
            .filter(|&e| matches!(self.node(e), Node::NameElement { args: Some(_), .. }))
            .count()
    }

    /// Returns the template parameter lists of a declaration's prefix (empty for an
    /// explicit instantiation or no prefix).
    pub fn template_lists(&self, decl: NodeId) -> List {
        let prefix = match *self.node(decl) {
            Node::Declaration { prefix, .. } | Node::Namespace { prefix, .. } => prefix?,
            _ => return None,
        };
        match *self.node(prefix) {
            Node::TemplatePrefix { lists, is_instantiation: false } => lists,
            _ => None,
        }
    }

    /// Returns the parameters of a `TemplateParams` or `TemplateIntro`.
    pub fn template_params(&self, list: NodeId) -> List {
        match *self.node(list) {
            Node::TemplateParams { params, .. } | Node::TemplateIntro { params, .. } => params,
            _ => None,
        }
    }

    /// Returns a template parameter's `Identifier`.
    pub fn param_identifier(&self, param: NodeId) -> Option<NodeId> {
        match *self.node(param) {
            Node::TpType(key) | Node::TpTemplate { key, .. } => match *self.node(key) {
                Node::TpKey { ident, .. } => ident,
                _ => None,
            },
            Node::TpNonType { param, .. } => {
                let name = self.declared_name(param)?;
                let Node::NestedName { elements, .. } = *self.node(name) else { return None };
                match *self.node(elements?) {
                    Node::NameElement { name, .. } => Some(name),
                    _ => None,
                }
            }
            Node::IntroParam { ident, .. } => Some(ident),
            _ => None,
        }
    }

    /// Returns `str(param.name)`: a template parameter's identifier, or `None` without
    /// one (Sphinx's `"None"`).
    pub fn param_name(&self, param: NodeId) -> Option<&str> {
        match *self.node(self.param_identifier(param)?) {
            Node::Identifier(text) => Some(text),
            _ => None,
        }
    }

    /// Tells whether a template parameter is a pack.
    pub fn param_is_pack(&self, param: NodeId) -> bool {
        match *self.node(param) {
            Node::TpType(key) | Node::TpTemplate { key, .. } => {
                matches!(self.node(key), Node::TpKey { is_pack: true, .. })
            }
            Node::TpNonType { param, is_pack } => {
                is_pack
                    || match *self.node(param) {
                        Node::TypeWithInit { ty, .. } | Node::ConstrainedInit { ty, .. } => {
                            match *self.node(ty) {
                                Node::Type { decl, .. } => self.declarator_is_pack(decl),
                                _ => false,
                            }
                        }
                        _ => false,
                    }
            }
            Node::IntroParam { is_pack, .. } => is_pack,
            _ => false,
        }
    }

    fn declarator_is_pack(&self, d: NodeId) -> bool {
        match *self.node(d) {
            Node::DPack(_) => true,
            Node::DPtr { next, .. } | Node::DRef { next, .. } => self.declarator_is_pack(next),
            Node::DMemPtr { next, .. } => self.declarator_is_pack(next),
            Node::DParen { inner, next } => {
                self.declarator_is_pack(inner) || self.declarator_is_pack(next)
            }
            _ => false,
        }
    }

    /// Returns the cv-qualifiers of declaration specifiers, before and after the type
    /// merged: `(is_const, is_volatile)`.
    pub fn specs_cv(&self, specs: NodeId) -> (bool, bool) {
        let Node::DeclSpecs { left, right, .. } = *self.node(specs) else { return (false, false) };
        let mut cv = (false, false);
        for side in [Some(left), right].into_iter().flatten() {
            if let Node::SpecsSimple(s) = self.node(side) {
                cv.0 |= s.is_const;
                cv.1 |= s.is_volatile;
            }
        }
        cv
    }

    /// Tells whether declaration specifiers are `constexpr` (before or after the type).
    pub fn specs_constexpr(&self, specs: NodeId) -> bool {
        let Node::DeclSpecs { left, right, .. } = *self.node(specs) else { return false };
        [Some(left), right]
            .into_iter()
            .flatten()
            .any(|side| matches!(self.node(side), Node::SpecsSimple(s) if s.is_constexpr))
    }

    /// Sphinx's `require_space_after_declSpecs`: a declarator with a name, or one that
    /// is a pointer to member or parenthesized, is separated from the specifiers.
    pub fn space_after_specs(&self, d: NodeId) -> bool {
        match *self.node(d) {
            Node::DName { name, .. } | Node::DBitfield { name, .. } => name.is_some(),
            Node::DPtr { next, .. } | Node::DRef { next, .. } => self.space_after_specs(next),
            Node::DPack(_) => false,
            _ => true,
        }
    }

    /// Tells whether the arguments do not name the parameters exactly, in order, packs
    /// expanded (Sphinx's `_is_specialization`).
    pub fn is_specialization(&self, params: NodeId, args: NodeId) -> bool {
        if !matches!(self.node(params), Node::TemplateParams { .. } | Node::TemplateIntro { .. }) {
            return true;
        }
        let Node::TemplateArgs { args, .. } = *self.node(args) else { return true };
        let first = self.template_params(params);
        let n = self.count(first);
        if n != self.count(args) || n == 0 {
            return true;
        }
        let mut text = String::new();
        for (param, arg) in self.list(first).zip(self.list(args)) {
            text.clear();
            self.write_text(arg, false, &mut text);
            let pack = text.ends_with("...");
            if self.param_is_pack(param) != pack {
                return true;
            }
            let shown = if pack { &text[..text.len() - 3] } else { &text[..] };
            if self.param_name(param).unwrap_or("None") != shown {
                return true;
            }
        }
        false
    }
}
