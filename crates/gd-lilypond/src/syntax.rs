use crate::{SourceFiles, Span};

/// An index into the syntax arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SyntaxId(pub u32);

/// A notation token with no inferred musical meaning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    /// Lexical category and owned spelling.
    pub kind: TokenKind,
    /// Exact byte location in a retained file.
    pub span: Span,
}

/// Lexical categories; numeric values are checked during interpretation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenKind {
    /// A note name, identifier or metadata word.
    Word(String),
    /// Digits, without sign or punctuation.
    Number(String),
    /// A decoded quoted string.
    Text(String),
    /// A backslash command, without its initial backslash.
    Command(String),
    /// A single punctuation character, including octave marks and Scheme introducers.
    Symbol(char),
    /// A pair of opening angle brackets.
    SimultaneousStart,
    /// A pair of closing angle brackets.
    SimultaneousEnd,
}

/// A syntax node in a flat arena. Children own indices, never recursive boxes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr {
    /// Grouping or atomic notation.
    pub kind: ExprKind,
    /// Original source range.
    pub span: Span,
}

/// Structural grouping before pitch, duration or context interpretation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExprKind {
    /// An ungrouped token.
    Atom(TokenKind),
    /// A brace-delimited sequence.
    Sequence(Vec<SyntaxId>),
    /// A double-angle-delimited simultaneous expression.
    Simultaneous(Vec<SyntaxId>),
    /// A single-angle-delimited chord.
    Chord(Vec<SyntaxId>),
}

/// Parsed input and its original files. It contains no engraving decisions.
#[derive(Clone, Debug, Default)]
pub struct Syntax {
    /// Nodes addressed by SyntaxId.
    pub nodes: Vec<Expr>,
    /// Top-level expressions in lexical order.
    pub roots: Vec<SyntaxId>,
    /// Original file text retained for reports.
    pub files: SourceFiles,
}

impl Syntax {
    /// Borrows a node, returning None for an invalid identifier.
    pub fn node(&self, id: SyntaxId) -> Option<&Expr> {
        self.nodes.get(id.0 as usize)
    }
}
