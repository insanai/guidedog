//! The parser's state and lexical helpers (Sphinx's `BaseParser`), and the guard every
//! recursive production passes. The productions live in `parse_expr`, `parse_decl`, and
//! `parse_top`.

use crate::Limits;
use crate::ast::{Ast, List, Node, NodeId, Slot};
use crate::error::{CppError, Stop};
use crate::lex;
use crate::parse_binop::BinLevel;
use crate::stack::StackGuard;

/// A production that did not match, as Sphinx raises `DefinitionError`. The parser
/// records why when it stopped at a limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Fail;

/// The result of a production.
pub(crate) type PResult<T> = Result<T, Fail>;

/// A parse bounds its steps by the definition's length. Alternatives are retried at each
/// level, so nested parentheses would otherwise cost time exponential in their depth;
/// ordinary declarations take a few steps per byte.
pub const STEPS_PER_BYTE: usize = 64;

/// The steps every parse may take besides those per byte.
const BASE_STEPS: usize = 4096;

/// The nodes a parse may make per byte of the definition, and besides them. A parse
/// that needs more fails, as the reader's fixed node pool did.
const NODES_PER_BYTE: usize = 16;
const BASE_NODES: usize = 256;

/// A position to return to, giving back the nodes made since.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Mark {
    pos: usize,
    used: usize,
}

/// Sphinx's `DefinitionParser` over one definition.
#[derive(Debug)]
pub(crate) struct Parser<'a> {
    pub def: &'a str,
    pub pos: usize,
    /// The text the last word or identifier match matched.
    pub last: &'a str,
    /// The tree built so far.
    pub ast: Ast<'a>,
    max_nodes: usize,
    /// The node table is full; the parse fails.
    pub is_full: bool,
    depth: usize,
    max_depth: usize,
    stack: StackGuard,
    steps: usize,
    pub max_steps: usize,
    /// Why the parse stopped at a limit; every production fails from then on.
    pub stopped: Option<Stop>,
    /// The precedence levels of binary operators being read (`parse_binop`), shared
    /// by nested expressions so the buffer is reused.
    pub bin_levels: Vec<BinLevel>,
}

impl<'a> Parser<'a> {
    /// Makes a parser over `def` with the limits' nesting and stack budget, measuring
    /// the stack from the caller's frame.
    pub fn new(def: &'a str, limits: &Limits) -> Parser<'a> {
        Parser {
            def,
            pos: 0,
            last: "",
            ast: Ast { def, slots: Vec::new(), used_fallback: false },
            max_nodes: NODES_PER_BYTE * def.len() + BASE_NODES,
            is_full: false,
            depth: 0,
            max_depth: limits.max_depth,
            stack: StackGuard::new(limits.stack_bytes),
            steps: 0,
            max_steps: STEPS_PER_BYTE * def.len() + BASE_STEPS,
            stopped: None,
            bin_levels: Vec::new(),
        }
    }

    /// Overrides the step budget (for tests of the bound).
    #[cfg(test)]
    pub fn with_max_steps(mut self, steps: usize) -> Parser<'a> {
        self.max_steps = steps;
        self
    }

    /// Runs a recursive production behind the guard: it fails once the parse passes
    /// one of its limits, and so does every later one.
    pub fn guarded<T>(&mut self, f: impl FnOnce(&mut Self) -> PResult<T>) -> PResult<T> {
        if self.stopped.is_some() {
            return Err(Fail);
        }
        self.steps += 1;
        let stop = if self.depth >= self.max_depth {
            Some(Stop::Depth)
        } else if self.stack.is_exhausted() {
            Some(Stop::Stack)
        } else if self.steps > self.max_steps {
            Some(Stop::Work)
        } else {
            None
        };
        if let Some(stop) = stop {
            self.stopped = Some(stop);
            return Err(Fail);
        }
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        result
    }

    /// Returns the limit error when the parse stopped at one, spanning the definition.
    pub fn stop_error(&self) -> Option<CppError> {
        let stop = self.stopped?;
        let limit = match stop {
            Stop::Depth => self.max_depth,
            Stop::Stack => self.stack.limit(),
            Stop::Work => self.max_steps,
        };
        Some(CppError::limit(stop, limit, 0..self.def.len()))
    }

    /// Turns the parser into the tree it built.
    pub fn into_ast(self) -> Ast<'a> {
        self.ast
    }

    // Nodes.

    pub fn mark(&self) -> Mark {
        Mark { pos: self.pos, used: self.ast.slots.len() }
    }

    pub fn reset(&mut self, m: Mark) {
        self.pos = m.pos;
        self.ast.slots.truncate(m.used);
    }

    /// Adds a node; fails when the table is full.
    pub fn add(&mut self, node: Node<'a>) -> PResult<NodeId> {
        if self.ast.slots.len() >= self.max_nodes {
            self.is_full = true;
            return Err(Fail);
        }
        let id = NodeId(u32::try_from(self.ast.slots.len()).map_err(|_| Fail)?);
        self.ast.slots.push(Slot { node, next: None });
        Ok(id)
    }

    pub fn node(&self, id: NodeId) -> &Node<'a> {
        self.ast.node(id)
    }

    /// Adds an `Identifier` of the last match.
    pub fn add_identifier(&mut self) -> PResult<NodeId> {
        self.add(Node::Identifier(self.last))
    }

    // Lexical helpers, as BaseParser's.

    pub fn bytes(&self) -> &'a [u8] {
        self.def.as_bytes()
    }

    pub fn is_eof(&self) -> bool {
        self.pos >= self.def.len()
    }

    /// The byte at the position, or 0 at the end.
    pub fn current(&self) -> u8 {
        self.bytes().get(self.pos).copied().unwrap_or(0)
    }

    /// The definition from `start` to the position.
    pub fn since(&self, start: usize) -> &'a str {
        self.def.get(start..self.pos).unwrap_or("")
    }

    /// The definition from the position to `end`, moving there.
    pub fn take_to(&mut self, end: usize) -> &'a str {
        let text = self.def.get(self.pos..end).unwrap_or("");
        self.pos = end;
        text
    }

    pub fn skip_ws(&mut self) -> bool {
        let start = self.pos;
        while self.pos < self.def.len() && lex::is_space(self.bytes()[self.pos]) {
            self.pos += 1;
        }
        self.pos > start
    }

    pub fn skip_string(&mut self, s: &str) -> bool {
        if !self.bytes().get(self.pos..).is_some_and(|rest| rest.starts_with(s.as_bytes())) {
            return false;
        }
        self.pos += s.len();
        true
    }

    pub fn skip_string_ws(&mut self, s: &str) -> bool {
        if !self.skip_string(s) {
            return false;
        }
        self.skip_ws();
        true
    }

    /// Tells whether `word` stands at the position, delimited as `\bword\b`.
    pub fn is_at_word(&self, word: &str) -> bool {
        let s = self.bytes();
        let end = self.pos + word.len();
        if s.get(self.pos..end) != Some(word.as_bytes()) {
            return false;
        }
        let first = word.as_bytes()[0];
        if lex::is_word_byte(first) && self.pos > 0 && lex::is_word_byte(s[self.pos - 1]) {
            return false;
        }
        let last = word.as_bytes()[word.len() - 1];
        !(lex::is_word_byte(last) && end < s.len() && lex::is_word_byte(s[end]))
    }

    pub fn skip_word(&mut self, word: &'static str) -> bool {
        if !self.is_at_word(word) {
            return false;
        }
        self.pos += word.len();
        self.last = word;
        true
    }

    pub fn skip_word_ws(&mut self, word: &'static str) -> bool {
        if !self.skip_word(word) {
            return false;
        }
        self.skip_ws();
        true
    }

    /// Skips a word or a string of symbols, as the operator tables mix them: an operator
    /// that starts with a letter is a word.
    pub fn skip_operator(&mut self, op: &'static str) -> bool {
        if op.as_bytes()[0].is_ascii_alphabetic() {
            self.skip_word(op)
        } else {
            self.skip_string(op)
        }
    }

    /// Matches `identifier_re`: an ordinary identifier, possibly after `~`, or `@` and
    /// the name of an anonymous entity.
    pub fn match_identifier(&mut self) -> bool {
        let d = self.bytes();
        let mut i = self.pos;
        if d.get(i) == Some(&b'@') {
            match d.get(i + 1) {
                Some(&c) if lex::is_word_byte(c) && c < 0x80 => i += 2,
                _ => return false,
            }
        } else {
            if d.get(i) == Some(&b'~') {
                i += 1;
            }
            match d.get(i) {
                Some(&c) if c.is_ascii_alphabetic() || c == b'_' => {}
                _ => return false,
            }
            if i == self.pos && i > 0 && lex::is_word_byte(d[i - 1]) {
                return false;
            }
            i += 1;
        }
        while i < d.len() && lex::is_ascii_word(d[i]) {
            i += 1;
        }
        if i < d.len() && lex::is_word_byte(d[i]) {
            return false; // the \b after the name.
        }
        self.last = self.take_to(i);
        true
    }

    /// Matches `\b(public|private|protected)\b`.
    pub fn match_visibility(&mut self) -> bool {
        ["public", "private", "protected"].into_iter().any(|w| self.skip_word(w))
    }

    /// Matches `_simple_type_specifiers_re`: one of the words, whole.
    pub fn match_simple_type(&mut self) -> Option<&'static str> {
        lex::SIMPLE_TYPES.into_iter().find(|w| self.skip_word(w))
    }
}

/// Builds a list of nodes by linking each new item after the last.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ListBuilder {
    head: Option<NodeId>,
    tail: Option<NodeId>,
}

impl ListBuilder {
    pub fn push(&mut self, p: &mut Parser<'_>, item: NodeId) {
        match self.tail {
            Some(tail) => p.ast.slots[tail.index()].next = Some(item),
            None => self.head = Some(item),
        }
        self.tail = Some(item);
    }

    pub fn head(&self) -> List {
        self.head
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }
}
