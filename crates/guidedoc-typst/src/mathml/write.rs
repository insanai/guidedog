//! The writer of the MathML translator: the output rules and the token elements
//! (`mi`, `mn`, `mo`, `mtext`, `ms`, `mspace`).
//!
//! The rules that keep the output valid Typst are those of the LaTeX adapter: atoms
//! are separated by one space, so letters never join into an identifier and a name is
//! never read as a call; delimiters, commas, semicolons and colons from the formula
//! are escaped, so they cannot split or name the arguments of a call.

use std::fmt::Write;

use crate::escape::write_string_body;
use crate::math::ascii_math;

use super::symbols::{
    COMPOUND_OPERATORS, OPERATOR_NAMES, is_invisible, symbol_name, variant_call,
};
use super::{ElementId, MathMlUnsupported, ROOT, Tree, decode_text, unsupported};

/// The widest space an `mspace` may ask for, in em.
const MAX_SPACE_EM: f64 = 20.0;

/// The translator of one formula.
pub(super) struct Writer<'t, 's, 'o> {
    pub(super) tree: &'t Tree<'s>,
    out: &'o mut String,
    /// An atom was written; the next one needs a separating space.
    space: bool,
    /// The text of the token element at hand, decoded.
    token: String,
    bad: Option<MathMlUnsupported>,
}

/// Counts the calls a Typst opening such as "bold(italic(" opens.
pub(super) fn calls(opening: &str) -> usize {
    opening.bytes().filter(|&c| c == b'(').count()
}

impl<'t, 's, 'o> Writer<'t, 's, 'o> {
    pub(super) fn new(tree: &'t Tree<'s>, out: &'o mut String) -> Writer<'t, 's, 'o> {
        Writer { tree, out, space: false, token: String::new(), bad: None }
    }

    /// Writes the whole formula.
    pub(super) fn root(&mut self) {
        self.top_level(ROOT);
    }

    /// The first construct that could not be written, if any.
    pub(super) fn finish(self) -> Result<(), MathMlUnsupported> {
        self.bad.map_or(Ok(()), Err)
    }

    /// Records the first unsupported construct.
    pub(super) fn unsupported(&mut self, construct: impl Into<String>) {
        if self.bad.is_none() {
            self.bad = Some(unsupported(construct));
        }
    }

    pub(super) fn failed(&self) -> bool {
        self.bad.is_some()
    }

    /// Starts an atom: one space after a previous atom.
    fn separate(&mut self) {
        if self.space {
            self.out.push(' ');
        }
        self.space = true;
    }

    pub(super) fn atom(&mut self, typst: &str) {
        self.separate();
        self.out.push_str(typst);
    }

    /// Starts a function call: the next atom follows "(" without a space.
    pub(super) fn open(&mut self, call: &str) {
        self.separate();
        self.out.push_str(call);
        self.space = false;
    }

    /// Writes text of this module between arguments, such as ", " or ", t: ".
    pub(super) fn between(&mut self, typst: &str) {
        self.out.push_str(typst);
        self.space = false;
    }

    pub(super) fn close(&mut self, count: usize) {
        for _ in 0..count {
            self.out.push(')');
        }
        self.space = true;
    }

    /// The output length, to tell later whether an argument wrote anything.
    pub(super) fn mark(&self) -> usize {
        self.out.len()
    }

    /// Writes an empty string where an argument or cell wrote nothing since `mark`;
    /// Typst requires every argument to hold something.
    pub(super) fn fill(&mut self, mark: usize) {
        if self.out.len() == mark {
            self.atom("\"\"");
        }
    }

    /// Writes `s` as a Typst string literal.
    fn string(&mut self, s: &str) {
        self.out.push('"');
        write_string_body(self.out, s, true);
        self.out.push('"');
    }

    /// Writes one character of the formula: by its Typst name, bare when it is a
    /// letter or a digit, and after a backslash otherwise, so that it is never syntax.
    pub(super) fn character(&mut self, c: char) {
        if is_invisible(c) || c.is_whitespace() || c.is_control() {
            return;
        }
        if let Some(name) = symbol_name(c) {
            return self.atom(name);
        }
        // A colon after a letter would name an argument; escaped, it is a character.
        // The adapter's `~` is LaTeX's space; here it is the character.
        let ascii = if matches!(c, ':' | '~') { None } else { u8::try_from(c).ok() };
        let known = ascii.and_then(ascii_math);
        if let Some(typst) = known {
            return self.atom(typst);
        }
        self.separate();
        if !c.is_alphanumeric() {
            self.out.push('\\');
        }
        self.out.push(c);
    }

    /// Decodes the text of token element `id` into `self.token`; false on a character
    /// reference the translator does not know.
    fn read_token(&mut self, id: ElementId) -> bool {
        self.token.clear();
        match decode_text(self.tree.text(id), &mut self.token) {
            Ok(()) => true,
            Err(error) => {
                self.bad.get_or_insert(error);
                false
            }
        }
    }

    /// Opens the calls of the element's `mathvariant` and returns how many to close.
    pub(super) fn open_variant(&mut self, id: ElementId) -> usize {
        match self.tree.attribute(id, "mathvariant").and_then(variant_call) {
            Some(call) => {
                self.open(call);
                calls(call)
            }
            None => 0,
        }
    }

    /// Writes an identifier. One character is a variable; a word is an operator name
    /// Typst knows (`sin`), or upright text, as MathML shows a longer `mi`.
    pub(super) fn identifier(&mut self, id: ElementId) {
        if !self.read_token(id) {
            return;
        }
        let token = std::mem::take(&mut self.token);
        let variant = self.tree.attribute(id, "mathvariant");
        let plain = variant.is_none_or(|v| v == "normal");
        let mut chars = token.chars();
        match (chars.next(), chars.next()) {
            (None, _) => {}
            (Some(c), None) => {
                let opened = self.open_variant(id);
                self.character(c);
                self.close(opened);
            }
            _ if plain && OPERATOR_NAMES.contains(&token.as_str()) => self.atom(&token),
            _ => {
                let opened = if plain { 0 } else { self.open_variant(id) };
                self.separate();
                self.string(&token);
                self.close(opened);
            }
        }
        self.token = token;
    }

    /// Writes a number: digits with inner decimal points as they are, anything else
    /// (a grouped or a non-decimal number) as upright text.
    pub(super) fn number(&mut self, id: ElementId) {
        if !self.read_token(id) {
            return;
        }
        let token = std::mem::take(&mut self.token);
        let opened = self.open_variant(id);
        let b = token.as_bytes();
        let decimal = b.iter().enumerate().all(|(i, &c)| {
            c.is_ascii_digit()
                || (c == b'.' && i > 0 && b.get(i + 1).is_some_and(u8::is_ascii_digit))
        });
        if !token.is_empty() {
            self.separate();
            if decimal {
                self.out.push_str(&token);
            } else {
                self.string(&token);
            }
        }
        self.close(opened);
        self.token = token;
    }

    /// Writes an operator: an operator name (`lim`), a compound such as `:=`, a word
    /// as `op("word")`, or its characters one by one.
    pub(super) fn operator(&mut self, id: ElementId) {
        if !self.read_token(id) {
            return;
        }
        let token = std::mem::take(&mut self.token);
        let opened = self.open_variant(id);
        let compound = COMPOUND_OPERATORS.iter().find(|c| c.0 == token);
        if OPERATOR_NAMES.contains(&token.as_str()) {
            self.atom(&token);
        } else if let Some(&(_, typst)) = compound {
            self.atom(typst);
        } else if token.chars().count() > 1 && token.chars().all(char::is_alphabetic) {
            self.open("op(");
            self.string(&token);
            self.close(1);
        } else {
            for c in token.chars() {
                self.character(c);
            }
        }
        self.close(opened);
        self.token = token;
    }

    /// Writes `mtext` as prose with normal text layout in the surrounding font, as the
    /// LaTeX adapter writes `\text`; `quoted` adds the quotes of `ms`.
    pub(super) fn text(&mut self, id: ElementId, quoted: bool) {
        if !self.read_token(id) {
            return;
        }
        let token = std::mem::take(&mut self.token);
        if !token.is_empty() || quoted {
            self.open("#box(text(");
            let style = match self.tree.attribute(id, "mathvariant").unwrap_or("") {
                "monospace" => "font: gd-mono + gd-math-font, ",
                "bold" => "font: gd-math-font, weight: \"bold\", ",
                "italic" => "font: gd-math-font, style: \"italic\", ",
                "bold-italic" => "font: gd-math-font, weight: \"bold\", style: \"italic\", ",
                _ => "font: gd-math-font, ",
            };
            self.out.push_str(style);
            self.out.push('"');
            if quoted {
                self.out.push_str("\\\"");
            }
            write_string_body(self.out, &token, true);
            if quoted {
                self.out.push_str("\\\"");
            }
            self.out.push('"');
            self.close(2);
        }
        self.token = token;
    }

    /// Writes the horizontal space of `mspace`. The width is parsed and printed again,
    /// so no text of the attribute reaches the output; a width that is no plain length
    /// writes nothing.
    pub(super) fn space_element(&mut self, id: ElementId) {
        let Some(width) = self.tree.attribute(id, "width") else { return };
        let digits = width.trim_end_matches(|c: char| c.is_ascii_alphabetic());
        let em_per_unit = match &width[digits.len()..] {
            "em" => 1.0,
            "ex" => 0.5,
            "pt" => 0.1,
            "px" => 0.075,
            _ => return,
        };
        let Ok(number) = digits.parse::<f64>() else { return };
        let em = number * em_per_unit;
        if !em.is_finite() || em == 0.0 {
            return;
        }
        let em = em.clamp(-MAX_SPACE_EM, MAX_SPACE_EM);
        self.separate();
        // Writing to a String cannot fail.
        let _ = write!(self.out, "#h({em:.4}em)");
    }
}
