//! The ids of declarators, template parameters, and declarations.

use crate::ast::{Node, NodeId, ObjectType};
use crate::ids::{Done, ID_PREFIX, Ids, SHORTHANDS_V1, lookup};

/// Writes `V` and the const mark after a pointer, as Sphinx does in every version.
fn ptr_cv(is_const: bool, is_volatile: bool, out: &mut String) {
    if is_volatile {
        out.push('V');
    }
    if is_const {
        out.push('C');
    }
}

impl Ids<'_, '_> {
    /// A function's cv- and ref-qualifiers.
    pub fn modifiers_id(&self, d: NodeId, out: &mut String) -> Done {
        match *self.ast.node(d) {
            Node::DName { params, .. } => {
                let Node::ParamQual(q) = *self.ast.node(params?) else { return None };
                if q.is_volatile {
                    out.push('V');
                }
                if q.is_const {
                    out.push(if self.version == 1 { 'C' } else { 'K' });
                }
                match q.ref_qualifier {
                    "&&" => out.push('O'),
                    "&" => out.push('R'),
                    _ => {}
                }
                Some(())
            }
            Node::DPtr { next, .. } | Node::DRef { next, .. } | Node::DPack(next) => {
                self.modifiers_id(next, out)
            }
            Node::DMemPtr { next, .. } => {
                if self.version == 1 {
                    return None;
                }
                self.modifiers_id(next, out)
            }
            Node::DParen { inner, .. } => self.modifiers_id(inner, out),
            _ => None,
        }
    }

    /// The parameters' ids: nothing without parameters; `v` for none in versions after
    /// 1; `__` and the ids joined by `.` in version 1.
    pub fn param_id(&self, d: NodeId, out: &mut String) -> Done {
        match *self.ast.node(d) {
            Node::DName { params: Some(params), .. } => {
                let Node::ParamQual(q) = *self.ast.node(params) else { return None };
                if self.version == 1 {
                    if q.params.is_none() {
                        return Some(());
                    }
                    out.push_str("__");
                    for (i, a) in self.ast.list(q.params).enumerate() {
                        if i > 0 {
                            out.push('.');
                        }
                        self.id(a, out)?;
                    }
                    return Some(());
                }
                if q.params.is_none() {
                    out.push('v');
                }
                for a in self.ast.list(q.params) {
                    self.id(a, out)?;
                }
                Some(())
            }
            Node::DPtr { next, .. } | Node::DRef { next, .. } | Node::DPack(next) => {
                self.param_id(next, out)
            }
            Node::DMemPtr { next, .. } => {
                if self.version == 1 {
                    return None;
                }
                self.param_id(next, out)
            }
            Node::DParen { inner, .. } => self.param_id(inner, out),
            _ => Some(()),
        }
    }

    /// The array, pointer, reference, and pack suffixes.
    pub fn ptr_suffix_id(&self, d: NodeId, out: &mut String) -> Done {
        let v1 = self.version == 1;
        match *self.ast.node(d) {
            Node::DName { arrays, .. } => {
                for a in self.ast.list(arrays) {
                    self.array_id(a, out)?;
                }
            }
            Node::DPtr { next, is_const, is_volatile, .. } => {
                if v1 {
                    out.push('P');
                    ptr_cv(is_const, is_volatile, out);
                    return self.ptr_suffix_id(next, out);
                }
                self.ptr_suffix_id(next, out)?;
                out.push('P');
                ptr_cv(is_const, is_volatile, out);
            }
            Node::DRef { next, .. } | Node::DPack(next) => {
                let mark = if matches!(self.ast.node(d), Node::DRef { .. }) { "R" } else { "Dp" };
                if v1 {
                    out.push_str(mark);
                    return self.ptr_suffix_id(next, out);
                }
                self.ptr_suffix_id(next, out)?;
                out.push_str(mark);
            }
            Node::DMemPtr { next, .. } => {
                if v1 {
                    return None;
                }
                self.ptr_suffix_id(next, out)?;
                out.push_str("Dp");
            }
            Node::DParen { inner, next } => {
                if v1 {
                    return None;
                }
                self.ptr_suffix_id(inner, out)?;
                self.ptr_suffix_id(next, out)?;
            }
            _ => {}
        }
        Some(())
    }

    fn array_id(&self, a: NodeId, out: &mut String) -> Done {
        let Node::Array(size) = *self.ast.node(a) else { return None };
        out.push('A');
        match self.version {
            1 => return Some(()),
            2 => self.ast.write_opt(size, false, out),
            _ => {
                if let Some(size) = size {
                    self.id(size, out)?;
                }
            }
        }
        out.push('_');
        Some(())
    }

    /// The type of a declarator whose innermost return type has id `ret`.
    pub fn type_id(&self, d: NodeId, ret: &str, out: &mut String) -> Done {
        match *self.ast.node(d) {
            Node::DName { params, .. } => {
                self.ptr_suffix_id(d, out)?;
                if params.is_none() {
                    out.push_str(ret);
                    return Some(());
                }
                self.modifiers_id(d, out)?;
                out.push('F');
                out.push_str(ret);
                self.param_id(d, out)?;
                out.push('E');
            }
            Node::DBitfield { .. } => out.push_str(ret),
            Node::DPtr { next, is_const, is_volatile, .. } => {
                let mut inner = String::from("P");
                ptr_cv(is_const, is_volatile, &mut inner);
                inner.push_str(ret);
                return self.type_id(next, &inner, out);
            }
            Node::DRef { next, .. } | Node::DPack(next) => {
                let mark = if matches!(self.ast.node(d), Node::DRef { .. }) { "R" } else { "Dp" };
                return self.type_id(next, &format!("{mark}{ret}"), out);
            }
            Node::DMemPtr { class, next, is_const, is_volatile } => {
                let mut inner = String::new();
                if is_volatile {
                    inner.push('V');
                }
                if is_const {
                    inner.push('K');
                }
                inner.push('M');
                self.id(class, &mut inner)?;
                inner.push_str(ret);
                return self.type_id(next, &inner, out);
            }
            Node::DParen { inner, next } => {
                let mut wrapped = String::new();
                self.type_id(next, ret, &mut wrapped)?;
                return self.type_id(inner, &wrapped, out);
            }
            _ => {}
        }
        Some(())
    }

    /// A template parameter's id within a parameter list (not a symbol's).
    fn template_param_id(&self, param: NodeId, out: &mut String) -> Done {
        match *self.ast.node(param) {
            Node::TpKey { is_pack, .. } | Node::IntroParam { is_pack, .. } => {
                out.push_str(if is_pack { "Dp" } else { "0" });
            }
            Node::TpType(key) => self.template_param_id(key, out)?,
            Node::TpTemplate { params, key } => {
                self.template_params_id(params, false, out)?;
                self.template_param_id(key, out)?;
            }
            Node::TpNonType { param, is_pack } => {
                out.push('_');
                if is_pack {
                    out.push_str("Dp");
                }
                self.id(param, out)?;
            }
            _ => {}
        }
        Some(())
    }

    /// `I` parameters `E`, and a requires clause unless excluded.
    fn template_params_id(&self, list: NodeId, exclude_requires: bool, out: &mut String) -> Done {
        match *self.ast.node(list) {
            Node::TemplateIntro { concept, params } => self.intro_id(concept, params, out),
            Node::TemplateParams { params, requires } => {
                out.push('I');
                for t in self.ast.list(params) {
                    self.template_param_id(t, out)?;
                }
                out.push('E');
                if let (false, Some(requires)) = (exclude_requires, requires) {
                    out.push_str("IQ");
                    self.requires_id(requires, out)?;
                    out.push('E');
                }
                Some(())
            }
            _ => None,
        }
    }

    fn requires_id(&self, requires: NodeId, out: &mut String) -> Done {
        let Node::Requires(expr) = *self.ast.node(requires) else { return None };
        self.id(expr, out)
    }

    fn intro_id(&self, concept: NodeId, params: Option<NodeId>, out: &mut String) -> Done {
        out.push('I');
        for t in self.ast.list(params) {
            self.template_param_id(t, out)?;
        }
        out.push_str("EX");
        self.id(concept, out)?;
        out.push('I');
        for t in self.ast.list(params) {
            let Node::IntroParam { ident, is_pack } = *self.ast.node(t) else { return None };
            if is_pack {
                out.push_str("sp");
            }
            self.id(ident, out)?;
        }
        out.push_str("EE");
        Some(())
    }

    /// Writes a declaration's id of the version, prefixed with its `_CPPvN` or not, from
    /// `elements`, the declared symbol's full nested name.
    pub fn declaration_id(&self, decl: NodeId, prefixed: bool, out: &mut String) -> Done {
        let Node::Declaration { prefix, requires, .. } = *self.ast.node(decl) else {
            return None;
        };
        if self.version == 1 {
            if prefix.is_some() || requires.is_some() {
                return None;
            }
            return self.declared_id(decl, out);
        }
        let lists = match prefix.map(|p| self.ast.node(p)) {
            // An explicit instantiation's prefix has no lists (where Sphinx itself fails).
            Some(Node::TemplatePrefix { is_instantiation: true, .. }) => return None,
            Some(Node::TemplatePrefix { lists, .. }) => *lists,
            _ => None,
        };
        if prefixed {
            out.push_str(ID_PREFIX[self.version]);
        }
        let last = self.ast.last(lists);
        for t in self.ast.list(lists) {
            self.template_params_id(t, Some(t) == last, out)?;
        }
        let last_requires = match last.map(|l| self.ast.node(l)) {
            Some(Node::TemplateParams { requires, .. }) => *requires,
            _ => None,
        };
        if last_requires.is_some() || requires.is_some() {
            if self.version < 4 {
                return None;
            }
            out.push_str("IQ");
            if last_requires.is_some() && requires.is_some() {
                out.push_str("aa");
            }
            for clause in [last_requires, requires].into_iter().flatten() {
                self.requires_id(clause, out)?;
            }
            out.push('E');
        }
        self.declared_id(decl, out)
    }

    /// The id of what is declared, from its full nested name.
    fn declared_id(&self, decl: NodeId, out: &mut String) -> Done {
        let Node::Declaration { object, decl: what, .. } = *self.ast.node(decl) else {
            return None;
        };
        let v1 = self.version == 1;
        match object {
            ObjectType::Function => self.function_id(decl, out),
            ObjectType::Type => {
                if v1 && matches!(self.ast.node(what), Node::TypeUsing { .. }) {
                    return None;
                }
                self.full_id("", out)
            }
            ObjectType::Member => {
                self.full_id("", out)?;
                if v1 {
                    let Node::TypeWithInit { ty, .. } = *self.ast.node(what) else { return None };
                    out.push_str("__");
                    self.id(ty, out)?;
                }
                Some(())
            }
            ObjectType::Class => self.full_id("", out),
            ObjectType::Concept
            | ObjectType::Union
            | ObjectType::Enum
            | ObjectType::Enumerator => {
                if v1 {
                    return None;
                }
                self.full_id("", out)
            }
        }
    }

    fn function_id(&self, decl: NodeId, out: &mut String) -> Done {
        let Node::Declaration { prefix, decl: ty, .. } = *self.ast.node(decl) else { return None };
        let Node::Type { specs, decl: d } = *self.ast.node(ty) else { return None };
        if self.version == 1 {
            self.full_id("", out)?;
            self.param_id(d, out)?;
            self.modifiers_id(d, out)?;
            if self.ast.specs_constexpr(specs) {
                out.push_str("CE");
            }
            return Some(());
        }
        let mut modifiers = String::new();
        self.modifiers_id(d, &mut modifiers)?;
        self.full_id(&modifiers, out)?;
        if self.version >= 4 && prefix.is_some() {
            self.ptr_suffix_id(d, out)?;
            self.id(self.ast.trailing_return(d).unwrap_or(specs), out)?;
        }
        self.param_id(d, out)
    }

    /// Writes the id of the full nested name in `elements`: version 1 joins the
    /// elements' ids with `::` (or uses a shorthand); later versions wrap several
    /// elements, or modifiers, in `N...E`.
    pub fn full_id(&self, modifiers: &str, out: &mut String) -> Done {
        if self.placeholder {
            // The host knows the symbol the name ends up in.
            out.push('\x1d');
            out.push_str(modifiers);
            out.push('\x1d');
            return Some(());
        }
        let v = self.version;
        if v == 1 {
            let mut full = String::new();
            for (i, e) in self.elements.iter().enumerate() {
                if i > 0 {
                    full.push_str("::");
                }
                full.push_str(&e.ident);
                full.push_str(&e.args);
            }
            if let Some(short) = lookup(&SHORTHANDS_V1, &full) {
                out.push_str(short);
                return Some(());
            }
            for (i, e) in self.elements.iter().enumerate() {
                if i > 0 {
                    out.push_str("::");
                }
                out.push_str(e.id(1)?);
            }
            return Some(());
        }
        let wrap = self.elements.len() > 1 || !modifiers.is_empty();
        if wrap {
            out.push('N');
        }
        out.push_str(modifiers);
        for e in self.elements {
            out.push_str(e.id(v)?);
        }
        if wrap {
            out.push('E');
        }
        Some(())
    }
}
