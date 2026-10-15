//! The part of Jinja that substitutions use: names and dotted names (env.docname,
//! env.config.<name>, key.sub), quoted strings, "+" and "~", parentheses, the filters
//! upper, lower, title, capitalize, trim/strip, replace, default, and format, and the
//! same names as string methods.

use super::trim_space;
use crate::host::Host;

/// The longest value an expression may build, so hostile expressions stay bounded.
const MAX_VALUE: usize = 1 << 20;

/// A value and whether it is defined.
type Value = (String, bool);

struct Eval<'e, 'h, 'a> {
    h: &'e Host<'h, 'a>,
    s: &'e [u8],
    text: &'e str,
    pos: usize,
    why: Option<&'static str>,
}

/// Computes a substitution expression: `Ok(Some(value))`, `Ok(None)` for an undefined
/// name, or `Err(why)` for what is not supported.
pub(crate) fn evaluate(h: &Host<'_, '_>, expr: &str) -> Result<Option<String>, &'static str> {
    let mut ev = Eval { h, s: expr.as_bytes(), text: expr, pos: 0, why: None };
    let (v, defined) = ev.expr();
    ev.space();
    if ev.why.is_none() && ev.pos < ev.s.len() {
        ev.why = Some("unexpected text after the expression");
    }
    if v.len() > MAX_VALUE {
        ev.why = Some("the value is too long");
    }
    match ev.why {
        Some(why) => Err(why),
        None => Ok(defined.then_some(v)),
    }
}

impl<'e> Eval<'e, '_, '_> {
    fn space(&mut self) {
        while self.pos < self.s.len() && (self.s[self.pos] == b' ' || self.s[self.pos] == b'\t') {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> u8 {
        self.space();
        self.s.get(self.pos).copied().unwrap_or(0)
    }

    fn expr(&mut self) -> Value {
        let (mut v, mut defined) = self.filtered();
        while self.why.is_none() && matches!(self.peek(), b'+' | b'~') {
            self.pos += 1;
            let (w, d) = self.filtered();
            v.push_str(&w);
            defined = defined && d;
            if v.len() > MAX_VALUE {
                self.why = Some("the value is too long");
            }
        }
        (v, defined)
    }

    fn filtered(&mut self) -> Value {
        let mut v = self.postfix();
        while self.why.is_none() && self.peek() == b'|' {
            self.pos += 1;
            let name = self.ident();
            let args = self.args();
            v = self.apply(name, v, &args);
        }
        v
    }

    /// Reads a primary (a string, a parenthesized expression, or a dotted name) and the
    /// method calls after it.
    fn postfix(&mut self) -> Value {
        let mut v = match self.peek() {
            b'"' | b'\'' => (self.string(), true),
            b'(' => {
                self.pos += 1;
                let v = self.expr();
                if self.peek() != b')' {
                    self.why = self.why.or(Some("a parenthesis is not closed"));
                }
                self.pos += 1;
                v
            }
            _ => self.path(),
        };
        while self.why.is_none() && self.s.get(self.pos) == Some(&b'.') {
            self.pos += 1;
            let m = self.ident();
            if self.peek() != b'(' {
                self.why = Some("attributes of values are not supported");
                break;
            }
            let args = self.args();
            v = self.apply(m, v, &args);
        }
        v
    }

    /// Reads a dotted name, stopping before a method call, and looks it up.
    fn path(&mut self) -> Value {
        let start = self.pos;
        let mut end = self.pos;
        while self.why.is_none() {
            if self.ident().is_empty() {
                self.why = Some("expected a name or a string");
                return (String::new(), false);
            }
            end = self.pos;
            if self.s.get(self.pos) != Some(&b'.') {
                break;
            }
            let dot = self.pos;
            self.pos += 1;
            self.ident();
            if self.peek() == b'(' {
                self.pos = dot;
                break;
            }
            self.pos = dot + 1;
        }
        let text = self.text;
        self.name(trim_space(&text[start..end]))
    }

    fn ident(&mut self) -> &'e str {
        self.space();
        let a = self.pos;
        while self.pos < self.s.len()
            && (self.s[self.pos].is_ascii_alphanumeric() || b"_-".contains(&self.s[self.pos]))
        {
            self.pos += 1;
        }
        let text: &'e str = self.text;
        &text[a..self.pos]
    }

    fn string(&mut self) -> String {
        let q = self.s[self.pos];
        self.pos += 1;
        let mut out = Vec::new();
        while self.pos < self.s.len() && self.s[self.pos] != q {
            let mut c = self.s[self.pos];
            if c == b'\\' && self.pos + 1 < self.s.len() {
                self.pos += 1;
                c = self.s[self.pos];
                if c == b'n' {
                    c = b'\n';
                }
            }
            out.push(c);
            self.pos += 1;
        }
        if self.pos >= self.s.len() {
            self.why = self.why.or(Some("a string is not closed"));
        }
        self.pos += 1;
        String::from_utf8_lossy(&out).into_owned()
    }

    /// Reads "(a, b)" when present.
    fn args(&mut self) -> Vec<String> {
        let mut args = Vec::new();
        if self.peek() != b'(' {
            return args;
        }
        self.pos += 1;
        while self.why.is_none() && self.peek() != b')' {
            if self.pos >= self.s.len() {
                self.why = Some("expected ',' or ')'");
                break;
            }
            let (v, _) = self.expr();
            args.push(v);
            match self.peek() {
                b',' => self.pos += 1,
                b')' => {}
                _ => self.why = self.why.or(Some("expected ',' or ')'")),
            }
        }
        self.pos += 1;
        args
    }

    /// Looks up a (dotted) name: env.docname, env.config.<name>, or a substitution.
    fn name(&self, name: &str) -> Value {
        let settings = self.h.settings;
        if name == "env.docname" {
            return (settings.docname.clone(), true);
        }
        if let Some(key) = name.strip_prefix("env.config.") {
            return match settings.env_config.iter().find(|c| c.name == key) {
                Some(c) => (c.value.clone(), true),
                None => (String::new(), false),
            };
        }
        match self.h.ms.subs.find(name) {
            Some(i) => (self.h.ms.subs.items[i].value.clone(), true),
            None => (String::new(), false),
        }
    }

    /// Applies a filter or string method to `v`.
    fn apply(&mut self, name: &str, v: Value, args: &[String]) -> Value {
        let (s, defined) = v;
        let out = match name {
            "default" if defined || args.is_empty() => return (s, defined),
            "default" => return (args[0].clone(), true),
            "upper" => s.to_ascii_uppercase(),
            "lower" => s.to_ascii_lowercase(),
            "title" | "capitalize" => change_case(&s, name == "title"),
            "trim" | "strip" => trim_space(&s).to_owned(),
            "format" => match format_args(&s, args) {
                Ok(f) => f,
                Err(why) => {
                    self.why = self.why.or(Some(why));
                    String::new()
                }
            },
            "replace" if args.len() < 2 || args[0].is_empty() => {
                self.why = self.why.or(Some("replace needs two arguments"));
                return (String::new(), false);
            }
            "replace" => s.replace(args[0].as_str(), &args[1]),
            _ => {
                self.why = self.why.or(Some("the filter or method is not supported"));
                return (String::new(), false);
            }
        };
        (out, defined)
    }
}

/// Upper-cases the first letter of each word ("title") or of the text, lowering the
/// rest ("capitalize").
fn change_case(s: &str, title: bool) -> String {
    let mut out = String::with_capacity(s.len());
    let mut word_start = true;
    for (k, c) in s.chars().enumerate() {
        let up = if title { word_start } else { k == 0 };
        if up {
            out.push(c.to_ascii_uppercase());
        } else if !title {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
        word_start = !c.is_ascii_alphanumeric();
    }
    out
}

/// str.format: "{}" takes the next argument, "{N}" argument N, "{{" and "}}" are
/// braces.
fn format_args(v: &str, args: &[String]) -> Result<String, &'static str> {
    let b = v.as_bytes();
    let mut out = String::new();
    let (mut next, mut i, mut run) = (0, 0, 0);
    while i < b.len() {
        let c = b[i];
        if (c == b'{' || c == b'}') && b.get(i + 1) == Some(&c) {
            out.push_str(&v[run..=i]);
            i += 2;
            run = i;
            continue;
        }
        if c != b'{' {
            i += 1;
            continue;
        }
        out.push_str(&v[run..i]);
        let Some(e) = b[i..].iter().position(|&x| x == b'}').map(|e| i + e) else {
            return Err("a format field is not closed");
        };
        let k = if e > i + 1 {
            v[i + 1..e].bytes().fold(0usize, |k, d| k * 10 + usize::from(d.wrapping_sub(b'0')))
        } else {
            next += 1;
            next - 1
        };
        if let Some(a) = args.get(k) {
            out.push_str(a);
        }
        i = e + 1;
        run = i;
    }
    out.push_str(&v[run..]);
    Ok(out)
}
