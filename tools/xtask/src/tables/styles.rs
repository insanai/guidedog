//! gd-highlight's style table: a `StyleInfo` for every style Pygments ships and for
//! Sphinx's own `sphinx` and `none` styles, read from their Python modules (as data,
//! with [`super::python`]): each style class's name, its colours and its `styles` dict,
//! whose token types become `TokenKind` names through `KIND_INFO` in gd-highlight's
//! `token.rs`. Pygments' name for a style is its class's `name`; Sphinx's classes have
//! none, so they get the names Sphinx's `get_style` gives.

use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;

use super::Table;
use super::pack::{WIDTH, pack};
use super::python::{Program, Value};
use crate::fsutil::{read_text, sorted_entries};
use crate::{Failure, Result, fail};

/// The Pygments release whose style modules are read.
const PYGMENTS: &str = "pygments-2.19.2";
/// The Sphinx release whose `pygments_styles.py` is read.
const SPHINX: &str = "sphinx-9.1.0";
/// The token kinds' file, and the generated table.
const TOKEN_RS: &str = "crates/gd-highlight/src/token.rs";
const TARGET: &str = "crates/gd-highlight/src/style_tables.rs";

/// Sphinx's style classes, which have no name attribute, by the names `get_style` takes.
const SPHINX_NAMES: [(&str, &str); 2] = [("SphinxStyle", "sphinx"), ("NoneStyle", "none")];

/// The style a `Style` defaults to, Sphinx's own.
const DEFAULT_STYLE: &str = "sphinx";

/// Pygments' `Style` attributes that become `StyleInfo`'s colours, with `Style`'s
/// defaults, and the fields they fill.
const COLORS: [(&str, &str, &str); 6] = [
    ("background_color", "#ffffff", "background"),
    ("highlight_color", "#ffffcc", "highlight"),
    ("line_number_color", "inherit", "line_number"),
    ("line_number_background_color", "transparent", "line_number_background"),
    ("line_number_special_color", "#000000", "special_line_number"),
    ("line_number_special_background_color", "#ffffc0", "special_background"),
];

/// One style, ready to write.
#[derive(Debug)]
struct Style {
    /// Pygments' name, such as "github-dark".
    name: String,
    dark: bool,
    colors: [String; 6],
    /// The definition of the root token type.
    base: String,
    /// Each token kind's variant name with its definition, in the dict's order.
    rules: Vec<(String, String)>,
}

/// Generates `style_tables.rs` from the style modules in `tools/data`.
pub(super) fn generate(root: &Path) -> Result<Vec<Table>> {
    let data = root.join("tools/data");
    let mut program = Program::default();
    let mut paths = Vec::new();
    for path in sorted_entries(&data.join(PYGMENTS).join("styles"))? {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if name.ends_with(".py") && !name.starts_with('_') {
            paths.push(path);
        }
    }
    paths.push(data.join(SPHINX).join("pygments_styles.py"));
    for path in &paths {
        let shown = path.strip_prefix(root).unwrap_or(path).display().to_string();
        program.read_module(&shown, &read_text(path)?).map_err(|err| style_failure(&err))?;
    }
    let kinds = read_kind_names(&root.join(TOKEN_RS))?;
    let mut styles = Vec::new();
    for class in 0..program.classes.len() {
        if let Some(name) = style_name(&program, class)? {
            styles.push(make_style(&program, class, name, &kinds)?);
        }
    }
    styles.sort_by(|a, b| a.name.cmp(&b.name));
    if let Some(pair) = styles.windows(2).find(|pair| pair[0].name == pair[1].name) {
        return Err(style_failure(&format!("the style {} is defined twice", pair[0].name)));
    }
    Ok(vec![Table::new(TARGET.to_string(), write_tables(&styles))])
}

fn style_failure(message: &str) -> Failure {
    fail(
        "tables-styles",
        "UNREADABLE STYLE",
        &format!("I could not read the Pygments styles: {message}."),
        "The style modules in tools/data use Python this reader does not know; extend \
         tools/xtask/src/tables/python.rs, or check that the files are intact.",
    )
}

/// The name `pygments_style` gives a class, if it is a style.
fn style_name(program: &Program, class: usize) -> Result<Option<String>> {
    let mut current = class;
    while program.classes[current].base != "Style" {
        match program.find_class(&program.classes[current].base) {
            Some(base) => current = base,
            None => return Ok(None),
        }
    }
    let own = &program.classes[class].name;
    if let Some((_, name)) = SPHINX_NAMES.iter().find(|(c, _)| c == own) {
        return Ok(Some(name.to_string()));
    }
    match program.class_attr(class, "name").map_err(|err| style_failure(&err))? {
        Some(Value::Str(name)) => Ok(Some(name)),
        _ => Ok(None),
    }
}

/// Reads `KIND_INFO`'s rows, `(K::String, "s", "Literal.String", ...)`: Pygments'
/// dotted name to the `TokenKind` variant. The root, "", is `StyleInfo`'s base.
fn read_kind_names(path: &Path) -> Result<HashMap<String, String>> {
    let text = read_text(path)?;
    let start = text.find("static KIND_INFO").unwrap_or(text.len());
    let block = &text[start..];
    let block = &block[..block.find("\n];").unwrap_or(block.len())];
    let mut kinds = HashMap::new();
    // A row may span two lines; its second string is the dotted name.
    for row in block.split("(K::").skip(1) {
        let variant = row.split(',').next().unwrap_or_default();
        if let Some(dotted) = row.split('"').nth(3) {
            kinds.insert(dotted.to_string(), variant.to_string());
        }
    }
    if kinds.is_empty() {
        return Err(fail(
            "tables-kinds",
            "NO TOKEN KINDS",
            &format!(
                "{} has no KIND_INFO rows such as `(K::Name, \"n\", \"Name\", None),`.",
                path.display()
            ),
            "The style table maps Pygments' token types through KIND_INFO; restore it.",
        ));
    }
    kinds.insert(String::new(), String::new());
    Ok(kinds)
}

fn make_style(
    program: &Program,
    class: usize,
    name: String,
    kinds: &HashMap<String, String>,
) -> Result<Style> {
    let problem = |what: String| {
        style_failure(&format!("{}: the style {name} {what}", program.class_path(class)))
    };
    let mut colors: [String; 6] = Default::default();
    for (slot, (attr, default, _)) in colors.iter_mut().zip(COLORS) {
        *slot = match program.class_attr(class, attr).map_err(|err| style_failure(&err))? {
            None => default.to_string(),
            Some(Value::Str(color)) => color,
            Some(_) => return Err(problem(format!("sets {attr} to something not a string"))),
        };
    }
    let Some(dark) = is_dark(&colors[0]) else {
        return Err(problem(format!("has the background {}, not #rgb or #rrggbb", colors[0])));
    };
    let mut style =
        Style { name: name.clone(), dark, colors, base: String::new(), rules: Vec::new() };
    let entries = match program.class_attr(class, "styles").map_err(|err| style_failure(&err))? {
        None => return Ok(style),
        Some(Value::Dict(entries)) => entries,
        Some(_) => return Err(problem("has a styles that is not a dict".to_string())),
    };
    for (key, value) in entries {
        let (Value::Token(token), Value::Str(text)) = (key, value) else {
            return Err(problem("has a styles entry that is not token: string".to_string()));
        };
        let definition = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if definition.is_empty() {
            continue;
        }
        if let Some(word) = definition.split(' ').find(|w| !is_style_word(w)) {
            return Err(problem(format!("has the style word {word}")));
        }
        match kinds.get(&token).map(String::as_str) {
            None => return Err(problem(format!("uses {token}, which TokenKind lacks; add it"))),
            Some("") => style.base = definition,
            Some(kind) => style.rules.push((kind.to_string(), definition)),
        }
    }
    Ok(style)
}

/// Whether a background is dark by its luma, as a reader sees it; `None` when it is not
/// a colour.
fn is_dark(background: &str) -> Option<bool> {
    let hex = six_digits(background)?;
    let channel = |at: usize| u32::from_str_radix(&hex[at..at + 2], 16).unwrap_or(0);
    Some(299 * channel(0) + 587 * channel(2) + 114 * channel(4) < 128_000)
}

/// `#rgb` and `#rrggbb` as six hex digits.
fn six_digits(color: &str) -> Option<String> {
    let digits = color.strip_prefix('#')?;
    if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    match digits.len() {
        6 => Some(digits.to_string()),
        3 => Some(digits.chars().flat_map(|c| [c, c]).collect()),
        _ => None,
    }
}

/// The words Pygments' StyleMeta accepts, with the colours its colorformat accepts that
/// the CSS writer can write: #rgb, #rrggbb, and nothing after `bg:` and `border:`, which
/// clears the colour.
fn is_style_word(word: &str) -> bool {
    const WORDS: [&str; 10] = [
        "bold",
        "nobold",
        "italic",
        "noitalic",
        "underline",
        "nounderline",
        "noinherit",
        "roman",
        "sans",
        "mono",
    ];
    if WORDS.contains(&word) {
        return true;
    }
    let color = word.strip_prefix("bg:").or_else(|| word.strip_prefix("border:"));
    match color {
        Some("") => true,
        Some(color) => six_digits(color).is_some(),
        None => six_digits(word).is_some(),
    }
}

/// "github-dark" as an enum variant, `GithubDark`.
fn variant(name: &str) -> String {
    name.split(['-', '_'])
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|c| c.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}

/// "github-dark" as the name of its rules, `GITHUB_DARK_RULES`.
fn rules_name(name: &str) -> String {
    name.split(['-', '_']).map(str::to_ascii_uppercase).collect::<Vec<_>>().join("_") + "_RULES"
}

fn write_tables(styles: &[Style]) -> String {
    let mut out = String::from(
        "//! The styles' definitions, generated from the style modules of Pygments 2.19.2 and\n\
         //! Sphinx 9.1.0 (both BSD-licensed) by `cargo xtask tables`; do not edit by hand.\n\n\
         use crate::styles::{StyleInfo, StyleRule};\nuse crate::token::TokenKind as K;\n\n\
         /// A `pygments_style`: every style Pygments ships, and Sphinx's `sphinx` and `none`.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]\n\
         pub enum Style {\n",
    );
    for style in styles {
        let _ = writeln!(out, "    /// Pygments' `{}` style.", style.name);
        if style.name == DEFAULT_STYLE {
            out.push_str("    #[default]\n");
        }
        let _ = writeln!(out, "    {},", variant(&style.name));
    }
    let count = styles.len();
    let _ = write!(
        out,
        "}}\n\nimpl Style {{\n    /// Every style, in declaration order.\n    \
         pub const ALL: [Style; {count}] = [\n"
    );
    for style in styles {
        let _ = writeln!(out, "        Style::{},", variant(&style.name));
    }
    let _ = write!(
        out,
        "    ];\n}}\n\n/// The definitions of every style, in [`Style::ALL`] order.\n\
         #[rustfmt::skip]\n\
         pub(crate) static STYLE_INFOS: [StyleInfo; {count}] = [\n"
    );
    for style in styles {
        write_info(&mut out, style);
    }
    out.push_str("];\n");
    for style in styles {
        let _ = write!(
            out,
            "\n#[rustfmt::skip]\nconst {}: &[StyleRule] = &[\n",
            rules_name(&style.name)
        );
        let items =
            style.rules.iter().map(|(kind, definition)| format!("(K::{kind}, \"{definition}\"),"));
        pack(&mut out, "    ", WIDTH, items);
        out.push_str("];\n");
    }
    out
}

fn write_info(out: &mut String, style: &Style) {
    out.push_str("    StyleInfo {\n");
    let mut fields = vec![format!("name: \"{}\",", style.name), format!("dark: {},", style.dark)];
    for (color, (_, _, field)) in style.colors.iter().zip(COLORS) {
        fields.push(format!("{field}: \"{color}\","));
    }
    fields.push(format!("base: \"{}\",", style.base));
    fields.push(format!("rules: {},", rules_name(&style.name)));
    pack(out, "        ", WIDTH, fields);
    out.push_str("    },\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_colours() {
        assert_eq!(variant("github-dark"), "GithubDark");
        assert_eq!(rules_name("rainbow_dash"), "RAINBOW_DASH_RULES");
        assert_eq!(is_dark("#000"), Some(true));
        assert_eq!(is_dark("#ffffff"), Some(false));
        assert_eq!(is_dark("white"), None);
        assert!(is_style_word("bg:") && is_style_word("border:#f00") && !is_style_word("#ff"));
    }
}
