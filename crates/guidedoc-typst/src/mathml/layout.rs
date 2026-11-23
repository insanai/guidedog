//! The layout elements of the MathML translator: rows and fences, fractions, roots,
//! scripts, accents and tables.

use super::symbols::{closing_fence, is_closing_fence, over_accent, under_accent};
use super::write::Writer;
use super::{ElementId, decode_text};

/// How a table is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TableForm {
    /// `mat(delim: .., a, b; c, d)`, with the delimiter as a Typst string or `#none`.
    Matrix(&'static str),
    /// `cases(a & b, c & d)`: a table after a lone opening brace.
    Cases,
    /// `a & b \ c & d`: the lines of an aligned equation, a table that is the whole
    /// formula.
    Lines,
}

impl Writer<'_, '_, '_> {
    /// Writes the formula below `id`, the root or a wrapper that holds all of it. A
    /// table that is the whole formula is a multi-line equation.
    pub(super) fn top_level(&mut self, id: ElementId) {
        if let Some([only]) = self.tree.parts::<1>(id) {
            match self.tree.name(only) {
                "mtable" => return self.table(only, TableForm::Lines),
                "mrow" | "mpadded" | "semantics" => return self.top_level(only),
                "mstyle" if self.is_unstyled(only) => return self.top_level(only),
                _ => {}
            }
        }
        self.row(id);
    }

    /// Writes one element.
    fn element(&mut self, id: ElementId) {
        if self.failed() {
            return;
        }
        match self.tree.name(id) {
            "mi" => self.identifier(id),
            "mn" => self.number(id),
            "mo" => self.operator(id),
            "mtext" => self.text(id, false),
            "ms" => self.text(id, true),
            "mspace" => self.space_element(id),
            "math" | "mrow" | "mpadded" | "merror" | "mtd" => self.row(id),
            // The first child is the presentation; annotations are passed over.
            "semantics" => {
                if let Some(first) = self.tree.children(id).next() {
                    self.element(first);
                }
            }
            "mstyle" => self.style(id),
            "mphantom" => {
                self.open("#hide($");
                self.row(id);
                self.between("$)");
                self.close(0);
            }
            "mfrac" => self.fraction(id),
            "msqrt" => {
                self.open("sqrt(");
                self.argument_row(id);
                self.close(1);
            }
            "mroot" => self.two("root(", id, true),
            "msub" => self.scripts(id, &["b: "]),
            "msup" => self.scripts(id, &["t: "]),
            "msubsup" => self.scripts(id, &["b: ", "t: "]),
            "munder" => self.limits(id, false),
            "mover" => self.limits(id, true),
            "munderover" => match self.tree.parts::<3>(id) {
                Some([base, under, over]) => self.under_over(base, Some(under), Some(over)),
                None => self.missing_parts(id),
            },
            "mmultiscripts" => self.multiscripts(id),
            "mtable" => self.table(id, TableForm::Matrix("#none")),
            // A row or a cell outside a table: its content.
            "mtr" => self.row(id),
            "mprescripts" => {}
            name => self.unsupported(format!("<{name}>")),
        }
    }

    /// Writes one element as an argument of a call; an empty one becomes `""`.
    fn argument(&mut self, id: ElementId) {
        let mark = self.mark();
        self.element(id);
        self.fill(mark);
    }

    /// Writes the children of `id` as one argument of a call.
    fn argument_row(&mut self, id: ElementId) {
        let mark = self.mark();
        self.row(id);
        self.fill(mark);
    }

    /// The one character of an `mo` element, when it holds exactly one.
    fn operator_character(&self, id: ElementId) -> Option<char> {
        if self.tree.name(id) != "mo" {
            return None;
        }
        let mut text = String::new();
        decode_text(self.tree.text(id), &mut text).ok()?;
        let mut chars = text.chars();
        chars.next().filter(|_| chars.next().is_none())
    }

    /// The character of an `mo` that may stretch as a fence.
    fn fence(&self, id: ElementId) -> Option<char> {
        let stretchy = self.tree.attribute(id, "stretchy") != Some("false");
        self.operator_character(id).filter(|_| stretchy)
    }

    /// Writes the children of `id` in a row. A row between two fences becomes
    /// `lr(...)`, whose fences grow with the content; a fenced table becomes a matrix
    /// or cases, and a fenced fraction without a line a binomial.
    fn row(&mut self, id: ElementId) {
        let mut children = self.tree.children(id);
        let first = children.next();
        let last = children.last();
        let opening = first.and_then(|f| self.fence(f)).filter(|&c| closing_fence(c).is_some());
        let closing = last.and_then(|l| self.fence(l)).filter(|&c| is_closing_fence(c));
        match (opening, closing) {
            (Some(open), Some(close)) => {
                if !self.fenced_part(id, open, close) {
                    self.open("lr(");
                    self.plain_sequence(id);
                    self.close(1);
                }
            }
            (Some('{'), None) => match self.tree.parts::<2>(id) {
                Some([_, table]) if self.tree.name(table) == "mtable" => {
                    self.table(table, TableForm::Cases);
                }
                _ => self.sequence(id),
            },
            _ => self.sequence(id),
        }
    }

    /// Writes the children one after another. Two bars among them (`|x|`, `‖x‖`) are
    /// a pair of fences, written as `lr(| x |)` so that they stand close to their
    /// content; more bars are ambiguous and stay as they are.
    fn sequence(&mut self, id: ElementId) {
        let pair = self.bar_pair(id);
        for child in self.tree.children(id) {
            if pair.is_some_and(|p| p.0 == child) {
                self.open("lr(");
            }
            self.element(child);
            if pair.is_some_and(|p| p.1 == child) {
                self.close(1);
            }
        }
    }

    /// The two bars of a row that holds exactly two, both the same character.
    fn bar_pair(&self, id: ElementId) -> Option<(ElementId, ElementId)> {
        let mut bars = self
            .tree
            .children(id)
            .filter_map(|child| Some((child, self.fence(child).filter(|c| "|‖".contains(*c))?)));
        let (first, second) = (bars.next()?, bars.next()?);
        (bars.next().is_none() && first.1 == second.1).then_some((first.0, second.0))
    }

    fn plain_sequence(&mut self, id: ElementId) {
        for child in self.tree.children(id) {
            self.element(child);
        }
    }

    /// Writes a row of two matching fences around one table or one lineless fraction
    /// as the Typst call that draws the fences itself; false when the row is another.
    fn fenced_part(&mut self, id: ElementId, open: char, close: char) -> bool {
        let Some([_, inner, _]) = self.tree.parts::<3>(id) else { return false };
        if closing_fence(open) != Some(close) {
            return false;
        }
        let delimiter = match open {
            '(' => "\"(\"",
            '[' => "\"[\"",
            '{' => "\"{\"",
            '|' => "\"|\"",
            '‖' => "\"‖\"",
            _ => return false,
        };
        match self.tree.name(inner) {
            "mtable" => self.table(inner, TableForm::Matrix(delimiter)),
            "mfrac" if open == '(' && self.is_lineless(inner) => self.two("binom(", inner, false),
            _ => return false,
        }
        true
    }

    /// Tells whether an `mfrac` draws no line (`linethickness="0"`, with any unit).
    fn is_lineless(&self, id: ElementId) -> bool {
        self.tree.attribute(id, "linethickness").is_some_and(|value| {
            let number = value.trim_end_matches(|c: char| c.is_ascii_alphabetic() || c == '%');
            number.parse::<f64>().is_ok_and(|n| n == 0.0)
        })
    }

    /// Writes `call(first, second)` for an element with two parts; `swap` writes the
    /// second part first, as `root(index, radicand)` needs.
    fn two(&mut self, call: &str, id: ElementId, swap: bool) {
        let Some([first, second]) = self.tree.parts::<2>(id) else {
            let name = self.tree.name(id);
            return self.unsupported(format!("an <{name}> that does not have two parts"));
        };
        let (first, second) = if swap { (second, first) } else { (first, second) };
        self.open(call);
        self.argument(first);
        self.between(", ");
        self.argument(second);
        self.close(1);
    }

    /// Writes a fraction. One without a line and without fences is a stack of two.
    fn fraction(&mut self, id: ElementId) {
        if !self.is_lineless(id) {
            return self.two("frac(", id, false);
        }
        let Some([above, below]) = self.tree.parts::<2>(id) else {
            return self.unsupported("an <mfrac> that does not have two parts");
        };
        self.open("mat(delim: #none, ");
        self.argument(above);
        self.between("; ");
        self.argument(below);
        self.close(1);
    }

    /// Tells whether an `mstyle` sets neither a letter style nor a display style.
    fn is_unstyled(&self, id: ElementId) -> bool {
        ["displaystyle", "mathvariant"].iter().all(|name| self.tree.attribute(id, name).is_none())
    }

    /// Writes `mstyle`: its content, in its letter style and its display style.
    fn style(&mut self, id: ElementId) {
        let size = match self.tree.attribute(id, "displaystyle") {
            Some("true") => "display(",
            Some("false") => "inline(",
            _ => "",
        };
        if !size.is_empty() {
            self.open(size);
        }
        let variant = self.open_variant(id);
        let mark = self.mark();
        self.row(id);
        if !size.is_empty() || variant > 0 {
            self.fill(mark);
        }
        self.close(variant + usize::from(!size.is_empty()));
    }

    /// Writes `attach(base, b: .., t: ..)` for `msub`, `msup` and `msubsup`; `places`
    /// names the argument of each script, in the order of the children.
    fn scripts(&mut self, id: ElementId, places: &[&str]) {
        let mut children = self.tree.children(id);
        let Some(base) = children.next() else {
            return self.missing_parts(id);
        };
        self.open("attach(");
        self.argument(base);
        for place in places {
            let Some(script) = children.next() else {
                return self.missing_parts(id);
            };
            self.between(", ");
            self.between(place);
            self.argument(script);
        }
        if children.next().is_some() {
            return self.missing_parts(id);
        }
        self.close(1);
    }

    fn missing_parts(&mut self, id: ElementId) {
        let name = self.tree.name(id);
        self.unsupported(format!("an <{name}> with the wrong number of parts"));
    }

    /// Whether the script of an `munder` or `mover` is an accent: the `accent` or
    /// `accentunder` attribute says so, or, without it, the script is an accent
    /// character, as MathML's operator dictionary has it.
    fn accent_call(&self, id: ElementId, script: ElementId, over: bool) -> Option<&'static str> {
        let attribute = if over { "accent" } else { "accentunder" };
        if self.tree.attribute(id, attribute) == Some("false") {
            return None;
        }
        let c = self.operator_character(script)?;
        if over { over_accent(c) } else { under_accent(c) }
    }

    /// Writes `munder` or `mover`: an accent on the base, or a limit below or above it.
    fn limits(&mut self, id: ElementId, over: bool) {
        let Some([base, script]) = self.tree.parts::<2>(id) else {
            return self.missing_parts(id);
        };
        if let Some(call) = self.accent_call(id, script, over) {
            self.open(call);
            self.argument(base);
            self.close(1);
        } else if over {
            self.under_over(base, None, Some(script));
        } else {
            self.under_over(base, Some(script), None);
        }
    }

    /// Writes `attach(limits(base), b: under, t: over)`: the scripts stand below and
    /// above the base in every style.
    fn under_over(&mut self, base: ElementId, under: Option<ElementId>, over: Option<ElementId>) {
        self.open("attach(limits(");
        self.argument(base);
        self.close(1);
        for (place, script) in [("b: ", under), ("t: ", over)] {
            if let Some(script) = script {
                self.between(", ");
                self.between(place);
                self.argument(script);
            }
        }
        self.close(1);
    }

    /// Writes `mmultiscripts`: pairs of sub- and superscripts after the base, then,
    /// after `mprescripts`, pairs before it. An empty script is left out.
    fn multiscripts(&mut self, id: ElementId) {
        let mut children = self.tree.children(id);
        let Some(base) = children.next() else {
            return self.missing_parts(id);
        };
        self.open("attach(");
        self.argument(base);
        // The places of the scripts in turn: below and above, after and then before.
        let mut places = ["b: ", "t: "];
        let mut at = 0;
        for script in children {
            if self.tree.name(script) == "mprescripts" {
                places = ["bl: ", "tl: "];
                at = 0;
                continue;
            }
            let place = places[at % 2];
            at += 1;
            if at > 2 {
                return self.unsupported("an <mmultiscripts> with several scripts in one place");
            }
            if self.tree.children(script).next().is_none() && self.tree.text(script).is_empty() {
                continue;
            }
            self.between(", ");
            self.between(place);
            self.argument(script);
        }
        self.close(1);
    }

    /// Writes a table in the given form. A child that is no `mtr` is a row of one cell,
    /// and a child of a row that is no `mtd` is a cell.
    fn table(&mut self, id: ElementId, form: TableForm) {
        match form {
            TableForm::Matrix(delimiter) => {
                self.open("mat(delim: ");
                self.between(delimiter);
                self.between(", ");
            }
            TableForm::Cases => self.open("cases("),
            TableForm::Lines => {}
        }
        let mut first_row = true;
        for row in self.tree.children(id) {
            if !first_row {
                match form {
                    TableForm::Matrix(_) => self.between("; "),
                    TableForm::Cases => self.between(", "),
                    TableForm::Lines => self.atom("\\"),
                }
            }
            first_row = false;
            if self.tree.name(row) == "mtr" {
                self.table_row(row, form);
            } else {
                self.argument(row);
            }
        }
        if form != TableForm::Lines {
            self.close(1);
        }
    }

    fn table_row(&mut self, row: ElementId, form: TableForm) {
        let mut first_cell = true;
        for cell in self.tree.children(row) {
            if !first_cell {
                match form {
                    TableForm::Matrix(_) => self.between(", "),
                    TableForm::Cases | TableForm::Lines => self.atom("&"),
                }
            }
            first_cell = false;
            if matches!(form, TableForm::Matrix(_)) {
                self.argument(cell);
            } else {
                self.element(cell);
            }
        }
        // A row without cells still stands for a line.
        if first_cell && form != TableForm::Lines {
            self.between("\"\"");
            self.close(0);
        }
    }
}
