//! The MathML filter: it passes MathML Core through and drops everything else.
//!
//! MathML reaches a page from two places: an author writes it (the `math` directive
//! with the `mathml` notation), or a math writer makes it from LaTeX. Both go through
//! this filter, so a page holds only the elements and attributes named here, whatever
//! the input was. The filter is a single pass over the text with a bounded stack; it
//! never panics and never copies a tag it does not know.
//!
//! The filter lives in the engine because every renderer reads MathML through it: the
//! HTML renderer writes what it keeps, and the Typst renderer translates what it keeps
//! to Typst math. So there is one allowlist, and one reader of text an author wrote.
//!
//! What is kept:
//!
//! - the elements in [`MATHML_ELEMENTS`], with matching end tags;
//! - the attributes in [`MATHML_ATTRIBUTES`], when their value is plain (ASCII letters,
//!   digits, space and `. _ % + - #`);
//! - text, with `&`, `<` and `>` escaped unless they form a character reference.
//!
//! What is dropped: any other element with all it contains, any other attribute
//! (`style`, `id`, `href`, event handlers), comments and declarations. `xmlns` is
//! dropped without being counted, because HTML needs none.

use std::fmt;

/// The elements the filter keeps: MathML Core's presentation elements.
pub const MATHML_ELEMENTS: [&str; 28] = [
    "annotation",
    "math",
    "merror",
    "mfrac",
    "mi",
    "mmultiscripts",
    "mn",
    "mo",
    "mover",
    "mpadded",
    "mphantom",
    "mprescripts",
    "mroot",
    "mrow",
    "ms",
    "mspace",
    "msqrt",
    "mstyle",
    "msub",
    "msubsup",
    "msup",
    "mtable",
    "mtd",
    "mtext",
    "mtr",
    "munder",
    "munderover",
    "semantics",
];

/// The attributes the filter keeps, on any kept element.
pub const MATHML_ATTRIBUTES: [&str; 26] = [
    "accent",
    "accentunder",
    "class",
    "columnspan",
    "depth",
    "dir",
    "display",
    "displaystyle",
    "encoding",
    "fence",
    "form",
    "height",
    "largeop",
    "linethickness",
    "lspace",
    "mathvariant",
    "maxsize",
    "minsize",
    "movablelimits",
    "rowspan",
    "rspace",
    "scriptlevel",
    "separator",
    "stretchy",
    "symmetric",
    "width",
];

/// The longest MathML text the filter reads, in bytes.
pub const MAX_MATHML_BYTES: usize = 256 << 10;

/// The deepest element nesting the filter follows.
pub const MAX_MATHML_DEPTH: usize = 128;

/// How the filter treats one formula. `Default` keeps the `display` attribute as it is
/// written and counts every dropped attribute.
#[derive(Debug, Clone, Copy, Default)]
pub struct MathMlOptions<'a> {
    /// `Some(true)` writes `display="block"` on the root, `Some(false)` writes none,
    /// whatever the input says; `None` keeps the input's.
    pub block: Option<bool>,
    /// Attributes dropped without being counted (`xmlns` always is).
    pub quiet_attributes: &'a [&'a str],
    /// Elements dropped, with all they contain, without being counted.
    pub quiet_elements: &'a [&'a str],
}

/// What the filter left out of a formula it wrote.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MathMlDropped {
    /// How many elements, attributes and declarations were dropped.
    pub count: usize,
    /// The first of them: an element as `<name>`, an attribute as its name.
    pub first: String,
    /// The first dropped element, as `<name>`; empty when no element was dropped. A
    /// dropped element takes content out of the formula; a dropped attribute does not.
    pub first_element: String,
}

/// Why the filter wrote nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathMlError {
    /// The text is longer than [`MAX_MATHML_BYTES`].
    TooLong,
    /// Elements nest deeper than [`MAX_MATHML_DEPTH`].
    TooDeep,
    /// The text does not start with a `math` element, or has text around it.
    NoRoot,
    /// A tag is not closed, or an end tag closes another element.
    Malformed,
}

impl fmt::Display for MathMlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            MathMlError::TooLong => "the MathML is longer than 256 KiB",
            MathMlError::TooDeep => "the MathML nests deeper than 128 elements",
            MathMlError::NoRoot => "the MathML is not one <math> element",
            MathMlError::Malformed => "a MathML tag is not closed or is closed out of order",
        })
    }
}

impl std::error::Error for MathMlError {}

/// Appends the MathML Core form of `input` (one `math` element) to `out` and returns
/// what it left out. It borrows both for the call. On failure `out` is as it was.
pub fn filter_mathml(
    input: &str,
    options: &MathMlOptions<'_>,
    out: &mut String,
) -> Result<MathMlDropped, MathMlError> {
    if input.len() > MAX_MATHML_BYTES {
        return Err(MathMlError::TooLong);
    }
    let start = out.len();
    let mut filter = Filter {
        src: input,
        pos: 0,
        out,
        options,
        open: Vec::new(),
        skipping: 0,
        closed: false,
        dropped: MathMlDropped::default(),
    };
    match filter.run() {
        Ok(()) => Ok(filter.dropped),
        Err(error) => {
            filter.out.truncate(start);
            Err(error)
        }
    }
}

/// One start or end tag as read.
struct Tag<'s> {
    name: &'s str,
    /// The text between the name and the closing `>`, without a final `/`.
    attributes: &'s str,
    end: bool,
    empty: bool,
}

struct Filter<'s, 'o, 'p> {
    src: &'s str,
    pos: usize,
    out: &'o mut String,
    options: &'p MathMlOptions<'p>,
    /// The kept elements now open.
    open: Vec<&'s str>,
    /// Inside a dropped element: how many of its elements are open.
    skipping: usize,
    /// The root element was closed.
    closed: bool,
    dropped: MathMlDropped,
}

impl<'s> Filter<'s, '_, '_> {
    fn run(&mut self) -> Result<(), MathMlError> {
        while self.pos < self.src.len() {
            let rest = &self.src[self.pos..];
            match rest.find('<') {
                Some(0) => self.markup()?,
                Some(at) => self.text(&rest[..at])?,
                None => self.text(rest)?,
            }
        }
        if self.closed && self.open.is_empty() && self.skipping == 0 {
            Ok(())
        } else if self.closed || !self.open.is_empty() {
            Err(MathMlError::Malformed)
        } else {
            Err(MathMlError::NoRoot)
        }
    }

    /// Copies text between tags; outside the root only white space may stand.
    fn text(&mut self, text: &'s str) -> Result<(), MathMlError> {
        self.pos += text.len();
        if self.skipping > 0 {
            return Ok(());
        }
        if self.open.is_empty() {
            return if text.trim().is_empty() { Ok(()) } else { Err(MathMlError::NoRoot) };
        }
        let mut rest = text;
        while let Some(at) = rest.find(['&', '>']) {
            self.out.push_str(&rest[..at]);
            let tail = &rest[at..];
            let reference = reference_length(tail);
            if reference > 0 {
                self.out.push_str(&tail[..reference]);
                rest = &tail[reference..];
            } else {
                self.out.push_str(if tail.starts_with('&') { "&amp;" } else { "&gt;" });
                rest = &tail[1..];
            }
        }
        self.out.push_str(rest);
        Ok(())
    }

    /// Reads what starts with `<`: a comment, a declaration, or a tag.
    fn markup(&mut self) -> Result<(), MathMlError> {
        let rest = &self.src[self.pos..];
        if let Some(comment) = rest.strip_prefix("<!--") {
            let end = comment.find("-->").ok_or(MathMlError::Malformed)?;
            self.pos += 4 + end + 3;
            return Ok(());
        }
        if rest.starts_with("<!") || rest.starts_with("<?") {
            let end = rest.find('>').ok_or(MathMlError::Malformed)?;
            self.pos += end + 1;
            self.drop_one("a declaration");
            return Ok(());
        }
        let tag = self.read_tag()?;
        if self.skipping > 0 {
            self.skip(&tag);
            Ok(())
        } else if tag.end {
            self.end_tag(&tag)
        } else {
            self.start_tag(&tag)
        }
    }

    /// Reads one tag and moves past it. Quoted attribute values may hold `>`.
    fn read_tag(&mut self) -> Result<Tag<'s>, MathMlError> {
        let rest = &self.src[self.pos..];
        let bytes = rest.as_bytes();
        let end = bytes.get(1) == Some(&b'/');
        let name_start = if end { 2 } else { 1 };
        let mut i = name_start;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
            i += 1;
        }
        if i == name_start {
            return Err(MathMlError::Malformed);
        }
        let name_end = i;
        let mut quote = 0u8;
        while i < bytes.len() && (quote != 0 || bytes[i] != b'>') {
            match bytes[i] {
                c if quote == c => quote = 0,
                b'"' | b'\'' if quote == 0 => quote = bytes[i],
                _ => {}
            }
            i += 1;
        }
        if i >= bytes.len() {
            return Err(MathMlError::Malformed);
        }
        let mut attributes = &rest[name_end..i];
        let empty = attributes.ends_with('/');
        if empty {
            attributes = &attributes[..attributes.len() - 1];
        }
        self.pos += i + 1;
        Ok(Tag { name: &rest[name_start..name_end], attributes, end, empty })
    }

    /// Counts the tags inside a dropped element until it is closed.
    fn skip(&mut self, tag: &Tag<'_>) {
        if tag.end {
            self.skipping -= 1;
        } else if !tag.empty {
            self.skipping += 1;
        }
    }

    fn start_tag(&mut self, tag: &Tag<'s>) -> Result<(), MathMlError> {
        let root = self.open.is_empty();
        if root && (self.closed || tag.name != "math") {
            return Err(MathMlError::NoRoot);
        }
        let known = MATHML_ELEMENTS.binary_search(&tag.name).is_ok();
        if !known || (!root && tag.name == "math") {
            if !self.options.quiet_elements.contains(&tag.name) {
                let name = format!("<{}>", tag.name);
                self.drop_one(&name);
                if self.dropped.first_element.is_empty() {
                    self.dropped.first_element = name;
                }
            }
            if !tag.empty {
                self.skipping = 1;
            }
            return Ok(());
        }
        if self.open.len() >= MAX_MATHML_DEPTH {
            return Err(MathMlError::TooDeep);
        }
        self.out.push('<');
        self.out.push_str(tag.name);
        self.attributes(tag.attributes, root);
        if tag.empty {
            self.out.push_str("/>");
            self.closed |= root;
        } else {
            self.out.push('>');
            self.open.push(tag.name);
        }
        Ok(())
    }

    fn end_tag(&mut self, tag: &Tag<'_>) -> Result<(), MathMlError> {
        if self.open.pop() != Some(tag.name) {
            return Err(MathMlError::Malformed);
        }
        self.out.push_str("</");
        self.out.push_str(tag.name);
        self.out.push('>');
        self.closed |= self.open.is_empty();
        Ok(())
    }

    /// Writes the kept attributes of a start tag; the root's `display` follows the
    /// options.
    fn attributes(&mut self, text: &'s str, root: bool) {
        let forced = if root { self.options.block } else { None };
        if forced == Some(true) {
            self.out.push_str(" display=\"block\"");
        }
        let mut rest = text;
        while let Some((name, value, tail)) = next_attribute(rest) {
            rest = tail;
            if name.is_empty() {
                break;
            }
            let quiet = name == "xmlns" || name.starts_with("xmlns:");
            if quiet || (name == "display" && forced.is_some()) {
                continue;
            }
            let known = MATHML_ATTRIBUTES.binary_search(&name).is_ok();
            if !known || !is_plain_value(value) {
                if !self.options.quiet_attributes.contains(&name) {
                    self.drop_one(name);
                }
                continue;
            }
            self.out.push(' ');
            self.out.push_str(name);
            self.out.push_str("=\"");
            self.out.push_str(value);
            self.out.push('"');
        }
    }

    fn drop_one(&mut self, what: &str) {
        if self.dropped.count == 0 {
            self.dropped.first = what.to_string();
        }
        self.dropped.count += 1;
    }
}

/// Splits the next `name="value"` off `text`: the name, the value without quotes (empty
/// when there is none), and the rest. `None` at the end.
fn next_attribute(text: &str) -> Option<(&str, &str, &str)> {
    let text = text.trim_start();
    if text.is_empty() {
        return None;
    }
    let name_end = text.find(|c: char| c == '=' || c.is_whitespace()).unwrap_or(text.len());
    let name = &text[..name_end];
    let rest = text[name_end..].trim_start();
    let Some(rest) = rest.strip_prefix('=') else { return Some((name, "", rest)) };
    let rest = rest.trim_start();
    match rest.chars().next() {
        Some(quote @ ('"' | '\'')) => {
            let body = &rest[1..];
            let end = body.find(quote).unwrap_or(body.len());
            Some((name, &body[..end], body.get(end + 1..).unwrap_or("")))
        }
        _ => {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            Some((name, &rest[..end], &rest[end..]))
        }
    }
}

/// Whether an attribute value needs no escaping and can carry no markup or URL.
fn is_plain_value(value: &str) -> bool {
    value.len() <= 256
        && value.bytes().all(|c| c.is_ascii_alphanumeric() || b" ._%+-#".contains(&c))
}

/// The length of the character reference `text` starts with (`&amp;`, `&#8747;`,
/// `&#x222B;`), or 0 when it starts with none.
fn reference_length(text: &str) -> usize {
    let Some(body) = text.strip_prefix('&') else { return 0 };
    let Some(end) = body.bytes().take(34).position(|c| c == b';') else { return 0 };
    let name = &body[..end];
    let valid = match name.strip_prefix('#') {
        Some(number) => match number.strip_prefix(['x', 'X']) {
            Some(hex) => !hex.is_empty() && hex.bytes().all(|c| c.is_ascii_hexdigit()),
            None => !number.is_empty() && number.bytes().all(|c| c.is_ascii_digit()),
        },
        None => {
            name.starts_with(|c: char| c.is_ascii_alphabetic())
                && name.bytes().all(|c| c.is_ascii_alphanumeric())
        }
    };
    if valid { end + 2 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter(input: &str) -> (String, MathMlDropped) {
        let mut out = String::new();
        let dropped = filter_mathml(input, &MathMlOptions::default(), &mut out)
            .unwrap_or_else(|e| panic!("{input}: {e}"));
        (out, dropped)
    }

    #[test]
    fn the_lists_are_sorted_for_binary_search() {
        assert!(MATHML_ELEMENTS.is_sorted());
        assert!(MATHML_ATTRIBUTES.is_sorted());
    }

    #[test]
    fn mathml_core_passes_unchanged() {
        let math = "<math display=\"block\"><mfrac linethickness=\"0\"><mi>a</mi>\
                    <mn>2</mn></mfrac><mspace width=\"0.1667em\"/><mo>&lt;</mo>\
                    <mtext>if a &amp; b &#8747;</mtext></math>";
        let (out, dropped) = filter(math);
        assert_eq!(out, math);
        assert_eq!(dropped.count, 0);
    }

    #[test]
    fn unknown_elements_and_attributes_are_dropped_and_counted() {
        let (out, dropped) = filter(
            "<math xmlns=\"http://www.w3.org/1998/Math/MathML\" onclick=\"x()\">\
             <mi style=\"color:red\" mathvariant='normal'>x</mi><script>alert(1)</script>\
             <annotation-xml><div><b>bold</b></div></annotation-xml><!-- c -->\
             <mo href=\"javascript:x\">+</mo><mi class=\"a&quot;b\">y</mi></math>",
        );
        assert_eq!(out, "<math><mi mathvariant=\"normal\">x</mi><mo>+</mo><mi>y</mi></math>");
        assert_eq!(dropped.count, 6);
        assert_eq!(dropped.first, "onclick");
        assert_eq!(dropped.first_element, "<script>");
    }

    #[test]
    fn text_is_escaped_and_quoted_values_may_hold_angle_brackets() {
        let (out, dropped) =
            filter("<math><mtext title=\"a>b\">a > b & c &bogus d</mtext></math>");
        assert_eq!(out, "<math><mtext>a &gt; b &amp; c &amp;bogus d</mtext></math>");
        assert_eq!(dropped.first, "title");
    }

    #[test]
    fn the_display_attribute_follows_the_options() {
        let mut out = String::new();
        let block = MathMlOptions {
            block: Some(true),
            quiet_attributes: &["style"],
            quiet_elements: &["annotation-xml"],
        };
        let quiet = "<math><mi style=\"x\">x</mi><annotation-xml><b>y</b></annotation-xml></math>";
        let dropped = filter_mathml(quiet, &block, &mut out);
        assert_eq!(out, "<math display=\"block\"><mi>x</mi></math>");
        assert_eq!(dropped.map(|d| d.count), Ok(0));
        out.clear();
        let inline = MathMlOptions { block: Some(false), ..MathMlOptions::default() };
        let _ = filter_mathml("<math display=\"block\"><mi>x</mi></math>", &inline, &mut out);
        assert_eq!(out, "<math><mi>x</mi></math>");
    }

    #[test]
    fn broken_input_writes_nothing() {
        let cases = [
            ("<mi>x</mi>", MathMlError::NoRoot),
            ("text <math></math>", MathMlError::NoRoot),
            ("<math></math><math></math>", MathMlError::NoRoot),
            ("<math><mi>x</math>", MathMlError::Malformed),
            ("<math><mi>x</mi>", MathMlError::Malformed),
            ("<math><mi", MathMlError::Malformed),
            ("<math>< mi></math>", MathMlError::Malformed),
            ("", MathMlError::NoRoot),
        ];
        for (input, error) in cases {
            let mut out = String::from("kept");
            let result = filter_mathml(input, &MathMlOptions::default(), &mut out);
            assert_eq!(result, Err(error), "{input}");
            assert_eq!(out, "kept", "{input}");
        }
    }

    #[test]
    fn depth_and_size_are_bounded() {
        let deep = format!("<math>{}{}</math>", "<mrow>".repeat(200), "</mrow>".repeat(200));
        let mut out = String::new();
        let result = filter_mathml(&deep, &MathMlOptions::default(), &mut out);
        assert_eq!(result, Err(MathMlError::TooDeep));
        let long = format!("<math><mtext>{}</mtext></math>", "x".repeat(MAX_MATHML_BYTES));
        let result = filter_mathml(&long, &MathMlOptions::default(), &mut out);
        assert_eq!(result, Err(MathMlError::TooLong));
        assert!(out.is_empty());
    }
}
