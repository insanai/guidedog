//! The bounded MathML-to-Typst math translator, beside the LaTeX adapter.
//!
//! A formula written as MathML (`:notation: mathml`) becomes Typst math, so a PDF shows
//! it typeset. The translator works in three steps:
//!
//! 1. `guidedoc::filter_mathml` reads the author's text. It is the only reader of that
//!    text and the only allowlist: what it keeps is well-formed MathML Core.
//! 2. `Tree::parse` reads the filter's output into a flat tree, within the element and
//!    depth bounds.
//! 3. `Writer` walks the tree and writes Typst math.
//!
//! The output is generated markup, not raw Typst, and it is safe to write under any
//! content policy, also where raw Typst is omitted. No text of the formula can become
//! Typst code, because text reaches the output in four forms only:
//!
//! - a name from this crate's own tables (`alpha`, `sum`, `arrow.r`);
//! - one letter or one run of digits, set apart by spaces, which Typst reads as text
//!   (an identifier needs two letters side by side, and none is ever written so);
//! - one other character after a backslash, which Typst reads as that character and
//!   never as syntax (`\#`, `\$`, `\,`, `\(`);
//! - a string literal, in which the quote, the backslash and control characters are
//!   escaped.
//!
//! Every function call, comma, semicolon and `#` in the output is written by this
//! module. An attribute value reaches the output only as a number this module parsed
//! and printed again.
//!
//! A formula that cannot be translated is an error that names the first construct; the
//! renderer then shows the formula's source, as it does for untranslatable LaTeX.

mod layout;
mod symbols;
mod write;

use std::fmt;

use guidedoc::{MathMlError, MathMlOptions, filter_mathml};

use write::Writer;

#[cfg(test)]
pub(crate) use symbols::{ENTITIES, OPERATOR_NAMES, SYMBOLS};

/// The most elements of one formula the translator reads.
pub const MAX_MATHML_ELEMENTS: usize = 4096;

/// The deepest element nesting the translator follows. Typst nests one call for each
/// level, so the bound is lower than the filter's.
pub const MAX_MATHML_NESTING: usize = 48;

/// The first construct of a MathML formula the translator cannot write as Typst math:
/// an element such as `<menclose>`, or a description such as "more than 4096 elements".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathMlUnsupported {
    /// The element or the description of the construct.
    pub construct: String,
}

impl fmt::Display for MathMlUnsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot translate {} in this MathML formula to Typst math", self.construct)
    }
}

impl std::error::Error for MathMlUnsupported {}

fn unsupported(construct: impl Into<String>) -> MathMlUnsupported {
    MathMlUnsupported { construct: construct.into() }
}

/// What the translator found out about a formula it wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MathMlMath {
    /// The root says `display="block"`: the formula is meant to stand on its own line.
    pub block: bool,
}

/// Appends the Typst math form of `mathml` (one `math` element, without `$`
/// delimiters) to `out`. It borrows both for the call. The output may name
/// `gd-math-font`, as the output of [`crate::translate_math`] does. On failure it
/// returns the first construct it cannot translate, and `out` is as it was. It never
/// panics, for any input.
pub fn translate_mathml(mathml: &str, out: &mut String) -> Result<MathMlMath, MathMlUnsupported> {
    let mut core = String::new();
    // `annotation-xml` holds another notation of the formula beside the presentation;
    // like `annotation`, it is passed over.
    let options = MathMlOptions { quiet_elements: &["annotation-xml"], ..Default::default() };
    let dropped = filter_mathml(mathml, &options, &mut core).map_err(|error| {
        unsupported(match error {
            MathMlError::TooLong => "a formula longer than 256 KiB",
            MathMlError::TooDeep => "nesting deeper than 128 elements",
            MathMlError::NoRoot => "text that is not one <math> element",
            MathMlError::Malformed => "a tag that is not closed or is closed out of order",
        })
    })?;
    // An element outside MathML Core has no translation, and leaving it out would
    // change the formula. An attribute outside it (style, id, href) changes nothing in
    // a PDF, so it is left out silently.
    if !dropped.first_element.is_empty() {
        return Err(unsupported(dropped.first_element));
    }
    let tree = Tree::parse(&core)?;
    let start = out.len();
    let mut writer = Writer::new(&tree, out);
    writer.root();
    match writer.finish() {
        Ok(()) => Ok(MathMlMath { block: tree.attribute(ROOT, "display") == Some("block") }),
        Err(error) => {
            out.truncate(start);
            Err(error)
        }
    }
}

/// The index of an element in a [`Tree`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElementId(u32);

/// The root `math` element.
pub(crate) const ROOT: ElementId = ElementId(0);

/// No element: the end of a list of children.
const NONE: u32 = u32::MAX;

/// One element of the filtered formula. All text is borrowed from the filter's output.
#[derive(Debug, Clone, Copy)]
struct Element<'s> {
    name: &'s str,
    /// The attributes as the filter wrote them: ` name="value"` and so on.
    attributes: &'s str,
    /// The text before the first child element, with its character references.
    text: &'s str,
    first_child: u32,
    next: u32,
}

/// A formula as a flat tree: elements in document order, linked by index.
pub(crate) struct Tree<'s> {
    elements: Vec<Element<'s>>,
}

impl<'s> Tree<'s> {
    /// Reads `core`, the output of the MathML filter, which is well-formed. A text that
    /// is not is refused, never trusted.
    fn parse(core: &'s str) -> Result<Tree<'s>, MathMlUnsupported> {
        let mut elements: Vec<Element<'s>> = Vec::new();
        // The open elements, each with its last child so far.
        let mut open: Vec<(u32, u32)> = Vec::new();
        let mut rest = core;
        while let Some(at) = rest.find('<') {
            if let Some(&(parent, last)) = open.last()
                && last == NONE
            {
                elements[parent as usize].text = &rest[..at];
            }
            let end = rest[at..].find('>').ok_or_else(malformed)? + at;
            let tag = &rest[at + 1..end];
            rest = &rest[end + 1..];
            if tag.starts_with('/') {
                open.pop().ok_or_else(malformed)?;
                continue;
            }
            let (tag, empty) = match tag.strip_suffix('/') {
                Some(tag) => (tag, true),
                None => (tag, false),
            };
            if elements.len() >= MAX_MATHML_ELEMENTS {
                return Err(unsupported(format!("more than {MAX_MATHML_ELEMENTS} elements")));
            }
            if open.len() >= MAX_MATHML_NESTING {
                return Err(unsupported(format!(
                    "nesting deeper than {MAX_MATHML_NESTING} elements"
                )));
            }
            let name_end = tag.find(' ').unwrap_or(tag.len());
            let id = u32::try_from(elements.len()).map_err(|_| malformed())?;
            match open.last_mut() {
                Some((parent, last)) if *last == NONE => {
                    elements[*parent as usize].first_child = id;
                    *last = id;
                }
                Some((_, last)) => {
                    elements[*last as usize].next = id;
                    *last = id;
                }
                None if elements.is_empty() => {}
                None => return Err(malformed()),
            }
            elements.push(Element {
                name: &tag[..name_end],
                attributes: &tag[name_end..],
                text: "",
                first_child: NONE,
                next: NONE,
            });
            if !empty {
                open.push((id, NONE));
            }
        }
        if elements.is_empty() || !open.is_empty() {
            return Err(malformed());
        }
        Ok(Tree { elements })
    }

    fn element(&self, id: ElementId) -> &Element<'s> {
        &self.elements[id.0 as usize]
    }

    /// The element's name, such as `mfrac`.
    pub(crate) fn name(&self, id: ElementId) -> &'s str {
        self.element(id).name
    }

    /// The text of a token element, as written: with character references, untrimmed.
    pub(crate) fn text(&self, id: ElementId) -> &'s str {
        self.element(id).text
    }

    /// The value of the attribute `name`, when the element has it.
    pub(crate) fn attribute(&self, id: ElementId, name: &str) -> Option<&'s str> {
        let mut rest = self.element(id).attributes;
        while let Some(start) = rest.find(' ') {
            let pair = &rest[start + 1..];
            let (key, value) = pair.split_once("=\"")?;
            let (value, tail) = value.split_once('"')?;
            if key == name {
                return Some(value);
            }
            rest = tail;
        }
        None
    }

    /// The child elements that show: an `annotation` holds another notation of the
    /// formula and is passed over.
    pub(crate) fn children(&self, id: ElementId) -> Children<'_, 's> {
        Children { tree: self, next: self.element(id).first_child }
    }

    /// The `N` children of an element that must have exactly that many.
    pub(crate) fn parts<const N: usize>(&self, id: ElementId) -> Option<[ElementId; N]> {
        let mut parts = [ROOT; N];
        let mut children = self.children(id);
        for part in &mut parts {
            *part = children.next()?;
        }
        children.next().is_none().then_some(parts)
    }
}

/// The children of an element, in order.
pub(crate) struct Children<'t, 's> {
    tree: &'t Tree<'s>,
    next: u32,
}

impl Iterator for Children<'_, '_> {
    type Item = ElementId;

    fn next(&mut self) -> Option<ElementId> {
        while self.next != NONE {
            let id = ElementId(self.next);
            self.next = self.tree.element(id).next;
            if self.tree.name(id) != "annotation" {
                return Some(id);
            }
        }
        None
    }
}

fn malformed() -> MathMlUnsupported {
    unsupported("MathML that is not well formed")
}

/// Appends the text of a token element to `out`: character references become their
/// characters, and white space is trimmed and collapsed, as MathML says. An unknown
/// named reference is the error.
pub(crate) fn decode_text(text: &str, out: &mut String) -> Result<(), MathMlUnsupported> {
    let start = out.len();
    let mut rest = text;
    let mut gap = false;
    while let Some(c) = rest.chars().next() {
        rest = &rest[c.len_utf8()..];
        if c.is_ascii_whitespace() {
            gap = out.len() > start;
            continue;
        }
        if gap {
            out.push(' ');
            gap = false;
        }
        if c != '&' {
            out.push(c);
            continue;
        }
        let (name, tail) = rest.split_once(';').ok_or_else(malformed)?;
        rest = tail;
        out.push(match name.strip_prefix('#') {
            Some(number) => numeric_reference(number),
            None => symbols::entity(name)
                .ok_or_else(|| unsupported(format!("the character reference &{name};")))?,
        });
    }
    Ok(())
}

/// The character of `&#N;` or `&#xN;`; U+FFFD for a number that is no character.
fn numeric_reference(number: &str) -> char {
    let code = match number.strip_prefix(['x', 'X']) {
        Some(hex) => u32::from_str_radix(hex, 16),
        None => number.parse(),
    };
    code.ok().and_then(char::from_u32).filter(|&c| c != '\0').unwrap_or('\u{fffd}')
}
