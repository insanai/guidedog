//! The i18n extension's `{% trans %}` tag. Translation is a pass-through unless the host
//! registers `gettext` / `ngettext` globals of its own.

use crate::ast::{Expr, ExprKind, Name, StmtKind, Trans};
use crate::calls::call_value;
use crate::convert::quoted;
use crate::convert::to_str;
use crate::eval::{eval, lookup};
use crate::lexer::TokenKind;
use crate::parser::Parser;
use crate::runtime::{Result, ScopeId, State};
use crate::text::is_space;
use crate::value::{Dict, DictKind, Value};

/// The variables a trans tag declares, and whether it trims.
#[derive(Default)]
struct TransHead {
    vars: Vec<(Name, Expr)>,
    count: Option<usize>,
    trimmed: bool,
}

impl Parser<'_> {
    pub(crate) fn parse_trans(&mut self) -> StmtKind {
        let line = self.next_token().line;
        let head = self.parse_trans_head();
        self.expect(TokenKind::BlockEnd, "");
        let TransHead { mut vars, mut count, trimmed } = head;
        let (mut singular, mut referenced) = self.trans_block(true);
        let mut plural = None;
        let mut count_name: Option<Name> = None;
        if count.is_none() && !referenced.is_empty() {
            count_name = Some(referenced[0].clone());
        }
        if self.at_name("pluralize", 0) {
            self.next_token();
            if self.peek(0).kind != TokenKind::BlockEnd {
                let (name, at) = self.expect_name();
                match vars.iter().position(|(n, _)| *n == name) {
                    Some(i) => (count, count_name) = (Some(i), None),
                    None => {
                        self.fail(
                            at,
                            format!("unknown variable {} for pluralization", quoted(&name)),
                        );
                    }
                }
            }
            self.expect(TokenKind::BlockEnd, "");
            let (text, more) = self.trans_block(false);
            plural = Some(text);
            referenced.extend(more);
        }
        self.next_token();
        self.expect(TokenKind::BlockEnd, "");
        for name in &referenced {
            if !vars.iter().any(|(n, _)| n == name) {
                vars.push((name.clone(), Expr { line, kind: ExprKind::Name(name.clone()) }));
            }
        }
        if let Some(name) = count_name {
            count = vars.iter().position(|(n, _)| *n == name);
        }
        if plural.is_some() && count.is_none() {
            self.fail(line, "pluralize without variables");
        }
        if trimmed {
            singular = trim_trans(&singular);
            plural = plural.map(|p| trim_trans(&p));
        }
        if referenced.is_empty() {
            // Without placeholders the text is not %-formatted, so %% means itself.
            singular = singular.replace("%%", "%");
            plural = plural.map(|p| p.replace("%%", "%"));
        }
        StmtKind::Trans(Box::new(Trans { singular, plural, count, vars }))
    }

    /// Parses `name=expr, other, trimmed` up to the end of the tag.
    fn parse_trans_head(&mut self) -> TransHead {
        let mut head = TransHead::default();
        let mut trimmed_set = false;
        while self.peek(0).kind != TokenKind::BlockEnd && !self.failed() {
            if !head.vars.is_empty() {
                self.expect(TokenKind::Operator, ",");
            }
            if self.skip_if(":") {
                break;
            }
            let (name, at) = self.expect_name();
            if self.failed() {
                break;
            }
            if head.vars.iter().any(|(n, _)| *n == name) {
                self.fail(at, format!("translatable variable {} defined twice.", quoted(&name)));
            }
            let value = if self.at_op("=") {
                self.next_token();
                self.parse_expr(true)
            } else if !trimmed_set && (&*name == "trimmed" || &*name == "notrimmed") {
                trimmed_set = true;
                head.trimmed = &*name == "trimmed";
                continue;
            } else {
                Expr { line: at, kind: ExprKind::Name(name.clone()) }
            };
            if head.count.is_none() {
                head.count = Some(head.vars.len());
            }
            head.vars.push((name, value));
        }
        head
    }

    /// Collects the text of a trans section with `%(name)s` placeholders, and the names
    /// it references.
    fn trans_block(&mut self, allow_plural: bool) -> (String, Vec<Name>) {
        let mut text = String::new();
        let mut referenced = Vec::new();
        while !self.failed() {
            let t = self.peek(0).clone();
            match t.kind {
                TokenKind::Data => {
                    self.next_token();
                    text.push_str(&t.text.replace('%', "%%"));
                }
                TokenKind::VariableBegin => {
                    self.next_token();
                    let (name, _) = self.expect_name();
                    text.push_str("%(");
                    text.push_str(&name);
                    text.push_str(")s");
                    referenced.push(name);
                    self.expect(TokenKind::VariableEnd, "");
                }
                TokenKind::BlockBegin => {
                    self.next_token();
                    if self.at_name("endtrans", 0)
                        || (allow_plural && self.at_name("pluralize", 0))
                    {
                        break;
                    }
                    let message = if self.at_name("pluralize", 0) {
                        "a translatable section can have only one pluralize section"
                    } else {
                        "control structures in translatable sections are not allowed"
                    };
                    self.fail(t.line, message);
                }
                TokenKind::Eof => self.fail(t.line, "unclosed translation block"),
                _ => self.fail_unexpected(),
            }
        }
        (text, referenced)
    }
}

/// Joins the lines of a trimmed trans section with single spaces.
fn trim_trans(s: &str) -> String {
    let text = s.trim_matches(is_space);
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(is_space) {
        out.push_str(&rest[..start]);
        let run = &rest[start..];
        let end = run.find(|c: char| !is_space(c)).unwrap_or(run.len());
        if run[..end].contains('\n') {
            out.push(' ');
        } else {
            out.push_str(&run[..end]);
        }
        rest = &run[end..];
    }
    out.push_str(rest);
    out
}

/// Renders a trans tag: its text through the `gettext` / `ngettext` in scope, with the
/// tag's variables formatted into the placeholders.
pub(crate) fn exec_trans(s: &mut State<'_>, scope: ScopeId, t: &Trans) -> Result<()> {
    let vars = Dict::new(DictKind::Dict);
    for (name, e) in &t.vars {
        let v = eval(s, scope, e)?;
        vars.set(Value::from(name.clone()), v);
    }
    let singular = Value::str(&t.singular);
    let message = match (&t.plural, t.count.and_then(|i| t.vars.get(i))) {
        (Some(plural), Some((_, count))) => {
            let n = eval(s, scope, count)?;
            let f = lookup(s, scope, "ngettext");
            call_value(s, &f, vec![singular, Value::str(plural), n], Vec::new())?
        }
        _ => {
            let f = lookup(s, scope, "gettext");
            call_value(s, &f, vec![singular], Vec::new())?
        }
    };
    let text = to_str(&message).into_owned();
    let result = if t.vars.is_empty() {
        s.captured(text)
    } else {
        crate::format::percent_format(s, s.autoescape, &text, &Value::Dict(vars))?
    };
    s.write_output(&result)
}
