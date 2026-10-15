//! The class, role, and default-role directives.

use guidedoc::{Fail, arg_int, arg_span, arg_text};
use guidedoc_rst_inline::{Role, RoleBase, standard_role};

use crate::directives::Directive;
use crate::lines::{join_lines, trim_blank_start, trim_space};
use crate::messages::*;
use crate::names::{is_simplename, write_id};
use crate::parser::Parser;
use crate::specs::OPTS_ROLE;

/// Splits "name(base)" into the new role name and its base ("" for none).
fn role_name(s: &str) -> Option<(&str, &str)> {
    let i = s.find(|c: char| c == '(' || c.is_ascii_whitespace()).unwrap_or(s.len());
    let (name, rest) = (&s[..i], trim_space(&s[i..]));
    if !is_simplename(name) {
        return None;
    }
    if rest.is_empty() {
        return Some((name, ""));
    }
    let base = trim_space(rest.strip_prefix('(')?.strip_suffix(')')?);
    is_simplename(base).then_some((name, base))
}

impl Parser<'_, '_> {
    /// Returns the index of the role the document defined under `name` with the role
    /// directive, if any. The search spends work budget.
    pub fn find_role(&mut self, name: &str) -> Result<Option<usize>, Fail> {
        self.ctx.b.spend(self.st.inl.roles.len() as u64 + 1)?;
        Ok(self.st.inl.find_role(name))
    }

    /// Runs the class directive: its classes go to the content's elements or, without
    /// content, to the next element; invalid names are reported.
    pub fn class_directive(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        let Some(classes) = self.pool_classes(&d.argument())? else {
            let span = self.lines_span(&d.arg);
            let why = arg_text("expected class names");
            self.report(DIRECTIVE_OPTION_INVALID, span, arg_span(span), why);
            return Ok(());
        };
        if d.content.is_empty() {
            return self.set_pending(classes, d.span);
        }
        self.parse_body(&d.content, false, Some(classes))
    }

    /// Defines a custom interpreted text role, optionally derived from a standard or an
    /// earlier custom role.
    pub(crate) fn role_directive(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        let first = d.content[0].trim();
        let Some((name, base_name)) = role_name(first.text) else {
            let span = self.line_span(&first);
            self.report_at(ROLE_INVALID, span);
            return Ok(());
        };
        let mut role = Role { base: RoleBase::Custom, ..Role::default() };
        if !base_name.is_empty() {
            if let Some(k) = self.find_role(base_name)? {
                role = self.st.inl.roles[k].clone();
            } else if let Some(b) = standard_role(base_name) {
                role.base = b;
            } else {
                let span = self.line_span(&first);
                self.report_at(ROLE_UNKNOWN, span);
                return Ok(());
            }
        }
        let mut opts = Vec::new();
        if !self.parse_options("role", OPTS_ROLE, trim_blank_start(&d.content[1..]), &mut opts) {
            return Ok(());
        }
        let options = Directive { options: opts, ..d.clone() };
        if !self.role_options(&options, &mut role) {
            return Ok(());
        }
        role.name = name.to_ascii_lowercase();
        if !options.has_option("class") {
            role.classes.clear();
            write_id(&mut role.classes, name);
        }
        self.store_role(role, &first)
    }

    fn role_options(&mut self, d: &Directive<'_>, role: &mut Role) -> bool {
        for (option, base) in [("format", RoleBase::Raw), ("language", RoleBase::Code)] {
            if let Some(o) = d.option(option)
                && role.base != base
            {
                self.report(DIRECTIVE_OPTION_UNKNOWN, o.span, arg_span(o.span), arg_text("role"));
                return false;
            }
        }
        if let Some(v) = d.option_value("format") {
            role.format = v.to_ascii_lowercase();
        }
        if let Some(o) = d.option("language") {
            role.language = join_lines(&o.lines, " ").into_owned();
        }
        if let Some(v) = d.option_value("class") {
            role.classes.clear();
            let words = v.split_ascii_whitespace();
            for (k, w) in words.enumerate() {
                if k > 0 {
                    role.classes.push(' ');
                }
                write_id(&mut role.classes, w);
            }
        }
        true
    }

    fn store_role(&mut self, role: Role, at: &crate::Line<'_>) -> Result<(), Fail> {
        if self.find_role(&role.name)?.is_none() && self.st.inl.roles.len() >= self.st.roles_limit
        {
            let span = self.line_span(at);
            let limit = arg_int(self.st.roles_limit as i64);
            self.report(ROLE_CAPACITY, span, arg_span(span), limit);
            return Ok(());
        }
        self.st.inl.define_role(role);
        Ok(())
    }

    pub(crate) fn default_role(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        if d.arg.is_empty() {
            self.st.inl.default_role.clear();
            return Ok(());
        }
        let name = d.argument();
        let name = trim_space(&name);
        if standard_role(name).is_none() && self.find_role(name)?.is_none() {
            let span = self.lines_span(&d.arg);
            self.report_at(ROLE_UNKNOWN, span);
            return Ok(());
        }
        self.st.inl.default_role = name.to_ascii_lowercase();
        Ok(())
    }
}
