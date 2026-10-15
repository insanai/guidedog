//! Directives: recognizing the marker, splitting the block into argument, options, and
//! content, checking them against the directive's rules, and dispatching.

use std::borrow::Cow;

use guidedoc::{
    Arg, AttributeKey, Fail, NodeId, NodeKind, SourceSpan, arg_int, arg_span, arg_text,
};

use crate::extension::DirectiveHandler;
use crate::fields::field_marker;
use crate::lines::{Line, dedent, indent_of, indented_block, join_lines, trim_blank_end};
use crate::lines::{trim_blank_start, trim_space};
use crate::messages::*;
use crate::options::option_problem;
use crate::parser::Parser;
use crate::specs::{ArgRule, ContentRule, DirKind, DirSpec, OptSpec, find_directive};

/// Directives take at most this many options.
const MAX_OPTIONS: usize = 32;

/// One option a directive was given: its specification, its value lines (the rest of
/// the marker line, then the continuation lines), and where it was written.
#[derive(Debug, Clone)]
pub struct DirOption<'d> {
    /// The option's specification.
    pub spec: OptSpec,
    /// The value lines, trimmed.
    pub lines: Vec<Line<'d>>,
    /// The option's lines in the source.
    pub span: SourceSpan,
}

/// What runs a directive: a standard specification or a dialect's handler.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Runner<'d> {
    Standard(&'static DirSpec),
    Handler(DirectiveHandler<'d>),
}

/// A directive split into its parts and checked against its rules. Handlers read it;
/// the lines are views of the current source (`Parser::src`).
#[derive(Debug, Clone)]
pub struct Directive<'d> {
    /// The directive name as written.
    pub name: &'d str,
    /// The whole directive block.
    pub span: SourceSpan,
    /// The name on the marker line.
    pub name_span: SourceSpan,
    /// The argument lines, trimmed; empty when there is no argument.
    pub arg: Vec<Line<'d>>,
    /// The argument lines with their relative indentation.
    pub arg_block: Vec<Line<'d>>,
    /// The options given, in order.
    pub options: Vec<DirOption<'d>>,
    /// The content lines, without leading and trailing blank lines.
    pub content: Vec<Line<'d>>,
    /// The directive stands where sections are allowed.
    pub sections: bool,
    /// The directive is inside a substitution definition.
    pub subst: bool,
    /// The name of the substitution definition it is in, as written.
    pub subst_name: String,
    pub(crate) runner: Runner<'d>,
}

impl<'d> Directive<'d> {
    /// Returns the dialect's handler that runs the directive; `None` for a standard one.
    pub fn handler(&self) -> Option<&DirectiveHandler<'d>> {
        match &self.runner {
            Runner::Handler(h) => Some(h),
            Runner::Standard(_) => None,
        }
    }

    /// Returns the option called `name` the directive was given.
    pub fn option(&self, name: &str) -> Option<&DirOption<'d>> {
        self.options.iter().find(|o| o.spec.name == name)
    }

    /// Tells whether the directive was given option `name`, flags included.
    pub fn has_option(&self, name: &str) -> bool {
        self.option(name).is_some()
    }

    /// Returns the value lines of option `name`.
    pub fn option_lines(&self, name: &str) -> Option<&[Line<'d>]> {
        self.option(name).map(|o| o.lines.as_slice())
    }

    /// Returns option `name`'s value lines joined by spaces and trimmed; `None` when the
    /// option was not given.
    pub fn option_value(&self, name: &str) -> Option<Cow<'d, str>> {
        self.option(name).map(|o| trimmed(join_lines(&o.lines, " ")))
    }

    /// Returns option `name`'s value lines joined by line breaks and trimmed, as text
    /// options such as `:start-after:` match text that spans lines.
    pub fn option_text(&self, name: &str) -> Option<Cow<'d, str>> {
        self.option(name).map(|o| trimmed(join_lines(&o.lines, "\n")))
    }

    /// The argument lines joined by spaces.
    pub fn argument(&self) -> Cow<'d, str> {
        join_lines(&self.arg, " ")
    }

    /// The directive's rules: argument, final white space, content, options.
    pub(crate) fn shape(&self) -> (ArgRule, bool, ContentRule, &'d [OptSpec]) {
        match self.runner {
            Runner::Standard(s) => (s.arg, s.final_ws, s.content, s.options),
            Runner::Handler(h) => (h.arg, h.final_ws, h.content, h.options),
        }
    }

    /// The name messages give: the specification's or the handler's.
    pub(crate) fn spec_name(&self) -> &'d str {
        match self.runner {
            Runner::Standard(s) => s.name,
            Runner::Handler(h) => h.name,
        }
    }

    /// The standard specification, for a standard directive.
    pub(crate) fn standard(&self) -> Option<&'static DirSpec> {
        match self.runner {
            Runner::Standard(s) => Some(s),
            Runner::Handler(_) => None,
        }
    }

    pub(crate) fn new(
        name: &'d str,
        runner: Runner<'d>,
        span: SourceSpan,
        name_span: SourceSpan,
        sections: bool,
    ) -> Self {
        Directive {
            name,
            span,
            name_span,
            arg: Vec::new(),
            arg_block: Vec::new(),
            options: Vec::new(),
            content: Vec::new(),
            sections,
            subst: false,
            subst_name: String::new(),
            runner,
        }
    }
}

/// Trims a joined value, keeping it borrowed when it was.
pub(crate) fn trimmed(c: Cow<'_, str>) -> Cow<'_, str> {
    match c {
        Cow::Borrowed(s) => Cow::Borrowed(trim_space(s)),
        Cow::Owned(s) => Cow::Owned(trim_space(&s).to_owned()),
    }
}

fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c >= 0x80
}

/// Recognizes "name::" at column `col`; the name is a simple reference name. Returns
/// the end of the name and the column of the arguments.
pub(crate) fn directive_marker(t: &str, col: usize) -> Option<(usize, usize)> {
    let b = t.as_bytes();
    let mut i = col;
    while i < b.len() {
        let c = b[i];
        if is_word(c) {
            i += 1;
            continue;
        }
        let internal = matches!(c, b'-' | b'.' | b'_' | b'+' | b':');
        if internal && i > col && i + 1 < b.len() && is_word(b[i + 1]) {
            i += 1;
            continue;
        }
        break;
    }
    if i == col || i + 2 > b.len() || &b[i..i + 2] != b"::" {
        return None;
    }
    if i + 2 < b.len() && b[i + 2] != b' ' {
        return None;
    }
    Some((i, i + 2 + indent_of(&t[i + 2..])))
}

impl<'p, 'a> Parser<'p, 'a> {
    /// Returns what runs directive `name`: the dialect's handler first, then the
    /// standard directive.
    pub(crate) fn runner_for<'d>(&self, name: &'d str) -> Option<Runner<'d>>
    where
        'p: 'd,
    {
        if let Some(h) = self.ext.and_then(|e| e.directive(name)) {
            return Some(Runner::Handler(h));
        }
        find_directive(name).map(Runner::Standard)
    }

    /// Reads the directive block at line `i`, column `col`. `None` when the line holds
    /// no directive marker; otherwise the lines consumed and the directive, which is
    /// `None` when it was refused (and reported).
    pub(crate) fn read_directive<'d>(
        &mut self,
        lines: &[Line<'d>],
        i: usize,
        col: usize,
        sections: bool,
    ) -> Option<(usize, Option<Directive<'d>>)>
    where
        'p: 'd,
    {
        let t = lines[i].text;
        let (name_end, args_col) = directive_marker(t, col)?;
        let blk = indented_block(lines, i, Some(args_col), false, false, self.line_buf());
        let consumed = blk.consumed.max(1);
        let span = self.lines_span(&lines[i..i + consumed]);
        let name_span = self.line_span(&lines[i].part(col, name_end));
        let name = &t[col..name_end];
        let Some(runner) = self.runner_for(name) else {
            self.report(DIRECTIVE_UNKNOWN, name_span, arg_span(name_span), Arg::None);
            self.give_lines(blk.lines);
            return Some((consumed, None));
        };
        let mut d = Directive::new(name, runner, span, name_span, sections);
        d.arg = self.line_buf();
        d.arg_block = self.line_buf();
        d.content = self.line_buf();
        let ok = self.directive_block(&mut d, &blk.lines);
        self.give_lines(blk.lines);
        if !ok {
            self.give_directive(d);
            return Some((consumed, None));
        }
        Some((consumed, Some(d)))
    }

    /// Gives a directive's line buffers back to the pool once it has run.
    pub(crate) fn give_directive(&mut self, d: Directive<'_>) {
        self.give_lines(d.arg);
        self.give_lines(d.arg_block);
        self.give_lines(d.content);
    }

    /// Splits a directive block into argument, options, and content, as the
    /// specification describes: the argument and options end at the first blank line,
    /// options start at the first field marker, and directives without arguments or
    /// options treat all of it as content.
    fn directive_block<'d>(&mut self, d: &mut Directive<'d>, block: &[Line<'d>]) -> bool {
        let (arg_rule, _, _, specs) = d.shape();
        let mut ind = block;
        if ind.first().is_some_and(|l| l.is_blank()) {
            ind = &ind[1..];
        }
        let ind = trim_blank_end(ind);
        let has_args = arg_rule != ArgRule::None;
        let (mut args, rest, content): (&[Line<'d>], &[Line<'d>], &[Line<'d>]);
        if !ind.is_empty() && (has_args || !specs.is_empty()) {
            let k = ind.iter().position(|l| l.is_blank()).unwrap_or(ind.len());
            (args, rest, content) = (&ind[..k], &ind[k..], &ind[(k + 1).min(ind.len())..]);
        } else {
            (args, rest, content) = (&[], &[], ind);
        }
        if !specs.is_empty() {
            let k = args.iter().position(|l| field_marker(l.text).is_some()).unwrap_or(args.len());
            let spec_name = d.spec_name();
            let mut opts = Vec::new();
            if !self.parse_options(spec_name, specs, &args[k..], &mut opts) {
                return false;
            }
            d.options = opts;
            args = &args[..k];
        }
        d.content.clear();
        if !args.is_empty() && !has_args {
            // The argument lines and the rest are all content.
            d.content.extend_from_slice(args);
            d.content.extend_from_slice(rest);
            let top = d.content.len() - trim_blank_start(&d.content).len();
            d.content.drain(..top);
            return self.directive_arguments(d, &[]);
        }
        d.content.extend_from_slice(trim_blank_start(content));
        self.directive_arguments(d, args)
    }

    /// Checks the argument words and the content against the directive's rules and
    /// stores the argument; reports and returns false when they do not fit.
    pub(crate) fn directive_arguments<'d>(
        &mut self,
        d: &mut Directive<'d>,
        args: &[Line<'d>],
    ) -> bool {
        let (arg_rule, final_ws, content_rule, _) = d.shape();
        let words: usize = args.iter().map(|l| word_count(l.text)).sum();
        let at = arg_span(d.name_span);
        if words == 0 && arg_rule == ArgRule::Required {
            self.report(DIRECTIVE_ARGUMENT_MISSING, d.name_span, at, Arg::None);
            return false;
        }
        if words > 1 && !final_ws {
            self.report(DIRECTIVE_ARGUMENT_EXTRA, d.name_span, at, Arg::None);
            return false;
        }
        if words > 0 {
            d.arg.clear();
            d.arg.extend(args.iter().map(|l| l.trim()));
            d.arg_block.clear();
            d.arg_block.extend_from_slice(args);
        }
        if !d.content.is_empty() && content_rule == ContentRule::None {
            let span = self.lines_span(&d.content);
            self.report(DIRECTIVE_CONTENT_FORBIDDEN, span, at, Arg::None);
            return false;
        }
        if d.content.is_empty() && content_rule == ContentRule::Required {
            self.report(DIRECTIVE_CONTENT_MISSING, d.span, at, Arg::None);
            return false;
        }
        true
    }

    /// Reads the option field list of a directive into `out` and checks every option.
    pub(crate) fn parse_options<'d>(
        &mut self,
        directive: &str,
        specs: &[OptSpec],
        lines: &[Line<'d>],
        out: &mut Vec<DirOption<'d>>,
    ) -> bool {
        let mut k = 0;
        while k < lines.len() {
            let l = lines[k];
            let Some((name_end, col)) = field_marker(l.text) else {
                let span = self.line_span(&l);
                self.report(DIRECTIVE_OPTION_BLOCK, span, arg_span(span), Arg::None);
                return false;
            };
            let mut e = k + 1;
            while e < lines.len() && lines[e].is_indented() {
                e += 1;
            }
            let mut value = Vec::with_capacity(e - k);
            value.push(l.cut(col));
            value.extend_from_slice(&lines[k + 1..e]);
            dedent(&mut value[1..]);
            for v in &mut value[1..] {
                *v = v.trim();
            }
            let span = self.lines_span(&lines[k..e]);
            let o = DirOption { spec: OptSpec::default(), lines: value, span };
            if !self.check_option(directive, specs, &l.text[1..name_end], o, out) {
                return false;
            }
            k = e;
        }
        true
    }

    /// Checks option `name` against `specs`, the options already given, and its value
    /// type, and appends it to `out`.
    pub(crate) fn check_option<'d>(
        &mut self,
        directive: &str,
        specs: &[OptSpec],
        name: &str,
        mut o: DirOption<'d>,
        out: &mut Vec<DirOption<'d>>,
    ) -> bool {
        let at = arg_span(o.span);
        let Some(spec) = specs.iter().find(|s| s.name == name) else {
            self.report(DIRECTIVE_OPTION_UNKNOWN, o.span, at, arg_text(directive));
            return false;
        };
        o.spec = *spec;
        if out.iter().any(|x| x.spec.name == name) {
            self.report(DIRECTIVE_OPTION_DUPLICATE, o.span, at, Arg::None);
            return false;
        }
        let value = join_lines(&o.lines, " ");
        if let Some(why) = option_problem(spec, trim_space(&value)) {
            self.report(DIRECTIVE_OPTION_INVALID, o.span, at, arg_text(why));
            return false;
        }
        if out.len() >= MAX_OPTIONS {
            self.report(DIRECTIVE_OPTIONS_LIMIT, o.span, arg_int(MAX_OPTIONS as i64), Arg::None);
            return false;
        }
        out.push(o);
        true
    }

    /// Reads and runs the directive at column `col` of line `i`; `None` when the line
    /// holds no directive.
    pub(crate) fn directive(
        &mut self,
        lines: &[Line<'_>],
        i: usize,
        col: usize,
        sections: bool,
    ) -> Option<Result<usize, Fail>> {
        let (consumed, d) = self.read_directive(lines, i, col, sections)?;
        let run = match d {
            Some(d) => {
                let r = self.run(&d);
                self.give_directive(d);
                r
            }
            None => Ok(()),
        };
        Some(run.map(|()| consumed))
    }

    /// Runs a checked directive with its handler or as a standard directive.
    pub(crate) fn run(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        match (d.runner, self.ext) {
            (Runner::Handler(_), Some(ext)) => ext.run_directive(self, d),
            (Runner::Standard(spec), _) => self.run_standard(d, spec.kind),
            (Runner::Handler(_), None) => Ok(()),
        }
    }

    fn run_standard(&mut self, d: &Directive<'_>, kind: DirKind) -> Result<(), Fail> {
        match kind {
            DirKind::Admonition | DirKind::GenericAdmonition => self.admonition(d),
            DirKind::Image => self.image_directive(d),
            DirKind::Figure => self.figure(d),
            DirKind::Topic => self.topic(d),
            DirKind::Sidebar => self.sidebar(d),
            DirKind::LineBlock => self.line_block_directive(d),
            DirKind::ParsedLiteral => self.parsed_literal(d),
            DirKind::Code => self.code_directive(d),
            DirKind::Math => self.math_directive(d),
            DirKind::Rubric => self.rubric(d),
            DirKind::Quote => self.emit_quotes(&d.content, d.spec_name()),
            DirKind::Compound => self.body_container(d, NodeKind::Compound),
            DirKind::Container => self.container(d),
            DirKind::Table => self.table_directive(d),
            DirKind::CsvTable => self.csv_table(d),
            DirKind::ListTable => self.list_table(d),
            DirKind::Contents => self.contents(d),
            DirKind::Sectnum => self.sectnum(d),
            DirKind::Header | DirKind::Footer => self.header_footer(d, kind),
            DirKind::TargetNotes => self.target_notes(d),
            DirKind::Meta => self.meta(d),
            DirKind::Include => self.include(d),
            DirKind::Raw => self.raw_directive(d),
            DirKind::Class => self.class_directive(d),
            DirKind::Role => self.role_directive(d),
            DirKind::DefaultRole => self.default_role(d),
            DirKind::Title => self.title_directive(d),
            DirKind::Replace | DirKind::Unicode | DirKind::Date => {
                let why = arg_text("it only works inside a substitution definition");
                self.report(DIRECTIVE_CONTEXT, d.name_span, arg_span(d.name_span), why);
                Ok(())
            }
        }
    }

    /// Adds the `:class:` (or `class_opt`) and `:name:` options as attributes. Directive
    /// nodes keep their Name attribute for the directive name, so `:name:` becomes
    /// Custom "name" there.
    pub fn add_common(
        &mut self,
        id: NodeId,
        d: &Directive<'_>,
        class_opt: &str,
    ) -> Result<(), Fail> {
        if let Some(o) = d.option(class_opt)
            && let Some(t) = self.pool_classes(&join_lines(&o.lines, " "))?
        {
            self.ctx.b.add_attribute(id, AttributeKey::Class, t)?;
        }
        if let Some(o) = d.option("name") {
            let name = self.pool_name(&join_lines(&o.lines, " "))?;
            if self.ctx.b.snap.nodes[id].kind == NodeKind::Directive {
                let key = self.ctx.b.add_text("name")?;
                self.ctx.b.add_attribute_named(id, AttributeKey::Custom, name, key)?;
            } else {
                self.ctx.b.add_attribute(id, AttributeKey::Name, name)?;
            }
        }
        Ok(())
    }

    /// Opens a generic Directive node named `name`.
    pub fn directive_node(&mut self, d: &Directive<'_>, name: &str) -> Result<NodeId, Fail> {
        let id = self.open(NodeKind::Directive, d.span)?;
        self.ctx.b.add_attribute_string(id, AttributeKey::Name, name)?;
        Ok(id)
    }

    /// Adds a Custom attribute `name` = `value` to node `id`; both are copied.
    pub fn custom(&mut self, id: NodeId, name: &str, value: &str) -> Result<(), Fail> {
        self.ctx.b.add_custom_attribute(id, name, value)
    }
}

/// Counts the space-separated words of `t`.
fn word_count(t: &str) -> usize {
    let b = t.as_bytes();
    (0..b.len()).filter(|&k| b[k] != b' ' && (k == 0 || b[k - 1] == b' ')).count()
}
