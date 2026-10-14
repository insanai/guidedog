//! Styles for `pygments_style`: tables of Pygments style definitions (`"bold #008000"`,
//! `"italic"`, `"noinherit"`, `"bg:#fff0f0"`, `"border:#FF0000"`), resolved through the
//! token hierarchy as Pygments resolves them, and written as the stylesheet Pygments'
//! `HtmlFormatter.get_style_defs` writes, line for line.

use crate::style_tables::{STYLE_INFOS, Style};
use crate::token::{LAST_STANDARD_KIND, TokenKind, kind_class, kind_is, kind_name, kind_parent};

/// One entry of a style's dict: a kind and its definition. A style has at most one per
/// kind.
pub type StyleRule = (TokenKind, &'static str);

/// A style's definition as Pygments' style class holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyleInfo {
    /// The `pygments_style` name: `"github-dark"`.
    pub name: &'static str,
    /// Whether the background is dark.
    pub dark: bool,
    /// The `.highlight` background.
    pub background: &'static str,
    /// The background of emphasized lines (`.hll`).
    pub highlight: &'static str,
    /// The colour of line numbers.
    pub line_number: &'static str,
    /// The background of line numbers.
    pub line_number_background: &'static str,
    /// The colour of special line numbers.
    pub special_line_number: &'static str,
    /// The background of special line numbers.
    pub special_background: &'static str,
    /// The root token's definition, which every kind inherits.
    pub base: &'static str,
    /// The style's dict.
    pub rules: &'static [StyleRule],
}

/// A CSS colour as Pygments normalizes it: six hex digits, case kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color(pub [u8; 6]);

impl Color {
    /// The six hex digits, without `#`.
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("000000")
    }
}

/// A token kind's resolved style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Look {
    /// The text colour.
    pub color: Option<Color>,
    /// The background colour.
    pub background: Option<Color>,
    /// The border colour.
    pub border: Option<Color>,
    /// Bold text.
    pub bold: bool,
    /// Italic text.
    pub italic: bool,
    /// Underlined text.
    pub underline: bool,
}

impl Look {
    /// Whether the look sets anything.
    pub fn is_styled(&self) -> bool {
        self.color.is_some()
            || self.background.is_some()
            || self.border.is_some()
            || self.bold
            || self.italic
            || self.underline
    }
}

/// Every kind's look in one style, from [`resolve_style`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleLooks {
    /// The root token's look, which kinds inherit.
    pub base: Look,
    /// The looks, indexed by `kind as usize`; read them with [`StyleLooks::look`].
    pub kinds: [Look; TokenKind::COUNT],
}

impl StyleLooks {
    /// The resolved look of `kind`.
    pub fn look(&self, kind: TokenKind) -> Look {
        self.kinds[kind as usize]
    }
}

/// The definition of `style`; it borrows static data.
pub fn style_info(style: Style) -> &'static StyleInfo {
    &STYLE_INFOS[style as usize]
}

/// Finds a style by its Pygments name, ignoring case and surrounding spaces; `None` when
/// no style has that name.
pub fn find_style(name: &str) -> Option<Style> {
    let name = name.trim();
    Style::ALL.into_iter().find(|&s| style_info(s).name.eq_ignore_ascii_case(name))
}

/// The style's `pygments_style` name, as [`find_style`] takes it: `"github-dark"`.
pub fn style_name(style: Style) -> &'static str {
    style_info(style).name
}

/// Resolves every kind's look: its parent's, changed by its own rules, as Pygments does.
pub fn resolve_style(style: Style) -> StyleLooks {
    let info = style_info(style);
    let mut base = Look::default();
    apply_definition(&mut base, info.base);
    let mut kinds = [Look::default(); TokenKind::COUNT];
    for kind in TokenKind::ALL {
        let mut look = kind_parent(kind).map_or(base, |p| kinds[p as usize]);
        for &(_, definition) in info.rules.iter().filter(|r| r.0 == kind) {
            if definition.split(' ').any(|w| w == "noinherit") {
                look = base;
            }
            apply_definition(&mut look, definition);
        }
        kinds[kind as usize] = look;
    }
    StyleLooks { base, kinds }
}

/// The resolved look of one kind in a style.
pub fn kind_look(style: Style, kind: TokenKind) -> Look {
    resolve_style(style).look(kind)
}

fn apply_definition(look: &mut Look, definition: &str) {
    for w in definition.split(' ').filter(|w| !w.is_empty()) {
        match w {
            "bold" => look.bold = true,
            "nobold" => look.bold = false,
            "italic" => look.italic = true,
            "noitalic" => look.italic = false,
            "underline" => look.underline = true,
            "nounderline" => look.underline = false,
            _ => {
                if let Some(c) = w.strip_prefix("bg:") {
                    look.background = parse_color(c);
                } else if let Some(c) = w.strip_prefix("border:") {
                    look.border = parse_color(c);
                } else if w.starts_with('#') {
                    look.color = parse_color(w);
                }
            }
        }
    }
}

/// Takes `#abc` or `#aabbcc`; `#abc` becomes `aabbcc`, as in Pygments.
fn parse_color(text: &str) -> Option<Color> {
    let b = text.as_bytes();
    match b {
        [b'#', rest @ ..] if rest.len() == 6 => {
            let mut hex = [0; 6];
            hex.copy_from_slice(rest);
            Some(Color(hex))
        }
        [b'#', r, g, bl] => Some(Color([*r, *r, *g, *g, *bl, *bl])),
        _ => None,
    }
}

/// Writes the stylesheet Pygments writes for `style` with `selector`, as Sphinx writes
/// `_static/pygments.css` with `".highlight"`. It appends to `out` and ends without a
/// newline.
pub fn write_css(style: Style, selector: &str, out: &mut String) {
    let info = style_info(style);
    let looks = resolve_style(style);
    out.push_str("pre { line-height: 125%; }\n");
    let numbers = ["td.linenos .normal", "span.linenos", "td.linenos .special"];
    for (i, n) in numbers.iter().chain(["span.linenos.special"].iter()).enumerate() {
        let (color, background) = if i < 2 {
            (info.line_number, info.line_number_background)
        } else {
            (info.special_line_number, info.special_background)
        };
        out.push_str(n);
        out.push_str(" { color: ");
        out.push_str(color);
        out.push_str("; background-color: ");
        out.push_str(background);
        out.push_str("; padding-left: 5px; padding-right: 5px; }\n");
    }
    for part in [selector, " .hll { background-color: ", info.highlight, " }\n"] {
        out.push_str(part);
    }
    for part in [selector, " { background: ", info.background, ";"] {
        out.push_str(part);
    }
    let text = looks.look(TokenKind::Text);
    if text.is_styled() {
        out.push(' ');
        put_look(out, text);
    }
    out.push_str(" }");
    for kind in css_order() {
        let look = looks.look(kind);
        if kind == TokenKind::Text || !look.is_styled() {
            continue;
        }
        if kind > LAST_STANDARD_KIND && !style_names(info, kind) {
            continue;
        }
        for part in ["\n", selector, " .", css_class(kind), " { "] {
            out.push_str(part);
        }
        put_look(out, look);
        for part in [" } /* ", kind_name(kind), " */"] {
            out.push_str(part);
        }
    }
}

/// Whether a style's dict names the kind or one below it: Pygments styles the standard
/// types and the ones a style names, with their ancestors.
fn style_names(info: &StyleInfo, kind: TokenKind) -> bool {
    info.rules.iter().any(|r| kind_is(r.0, kind))
}

/// The one class a stylesheet rule selects: the last of the kind's classes.
fn css_class(kind: TokenKind) -> &'static str {
    let class = kind_class(kind);
    class.rsplit(' ').next().unwrap_or(class)
}

/// Writes a look's declarations without the final `"; "`, as Pygments does.
fn put_look(out: &mut String, l: Look) {
    let mut first = true;
    let mut sep = |out: &mut String| {
        if !first {
            out.push_str("; ");
        }
        first = false;
    };
    if let Some(c) = l.color {
        sep(out);
        out.push_str("color: ");
        put_color(out, c);
    }
    if l.bold {
        sep(out);
        out.push_str("font-weight: bold");
    }
    if l.italic {
        sep(out);
        out.push_str("font-style: italic");
    }
    if l.underline {
        sep(out);
        out.push_str("text-decoration: underline");
    }
    if let Some(c) = l.background {
        sep(out);
        out.push_str("background-color: ");
        put_color(out, c);
    }
    if let Some(c) = l.border {
        sep(out);
        out.push_str("border: 1px solid ");
        put_color(out, c);
    }
}

/// Pygments' stylesheet order: by depth in the hierarchy, then by name.
fn css_order() -> [TokenKind; TokenKind::COUNT] {
    let mut order = TokenKind::ALL;
    order.sort_by(|&a, &b| {
        kind_depth(a).cmp(&kind_depth(b)).then_with(|| kind_name(a).cmp(kind_name(b)))
    });
    order
}

fn kind_depth(kind: TokenKind) -> usize {
    1 + kind_name(kind).bytes().filter(|&b| b == b'.').count()
}

/// Writes a token colour as Pygments' webify does: upper case, and `#AABBCC` shortened
/// to `#ABC`.
fn put_color(out: &mut String, c: Color) {
    let hex = c.0.map(|h| h.to_ascii_uppercase());
    out.push('#');
    if hex[0] == hex[1] && hex[2] == hex[3] && hex[4] == hex[5] {
        for i in [0, 2, 4] {
            out.push(char::from(hex[i]));
        }
    } else {
        hex.iter().for_each(|&h| out.push(char::from(h)));
    }
}

/// Whether the kind is in Pygments' standard set.
pub(crate) fn is_standard(kind: TokenKind) -> bool {
    kind <= LAST_STANDARD_KIND
}
