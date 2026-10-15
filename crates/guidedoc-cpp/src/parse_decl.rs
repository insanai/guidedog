//! Names, types, and declarators, as `DefinitionParser` parses them.

use std::borrow::Cow;

use crate::ast::{List, Node, NodeId, ParamQual, SimpleSpecs};
use crate::lex;
use crate::parser::{Fail, ListBuilder, PResult, Parser};

/// `_parse_type`'s and `_parse_declarator`'s `named`: False, 'maybe', 'single', or True.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Named {
    False,
    Maybe,
    Single,
    True,
}

/// The `outer` of `_parse_type` and `_parse_decl_specs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outer {
    None,
    Type,
    Member,
    Function,
    OperatorCast,
    TemplateParam,
}

/// What may follow a declarator-id: `_parse_declarator`'s `paramMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParamMode {
    Type,
    Function,
    OperatorCast,
    New,
}

/// The fundamental type words read so far.
#[derive(Debug, Clone, Copy, Default)]
struct Fundamental {
    modifier: &'static str,
    signedness: &'static str,
    width: [&'static str; 2],
    widths: usize,
    ty: &'static str,
}

impl Fundamental {
    /// Adds a word; fails on a combination Sphinx refuses as it reads.
    fn add(&mut self, word: &'static str) -> PResult<()> {
        match word {
            "signed" | "unsigned" => {
                if !self.signedness.is_empty() {
                    return Err(Fail);
                }
                self.signedness = word;
            }
            "short" => {
                if self.widths != 0 {
                    return Err(Fail);
                }
                self.width[0] = word;
                self.widths = 1;
            }
            "long" => {
                if self.widths >= 2 || (self.widths == 1 && self.width[0] != "long") {
                    return Err(Fail);
                }
                self.width[self.widths] = word;
                self.widths += 1;
            }
            "_Imaginary" | "_Complex" => {
                if !self.modifier.is_empty() {
                    return Err(Fail);
                }
                self.modifier = word;
            }
            _ => {
                if !self.ty.is_empty() {
                    return Err(Fail);
                }
                self.ty = word;
            }
        }
        Ok(())
    }

    /// Tells whether the words make a type Sphinx accepts.
    fn is_valid(&self) -> bool {
        let plain = self.modifier.is_empty();
        let unsigned = self.signedness.is_empty();
        let no_width = self.widths == 0;
        match self.ty {
            "auto" | "void" | "bool" | "wchar_t" | "char8_t" | "char16_t" | "char32_t"
            | "__float80" | "_Float64x" | "__float128" | "_Float128" => {
                plain && unsigned && no_width
            }
            "char" => plain && no_width,
            "int" => plain,
            "__int64" | "__int128" => plain && no_width,
            "float" => unsigned && no_width,
            "double" => unsigned && (no_width || (self.widths == 1 && self.width[0] == "long")),
            "" => plain,
            _ => false,
        }
    }

    /// The words in canonical order: modifier, signedness, width, and type.
    fn canonical(&self) -> String {
        let words = [self.modifier, self.signedness, self.width[0], self.width[1], self.ty];
        let used = [true, true, self.widths > 0, self.widths > 1, true];
        let mut out = String::new();
        for (word, _) in words.iter().zip(used).filter(|(w, u)| *u && !w.is_empty()) {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(word);
        }
        out
    }
}

/// Joins the words as Sphinx's `' '.join(names)` does.
fn collapse_spaces(s: &str) -> Cow<'_, str> {
    let b = s.as_bytes();
    let single = b.iter().enumerate().all(|(i, &c)| {
        !lex::is_space(c) || (c == b' ' && !b.get(i + 1).is_some_and(|&n| lex::is_space(n)))
    });
    if single {
        return Cow::Borrowed(s);
    }
    let words: Vec<&str> = s
        .split(|c: char| c.is_ascii() && lex::is_space(c as u8))
        .filter(|w| !w.is_empty())
        .collect();
    Cow::Owned(words.join(" "))
}

/// Matches a balanced token sequence up to `end`, as `_parse_balanced_token_seq`.
fn balanced(p: &mut Parser<'_>, end: u8) -> bool {
    let mut stack = [0u8; 64];
    let mut depth = 0usize;
    while !p.is_eof() {
        let c = p.current();
        if depth == 0 && c == end {
            return true;
        }
        match c {
            b'(' | b'[' | b'{' => {
                if depth < stack.len() {
                    stack[depth] = match c {
                        b'(' => b')',
                        b'[' => b']',
                        _ => b'}',
                    };
                }
                depth += 1;
            }
            b')' | b']' | b'}' => {
                if depth == 0 || (depth <= stack.len() && stack[depth - 1] != c) {
                    return false;
                }
                depth -= 1;
            }
            _ => {}
        }
        p.pos += 1;
    }
    false
}

impl<'a> Parser<'a> {
    /// What follows `operator`: a built-in operator, `new` or `delete` (with `[]`), a
    /// user-defined literal suffix, or a conversion type.
    fn operator(&mut self) -> PResult<NodeId> {
        self.skip_ws();
        if let Some(e) = lex::operator(self.bytes(), self.pos) {
            let text = self.take_to(e);
            return self.add(Node::OpBuiltin(text));
        }
        for (op, array) in [("new", "new[]"), ("delete", "delete[]")] {
            if !self.skip_word(op) {
                continue;
            }
            self.skip_ws();
            let mut text = op;
            if self.skip_string("[") {
                self.skip_ws();
                if !self.skip_string("]") {
                    return Err(Fail);
                }
                text = array;
            }
            return self.add(Node::OpBuiltin(text));
        }
        if self.skip_string("\"\"") {
            self.skip_ws();
            if !self.match_identifier() {
                return Err(Fail);
            }
            let ident = self.add_identifier()?;
            return self.add(Node::OpLiteral(ident));
        }
        let ty = self.parse_type(Named::False, Outer::OperatorCast)?;
        self.add(Node::OpType(ty))
    }

    /// `<` arguments `>`, each a type or a constant expression; `None` without `<`.
    fn template_argument_list(&mut self) -> PResult<Option<NodeId>> {
        self.skip_ws();
        if !self.skip_string_ws("<") {
            return Ok(None);
        }
        let mut args = ListBuilder::default();
        let mut is_pack = false;
        if !self.skip_string(">") {
            loop {
                let m = self.mark();
                let (arg, end, pack) = match self.template_argument(false) {
                    Ok(read) => read,
                    Err(Fail) => {
                        self.reset(m);
                        self.template_argument(true)?
                    }
                };
                args.push(self, arg);
                is_pack |= pack;
                if end {
                    break;
                }
            }
        }
        self.add(Node::TemplateArgs { args: args.head(), is_pack }).map(Some)
    }

    /// One argument, as a type or as a constant expression, and what follows it:
    /// whether it ends the list and whether `...` expands it.
    fn template_argument(&mut self, constant: bool) -> PResult<(NodeId, bool, bool)> {
        let arg = if constant {
            let value = self.constant_expression(true)?;
            self.add(Node::ArgConstant(value))?
        } else {
            self.parse_type(Named::False, Outer::None)?
        };
        self.skip_ws();
        if self.skip_string_ws("...") {
            return if self.skip_string(">") { Ok((arg, true, true)) } else { Err(Fail) };
        }
        if self.skip_string(">") {
            return Ok((arg, true, false));
        }
        if self.skip_string(",") {
            return Ok((arg, false, false));
        }
        Err(Fail)
    }

    /// `::`? then names (identifiers or operators, each with template arguments when
    /// they parse) joined by `::`; a member pointer's ends before `*`.
    pub(crate) fn nested_name(&mut self, member_pointer: bool) -> PResult<NodeId> {
        self.guarded(|p| p.nested_name_inner(member_pointer))
    }

    fn nested_name_inner(&mut self, member_pointer: bool) -> PResult<NodeId> {
        self.skip_ws();
        let is_rooted = self.skip_string("::");
        let mut elements = ListBuilder::default();
        loop {
            self.skip_ws();
            let has_template_keyword = !elements.is_empty() && self.skip_word_ws("template");
            let name = if self.skip_word_ws("operator") {
                self.operator()?
            } else {
                if !self.match_identifier() {
                    if member_pointer && !elements.is_empty() {
                        break;
                    }
                    return Err(Fail);
                }
                if lex::is_keyword(self.last) {
                    return Err(Fail);
                }
                self.add_identifier()?
            };
            let m = self.mark();
            let args = match self.template_argument_list() {
                Ok(args) => args,
                Err(Fail) => {
                    self.reset(m);
                    None
                }
            };
            let element = self.add(Node::NameElement { name, args, has_template_keyword })?;
            elements.push(self, element);
            self.skip_ws();
            if !self.skip_string("::") {
                if member_pointer {
                    return Err(Fail);
                }
                break;
            }
        }
        self.add(Node::NestedName { is_rooted, elements: elements.head() })
    }

    /// Fundamental type words; `None` when there are none.
    fn simple_type_specifiers(&mut self) -> PResult<Option<NodeId>> {
        let mut words = Fundamental::default();
        self.skip_ws();
        let start = self.pos;
        let mut written = self.pos;
        while let Some(word) = self.match_simple_type() {
            written = self.pos;
            words.add(word)?;
            self.skip_ws();
        }
        if written == start {
            return Ok(None);
        }
        if !words.is_valid() {
            return Err(Fail);
        }
        let text = self.def.get(start..written).unwrap_or("");
        let canonical = words.canonical();
        let canonical =
            if canonical == text { Cow::Borrowed(text) } else { Cow::Owned(canonical) };
        let written = collapse_spaces(text);
        self.add(Node::Fundamental { written, canonical }).map(Some)
    }

    /// Fundamental types, `decltype`, or a (prefixed) nested name with an optional
    /// placeholder.
    fn trailing_type_spec(&mut self) -> PResult<NodeId> {
        self.skip_ws();
        if let Some(res) = self.simple_type_specifiers()? {
            return Ok(res);
        }
        self.skip_ws();
        if self.skip_word_ws("decltype") {
            if !self.skip_string_ws("(") {
                return Err(Fail);
            }
            if self.skip_word_ws("auto") {
                if !self.skip_string(")") {
                    return Err(Fail);
                }
                return self.add(Node::DecltypeAuto);
            }
            let expr = self.expression()?;
            self.skip_ws();
            if !self.skip_string(")") {
                return Err(Fail);
            }
            return self.add(Node::Decltype(expr));
        }
        self.skip_ws();
        let keys = ["class", "struct", "enum", "union", "typename"];
        let key = keys.into_iter().find(|&k| self.skip_word_ws(k)).unwrap_or("");
        let name = self.nested_name(false)?;
        self.skip_ws();
        let mut placeholder = "";
        if self.skip_word("auto") {
            placeholder = "auto";
        } else if self.skip_word_ws("decltype") {
            if !self.skip_string_ws("(") || !self.skip_word_ws("auto") || !self.skip_string_ws(")")
            {
                return Err(Fail);
            }
            placeholder = "decltype(auto)";
        }
        self.add(Node::TypeName { key, name, placeholder })
    }

    /// `(` parameters `)` and the qualifiers, exception specification, trailing return
    /// type, `override` and `final`, attributes, and (for a function) `= 0`,
    /// `= delete`, or `= default`; `None` without `(` unless it is required.
    fn parameters_and_qualifiers(&mut self, mode: ParamMode) -> PResult<Option<NodeId>> {
        if mode == ParamMode::New {
            return Ok(None);
        }
        self.skip_ws();
        if !self.skip_string("(") {
            return if mode == ParamMode::Function { Err(Fail) } else { Ok(None) };
        }
        let mut q = ParamQual { params: self.parameter_list()?, ..ParamQual::default() };
        self.skip_ws();
        q.is_const = self.skip_word_ws("const");
        q.is_volatile = self.skip_word_ws("volatile");
        if !q.is_const {
            q.is_const = self.skip_word_ws("const");
        }
        if self.skip_string("&&") {
            q.ref_qualifier = "&&";
        } else if self.skip_string("&") {
            q.ref_qualifier = "&";
        }
        self.skip_ws();
        if self.skip_string("noexcept") {
            let mut condition = None;
            if self.skip_string_ws("(") {
                condition = Some(self.constant_expression(false)?);
                self.skip_ws();
                if !self.skip_string(")") {
                    return Err(Fail);
                }
            }
            q.noexcept = Some(self.add(Node::NoexceptSpec(condition))?);
        }
        self.skip_ws();
        if self.skip_string("->") {
            q.trailing_return = Some(self.parse_type(Named::False, Outer::None)?);
        }
        self.skip_ws();
        q.is_override = self.skip_word_ws("override");
        q.is_final = self.skip_word_ws("final");
        if !q.is_override {
            q.is_override = self.skip_word_ws("override");
        }
        q.attrs = self.attribute_list()?;
        self.skip_ws();
        if mode == ParamMode::Function && self.skip_string("=") {
            self.skip_ws();
            let inits = ["0", "delete", "default"];
            q.initializer = inits.into_iter().find(|&w| self.skip_word_ws(w)).ok_or(Fail)?;
        }
        self.add(Node::ParamQual(q)).map(Some)
    }

    /// The parameters up to and with `)`.
    fn parameter_list(&mut self) -> PResult<List> {
        let mut params = ListBuilder::default();
        self.skip_ws();
        if self.skip_string(")") {
            return Ok(None);
        }
        loop {
            self.skip_ws();
            if self.skip_string("...") {
                let param = self.add(Node::FunctionParam(None))?;
                params.push(self, param);
                self.skip_ws();
                return if self.skip_string(")") { Ok(params.head()) } else { Err(Fail) };
            }
            let arg = self.type_with_init(Named::Single, Outer::None)?;
            let param = self.add(Node::FunctionParam(Some(arg)))?;
            params.push(self, param);
            self.skip_ws();
            if self.skip_string(",") {
                continue;
            }
            return if self.skip_string(")") { Ok(params.head()) } else { Err(Fail) };
        }
    }

    /// Storage classes, function specifiers, cv-qualifiers, and attributes, in any
    /// order, as far as the outer kind allows them.
    fn decl_specs_simple(&mut self, outer: Outer, typed: bool) -> PResult<NodeId> {
        let mut s = SimpleSpecs::default();
        let mut attrs = ListBuilder::default();
        let member = outer == Outer::Member;
        let function = outer == Outer::Function;
        loop {
            self.skip_ws();
            if typed && !s.is_const && self.skip_word("const") {
                s.is_const = true;
                continue;
            }
            if typed && !s.is_volatile && self.skip_word("volatile") {
                s.is_volatile = true;
                continue;
            }
            if s.storage.is_empty() && self.storage_class(member, function, &mut s) {
                continue;
            }
            if self.function_specifier(member, function, &mut s) {
                continue;
            }
            if function && s.explicit.is_none() && self.skip_word_ws("explicit") {
                s.explicit = Some(self.explicit_spec()?);
                continue;
            }
            let Some(attr) = self.attribute()? else { break };
            attrs.push(self, attr);
        }
        s.attrs = attrs.head();
        self.add(Node::SpecsSimple(s))
    }

    /// Reads a storage class the outer kind allows.
    fn storage_class(&mut self, member: bool, function: bool, s: &mut SimpleSpecs<'a>) -> bool {
        if (member || function) && (self.skip_word("static") || self.skip_word("extern")) {
            s.storage = self.last;
            return true;
        }
        if member && self.skip_word("mutable") {
            s.storage = "mutable";
            return true;
        }
        if self.skip_word("register") {
            s.storage = "register";
            return true;
        }
        false
    }

    /// Reads a function specifier (or `constinit`, `thread_local`) the outer kind
    /// allows, once each.
    fn function_specifier(
        &mut self,
        member: bool,
        function: bool,
        s: &mut SimpleSpecs<'a>,
    ) -> bool {
        let either = member || function;
        let flags: [(bool, &mut bool, &'static str); 7] = [
            (either, &mut s.is_inline, "inline"),
            (either, &mut s.is_constexpr, "constexpr"),
            (member, &mut s.is_constinit, "constinit"),
            (member, &mut s.is_thread_local, "thread_local"),
            (function, &mut s.is_consteval, "consteval"),
            (function, &mut s.is_friend, "friend"),
            (function, &mut s.is_virtual, "virtual"),
        ];
        for (allowed, flag, word) in flags {
            if allowed && !*flag && self.skip_word(word) {
                *flag = true;
                return true;
            }
        }
        false
    }

    fn explicit_spec(&mut self) -> PResult<NodeId> {
        let mut condition = None;
        if self.skip_string("(") {
            condition = Some(self.constant_expression(false)?);
            self.skip_ws();
            if !self.skip_string(")") {
                return Err(Fail);
            }
        }
        self.add(Node::ExplicitSpec(condition))
    }

    /// Simple specifiers, the trailing type specifier, and simple specifiers after it.
    pub(crate) fn decl_specs(&mut self, outer: Outer, typed: bool) -> PResult<NodeId> {
        let left = self.decl_specs_simple(outer, typed)?;
        let (mut right, mut trailing) = (None, None);
        if typed {
            trailing = Some(self.trailing_type_spec()?);
            right = Some(self.decl_specs_simple(outer, typed)?);
        }
        self.add(Node::DeclSpecs { left, right, trailing })
    }

    /// The declarator-id (as `named` asks), array operators, and parameters; or a
    /// bit-field's size.
    fn declarator_name_suffix(
        &mut self,
        named: Named,
        mode: ParamMode,
        typed: bool,
    ) -> PResult<NodeId> {
        let mut name = None;
        match named {
            Named::Maybe => {
                let m = self.mark();
                match self.nested_name(false) {
                    Ok(nn) => name = Some(nn),
                    Err(Fail) => self.reset(m),
                }
            }
            Named::Single => {
                if self.match_identifier() {
                    let ident = self.add_identifier()?;
                    name = Some(self.single_name(ident)?);
                    self.skip_ws();
                    if self.current() == b':' {
                        return Err(Fail);
                    }
                }
            }
            Named::True => name = Some(self.nested_name(false)?),
            Named::False => {}
        }
        let arrays = self.array_operators(typed)?;
        let params = self.parameters_and_qualifiers(mode)?;
        let plain = params.is_none() && arrays.is_none();
        if plain && named != Named::False && mode == ParamMode::Type && typed {
            self.skip_ws();
            if self.skip_string(":") {
                let size = self.constant_expression(false)?;
                return self.add(Node::DBitfield { name, size });
            }
        }
        self.add(Node::DName { name, arrays, params })
    }

    /// `[size]` operators after a declarator-id.
    fn array_operators(&mut self, typed: bool) -> PResult<List> {
        let mut arrays = ListBuilder::default();
        loop {
            self.skip_ws();
            if !typed || !self.skip_string("[") {
                break;
            }
            self.skip_ws();
            let mut size = None;
            if !self.skip_string("]") {
                size = Some(self.expression_fallback("]", true, Parser::expression)?);
                if !self.skip_string("]") {
                    return Err(Fail);
                }
            }
            let array = self.add(Node::Array(size))?;
            arrays.push(self, array);
        }
        Ok(arrays.head())
    }

    /// Makes a `NestedName` of one element.
    pub(crate) fn single_name(&mut self, ident: NodeId) -> PResult<NodeId> {
        let element =
            self.add(Node::NameElement { name: ident, args: None, has_template_keyword: false })?;
        self.add(Node::NestedName { is_rooted: false, elements: Some(element) })
    }

    /// Pointers, references, packs, a parenthesized declarator, a pointer to member, or
    /// the declarator-id and its suffixes.
    pub(crate) fn declarator(
        &mut self,
        named: Named,
        mode: ParamMode,
        typed: bool,
    ) -> PResult<NodeId> {
        self.guarded(|p| p.declarator_inner(named, mode, typed))
    }

    fn declarator_inner(&mut self, named: Named, mode: ParamMode, typed: bool) -> PResult<NodeId> {
        self.skip_ws();
        if typed && self.skip_string("*") {
            return self.pointer_declarator(named, mode);
        }
        if typed && self.skip_string("&") {
            let attrs = self.attribute_list()?;
            let next = self.declarator(named, mode, typed)?;
            return self.add(Node::DRef { next, attrs });
        }
        if typed && self.skip_string("...") {
            let next = self.declarator(named, mode, false)?;
            return self.add(Node::DPack(next));
        }
        if typed && self.current() == b'(' {
            return self.paren_declarator(named, mode, typed);
        }
        if typed {
            let m = self.mark();
            if let Some(res) = self.member_pointer(named, mode, typed)? {
                return Ok(res);
            }
            self.reset(m);
        }
        self.declarator_name_suffix(named, mode, typed)
    }

    /// `*` cv-qualifiers and attributes, and the declarator after them.
    fn pointer_declarator(&mut self, named: Named, mode: ParamMode) -> PResult<NodeId> {
        self.skip_ws();
        let (mut is_const, mut is_volatile) = (false, false);
        let mut attrs = ListBuilder::default();
        loop {
            if !is_volatile && self.skip_word_ws("volatile") {
                is_volatile = true;
                continue;
            }
            if !is_const && self.skip_word_ws("const") {
                is_const = true;
                continue;
            }
            let Some(attr) = self.attribute()? else { break };
            attrs.push(self, attr);
        }
        let next = self.declarator(named, mode, true)?;
        self.add(Node::DPtr { next, is_const, is_volatile, attrs: attrs.head() })
    }

    /// Parameters and qualifiers first, else `(` ptr-declarator `)`.
    fn paren_declarator(&mut self, named: Named, mode: ParamMode, typed: bool) -> PResult<NodeId> {
        if mode == ParamMode::OperatorCast {
            return self.add(Node::DName { name: None, arrays: None, params: None });
        }
        let m = self.mark();
        if let Ok(res) = self.declarator_name_suffix(named, mode, typed) {
            return Ok(res);
        }
        self.reset(m);
        self.pos += 1; // "("
        let inner = self.declarator(named, mode, typed)?;
        if !self.skip_string(")") {
            return Err(Fail);
        }
        let next = self.declarator(Named::False, ParamMode::Type, typed)?;
        self.add(Node::DParen { inner, next })
    }

    /// `name::*` cv-qualifiers and a declarator; `None` when the text is not one, and
    /// `Fail` when it is one that does not parse.
    fn member_pointer(
        &mut self,
        named: Named,
        mode: ParamMode,
        typed: bool,
    ) -> PResult<Option<NodeId>> {
        let Ok(class) = self.nested_name(true) else { return Ok(None) };
        self.skip_ws();
        if !self.skip_string("*") {
            return Ok(None);
        }
        self.skip_ws();
        let (mut is_const, mut is_volatile) = (false, false);
        loop {
            if !is_volatile && self.skip_word_ws("volatile") {
                is_volatile = true;
            } else if !is_const && self.skip_word_ws("const") {
                is_const = true;
            } else {
                break;
            }
        }
        let next = self.declarator(named, mode, typed)?;
        self.add(Node::DMemPtr { class, next, is_const, is_volatile }).map(Some)
    }

    /// For a member a braced list, or `=` and a braced list or expression (scanned by
    /// the fallback when it does not parse and `allow_fallback` holds).
    pub(crate) fn initializer(
        &mut self,
        outer: Outer,
        allow_fallback: bool,
    ) -> PResult<Option<NodeId>> {
        self.skip_ws();
        if outer == Outer::Member
            && let Some(braced) = self.braced_init()?
        {
            return self.add(Node::Initializer { value: braced, has_assign: false }).map(Some);
        }
        if !self.skip_string("=") {
            return Ok(None);
        }
        let value = match self.braced_init()? {
            Some(braced) => braced,
            None => match outer {
                Outer::TemplateParam => {
                    self.expression_fallback(",>", allow_fallback, Parser::assignment_template)?
                }
                Outer::None => {
                    self.expression_fallback(",)", allow_fallback, Parser::assignment_plain)?
                }
                _ => self.expression_fallback("", allow_fallback, Parser::assignment_plain)?,
            },
        };
        self.add(Node::Initializer { value, has_assign: true }).map(Some)
    }

    /// Declaration specifiers and a declarator. For a type or a function, first without
    /// a type (a bare name, a constructor), then with one.
    pub(crate) fn parse_type(&mut self, named: Named, outer: Outer) -> PResult<NodeId> {
        self.guarded(|p| p.parse_type_inner(named, outer))
    }

    fn parse_type_inner(&mut self, named: Named, outer: Outer) -> PResult<NodeId> {
        let (specs, decl) = if outer == Outer::Type || outer == Outer::Function {
            let mode = if outer == Outer::Type { ParamMode::Type } else { ParamMode::Function };
            let start = self.mark();
            match self.untyped(outer, mode) {
                Some(parts) => parts,
                None => {
                    self.reset(start);
                    let specs = self.decl_specs(outer, true)?;
                    (specs, self.declarator(Named::True, mode, true)?)
                }
            }
        } else {
            let (named, mode, outer) = match outer {
                Outer::Member => (Named::True, ParamMode::Type, outer),
                Outer::OperatorCast => (named, ParamMode::OperatorCast, Outer::None),
                Outer::TemplateParam => (Named::Single, ParamMode::Type, outer),
                _ => (named, ParamMode::Type, outer),
            };
            let specs = self.decl_specs(outer, true)?;
            (specs, self.declarator(named, mode, true)?)
        };
        self.add(Node::Type { specs, decl })
    }

    /// A declaration without a type, which must end there (a function may go on with a
    /// requires clause).
    fn untyped(&mut self, outer: Outer, mode: ParamMode) -> Option<(NodeId, NodeId)> {
        let specs = self.decl_specs(outer, false).ok()?;
        let decl = self.declarator(Named::True, mode, false).ok()?;
        self.skip_ws();
        let at_end = self.is_eof() || self.def.get(self.pos..) == Some(";");
        let requires = outer == Outer::Function && self.is_at_word("requires");
        (at_end || requires).then_some((specs, decl))
    }

    /// A type and its initializer; for a template parameter, possibly a constrained
    /// parameter with a default type.
    pub(crate) fn type_with_init(&mut self, named: Named, outer: Outer) -> PResult<NodeId> {
        let ty = self.parse_type(named, outer)?;
        if outer != Outer::TemplateParam {
            let init = self.initializer(outer, true)?;
            return self.add(Node::TypeWithInit { ty, init });
        }
        let m = self.mark();
        if let Ok(init) = self.initializer(outer, false) {
            let Some(init) = init else { return self.add(Node::TypeWithInit { ty, init: None }) };
            self.skip_ws();
            if matches!(self.current(), b',' | b'>') {
                return self.add(Node::TypeWithInit { ty, init: Some(init) });
            }
        }
        self.reset(m);
        if !self.skip_string("=") {
            return self.add(Node::TypeWithInit { ty, init: None });
        }
        let default = self.parse_type(Named::False, Outer::None)?;
        self.add(Node::ConstrainedInit { ty, default })
    }

    /// `[[...]]`, or `__attribute__((...))`; `None` for none.
    pub(crate) fn attribute(&mut self) -> PResult<Option<NodeId>> {
        self.guarded(Parser::attribute_inner)
    }

    fn attribute_inner(&mut self) -> PResult<Option<NodeId>> {
        self.skip_ws();
        let m = self.mark();
        if self.skip_string_ws("[") {
            if self.skip_string("[") {
                let start = self.pos;
                if !balanced(self, b']') {
                    return Err(Fail);
                }
                let arg = self.since(start);
                if !self.skip_string_ws("]") || !self.skip_string_ws("]") {
                    return Err(Fail);
                }
                return self.add(Node::AttrCpp(arg)).map(Some);
            }
            self.reset(m);
        }
        if !self.skip_word_ws("__attribute__") {
            return Ok(None);
        }
        if !self.skip_string_ws("(") || !self.skip_string_ws("(") {
            return Err(Fail);
        }
        let mut items = ListBuilder::default();
        loop {
            if self.match_identifier() {
                let name = self.last;
                let args = self.paren_list()?;
                let gnu = self.add(Node::AttrGnu { name, args })?;
                items.push(self, gnu);
            }
            if self.skip_string_ws(",") {
                continue;
            }
            if self.skip_string_ws(")") {
                break;
            }
            return Err(Fail);
        }
        if !self.skip_string_ws(")") {
            return Err(Fail);
        }
        self.add(Node::AttrGnuList(items.head())).map(Some)
    }

    pub(crate) fn attribute_list(&mut self) -> PResult<List> {
        let mut attrs = ListBuilder::default();
        while let Some(attr) = self.attribute()? {
            attrs.push(self, attr);
        }
        Ok(attrs.head())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spaces_collapse_only_when_needed() {
        assert!(matches!(collapse_spaces("unsigned int"), Cow::Borrowed(_)));
        assert_eq!(collapse_spaces("unsigned \t long  int"), "unsigned long int");
    }

    #[test]
    fn fundamental_words_take_canonical_order() {
        let mut f = Fundamental::default();
        for w in ["long", "unsigned", "long", "int"] {
            assert!(f.add(w).is_ok());
        }
        assert!(f.is_valid());
        assert_eq!(f.canonical(), "unsigned long long int");
    }
}
