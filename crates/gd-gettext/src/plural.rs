//! Plural-Forms expressions: the C subset GNU gettext accepts.
//!
//! The variable `n`, unsigned decimal numbers, parentheses, `! * / % + - < > <= >= == !=
//! && || ?:`. Arithmetic is unsigned 64-bit, as in the C library; division or remainder
//! by zero yields 0 instead of trapping. A rule compiles once into at most
//! [`MAX_PLURAL_NODES`] nodes and [`MAX_PLURAL_DEPTH`] levels, so a hostile header can
//! neither recurse nor allocate without bound.

use crate::error::{Error, ErrorKind};

/// The most nodes a compiled plural expression may have.
pub const MAX_PLURAL_NODES: usize = 128;
/// The deepest a plural expression may nest (parser levels).
pub const MAX_PLURAL_DEPTH: usize = 100;
/// The most plural forms a catalog may have.
pub const MAX_NPLURALS: usize = 32;

/// A node number in a [`PluralRule`]; `NONE` marks a failed parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NodeId(u16);

const NONE: NodeId = NodeId(u16::MAX);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Number(u64),
    N,
    Not(NodeId),
    Binary(BinOp, NodeId, NodeId),
    Choose(NodeId, NodeId, NodeId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinOp {
    Mul,
    Div,
    Mod,
    Add,
    Sub,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    Equal,
    NotEqual,
    And,
    Or,
}

/// A compiled Plural-Forms header: `nplurals` and the plural expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluralRule {
    /// The number of plural forms, 1 to [`MAX_NPLURALS`].
    pub nplurals: usize,
    nodes: Vec<Op>,
    root: NodeId,
}

impl Default for PluralRule {
    /// gettext's rule without a header: two forms, `plural = n != 1`.
    fn default() -> PluralRule {
        let nodes = vec![Op::N, Op::Number(1), Op::Binary(BinOp::NotEqual, NodeId(0), NodeId(1))];
        PluralRule { nplurals: 2, nodes, root: NodeId(2) }
    }
}

/// Returns the msgstr index for `n`; a result out of range selects form 0, as the C
/// library does.
pub fn plural_index(rule: &PluralRule, n: u64) -> usize {
    if rule.nodes.is_empty() {
        return usize::from(n != 1);
    }
    let v = eval(rule, rule.root, n, 0);
    if v >= rule.nplurals as u64 { 0 } else { v as usize }
}

fn eval(r: &PluralRule, id: NodeId, n: u64, depth: usize) -> u64 {
    let Some(&op) = r.nodes.get(usize::from(id.0)) else { return 0 };
    if depth > MAX_PLURAL_NODES {
        return 0;
    }
    let d = depth + 1;
    let truth = |v: u64| u64::from(v != 0);
    match op {
        Op::Number(v) => v,
        Op::N => n,
        Op::Not(a) => u64::from(eval(r, a, n, d) == 0),
        Op::Choose(c, a, b) => eval(r, if eval(r, c, n, d) != 0 { a } else { b }, n, d),
        Op::Binary(BinOp::And, a, b) => {
            if eval(r, a, n, d) == 0 {
                0
            } else {
                truth(eval(r, b, n, d))
            }
        }
        Op::Binary(BinOp::Or, a, b) => {
            if eval(r, a, n, d) != 0 {
                1
            } else {
                truth(eval(r, b, n, d))
            }
        }
        Op::Binary(op, a, b) => arithmetic(op, eval(r, a, n, d), eval(r, b, n, d)),
    }
}

fn arithmetic(op: BinOp, x: u64, y: u64) -> u64 {
    match op {
        BinOp::Mul => x.wrapping_mul(y),
        BinOp::Div => x.checked_div(y).unwrap_or(0),
        BinOp::Mod => x.checked_rem(y).unwrap_or(0),
        BinOp::Add => x.wrapping_add(y),
        BinOp::Sub => x.wrapping_sub(y),
        BinOp::Less => u64::from(x < y),
        BinOp::Greater => u64::from(x > y),
        BinOp::LessEqual => u64::from(x <= y),
        BinOp::GreaterEqual => u64::from(x >= y),
        BinOp::Equal => u64::from(x == y),
        BinOp::NotEqual => u64::from(x != y),
        BinOp::And => u64::from(x != 0 && y != 0),
        BinOp::Or => u64::from(x != 0 || y != 0),
    }
}

/// Where and why an expression was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluralProblem {
    /// What is wrong, in lower case without a full stop.
    pub message: &'static str,
    /// The 1-based byte column in the expression.
    pub column: usize,
}

/// Compiles a plural expression for `nplurals` forms. Fails with the first problem found:
/// a syntax error, nplurals out of 1 to 32, or an expression past the node or depth
/// bounds.
pub fn compile_plural(expr: &str, nplurals: usize) -> Result<PluralRule, PluralProblem> {
    if !(1..=MAX_NPLURALS).contains(&nplurals) {
        return Err(PluralProblem { message: "nplurals must be between 1 and 32", column: 1 });
    }
    let mut p = Parser { src: expr.as_bytes(), at: 0, nodes: Vec::new(), problem: None };
    let root = p.choose(0);
    p.skip_space();
    if p.problem.is_none() && p.at < p.src.len() {
        p.fail("unexpected text after the expression");
    }
    match p.problem {
        Some(problem) => Err(problem),
        None => Ok(PluralRule { nplurals, nodes: p.nodes, root }),
    }
}

struct Parser<'a> {
    src: &'a [u8],
    at: usize,
    nodes: Vec<Op>,
    problem: Option<PluralProblem>,
}

/// Binary operators from the loosest to the tightest binding, as in C.
const LEVELS: [&[(&str, BinOp)]; 6] = [
    &[("||", BinOp::Or)],
    &[("&&", BinOp::And)],
    &[("==", BinOp::Equal), ("!=", BinOp::NotEqual)],
    &[
        ("<=", BinOp::LessEqual),
        (">=", BinOp::GreaterEqual),
        ("<", BinOp::Less),
        (">", BinOp::Greater),
    ],
    &[("+", BinOp::Add), ("-", BinOp::Sub)],
    &[("*", BinOp::Mul), ("/", BinOp::Div), ("%", BinOp::Mod)],
];

impl Parser<'_> {
    fn fail(&mut self, message: &'static str) -> NodeId {
        if self.problem.is_none() {
            self.problem = Some(PluralProblem { message, column: self.at + 1 });
        }
        NONE
    }

    fn add(&mut self, op: Op) -> NodeId {
        if self.problem.is_some() {
            return NONE;
        }
        if self.nodes.len() >= MAX_PLURAL_NODES {
            return self.fail("the expression is too long");
        }
        self.nodes.push(op);
        NodeId((self.nodes.len() - 1) as u16)
    }

    fn skip_space(&mut self) {
        while matches!(self.src.get(self.at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    /// Consumes an operator token when it is next; `<` does not match `<=`.
    fn accept(&mut self, token: &str) -> bool {
        self.skip_space();
        if !self.src[self.at..].starts_with(token.as_bytes()) {
            return false;
        }
        let next = self.at + token.len();
        if let (&[c], Some(&d)) = (token.as_bytes(), self.src.get(next)) {
            if matches!(c, b'<' | b'>' | b'!' | b'=') && d == b'=' {
                return false;
            }
            if (c == b'&' && d == b'&') || (c == b'|' && d == b'|') {
                return false;
            }
        }
        self.at = next;
        true
    }

    fn choose(&mut self, depth: usize) -> NodeId {
        if depth > MAX_PLURAL_DEPTH {
            return self.fail("the expression nests too deeply");
        }
        let cond = self.binary(0, depth + 1);
        if !self.accept("?") {
            return cond;
        }
        let yes = self.choose(depth + 1);
        if !self.accept(":") {
            return self.fail("expected ':' of a ?: choice");
        }
        let no = self.choose(depth + 1);
        self.add(Op::Choose(cond, yes, no))
    }

    fn binary(&mut self, level: usize, depth: usize) -> NodeId {
        if depth > MAX_PLURAL_DEPTH {
            return self.fail("the expression nests too deeply");
        }
        let Some(ops) = LEVELS.get(level) else { return self.unary(depth + 1) };
        let mut left = self.binary(level + 1, depth + 1);
        'outer: while self.problem.is_none() {
            for &(token, op) in ops.iter() {
                if self.accept(token) {
                    let right = self.binary(level + 1, depth + 1);
                    left = self.add(Op::Binary(op, left, right));
                    continue 'outer;
                }
            }
            break;
        }
        left
    }

    fn unary(&mut self, depth: usize) -> NodeId {
        if depth > MAX_PLURAL_DEPTH {
            return self.fail("the expression nests too deeply");
        }
        self.skip_space();
        if self.accept("!") {
            let operand = self.unary(depth + 1);
            return self.add(Op::Not(operand));
        }
        if self.accept("(") {
            let inner = self.choose(depth + 1);
            if !self.accept(")") {
                return self.fail("expected ')'");
            }
            return inner;
        }
        match self.src.get(self.at) {
            Some(b'n') if !self.src.get(self.at + 1).is_some_and(|&c| is_word(c)) => {
                self.at += 1;
                self.add(Op::N)
            }
            Some(c) if c.is_ascii_digit() => self.number(),
            None => self.fail("the expression ends too early"),
            Some(_) => self.fail("expected n, a number, '!' or '('"),
        }
    }

    fn number(&mut self) -> NodeId {
        let mut value: u64 = 0;
        while let Some(&c) = self.src.get(self.at).filter(|c| c.is_ascii_digit()) {
            let digit = u64::from(c - b'0');
            match value.checked_mul(10).and_then(|v| v.checked_add(digit)) {
                Some(v) => value = v,
                None => return self.fail("the number is too large"),
            }
            self.at += 1;
        }
        self.add(Op::Number(value))
    }
}

fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Compiles a Plural-Forms header value such as `nplurals=2; plural=(n != 1);`. Fails
/// with an error of kind [`ErrorKind::Plural`] whose excerpt is the value and whose
/// column points at the problem; the caller fills in the file and line.
pub fn parse_plural_forms(value: &str) -> Result<PluralRule, Error> {
    let mut nplurals = None;
    let (mut expr, mut expr_at) = ("", 0);
    let mut offset = 0;
    for part in value.split(';') {
        let start = offset;
        offset += part.len() + 1;
        let Some(eq) = part.find('=') else { continue };
        let val = &part[eq + 1..];
        match part[..eq].trim() {
            "nplurals" => match parse_small_int(val.trim()) {
                Some(n) => nplurals = Some(n),
                None => {
                    let msg = "nplurals is not a number from 1 to 32.";
                    return Err(plural_error(value, start + eq + 2, msg));
                }
            },
            "plural" => (expr, expr_at) = (val, start + eq + 1),
            _ => {}
        }
    }
    let Some(nplurals) = nplurals.filter(|_| !expr.trim().is_empty()) else {
        return Err(plural_error(value, 0, "The header needs both nplurals=N and plural=EXPR."));
    };
    compile_plural(expr, nplurals).map_err(|problem| {
        let msg = format!("The plural expression is not valid: {}.", problem.message);
        plural_error(value, expr_at + problem.column, &msg)
    })
}

fn parse_small_int(s: &str) -> Option<usize> {
    if s.is_empty() || s.len() > 3 || !s.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse().ok().filter(|n| (1..=MAX_NPLURALS).contains(n))
}

fn plural_error(value: &str, column: usize, message: &str) -> Error {
    let hint = "Use a rule from the gettext manual, e.g. \"nplurals=2; plural=(n != 1);\".";
    Error::new(ErrorKind::Plural, "", message, hint).at(0, column, value)
}

/// Returns a Plural-Forms header value for a new catalog of a language: the rules the GNU
/// gettext manual lists for common languages, and the germanic rule (`n != 1`) for
/// others. `pt_BR` and `pt-BR` find `pt_BR`, then `pt`.
pub fn plural_forms_for(language: &str) -> &'static str {
    for candidate in crate::language_candidates(language) {
        for (languages, forms) in PLURAL_FORMS {
            if languages.contains(&candidate.as_str()) {
                return forms;
            }
        }
    }
    "nplurals=2; plural=(n != 1);"
}

const PLURAL_FORMS: [(&[&str], &str); 6] = [
    (
        &["ja", "zh", "zh_CN", "zh_TW", "ko", "vi", "th", "id", "ms", "tr_TR"],
        "nplurals=1; plural=0;",
    ),
    (&["fr", "pt_BR", "tr"], "nplurals=2; plural=(n > 1);"),
    (
        &["ru", "uk", "be", "sr", "hr", "bs"],
        "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && \
         (n%100<10 || n%100>=20) ? 1 : 2);",
    ),
    (
        &["pl"],
        "nplurals=3; plural=(n==1 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);",
    ),
    (&["cs", "sk"], "nplurals=3; plural=((n==1) ? 0 : (n>=2 && n<=4) ? 1 : 2);"),
    (
        &["ar"],
        "nplurals=6; plural=(n==0 ? 0 : n==1 ? 1 : n==2 ? 2 : n%100>=3 && n%100<=10 ? 3 : \
         n%100>=11 ? 4 : 5);",
    ),
];
