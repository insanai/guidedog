//! Commands with an argument structure of their own: roots, `\left ... \right`,
//! environments (matrices, cases, alignments), negations, and stacked symbols.

use super::{Math, Mode, Stop, ascii_math};

/// Delimiters after `\left`, `\right`, `\middle`, and `\big` and friends.
const DELIMITERS: [(&str, &str); 25] = [
    ("\\{", "\\{"),
    ("\\}", "\\}"),
    ("\\lbrace", "\\{"),
    ("\\rbrace", "\\}"),
    ("\\|", "‖"),
    ("\\Vert", "‖"),
    ("\\lVert", "‖"),
    ("\\rVert", "‖"),
    ("\\vert", "|"),
    ("\\lvert", "|"),
    ("\\rvert", "|"),
    ("\\langle", "chevron.l"),
    ("\\rangle", "chevron.r"),
    ("\\lfloor", "floor.l"),
    ("\\rfloor", "floor.r"),
    ("\\lceil", "ceil.l"),
    ("\\rceil", "ceil.r"),
    ("\\lbrack", "\\["),
    ("\\rbrack", "\\]"),
    ("\\backslash", "backslash"),
    ("\\uparrow", "arrow.t"),
    ("\\downarrow", "arrow.b"),
    ("\\updownarrow", "arrow.t.b"),
    ("\\Uparrow", "arrow.t.double"),
    ("\\Downarrow", "arrow.b.double"),
];

/// How an environment's content is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnvKind {
    Matrix,
    /// A matrix whose column specification is skipped.
    Array,
    Cases,
    Align,
    /// An alignment environment with one argument to skip.
    AlignArg,
}

/// The supported environments and the call that opens them.
const ENVIRONMENTS: [(&str, EnvKind, &str); 30] = [
    ("matrix", EnvKind::Matrix, "mat(delim: #none, "),
    ("smallmatrix", EnvKind::Matrix, "mat(delim: #none, "),
    ("pmatrix", EnvKind::Matrix, "mat(delim: \"(\", "),
    ("bmatrix", EnvKind::Matrix, "mat(delim: \"[\", "),
    ("Bmatrix", EnvKind::Matrix, "mat(delim: \"{\", "),
    ("vmatrix", EnvKind::Matrix, "mat(delim: \"|\", "),
    ("Vmatrix", EnvKind::Matrix, "mat(delim: \"‖\", "),
    ("array", EnvKind::Array, "mat(delim: #none, "),
    ("cases", EnvKind::Cases, "cases("),
    ("dcases", EnvKind::Cases, "cases("),
    ("rcases", EnvKind::Cases, "cases(reverse: #true, "),
    ("aligned", EnvKind::Align, ""),
    ("align", EnvKind::Align, ""),
    ("align*", EnvKind::Align, ""),
    ("gather", EnvKind::Align, ""),
    ("gather*", EnvKind::Align, ""),
    ("gathered", EnvKind::Align, ""),
    ("equation", EnvKind::Align, ""),
    ("equation*", EnvKind::Align, ""),
    ("split", EnvKind::Align, ""),
    ("multline", EnvKind::Align, ""),
    ("multline*", EnvKind::Align, ""),
    ("eqnarray", EnvKind::Align, ""),
    ("eqnarray*", EnvKind::Align, ""),
    ("flalign", EnvKind::Align, ""),
    ("flalign*", EnvKind::Align, ""),
    ("alignat", EnvKind::AlignArg, ""),
    ("alignat*", EnvKind::AlignArg, ""),
    ("alignedat", EnvKind::AlignArg, ""),
    ("subarray", EnvKind::AlignArg, ""),
];

/// Relations `\not` negates into one symbol.
const NEGATIONS: [(&str, &str); 21] = [
    ("=", "eq.not"),
    ("<", "lt.not"),
    (">", "gt.not"),
    ("\\in", "in.not"),
    ("\\equiv", "equiv.not"),
    ("\\subset", "subset.not"),
    ("\\supset", "supset.not"),
    ("\\subseteq", "subset.eq.not"),
    ("\\supseteq", "supset.eq.not"),
    ("\\leq", "lt.eq.not"),
    ("\\le", "lt.eq.not"),
    ("\\geq", "gt.eq.not"),
    ("\\ge", "gt.eq.not"),
    ("\\sim", "tilde.not"),
    ("\\approx", "approx.not"),
    ("\\mid", "divides.not"),
    ("\\parallel", "parallel.not"),
    ("\\exists", "exists.not"),
    ("\\ni", "in.rev.not"),
    ("\\prec", "prec.not"),
    ("\\succ", "succ.not"),
];

impl<'s> Math<'s, '_> {
    /// Translates the commands with their own argument structure; false for commands
    /// the table handles.
    pub(super) fn special_command(&mut self, name: &str) -> bool {
        match name {
            "\\sqrt" => self.sqrt_command(),
            "\\left" => self.left_command(),
            "\\middle" => {
                self.skip_space();
                if self.peek() == Some(b'.') {
                    self.pos += 1;
                } else {
                    self.open("mid(");
                    self.delimiter();
                    self.close(1);
                }
            }
            "\\begin" => self.environment(),
            "\\operatorname" => self.operatorname(),
            "\\not" => self.not_command(),
            "\\pmod" => {
                self.atom("\\(");
                self.atom("mod");
                self.argument();
                self.atom("\\)");
            }
            "\\overset" | "\\stackrel" => self.attach_command("^("),
            "\\underset" => self.attach_command("_("),
            "\\boxed" | "\\phantom" => {
                let call = if name == "\\boxed" {
                    "#box(stroke: 0.5pt, inset: 2pt, $"
                } else {
                    "#hide($"
                };
                self.atom(call);
                self.space = false;
                self.argument();
                self.out.push_str("$)");
                self.space = true;
            }
            _ => return false,
        }
        true
    }

    fn operatorname(&mut self) {
        let limits = self.peek() == Some(b'*');
        if limits {
            self.pos += 1;
        }
        let (content, _) = self.braced_raw();
        self.open("op(");
        self.text_literal(content);
        if limits {
            self.out.push_str(", limits: #true");
        }
        self.close(1);
    }

    fn sqrt_command(&mut self) {
        self.skip_space();
        if self.peek() == Some(b'[') {
            self.pos += 1;
            self.open("root(");
            self.sequence(Stop::Bracket);
            if self.bad.is_some() {
                return;
            }
            self.pos += 1;
            self.next_arg();
        } else {
            self.open("sqrt(");
        }
        self.argument();
        self.close(1);
    }

    /// Writes `\left X ... \right Y` as `lr(X ... Y)`.
    fn left_command(&mut self) {
        self.open("lr(");
        self.skip_space();
        self.delimiter();
        self.sequence(Stop::Right);
        if self.bad.is_some() {
            return;
        }
        self.pos += "\\right".len();
        self.skip_space();
        self.delimiter();
        self.close(1);
    }

    /// Translates one delimiter token; "." is the empty delimiter.
    pub(super) fn delimiter(&mut self) {
        let Some(c) = self.peek() else {
            self.unsupported("a missing delimiter");
            return;
        };
        if c == b'\\' {
            let name = self.peek_command();
            self.pos += name.len();
            match DELIMITERS.iter().find(|d| d.0 == name) {
                Some(&(_, typst)) => self.atom(typst),
                None => self.unsupported(name),
            }
            return;
        }
        let token = self.char_here();
        self.pos += token.len();
        match c {
            b'.' => {}
            b'(' | b')' | b'[' | b']' | b'|' | b'/' => self.atom(ascii_math(c).unwrap_or("")),
            b'<' => self.atom("chevron.l"),
            b'>' => self.atom("chevron.r"),
            _ => self.unsupported(token),
        }
    }

    /// Translates `\begin{name} ... \end{name}`.
    fn environment(&mut self) {
        let (name, braced) = self.braced_raw();
        let Some(&(_, kind, typst)) = ENVIRONMENTS.iter().find(|e| braced && e.0 == name) else {
            self.unsupported("an unsupported \\begin environment");
            return;
        };
        let (saved_mode, saved_cell) = (self.mode, self.cell);
        let matrix = matches!(kind, EnvKind::Matrix | EnvKind::Array | EnvKind::Cases);
        if matches!(kind, EnvKind::Array | EnvKind::AlignArg) {
            self.braced_raw();
        }
        if matrix {
            self.open(typst);
            self.mode = if kind == EnvKind::Cases { Mode::Cases } else { Mode::Matrix };
            self.cell = self.out.len();
        } else {
            self.mode = Mode::Plain;
        }
        self.sequence(Stop::Env);
        if self.bad.is_none() {
            self.pos += "\\end".len();
            match self.braced_raw() {
                (end, true) if end == name => {
                    if matrix {
                        self.fill_cell();
                        self.close(1);
                    }
                }
                _ => self.unsupported("an \\end that does not match its \\begin"),
            }
        }
        self.mode = saved_mode;
        self.cell = saved_cell;
    }

    /// Negates the next relation, or strikes through anything else.
    fn not_command(&mut self) {
        self.skip_space();
        if self.peek().is_none() {
            self.unsupported("\\not without an argument");
            return;
        }
        let token =
            if self.peek() == Some(b'\\') { self.peek_command() } else { self.char_here() };
        if let Some(&(_, typst)) = NEGATIONS.iter().find(|n| n.0 == token) {
            self.pos += token.len();
            self.atom(typst);
            return;
        }
        self.open("cancel(");
        self.argument();
        self.close(1);
    }

    /// Writes `\overset{a}{b}` as `limits(b)^(a)`: b is translated first, then the
    /// parser returns to a.
    fn attach_command(&mut self, script: &str) {
        self.skip_space();
        let first = self.pos;
        if self.peek() == Some(b'\\') {
            self.pos += self.peek_command().len();
        } else {
            self.braced_raw();
        }
        if self.bad.is_some() {
            return;
        }
        self.open("limits(");
        self.argument();
        self.close(1);
        let after = self.pos;
        self.out.push_str(script);
        self.space = false;
        self.pos = first;
        self.argument();
        self.close(1);
        self.pos = after;
    }
}
