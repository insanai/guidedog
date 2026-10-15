//! Binary operators by precedence (Sphinx's `_parse_logical_or_expression` down to
//! `_parse_pm_expression`). Sphinx recurses once per precedence level, so every
//! parenthesized operand cost eleven stack frames; here the levels being read are kept
//! in `Parser::bin_levels` and one loop plays the recursion, which builds the same tree.

use crate::ast::{Node, NodeId};
use crate::parser::{Fail, ListBuilder, Mark, PResult, Parser};

/// The binary operators by precedence, loosest first.
const BIN_OPS: [&[&str]; 11] = [
    &["||", "or"],
    &["&&", "and"],
    &["|", "bitor"],
    &["^", "xor"],
    &["&", "bitand"],
    &["==", "!=", "not_eq"],
    &["<=>", "<=", ">=", "<", ">"],
    &["<<", ">>"],
    &["+", "-"],
    &["*", "/", "%"],
    &[".*", "->*"],
];

/// One precedence level being read: its first operand, the operands joined to it so
/// far, and the operator whose right operand is being read.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BinLevel {
    level: usize,
    first: Option<NodeId>,
    rest: ListBuilder,
    /// The operator, its index in the level's table, and where the text stood before
    /// it, so a right operand that fails gives way to the next operator.
    pending: Option<(&'static str, usize, Mark)>,
}

/// What the level on top does next.
enum Step {
    /// It is finished (or failed); the result is the operand of the level below.
    Done(PResult<NodeId>),
    /// It reads a right operand, which starts at precedence level `from`.
    Read(usize),
}

impl Parser<'_> {
    /// `logical-or-expression`: operands of every precedence level joined by their
    /// operators. A single operand stands for itself (Sphinx wraps it in a list of one,
    /// which reads and prints the same). In a template argument a `>` ends it.
    pub(crate) fn logical_or_expression(&mut self, in_template: bool) -> PResult<NodeId> {
        let base = self.bin_levels.len();
        let result = self.read_bin_levels(base, in_template);
        // A nested expression (in parentheses) leaves the levels below `base` as it
        // found them; a failure may leave its own.
        self.bin_levels.truncate(base);
        result
    }

    /// Runs the levels above `base` until the loosest one is finished.
    fn read_bin_levels(&mut self, base: usize, in_template: bool) -> PResult<NodeId> {
        let mut operand = self.open_bin_levels(0);
        while self.bin_levels.len() > base {
            let top = self.bin_levels.len() - 1;
            match self.take_operand(top, operand, in_template) {
                Step::Done(result) => {
                    self.bin_levels.pop();
                    operand = result;
                }
                Step::Read(from) => operand = self.open_bin_levels(from),
            }
        }
        operand
    }

    /// Opens the levels `from..` and reads the cast-expression that is the first
    /// operand of the tightest one.
    fn open_bin_levels(&mut self, from: usize) -> PResult<NodeId> {
        for level in from..BIN_OPS.len() {
            let rest = ListBuilder::default();
            self.bin_levels.push(BinLevel { level, first: None, rest, pending: None });
        }
        self.cast_expression()
    }

    /// Gives the level on top the operand just read: its first one, or the right
    /// operand of its pending operator.
    fn take_operand(&mut self, top: usize, operand: PResult<NodeId>, in_template: bool) -> Step {
        let Some((op, index, m)) = self.bin_levels[top].pending else {
            return match operand {
                Ok(first) => {
                    self.bin_levels[top].first = Some(first);
                    self.next_operator(top, in_template)
                }
                Err(fail) => Step::Done(Err(fail)),
            };
        };
        self.bin_levels[top].pending = None;
        let Ok(expr) = operand else {
            // This operator does not fit; the level tries its next ones.
            self.reset(m);
            return self.try_operators(top, index + 1, m);
        };
        let joined = match self.add(Node::BinOperand { op, expr }) {
            Ok(joined) => joined,
            Err(fail) => return Step::Done(Err(fail)),
        };
        let mut rest = self.bin_levels[top].rest;
        rest.push(self, joined);
        self.bin_levels[top].rest = rest;
        self.next_operator(top, in_template)
    }

    /// Looks for the next operator of the level on top, after an operand.
    fn next_operator(&mut self, top: usize, in_template: bool) -> Step {
        self.skip_ws();
        if in_template && self.current() == b'>' {
            return self.close_level(top);
        }
        let m = self.mark();
        self.try_operators(top, 0, m)
    }

    /// Tries the level's operators from `index` on at mark `m`: the first that is there
    /// starts a right operand; with none, the level is finished.
    fn try_operators(&mut self, top: usize, index: usize, m: Mark) -> Step {
        let level = self.bin_levels[top].level;
        for (i, &op) in BIN_OPS[level].iter().enumerate().skip(index) {
            if !self.skip_operator(op) {
                continue;
            }
            if op == "&" && self.current() == b'&' {
                self.pos -= 1; // "&&" is one token, of lower precedence.
                break;
            }
            self.bin_levels[top].pending = Some((op, i, m));
            return Step::Read(level + 1);
        }
        self.close_level(top)
    }

    /// Finishes the level on top: its single operand, or the operands joined.
    fn close_level(&mut self, top: usize) -> Step {
        let BinLevel { first, rest, .. } = self.bin_levels[top];
        let Some(first) = first else {
            return Step::Done(Err(Fail)); // a level closes only after its first operand.
        };
        if rest.is_empty() {
            return Step::Done(Ok(first));
        }
        Step::Done(self.add(Node::BinOp { first, rest: rest.head() }))
    }
}
