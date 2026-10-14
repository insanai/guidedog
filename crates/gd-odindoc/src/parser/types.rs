//! Procedure signatures, field lists, and record types (struct, union, enum,
//! bit_field), with the comments that document their fields.

use super::Parser;
use crate::ast::{BitFieldField, EnumType, Expr, Field, GroupId, NodeId, Span, flag};
use crate::tokenizer::{Tk, Token};

/// The `#flag` names a field may be prefixed with.
const HASH_FLAGS: [(&str, u16); 8] = [
    ("no_alias", flag::NO_ALIAS),
    ("c_vararg", flag::C_VARARG),
    ("const", flag::CONST),
    ("any_int", flag::ANY_INT),
    ("subtype", flag::SUBTYPE),
    ("by_ptr", flag::BY_PTR),
    ("no_broadcast", flag::NO_BROADCAST),
    ("no_capture", flag::NO_CAPTURE),
];

/// A field prefix, as Odin's `is_token_field_prefix` reads one.
enum Prefix {
    None,
    Unknown,
    Flag(u16),
}

impl Parser<'_> {
    /// `proc "cc" (params) -> results`, after `proc`.
    pub(super) fn parse_proc_type(&mut self, tok: Token) -> NodeId {
        if self.curr.kind == Tk::String {
            let cc = self.expect(Tk::String);
            if cc.end - cc.start == 2 {
                self.error();
            }
        }
        self.expect(Tk::OpenParen);
        self.expr_level += 1;
        self.parse_field_list(Tk::CloseParen, flag::PARAMS);
        self.expr_level -= 1;
        self.expect_closing_parens();
        self.parse_results();
        self.push(Expr::ProcType, tok.start, self.prev.end)
    }

    fn parse_results(&mut self) {
        if !self.allow(Tk::ArrowRight) || self.allow(Tk::Not) {
            return;
        }
        let level = self.expr_level;
        if self.curr.kind != Tk::OpenParen {
            self.parse_type();
        } else {
            self.expect(Tk::OpenParen);
            self.parse_field_list(Tk::CloseParen, flag::RESULTS_LIST);
            self.expect(Tk::CloseParen);
        }
        self.expr_level = level;
    }

    fn token_field_prefix(&mut self) -> Prefix {
        match self.curr.kind {
            Tk::Using => {
                self.advance();
                Prefix::Flag(flag::USING)
            }
            Tk::Hash => {
                let next = self.peek(0);
                let name = self.text(self.span_of(next));
                let types = ["simd", "type", "row_major", "column_major", "sparse", "soa"];
                if next.kind == Tk::Ident && types.contains(&name) {
                    return Prefix::None;
                }
                self.advance();
                let tok = self.curr;
                self.advance();
                let name = self.text(self.span_of(tok));
                match HASH_FLAGS.iter().find(|(key, _)| *key == name) {
                    Some(&(_, f)) if tok.kind == Tk::Ident => Prefix::Flag(f),
                    _ => Prefix::Unknown,
                }
            }
            _ => Prefix::None,
        }
    }

    fn parse_field_prefixes(&mut self) -> u16 {
        let mut flags = 0;
        loop {
            match self.token_field_prefix() {
                Prefix::None => return flags,
                Prefix::Unknown => self.error(),
                Prefix::Flag(f) => {
                    if flags & f != 0 {
                        self.error();
                    }
                    flags |= f;
                }
            }
        }
    }

    fn check_field_flags(&mut self, name_count: usize, allowed: u16, set: u16) -> u16 {
        let mut flags = set;
        if name_count > 1 && flags & flag::USING != 0 {
            self.error();
            flags &= !flag::USING;
        }
        if flags & !allowed != 0 {
            self.error();
            flags &= allowed;
        }
        flags
    }

    /// A field's type: `..T` when variadic, `typeid/T`, or a type.
    fn parse_var_type(&mut self, allowed: u16) -> NodeId {
        if allowed & flag::ELLIPSIS != 0 && self.curr.kind == Tk::Ellipsis {
            let tok = self.advance();
            let ty = match self.parse_type_or_ident() {
                Some(ty) => ty,
                None => {
                    self.error();
                    self.push(Expr::Bad, tok.start, tok.end)
                }
            };
            let (start, end) = (self.start_of(ty), self.end_of(ty));
            return self.push(Expr::Ellipsis, start, end);
        }
        if allowed & flag::TYPEID_TOKEN != 0 && self.curr.kind == Tk::Typeid {
            let tok = self.expect(Tk::Typeid);
            // Odin ends a typeid type without a specialization at its start.
            let mut end = tok.start;
            if self.allow(Tk::Quo) {
                let spec = self.parse_type();
                end = self.end_of(spec);
            }
            return self.push(Expr::Typeid, tok.start, end);
        }
        self.parse_type()
    }

    fn parse_ident_list(&mut self, allow_poly: bool) -> Vec<NodeId> {
        let mut list = Vec::new();
        loop {
            if allow_poly && self.curr.kind == Tk::Dollar {
                let tok = self.expect(Tk::Dollar);
                let ident = self.parse_ident();
                let end = self.end_of(ident);
                list.push(self.push(Expr::Poly, tok.start, end));
            } else {
                list.push(self.parse_ident());
            }
            if self.curr.kind != Tk::Comma {
                return list;
            }
            self.advance();
        }
    }

    /// Fields up to `follow`: `a, b: T`, or types alone (`(int, bool)`). Each field's doc
    /// comment is the one before it; its line comment, the one after it (or after its
    /// comma) on its line.
    pub(super) fn parse_field_list(&mut self, follow: Tk, allowed: u16) -> (Vec<Field>, usize) {
        let mut docs = self.lead_comment;
        let mut list: Vec<(NodeId, u16)> = Vec::new();
        let mut fields = Vec::new();
        let var_flags = allowed & (flag::TYPEID_TOKEN | flag::ELLIPSIS);
        while !matches!(self.curr.kind, Tk::Colon | Tk::Eof) && self.curr.kind != follow {
            let prefixes = self.parse_field_prefixes();
            let param = self.parse_var_type(var_flags);
            list.push((param, prefixes));
            if !self.allow(Tk::Comma) {
                break;
            }
        }
        if self.curr.kind != Tk::Colon {
            for &(ty, prefixes) in &list {
                let flags = self.check_field_flags(list.len(), allowed, prefixes);
                let comment = self.line_comment;
                // Odin names a field written as a type alone by a made-up "_" token at
                // the type's start, one byte long (none in a result list).
                let start = self.start_of(ty);
                let width = u32::from(allowed & flag::RESULTS == 0);
                let names = vec![self.push(Expr::Ident { blank: true }, start, start + width)];
                fields.push(Field { names, ty: Some(ty), flags, tag: None, docs, comment });
            }
            return (fields, 0);
        }
        let allow_poly = allowed & flag::TYPEID_TOKEN != 0;
        let names: Vec<NodeId> =
            list.iter().map(|&(id, _)| self.ident_of(id, allow_poly)).collect();
        if names.is_empty() {
            self.error();
        }
        let mut total = names.len();
        let set = list.first().map_or(0, |&(_, f)| f);
        let mut more = self.parse_field(&mut fields, docs, names, allowed, set);
        while more && !matches!(self.curr.kind, Tk::Eof) && self.curr.kind != follow {
            docs = self.lead_comment;
            let set = self.parse_field_prefixes();
            let names = self.parse_ident_list(allow_poly);
            total += names.len();
            more = self.parse_field(&mut fields, docs, names, allowed, set);
        }
        (fields, total)
    }

    /// A name read as a type before the `:` showed it was a name.
    fn ident_of(&mut self, id: NodeId, allow_poly: bool) -> NodeId {
        match self.file.node(id).kind {
            Expr::Ident { .. } | Expr::Bad => id,
            Expr::Poly if allow_poly => id,
            Expr::Poly => {
                self.error();
                id
            }
            _ => {
                self.error();
                let span = self.file.node(id).span;
                self.push(Expr::Ident { blank: false }, span.start, span.end)
            }
        }
    }

    /// The rest of one `names: type = default "tag"` field; false when no separator
    /// follows it.
    fn parse_field(
        &mut self,
        fields: &mut Vec<Field>,
        docs: Option<GroupId>,
        names: Vec<NodeId>,
        allowed: u16,
        set: u16,
    ) -> bool {
        let flags = self.check_field_flags(names.len(), allowed, set);
        self.expect(Tk::Colon);
        let mut ty = None;
        if self.curr.kind != Tk::Eq {
            ty = Some(self.parse_var_type(allowed));
        }
        let mut has_default = false;
        if self.allow(Tk::Eq) {
            self.parse_expr(false);
            has_default = allowed & flag::DEFAULT_PARAMETERS != 0;
            if !has_default || names.len() > 1 {
                self.error();
            }
        }
        let mut tag = None;
        if ty.is_some() && !has_default && self.curr.kind == Tk::String {
            let tok = self.expect(Tk::String);
            tag = Some(self.span_of(tok));
            if allowed & flag::TAGS == 0 {
                self.error();
            }
        }
        let line_comment = self.line_comment;
        let ok = self.expect_field_separator();
        let comment = self.line_comment.or(line_comment);
        fields.push(Field { names, ty, flags, tag, docs, comment });
        ok
    }

    fn expect_field_separator(&mut self) -> bool {
        let tok = self.curr;
        if self.allow(Tk::Comma) {
            return true;
        }
        if self.allow(Tk::Semicolon) {
            if !tok.is_newline() {
                self.error();
            }
            return true;
        }
        false
    }

    /// `(params)` after `struct` or `union`.
    fn parse_poly_params(&mut self) {
        if self.allow(Tk::OpenParen) {
            let (_, count) = self.parse_field_list(Tk::CloseParen, flag::POLY_PARAMS);
            if count == 0 {
                self.error();
            }
            self.expect(Tk::CloseParen);
        }
    }

    /// `#packed`, `#align(N)`, and other record tags.
    fn parse_record_tags(&mut self) {
        let level = self.expr_level;
        self.expr_level = -1;
        while self.allow(Tk::Hash) {
            let tag = self.expect(Tk::Ident);
            match self.text(self.span_of(tag)) {
                "align" | "field_align" | "min_field_align" | "max_field_align" => {
                    self.parse_expr(true);
                }
                "packed" | "all_or_none" | "simple" | "raw_union" | "no_copy" | "no_nil"
                | "shared_nil" => {}
                _ => self.error(),
            }
        }
        self.expr_level = level;
    }

    fn parse_where_clauses(&mut self) {
        self.skip_possible_newline_for_literal();
        if self.curr.kind == Tk::Where {
            self.expect(Tk::Where);
            let level = self.expr_level;
            self.expr_level = -1;
            self.parse_expr_list(false);
            self.expr_level = level;
        }
        self.skip_possible_newline_for_literal();
    }

    pub(super) fn parse_struct_type(&mut self) -> NodeId {
        let tok = self.expect(Tk::Struct);
        self.parse_poly_params();
        self.parse_record_tags();
        self.parse_where_clauses();
        self.expect(Tk::OpenBrace);
        let (fields, _) = self.parse_field_list(Tk::CloseBrace, flag::STRUCT);
        let close = self.expect_closing(Tk::CloseBrace);
        let index = self.file.structs.len() as u32;
        self.file.structs.push(fields);
        self.push(Expr::Struct(index), tok.start, close.end)
    }

    pub(super) fn parse_union_type(&mut self) -> NodeId {
        let tok = self.expect(Tk::Union);
        self.parse_poly_params();
        self.parse_record_tags();
        self.parse_where_clauses();
        self.expect(Tk::OpenBrace);
        while !matches!(self.curr.kind, Tk::CloseBrace | Tk::Eof) {
            self.parse_type();
            if !self.allow(Tk::Comma) {
                break;
            }
        }
        let close = self.expect_closing(Tk::CloseBrace);
        self.push(Expr::Union, tok.start, close.end)
    }

    pub(super) fn parse_enum_type(&mut self) -> NodeId {
        let tok = self.expect(Tk::Enum);
        if self.curr.kind != Tk::OpenBrace {
            self.parse_type();
        }
        self.skip_possible_newline_for_literal();
        let open = self.expect(Tk::OpenBrace);
        let fields = self.parse_elem_list();
        let close = self.expect_closing(Tk::CloseBrace);
        let index = self.file.enums.len() as u32;
        self.file.enums.push(EnumType { open_line: open.line, fields });
        self.push(Expr::Enum(index), tok.start, close.end)
    }

    pub(super) fn parse_bit_field_type(&mut self) -> NodeId {
        let tok = self.expect(Tk::BitField);
        if self.parse_type_or_ident().is_none() {
            self.advance();
            self.error();
        }
        self.skip_possible_newline_for_literal();
        self.expect(Tk::OpenBrace);
        let mut fields = Vec::new();
        while !matches!(self.curr.kind, Tk::CloseBrace | Tk::Eof) {
            let docs = self.lead_comment;
            let name = self.parse_ident();
            self.expect(Tk::Colon);
            let ty = self.parse_type();
            self.expect(Tk::Or);
            let bit_size = self.parse_expr(true);
            let mut tag = None;
            if self.curr.kind == Tk::String {
                let t = self.expect(Tk::String);
                tag = Some(Span { start: t.start, end: t.end });
            }
            let ok = self.allow(Tk::Comma);
            let comment = self.line_comment;
            fields.push(BitFieldField { name, ty, bit_size, tag, docs, comment });
            if !ok {
                break;
            }
        }
        let close = self.expect_closing(Tk::CloseBrace);
        let index = self.file.bit_fields.len() as u32;
        self.file.bit_fields.push(fields);
        self.push(Expr::BitField(index), tok.start, close.end)
    }
}
