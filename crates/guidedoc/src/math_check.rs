//! The LaTeX math checker. It finds the first fault of a formula that no output can
//! typeset: a command or environment nobody knows, unbalanced braces, environments and
//! `\left` delimiters, a missing argument, and `&` or `\\` outside an alignment. The
//! engine runs it once on every formula a reader made, whatever the output is.
//!
//! The work is bounded: one pass over a formula of at most `MAX_FORMULA_BYTES`, a stack
//! of at most `MAX_OPEN` open groups, and one spelling search over the name tables.
//!
//! A command MathJax typesets is never reported here, even when the Typst adapter
//! cannot translate it. That narrower fault belongs to the renderer that needs Typst.

use crate::math_names::{COMMANDS, ENVIRONMENTS};
use crate::report::{Arg, Diagnostic, Message, Severity, arg_text, diagnostic};
use crate::types::{SourceSpan, Status};

/// The longest formula checked, in bytes. A longer one is left alone.
pub const MAX_FORMULA_BYTES: usize = 64 << 10;
/// The most groups, environments and `\left` delimiters open at once.
const MAX_OPEN: usize = 256;
/// The most names a document may define with `\newcommand` and its relatives.
const MAX_DEFINED: usize = 512;
/// The longest name a spelling suggestion is searched for.
const MAX_NAME: usize = 40;

const fn fault(code: &'static str, title: &'static str, message: &'static str) -> Message {
    Message {
        code,
        severity: Severity::Warning,
        status: Status::InvalidInput,
        title,
        message,
        hint: "{1}",
    }
}

/// A command no LaTeX math vocabulary we know has.
pub const MATH_COMMAND_UNKNOWN: Message = fault(
    "math.command.unknown",
    "UNKNOWN MATH COMMAND",
    "I do not know the LaTeX math command `{0}`. A page shows it as an error or as text.",
);

/// An environment no LaTeX math vocabulary we know has.
pub const MATH_ENVIRONMENT_UNKNOWN: Message = fault(
    "math.environment.unknown",
    "UNKNOWN MATH ENVIRONMENT",
    "I do not know the LaTeX math environment `{0}`.",
);

/// A `{` without its `}`, or a `}` without its `{`.
pub const MATH_BRACE: Message =
    fault("math.brace.unbalanced", "UNBALANCED BRACE IN MATH", "This formula has {0}.");

/// A `\begin` without its `\end`, or an `\end` without its `\begin`.
pub const MATH_ENVIRONMENT_UNCLOSED: Message =
    fault("math.environment.unclosed", "MATH ENVIRONMENT NOT CLOSED", "This formula has {0}.");

/// An `\end` that names another environment than the open one.
pub const MATH_ENVIRONMENT_MISMATCH: Message =
    fault("math.environment.mismatch", "MATH ENVIRONMENT CLOSED BY ANOTHER NAME", "{0}");

/// A `\left` without its `\right`, or a `\right` without its `\left`.
pub const MATH_DELIMITER: Message =
    fault("math.delimiter.unpaired", "UNPAIRED MATH DELIMITER", "This formula has {0}.");

/// A command followed by fewer arguments than it takes.
pub const MATH_ARGUMENT: Message = fault(
    "math.argument.missing",
    "MISSING MATH ARGUMENT",
    "The command `{0}` is followed by fewer arguments than it takes.",
);

/// `&` or `\\` where nothing aligns.
pub const MATH_ALIGNMENT: Message = fault(
    "math.alignment.outside",
    "ALIGNMENT OUTSIDE AN ENVIRONMENT",
    "`{0}` stands outside an alignment, where it has no meaning.",
);

/// How a formula is checked. `Default` checks inline math with no host macros.
#[derive(Debug, Clone, Copy, Default)]
pub struct MathCheck<'a> {
    /// `&` and `\\` are allowed outside an environment, because the renderer wraps
    /// the formula in an alignment (displayed math with line breaks).
    pub alignment: bool,
    /// `\\` alone is allowed outside an environment (displayed math).
    pub line_breaks: bool,
    /// Commands the host defines, without the backslash (MathJax `macros`).
    pub macros: &'a [&'a str],
}

/// The first fault of a formula.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathFault {
    /// What is wrong; its hint template is `{1}`.
    pub message: Message,
    /// The offending text starts at this byte of the formula.
    pub start: usize,
    /// One past its last byte.
    pub end: usize,
    /// The `{0}` argument: the command, or a description.
    pub subject: String,
    /// The hint, with the suggested correction when there is one.
    pub hint: String,
}

impl MathFault {
    /// The fault as a diagnostic at `span`.
    pub fn diagnostic(&self, span: SourceSpan) -> Diagnostic {
        let subject = arg_text(&self.subject);
        diagnostic(self.message, span, subject, Arg::Text(self.hint.clone().into()))
    }
}

/// Checks the formulas of one document. It remembers the commands and environments
/// the formulas define, since a definition holds for the rest of the page.
#[derive(Debug, Default)]
pub struct MathChecker {
    commands: Vec<Box<str>>,
    environments: Vec<Box<str>>,
}

/// Checks one formula alone. See [`MathChecker::check`].
pub fn check_math(latex: &str, options: &MathCheck<'_>) -> Option<MathFault> {
    MathChecker::default().check(latex, options)
}

/// Tells whether `name` (without the backslash) is a LaTeX math command we know.
pub fn is_math_command(name: &str) -> bool {
    COMMANDS.binary_search(&name).is_ok()
}

/// Tells whether `name` is a LaTeX math environment we know.
pub fn is_math_environment(name: &str) -> bool {
    ENVIRONMENTS.binary_search(&name).is_ok()
}

/// The known command closest to the misspelled `name`, if one is close.
pub fn suggest_math_command(name: &str) -> Option<&'static str> {
    closest(name, &COMMANDS)
}

/// The known environment closest to the misspelled `name`, if one is close.
pub fn suggest_math_environment(name: &str) -> Option<&'static str> {
    closest(name, &ENVIRONMENTS)
}

impl MathChecker {
    /// Makes a checker that knows no definition yet.
    pub fn new() -> MathChecker {
        MathChecker::default()
    }

    /// Returns the first fault of `latex`, or None when it has none or is longer than
    /// [`MAX_FORMULA_BYTES`]. Definitions in `latex` are remembered.
    pub fn check(&mut self, latex: &str, options: &MathCheck<'_>) -> Option<MathFault> {
        if latex.len() > MAX_FORMULA_BYTES {
            return None;
        }
        let mut scan = Scan { src: latex, pos: 0, open: Vec::new(), options, known: self };
        scan.run()
    }

    fn define(list: &mut Vec<Box<str>>, name: &str) {
        if list.len() < MAX_DEFINED && !list.iter().any(|n| &**n == name) {
            list.push(name.into());
        }
    }
}

/// What is open at a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Open<'a> {
    Brace,
    Left,
    Environment(&'a str),
}

struct Scan<'a, 'c> {
    src: &'a str,
    pos: usize,
    /// The open groups with the byte each began at.
    open: Vec<(Open<'a>, usize)>,
    options: &'c MathCheck<'c>,
    known: &'c mut MathChecker,
}

/// The commands that define a command with the next name.
const DEFINERS: [&str; 9] = [
    "DeclareMathOperator",
    "DeclarePairedDelimiter",
    "def",
    "edef",
    "gdef",
    "let",
    "newcommand",
    "providecommand",
    "renewcommand",
];

/// How many arguments a command needs; only commands whose arguments we are sure of.
fn arity(name: &str) -> usize {
    const TWO: [&str; 11] = [
        "binom", "cfrac", "dbinom", "dfrac", "frac", "overset", "sfrac", "stackrel", "tbinom",
        "tfrac", "underset",
    ];
    const ONE: [&str; 38] = [
        "acute",
        "bar",
        "bm",
        "boldsymbol",
        "boxed",
        "breve",
        "check",
        "ddot",
        "dot",
        "grave",
        "hat",
        "mathbb",
        "mathbf",
        "mathcal",
        "mathfrak",
        "mathit",
        "mathring",
        "mathrm",
        "mathscr",
        "mathsf",
        "mathtt",
        "mbox",
        "operatorname",
        "overbrace",
        "overleftarrow",
        "overline",
        "overrightarrow",
        "sqrt",
        "text",
        "textbf",
        "textit",
        "textrm",
        "tilde",
        "underbrace",
        "underline",
        "vec",
        "widehat",
        "widetilde",
    ];
    if TWO.contains(&name) { 2 } else { usize::from(ONE.contains(&name)) }
}

impl<'a> Scan<'a, '_> {
    fn run(&mut self) -> Option<MathFault> {
        // A definition's body is a fragment: its braces balance, its environments and
        // delimiters need not. Such a formula is checked for names only.
        let defines = self.src.contains("\\def")
            || self.src.contains("command")
            || self.src.contains("environment")
            || self.src.contains("\\let")
            || self.src.contains("\\Declare");
        let bytes = self.src.as_bytes();
        while self.pos < bytes.len() {
            let at = self.pos;
            let found = match bytes[at] {
                b'%' => {
                    self.pos = self.src[at..].find('\n').map_or(bytes.len(), |n| at + n);
                    None
                }
                b'{' => {
                    self.pos += 1;
                    self.push(Open::Brace, at)
                }
                b'}' => {
                    self.pos += 1;
                    self.close_brace(at, defines)
                }
                b'&' => {
                    self.pos += 1;
                    self.alignment(at, "&", self.options.alignment)
                }
                b'\\' => self.command(at, defines),
                _ => {
                    self.pos += 1;
                    None
                }
            };
            match found {
                Some(Found::Fault(f)) => return Some(f),
                Some(Found::Stop) => return None,
                None => {}
            }
        }
        if defines {
            return None;
        }
        self.open.last().map(|&(open, at)| self.unclosed(open, at))
    }

    fn push(&mut self, open: Open<'a>, at: usize) -> Option<Found> {
        if self.open.len() >= MAX_OPEN {
            return Some(Found::Stop);
        }
        self.open.push((open, at));
        None
    }

    /// The fault of something still open where it must be closed.
    fn unclosed(&self, open: Open<'a>, at: usize) -> MathFault {
        match open {
            Open::Brace => self.fault(
                MATH_BRACE,
                at..at + 1,
                "a `{` that no `}` closes",
                "Add the missing `}`, or write `\\{` for a brace that is shown.".into(),
            ),
            Open::Left => self.fault(
                MATH_DELIMITER,
                at..at + 5,
                "a `\\left` without its `\\right`",
                "Add `\\right` with a delimiter, or `\\right.` for none.".into(),
            ),
            Open::Environment(name) => self.fault(
                MATH_ENVIRONMENT_UNCLOSED,
                at..at + 6,
                &format!("`\\begin{{{name}}}` without `\\end{{{name}}}`"),
                format!("Add `\\end{{{name}}}` where the environment ends."),
            ),
        }
    }

    fn close_brace(&mut self, at: usize, defines: bool) -> Option<Found> {
        match self.open.last() {
            Some(&(Open::Brace, _)) => {
                self.open.pop();
                None
            }
            _ if defines => None,
            Some(&(open, begun)) => Some(Found::Fault(self.unclosed(open, begun))),
            None => Some(Found::Fault(self.fault(
                MATH_BRACE,
                at..at + 1,
                "a `}` that closes nothing",
                "Remove it, add the missing `{`, or write `\\}` for a brace that is shown.".into(),
            ))),
        }
    }

    fn alignment(&self, at: usize, what: &str, allowed: bool) -> Option<Found> {
        if allowed || !self.open.is_empty() {
            return None;
        }
        let hint = if what == "&" {
            "Put the formula in an environment such as `aligned` or `matrix`, or write `\\&` \
             for an ampersand."
        } else {
            "Put the lines in an environment such as `aligned` or `gathered`, or use \
             displayed math."
        };
        Some(Found::Fault(self.fault(MATH_ALIGNMENT, at..at + what.len(), what, hint.into())))
    }

    /// Reads the command at `at` (a backslash) and checks it.
    fn command(&mut self, at: usize, defines: bool) -> Option<Found> {
        let rest = &self.src[at + 1..];
        let letters = rest.bytes().take_while(u8::is_ascii_alphabetic).count();
        if letters == 0 {
            let symbol = rest.chars().next().map_or(0, char::len_utf8);
            self.pos = at + 1 + symbol;
            let breaks = self.options.alignment || self.options.line_breaks;
            return if rest.starts_with('\\') { self.alignment(at, "\\\\", breaks) } else { None };
        }
        let name = &rest[..letters];
        self.pos = at + 1 + letters;
        match name {
            "begin" | "end" => return self.environment(at, name == "begin", defines),
            "newenvironment" | "renewenvironment" => {
                if let Some(env) = self.braced_name() {
                    MathChecker::define(&mut self.known.environments, env);
                }
                return None;
            }
            "left" if !defines => return self.push(Open::Left, at),
            "right" if !defines => return self.right(at),
            _ => {}
        }
        if DEFINERS.contains(&name) {
            self.define_next();
            return None;
        }
        if !self.is_known(name) {
            return Some(Found::Fault(self.unknown_command(at, name)));
        }
        let needs = arity(name);
        if needs > 0 && !defines && !self.has_arguments(needs) {
            let hint = match needs {
                1 => format!("Write its argument after it, as in `\\{name}{{x}}`."),
                _ => format!("Write both arguments, as in `\\{name}{{a}}{{b}}`."),
            };
            let end = at + 1 + letters;
            let subject = format!("\\{name}");
            return Some(Found::Fault(self.fault(MATH_ARGUMENT, at..end, &subject, hint)));
        }
        None
    }

    fn is_known(&self, name: &str) -> bool {
        is_math_command(name)
            || self.options.macros.contains(&name)
            || self.known.commands.iter().any(|n| &**n == name)
    }

    fn unknown_command(&self, at: usize, name: &str) -> MathFault {
        let hint = match suggest_math_command(name) {
            Some(known) => format!("Did you mean `\\{known}`?"),
            None => "Check the spelling. If the page defines this command, define it with \
                     `\\newcommand` in the formula or name it in the MathJax `macros` setting."
                .to_string(),
        };
        self.fault(MATH_COMMAND_UNKNOWN, at..at + 1 + name.len(), &format!("\\{name}"), hint)
    }

    /// Remembers the command a definition names: `\newcommand{\x}`, `\def\x`.
    fn define_next(&mut self) {
        let rest = &self.src[self.pos..];
        let skipped = rest.len() - rest.trim_start_matches([' ', '*', '{', '\n', '\t']).len();
        let rest = &rest[skipped..];
        let Some(after) = rest.strip_prefix('\\') else { return };
        let letters = after.bytes().take_while(u8::is_ascii_alphabetic).count();
        if letters > 0 {
            MathChecker::define(&mut self.known.commands, &after[..letters]);
            self.pos += skipped + 1 + letters;
        }
    }

    /// Reads `{name}` at the position and moves past it.
    fn braced_name(&mut self) -> Option<&'a str> {
        let rest = &self.src[self.pos..];
        let inner = rest.trim_start().strip_prefix('{')?;
        let end = inner.find('}')?;
        let name = inner[..end].trim();
        self.pos = self.src.len() - inner.len() + end + 1;
        Some(name)
    }

    fn right(&mut self, at: usize) -> Option<Found> {
        match self.open.last() {
            Some(&(Open::Left, _)) => {
                self.open.pop();
                None
            }
            Some(&(Open::Brace, begun)) => Some(Found::Fault(self.unclosed(Open::Brace, begun))),
            _ => Some(Found::Fault(self.fault(
                MATH_DELIMITER,
                at..at + 6,
                "a `\\right` without its `\\left`",
                "Add `\\left` with a delimiter before it, or `\\left.` for none.".into(),
            ))),
        }
    }

    fn environment(&mut self, at: usize, begin: bool, defines: bool) -> Option<Found> {
        let name = self.braced_name()?;
        let known =
            is_math_environment(name) || self.known.environments.iter().any(|n| &**n == name);
        if !known {
            let hint = match suggest_math_environment(name) {
                Some(known) => format!("Did you mean `{known}`?"),
                None => "Check the spelling, or use an environment such as `aligned`, \
                         `matrix` or `cases`."
                    .to_string(),
            };
            let range = at..self.pos;
            return Some(Found::Fault(self.fault(MATH_ENVIRONMENT_UNKNOWN, range, name, hint)));
        }
        if defines {
            return None;
        }
        if begin {
            return self.push(Open::Environment(name), at);
        }
        let fault = match self.open.last() {
            Some(&(Open::Environment(open), _)) if open == name => {
                self.open.pop();
                return None;
            }
            Some(&(Open::Environment(open), _)) => self.fault(
                MATH_ENVIRONMENT_MISMATCH,
                at..self.pos,
                &format!("`\\end{{{name}}}` closes `\\begin{{{open}}}`."),
                format!("Write `\\end{{{open}}}`, or begin the environment as `{name}`."),
            ),
            Some(&(open, begun)) => self.unclosed(open, begun),
            None => self.fault(
                MATH_ENVIRONMENT_UNCLOSED,
                at..self.pos,
                &format!("`\\end{{{name}}}` without `\\begin{{{name}}}`"),
                format!("Add `\\begin{{{name}}}` before it, or remove this `\\end`."),
            ),
        };
        Some(Found::Fault(fault))
    }

    /// Tells whether `count` arguments follow the position, without moving.
    fn has_arguments(&self, count: usize) -> bool {
        let bytes = self.src.as_bytes();
        let mut at = self.pos;
        let mut seen = 0;
        while seen < count {
            while at < bytes.len() && (bytes[at].is_ascii_whitespace() || bytes[at] == b'*') {
                at += 1;
            }
            let rest = &self.src[at..];
            let ends = rest.is_empty()
                || rest.starts_with(['}', '&'])
                || rest.starts_with("\\\\")
                || rest.starts_with("\\end")
                || rest.starts_with("\\right");
            if ends {
                return false;
            }
            match bytes[at] {
                // An optional argument, as in `\sqrt[3]{x}`, does not count.
                b'[' if seen == 0 => match rest.find(']') {
                    Some(end) => at += end + 1,
                    None => return true,
                },
                b'{' => match matching_brace(rest) {
                    Some(end) => {
                        at += end + 1;
                        seen += 1;
                    }
                    // The unclosed brace is the fault to report, not the argument.
                    None => return true,
                },
                b'\\' => {
                    let letters =
                        rest[1..].bytes().take_while(u8::is_ascii_alphabetic).count().max(1);
                    at += 1 + letters;
                    seen += 1;
                }
                _ => {
                    at += rest.chars().next().map_or(1, char::len_utf8);
                    seen += 1;
                }
            }
        }
        true
    }

    fn fault(
        &self,
        message: Message,
        range: std::ops::Range<usize>,
        subject: &str,
        hint: String,
    ) -> MathFault {
        let end = range.end.min(self.src.len());
        MathFault { message, start: range.start, end, subject: subject.to_string(), hint }
    }
}

/// What a step found: a fault, or a bound that ends the check without one.
enum Found {
    Fault(MathFault),
    Stop,
}

/// The offset of the `}` that closes the `{` `text` starts with.
fn matching_brace(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut escaped = false;
    for (at, byte) in text.bytes().enumerate() {
        match byte {
            _ if escaped => escaped = false,
            b'\\' => escaped = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(at);
                }
            }
            _ => {}
        }
    }
    None
}

/// The name of `names` closest to `name`: within two edits (one for a short name), or
/// else the longest known name `name` begins with, as `frac` for `fraction`.
fn closest(name: &str, names: &[&'static str]) -> Option<&'static str> {
    if name.is_empty() || name.len() > MAX_NAME {
        return None;
    }
    let limit = if name.len() <= 3 { 1 } else { 2 };
    // Among equally close names, the one that shares the longest beginning wins.
    let shared = |c: &str| c.bytes().zip(name.bytes()).take_while(|(a, b)| a == b).count();
    let mut best: Option<(usize, usize, &'static str)> = None;
    for &candidate in names {
        if candidate.len().abs_diff(name.len()) > limit {
            continue;
        }
        let distance = edit_distance(name.as_bytes(), candidate.as_bytes(), limit);
        let common = shared(candidate);
        let better = best.is_none_or(|(d, c, _)| distance < d || (distance == d && common > c));
        if distance <= limit && better {
            best = Some((distance, common, candidate));
        }
    }
    if let Some((_, _, found)) = best {
        return Some(found);
    }
    let begins = |c: &&&'static str| c.len() >= 4 && name.starts_with(**c);
    names.iter().filter(begins).max_by_key(|c| c.len()).copied()
}

/// The edit distance between two short names, counting an exchange of two neighbours
/// as one edit; any value above `limit` means "more than `limit`".
fn edit_distance(a: &[u8], b: &[u8], limit: usize) -> usize {
    let width = b.len() + 1;
    let mut rows = vec![0usize; 3 * width];
    for (j, cell) in rows[width..2 * width].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=a.len() {
        let (before, rest) = rows.split_at_mut(width);
        let (previous, current) = rest.split_at_mut(width);
        current[0] = i;
        let mut least = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (previous[j] + 1).min(current[j - 1] + 1).min(previous[j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(before[j - 2] + 1);
            }
            current[j] = best;
            least = least.min(best);
        }
        if least > limit {
            return limit + 1;
        }
        rows.copy_within(width..2 * width, 0);
        rows.copy_within(2 * width.., width);
    }
    rows[width + b.len()]
}

/// The most math findings one document gets; a page of broken formulas needs no more.
const MAX_FINDINGS: usize = 64;

/// Checks every LaTeX formula of the document a reader built and reports the first
/// fault of each as a warning, with the caret on the offending text.
pub(crate) fn check_formulas(ctx: &mut crate::adapter::ReadContext<'_>) {
    let mut checker = MathChecker::new();
    let mut findings: Vec<Diagnostic> = Vec::new();
    let doc = ctx.document();
    for index in 0..doc.node_count() {
        let id = crate::types::NodeId::from_index(index);
        let display = match doc.kind(id) {
            crate::types::NodeKind::Math => false,
            crate::types::NodeKind::MathBlock => true,
            _ => continue,
        };
        if crate::options::is_mathml(doc, id) {
            continue;
        }
        let latex = doc.node_text(id);
        // A displayed formula with line breaks is wrapped in an alignment, as Sphinx
        // and Docutils do, unless it is marked nowrap.
        let wrapped = display && doc.custom_attribute(id, "nowrap").is_none();
        let options = MathCheck {
            alignment: wrapped && latex.contains("\\\\"),
            line_breaks: display,
            macros: ctx.options.math_macros,
        };
        let Some(fault) = checker.check(latex, &options) else { continue };
        let span = doc.node(id).span;
        findings.push(fault.diagnostic(fault_span(ctx.source_text(span), span, latex, &fault)));
        if findings.len() >= MAX_FINDINGS {
            break;
        }
    }
    for finding in findings {
        ctx.b.reports().report(finding);
    }
}

/// Where the fault's text stands in the source. The formula's text is part of the
/// node's source, perhaps indented, so we find the same occurrence counted from the
/// end. When it is not there, the whole node is the place.
fn fault_span(source: &str, node: SourceSpan, latex: &str, fault: &MathFault) -> SourceSpan {
    let Some(token) = latex.get(fault.start..fault.end).filter(|t| !t.is_empty()) else {
        return node;
    };
    let after = latex[fault.end..].matches(token).count();
    match source.rmatch_indices(token).nth(after) {
        Some((at, _)) => {
            let start = node.start + u32::try_from(at).unwrap_or(0);
            let end = start + u32::try_from(token.len()).unwrap_or(0);
            SourceSpan::new(node.source, start, end.min(node.end))
        }
        None => node,
    }
}

#[cfg(test)]
mod tests;
