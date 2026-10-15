//! The styles' definitions, generated from the style modules of Pygments 2.19.2 and
//! Sphinx 9.1.0 (both BSD-licensed) by `cargo xtask tables`; do not edit by hand.

use crate::styles::{StyleInfo, StyleRule};
use crate::token::TokenKind as K;

/// A `pygments_style`: every style Pygments ships, and Sphinx's `sphinx` and `none`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Style {
    /// Pygments' `abap` style.
    Abap,
    /// Pygments' `algol` style.
    Algol,
    /// Pygments' `algol_nu` style.
    AlgolNu,
    /// Pygments' `arduino` style.
    Arduino,
    /// Pygments' `autumn` style.
    Autumn,
    /// Pygments' `borland` style.
    Borland,
    /// Pygments' `bw` style.
    Bw,
    /// Pygments' `coffee` style.
    Coffee,
    /// Pygments' `colorful` style.
    Colorful,
    /// Pygments' `default` style.
    Default,
    /// Pygments' `dracula` style.
    Dracula,
    /// Pygments' `emacs` style.
    Emacs,
    /// Pygments' `friendly` style.
    Friendly,
    /// Pygments' `friendly_grayscale` style.
    FriendlyGrayscale,
    /// Pygments' `fruity` style.
    Fruity,
    /// Pygments' `github-dark` style.
    GithubDark,
    /// Pygments' `gruvbox-dark` style.
    GruvboxDark,
    /// Pygments' `gruvbox-light` style.
    GruvboxLight,
    /// Pygments' `igor` style.
    Igor,
    /// Pygments' `inkpot` style.
    Inkpot,
    /// Pygments' `lightbulb` style.
    Lightbulb,
    /// Pygments' `lilypond` style.
    Lilypond,
    /// Pygments' `lovelace` style.
    Lovelace,
    /// Pygments' `manni` style.
    Manni,
    /// Pygments' `material` style.
    Material,
    /// Pygments' `monokai` style.
    Monokai,
    /// Pygments' `murphy` style.
    Murphy,
    /// Pygments' `native` style.
    Native,
    /// Pygments' `none` style.
    None,
    /// Pygments' `nord` style.
    Nord,
    /// Pygments' `nord-darker` style.
    NordDarker,
    /// Pygments' `one-dark` style.
    OneDark,
    /// Pygments' `paraiso-dark` style.
    ParaisoDark,
    /// Pygments' `paraiso-light` style.
    ParaisoLight,
    /// Pygments' `pastie` style.
    Pastie,
    /// Pygments' `perldoc` style.
    Perldoc,
    /// Pygments' `rainbow_dash` style.
    RainbowDash,
    /// Pygments' `rrt` style.
    Rrt,
    /// Pygments' `sas` style.
    Sas,
    /// Pygments' `solarized-dark` style.
    SolarizedDark,
    /// Pygments' `solarized-light` style.
    SolarizedLight,
    /// Pygments' `sphinx` style.
    #[default]
    Sphinx,
    /// Pygments' `staroffice` style.
    Staroffice,
    /// Pygments' `stata-dark` style.
    StataDark,
    /// Pygments' `stata-light` style.
    StataLight,
    /// Pygments' `tango` style.
    Tango,
    /// Pygments' `trac` style.
    Trac,
    /// Pygments' `vim` style.
    Vim,
    /// Pygments' `vs` style.
    Vs,
    /// Pygments' `xcode` style.
    Xcode,
    /// Pygments' `zenburn` style.
    Zenburn,
}

impl Style {
    /// Every style, in declaration order.
    pub const ALL: [Style; 51] = [
        Style::Abap,
        Style::Algol,
        Style::AlgolNu,
        Style::Arduino,
        Style::Autumn,
        Style::Borland,
        Style::Bw,
        Style::Coffee,
        Style::Colorful,
        Style::Default,
        Style::Dracula,
        Style::Emacs,
        Style::Friendly,
        Style::FriendlyGrayscale,
        Style::Fruity,
        Style::GithubDark,
        Style::GruvboxDark,
        Style::GruvboxLight,
        Style::Igor,
        Style::Inkpot,
        Style::Lightbulb,
        Style::Lilypond,
        Style::Lovelace,
        Style::Manni,
        Style::Material,
        Style::Monokai,
        Style::Murphy,
        Style::Native,
        Style::None,
        Style::Nord,
        Style::NordDarker,
        Style::OneDark,
        Style::ParaisoDark,
        Style::ParaisoLight,
        Style::Pastie,
        Style::Perldoc,
        Style::RainbowDash,
        Style::Rrt,
        Style::Sas,
        Style::SolarizedDark,
        Style::SolarizedLight,
        Style::Sphinx,
        Style::Staroffice,
        Style::StataDark,
        Style::StataLight,
        Style::Tango,
        Style::Trac,
        Style::Vim,
        Style::Vs,
        Style::Xcode,
        Style::Zenburn,
    ];
}

/// The definitions of every style, in [`Style::ALL`] order.
#[rustfmt::skip]
pub(crate) static STYLE_INFOS: [StyleInfo; 51] = [
    StyleInfo {
        name: "abap", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "", rules: ABAP_RULES,
    },
    StyleInfo {
        name: "algol", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: ALGOL_RULES,
    },
    StyleInfo {
        name: "algol_nu", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: ALGOL_NU_RULES,
    },
    StyleInfo {
        name: "arduino", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: ARDUINO_RULES,
    },
    StyleInfo {
        name: "autumn", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: AUTUMN_RULES,
    },
    StyleInfo {
        name: "borland", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: BORLAND_RULES,
    },
    StyleInfo {
        name: "bw", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "", rules: BW_RULES,
    },
    StyleInfo {
        name: "coffee", dark: true, background: "#262220", highlight: "#ddd0c0",
        line_number: "#4e4e4e", line_number_background: "transparent",
        special_line_number: "#8f9494", special_background: "#ffffc0", base: "#ddd0c0",
        rules: COFFEE_RULES,
    },
    StyleInfo {
        name: "colorful", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: COLORFUL_RULES,
    },
    StyleInfo {
        name: "default", dark: false, background: "#f8f8f8", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: DEFAULT_RULES,
    },
    StyleInfo {
        name: "dracula", dark: true, background: "#282a36", highlight: "#44475a",
        line_number: "#f1fa8c", line_number_background: "#44475a", special_line_number: "#50fa7b",
        special_background: "#6272a4", base: "", rules: DRACULA_RULES,
    },
    StyleInfo {
        name: "emacs", dark: false, background: "#f8f8f8", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: EMACS_RULES,
    },
    StyleInfo {
        name: "friendly", dark: false, background: "#f0f0f0", highlight: "#ffffcc",
        line_number: "#666666", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: FRIENDLY_RULES,
    },
    StyleInfo {
        name: "friendly_grayscale", dark: false, background: "#f0f0f0", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: FRIENDLY_GRAYSCALE_RULES,
    },
    StyleInfo {
        name: "fruity", dark: true, background: "#111111", highlight: "#333333",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "#ffffff",
        rules: FRUITY_RULES,
    },
    StyleInfo {
        name: "github-dark", dark: true, background: "#0d1117", highlight: "#6e7681",
        line_number: "#6e7681", line_number_background: "#0d1117", special_line_number: "#e6edf3",
        special_background: "#6e7681", base: "#e6edf3", rules: GITHUB_DARK_RULES,
    },
    StyleInfo {
        name: "gruvbox-dark", dark: true, background: "#282828", highlight: "#ebdbb2",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "#dddddd",
        rules: GRUVBOX_DARK_RULES,
    },
    StyleInfo {
        name: "gruvbox-light", dark: false, background: "#fbf1c7", highlight: "#3c3836",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: GRUVBOX_LIGHT_RULES,
    },
    StyleInfo {
        name: "igor", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "", rules: IGOR_RULES,
    },
    StyleInfo {
        name: "inkpot", dark: true, background: "#1e1e27", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: INKPOT_RULES,
    },
    StyleInfo {
        name: "lightbulb", dark: true, background: "#1d2331", highlight: "#6e7681",
        line_number: "#3c4354", line_number_background: "transparent",
        special_line_number: "#3c4354", special_background: "#ffffc0", base: "#d4d2c8",
        rules: LIGHTBULB_RULES,
    },
    StyleInfo {
        name: "lilypond", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: LILYPOND_RULES,
    },
    StyleInfo {
        name: "lovelace", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: LOVELACE_RULES,
    },
    StyleInfo {
        name: "manni", dark: false, background: "#f0f3f3", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: MANNI_RULES,
    },
    StyleInfo {
        name: "material", dark: true, background: "#263238", highlight: "#2C3B41",
        line_number: "#37474F", line_number_background: "#263238", special_line_number: "#607A86",
        special_background: "#263238", base: "", rules: MATERIAL_RULES,
    },
    StyleInfo {
        name: "monokai", dark: true, background: "#272822", highlight: "#49483e",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "#f8f8f2",
        rules: MONOKAI_RULES,
    },
    StyleInfo {
        name: "murphy", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: MURPHY_RULES,
    },
    StyleInfo {
        name: "native", dark: true, background: "#202020", highlight: "#404040",
        line_number: "#aaaaaa", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "#d0d0d0",
        rules: NATIVE_RULES,
    },
    StyleInfo {
        name: "none", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "", rules: NONE_RULES,
    },
    StyleInfo {
        name: "nord", dark: true, background: "#2E3440", highlight: "#3B4252",
        line_number: "#D8DEE9", line_number_background: "#242933", special_line_number: "#242933",
        special_background: "#D8DEE9", base: "#d8dee9", rules: NORD_RULES,
    },
    StyleInfo {
        name: "nord-darker", dark: true, background: "#242933", highlight: "#3B4252",
        line_number: "#D8DEE9", line_number_background: "#242933", special_line_number: "#242933",
        special_background: "#D8DEE9", base: "#d8dee9", rules: NORD_DARKER_RULES,
    },
    StyleInfo {
        name: "one-dark", dark: true, background: "#282C34", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "#ABB2BF",
        rules: ONE_DARK_RULES,
    },
    StyleInfo {
        name: "paraiso-dark", dark: true, background: "#2f1e2e", highlight: "#4f424c",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: PARAISO_DARK_RULES,
    },
    StyleInfo {
        name: "paraiso-light", dark: false, background: "#e7e9db", highlight: "#a39e9b",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: PARAISO_LIGHT_RULES,
    },
    StyleInfo {
        name: "pastie", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: PASTIE_RULES,
    },
    StyleInfo {
        name: "perldoc", dark: false, background: "#eeeedd", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: PERLDOC_RULES,
    },
    StyleInfo {
        name: "rainbow_dash", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: RAINBOW_DASH_RULES,
    },
    StyleInfo {
        name: "rrt", dark: true, background: "#000000", highlight: "#0000ff",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "#dddddd",
        rules: RRT_RULES,
    },
    StyleInfo {
        name: "sas", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "", rules: SAS_RULES,
    },
    StyleInfo {
        name: "solarized-dark", dark: true, background: "#002b36", highlight: "#073642",
        line_number: "#586e75", line_number_background: "#073642", special_line_number: "#000000",
        special_background: "#ffffc0", base: "#839496", rules: SOLARIZED_DARK_RULES,
    },
    StyleInfo {
        name: "solarized-light", dark: false, background: "#fdf6e3", highlight: "#eee8d5",
        line_number: "#93a1a1", line_number_background: "#eee8d5", special_line_number: "#000000",
        special_background: "#ffffc0", base: "#657b83", rules: SOLARIZED_LIGHT_RULES,
    },
    StyleInfo {
        name: "sphinx", dark: false, background: "#eeffcc", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: SPHINX_RULES,
    },
    StyleInfo {
        name: "staroffice", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "#000080",
        rules: STAROFFICE_RULES,
    },
    StyleInfo {
        name: "stata-dark", dark: true, background: "#232629", highlight: "#49483e",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "#cccccc",
        rules: STATA_DARK_RULES,
    },
    StyleInfo {
        name: "stata-light", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: STATA_LIGHT_RULES,
    },
    StyleInfo {
        name: "tango", dark: false, background: "#f8f8f8", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: TANGO_RULES,
    },
    StyleInfo {
        name: "trac", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "", rules: TRAC_RULES,
    },
    StyleInfo {
        name: "vim", dark: true, background: "#000000", highlight: "#222222",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "#cccccc",
        rules: VIM_RULES,
    },
    StyleInfo {
        name: "vs", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "", rules: VS_RULES,
    },
    StyleInfo {
        name: "xcode", dark: false, background: "#ffffff", highlight: "#ffffcc",
        line_number: "inherit", line_number_background: "transparent",
        special_line_number: "#000000", special_background: "#ffffc0", base: "",
        rules: XCODE_RULES,
    },
    StyleInfo {
        name: "zenburn", dark: true, background: "#3f3f3f", highlight: "#484848",
        line_number: "#5d6262", line_number_background: "#353535", special_line_number: "#7a8080",
        special_background: "#353535", base: "#dcdccc", rules: ZENBURN_RULES,
    },
];

#[rustfmt::skip]
const ABAP_RULES: &[StyleRule] = &[
    (K::Comment, "italic #888"), (K::CommentSpecial, "#888"), (K::Keyword, "#00f"),
    (K::OperatorWord, "#00f"), (K::Name, "#000"), (K::Number, "#3af"), (K::String, "#5a2"),
    (K::Error, "#F00"),
];

#[rustfmt::skip]
const ALGOL_RULES: &[StyleRule] = &[
    (K::Comment, "italic #888"), (K::CommentPreproc, "bold noitalic #888"),
    (K::CommentSpecial, "bold noitalic #888"), (K::Keyword, "underline bold"),
    (K::KeywordDeclaration, "italic"), (K::NameBuiltin, "bold italic"),
    (K::NameBuiltinPseudo, "bold italic"), (K::NameNamespace, "bold italic #666"),
    (K::NameClass, "bold italic #666"), (K::NameFunction, "bold italic #666"),
    (K::NameVariable, "bold italic #666"), (K::NameConstant, "bold italic #666"),
    (K::OperatorWord, "bold"), (K::String, "italic #666"), (K::Error, "border:#FF0000"),
];

#[rustfmt::skip]
const ALGOL_NU_RULES: &[StyleRule] = &[
    (K::Comment, "italic #888"), (K::CommentPreproc, "bold noitalic #888"),
    (K::CommentSpecial, "bold noitalic #888"), (K::Keyword, "bold"),
    (K::KeywordDeclaration, "italic"), (K::NameBuiltin, "bold italic"),
    (K::NameBuiltinPseudo, "bold italic"), (K::NameNamespace, "bold italic #666"),
    (K::NameClass, "bold italic #666"), (K::NameFunction, "bold italic #666"),
    (K::NameVariable, "bold italic #666"), (K::NameConstant, "bold italic #666"),
    (K::OperatorWord, "bold"), (K::String, "italic #666"), (K::Error, "border:#FF0000"),
];

#[rustfmt::skip]
const ARDUINO_RULES: &[StyleRule] = &[
    (K::Error, "#a61717"), (K::Comment, "#95a5a6"), (K::CommentPreproc, "#728E00"),
    (K::Keyword, "#728E00"), (K::KeywordConstant, "#00979D"), (K::KeywordPseudo, "#00979D"),
    (K::KeywordReserved, "#00979D"), (K::KeywordType, "#00979D"), (K::Operator, "#728E00"),
    (K::Name, "#434f54"), (K::NameBuiltin, "#728E00"), (K::NameFunction, "#D35400"),
    (K::NameOther, "#728E00"), (K::Number, "#8A7B52"), (K::String, "#7F8C8D"),
];

#[rustfmt::skip]
const AUTUMN_RULES: &[StyleRule] = &[
    (K::Whitespace, "#bbbbbb"), (K::Comment, "italic #aaaaaa"),
    (K::CommentPreproc, "noitalic #4c8317"), (K::CommentSpecial, "italic #0000aa"),
    (K::Keyword, "#0000aa"), (K::KeywordType, "#00aaaa"), (K::OperatorWord, "#0000aa"),
    (K::NameBuiltin, "#00aaaa"), (K::NameFunction, "#00aa00"), (K::NameClass, "underline #00aa00"),
    (K::NameNamespace, "underline #00aaaa"), (K::NameVariable, "#aa0000"),
    (K::NameConstant, "#aa0000"), (K::NameEntity, "bold #800"), (K::NameAttribute, "#1e90ff"),
    (K::NameTag, "bold #1e90ff"), (K::NameDecorator, "#888888"), (K::String, "#aa5500"),
    (K::StringSymbol, "#0000aa"), (K::StringRegex, "#009999"), (K::Number, "#009999"),
    (K::GenericHeading, "bold #000080"), (K::GenericSubheading, "bold #800080"),
    (K::GenericDeleted, "#aa0000"), (K::GenericInserted, "#00aa00"), (K::GenericError, "#aa0000"),
    (K::GenericEmph, "italic"), (K::GenericStrong, "bold"), (K::GenericEmphStrong, "bold italic"),
    (K::GenericPrompt, "#555555"), (K::GenericOutput, "#888888"), (K::GenericTraceback, "#aa0000"),
    (K::Error, "#F00 bg:#FAA"),
];

#[rustfmt::skip]
const BORLAND_RULES: &[StyleRule] = &[
    (K::Whitespace, "#bbbbbb"), (K::Comment, "italic #008800"),
    (K::CommentPreproc, "noitalic #008080"), (K::CommentSpecial, "noitalic bold"),
    (K::String, "#0000FF"), (K::StringChar, "#800080"), (K::Number, "#0000FF"),
    (K::Keyword, "bold #000080"), (K::OperatorWord, "bold"), (K::NameTag, "bold #000080"),
    (K::NameAttribute, "#FF0000"), (K::GenericHeading, "#999999"),
    (K::GenericSubheading, "#aaaaaa"), (K::GenericDeleted, "bg:#ffdddd #000000"),
    (K::GenericInserted, "bg:#ddffdd #000000"), (K::GenericError, "#aa0000"),
    (K::GenericEmph, "italic"), (K::GenericStrong, "bold"), (K::GenericEmphStrong, "bold italic"),
    (K::GenericPrompt, "#555555"), (K::GenericOutput, "#888888"), (K::GenericTraceback, "#aa0000"),
    (K::Error, "bg:#e3d2d2 #a61717"),
];

#[rustfmt::skip]
const BW_RULES: &[StyleRule] = &[
    (K::Comment, "italic"), (K::CommentPreproc, "noitalic"), (K::Keyword, "bold"),
    (K::KeywordPseudo, "nobold"), (K::KeywordType, "nobold"), (K::OperatorWord, "bold"),
    (K::NameClass, "bold"), (K::NameNamespace, "bold"), (K::NameException, "bold"),
    (K::NameEntity, "bold"), (K::NameTag, "bold"), (K::String, "italic"),
    (K::StringInterpol, "bold"), (K::StringEscape, "bold"), (K::GenericHeading, "bold"),
    (K::GenericSubheading, "bold"), (K::GenericEmph, "italic"), (K::GenericStrong, "bold"),
    (K::GenericEmphStrong, "bold italic"), (K::GenericPrompt, "bold"),
    (K::Error, "border:#FF0000"),
];

#[rustfmt::skip]
const COFFEE_RULES: &[StyleRule] = &[
    (K::Comment, "#70757A"), (K::CommentHashbang, "#8f9f9f"), (K::CommentPreproc, "#fdd0c0"),
    (K::CommentPreprocFile, "#c9b98f"), (K::CommentSpecial, "#af5f5f"), (K::Error, "#af5f5f"),
    (K::GenericDeleted, "#bb6868"), (K::GenericEmph, "italic"), (K::GenericError, "#af5f5f"),
    (K::GenericInserted, "#849155"), (K::GenericOutput, "#ddd0c0"), (K::GenericStrong, "bold"),
    (K::GenericTraceback, "#af5f5f"), (K::Keyword, "#919191"), (K::KeywordConstant, "#875f5f"),
    (K::KeywordDeclaration, "#875f5f"), (K::KeywordNamespace, "#875f5f"),
    (K::KeywordReserved, "#b46276"), (K::KeywordType, "#af875f"), (K::Literal, "#af875f"),
    (K::Name, "#ddd0c0"), (K::NameAttribute, "#ddd0c0"), (K::NameBuiltin, "#ddd0c0"),
    (K::NameBuiltinPseudo, "#87afaf"), (K::NameClass, "#875f5f"), (K::NameConstant, "#af8787"),
    (K::NameDecorator, "#fdd0c0"), (K::NameEntity, "#ddd0c0"), (K::NameException, "#877575"),
    (K::NameFunction, "#fdd0c0"), (K::NameFunctionMagic, "#fdd0c0"), (K::NameOther, "#ddd0c0"),
    (K::NameProperty, "#dfaf87"), (K::NameTag, "#87afaf"), (K::NameVariable, "#ddd0c0"),
    (K::Number, "#87afaf"), (K::Operator, "#878787"), (K::OperatorWord, "#878787"),
    (K::Punctuation, "#ddd0c0"), (K::String, "#c9b98f"), (K::StringAffix, "#dfaf87"),
    (K::StringDoc, "#878787"), (K::StringEscape, "#af5f5f"), (K::StringInterpol, "#af5f5f"),
    (K::StringOther, "#fdd0c0"), (K::StringRegex, "#af5f5f"), (K::StringSymbol, "#af5f5f"),
];

#[rustfmt::skip]
const COLORFUL_RULES: &[StyleRule] = &[
    (K::Whitespace, "#bbbbbb"), (K::Comment, "#888"), (K::CommentPreproc, "#579"),
    (K::CommentSpecial, "bold #cc0000"), (K::Keyword, "bold #080"), (K::KeywordPseudo, "#038"),
    (K::KeywordType, "#339"), (K::Operator, "#333"), (K::OperatorWord, "bold #000"),
    (K::NameBuiltin, "#007020"), (K::NameFunction, "bold #06B"), (K::NameClass, "bold #B06"),
    (K::NameNamespace, "bold #0e84b5"), (K::NameException, "bold #F00"), (K::NameVariable, "#963"),
    (K::NameVariableInstance, "#33B"), (K::NameVariableClass, "#369"),
    (K::NameVariableGlobal, "bold #d70"), (K::NameConstant, "bold #036"),
    (K::NameLabel, "bold #970"), (K::NameEntity, "bold #800"), (K::NameAttribute, "#00C"),
    (K::NameTag, "#070"), (K::NameDecorator, "bold #555"), (K::String, "bg:#fff0f0"),
    (K::StringChar, "#04D bg:"), (K::StringDoc, "#D42 bg:"), (K::StringInterpol, "bg:#eee"),
    (K::StringEscape, "bold #666"), (K::StringRegex, "bg:#fff0ff #000"),
    (K::StringSymbol, "#A60 bg:"), (K::StringOther, "#D20"), (K::Number, "bold #60E"),
    (K::NumberInteger, "bold #00D"), (K::NumberFloat, "bold #60E"), (K::NumberHex, "bold #058"),
    (K::NumberOct, "bold #40E"), (K::GenericHeading, "bold #000080"),
    (K::GenericSubheading, "bold #800080"), (K::GenericDeleted, "#A00000"),
    (K::GenericInserted, "#00A000"), (K::GenericError, "#FF0000"), (K::GenericEmph, "italic"),
    (K::GenericStrong, "bold"), (K::GenericEmphStrong, "bold italic"),
    (K::GenericPrompt, "bold #c65d09"), (K::GenericOutput, "#888"), (K::GenericTraceback, "#04D"),
    (K::Error, "#F00 bg:#FAA"),
];

#[rustfmt::skip]
const DEFAULT_RULES: &[StyleRule] = &[
    (K::Whitespace, "#bbbbbb"), (K::Comment, "italic #3D7B7B"),
    (K::CommentPreproc, "noitalic #9C6500"), (K::Keyword, "bold #008000"),
    (K::KeywordPseudo, "nobold"), (K::KeywordType, "nobold #B00040"), (K::Operator, "#666666"),
    (K::OperatorWord, "bold #AA22FF"), (K::NameBuiltin, "#008000"), (K::NameFunction, "#0000FF"),
    (K::NameClass, "bold #0000FF"), (K::NameNamespace, "bold #0000FF"),
    (K::NameException, "bold #CB3F38"), (K::NameVariable, "#19177C"), (K::NameConstant, "#880000"),
    (K::NameLabel, "#767600"), (K::NameEntity, "bold #717171"), (K::NameAttribute, "#687822"),
    (K::NameTag, "bold #008000"), (K::NameDecorator, "#AA22FF"), (K::String, "#BA2121"),
    (K::StringDoc, "italic"), (K::StringInterpol, "bold #A45A77"),
    (K::StringEscape, "bold #AA5D1F"), (K::StringRegex, "#A45A77"), (K::StringSymbol, "#19177C"),
    (K::StringOther, "#008000"), (K::Number, "#666666"), (K::GenericHeading, "bold #000080"),
    (K::GenericSubheading, "bold #800080"), (K::GenericDeleted, "#A00000"),
    (K::GenericInserted, "#008400"), (K::GenericError, "#E40000"), (K::GenericEmph, "italic"),
    (K::GenericStrong, "bold"), (K::GenericEmphStrong, "bold italic"),
    (K::GenericPrompt, "bold #000080"), (K::GenericOutput, "#717171"),
    (K::GenericTraceback, "#04D"), (K::Error, "border:#FF0000"),
];

#[rustfmt::skip]
const DRACULA_RULES: &[StyleRule] = &[
    (K::Whitespace, "#f8f8f2"), (K::Comment, "#6272a4"), (K::CommentPreproc, "#ff79c6"),
    (K::Generic, "#f8f8f2"), (K::GenericDeleted, "#8b080b"), (K::GenericEmph, "underline"),
    (K::GenericHeading, "bold"), (K::GenericInserted, "bold"), (K::GenericOutput, "#44475a"),
    (K::GenericEmphStrong, "underline"), (K::GenericSubheading, "bold"), (K::Error, "#f8f8f2"),
    (K::Keyword, "#ff79c6"), (K::KeywordConstant, "#ff79c6"),
    (K::KeywordDeclaration, "#8be9fd italic"), (K::KeywordType, "#8be9fd"),
    (K::Literal, "#f8f8f2"), (K::Name, "#f8f8f2"), (K::NameAttribute, "#50fa7b"),
    (K::NameBuiltin, "#8be9fd italic"), (K::NameBuiltinPseudo, "#f8f8f2"),
    (K::NameClass, "#50fa7b"), (K::NameFunction, "#50fa7b"), (K::NameLabel, "#8be9fd italic"),
    (K::NameTag, "#ff79c6"), (K::NameVariable, "#8be9fd italic"), (K::Number, "#ffb86c"),
    (K::Operator, "#ff79c6"), (K::Other, "#f8f8f2"), (K::Punctuation, "#f8f8f2"),
    (K::String, "#bd93f9"), (K::Text, "#f8f8f2"),
];

#[rustfmt::skip]
const EMACS_RULES: &[StyleRule] = &[
    (K::Whitespace, "#bbbbbb"), (K::Comment, "italic #008800"), (K::CommentPreproc, "noitalic"),
    (K::CommentSpecial, "noitalic bold"), (K::Keyword, "bold #AA22FF"),
    (K::KeywordPseudo, "nobold"), (K::KeywordType, "bold #00BB00"), (K::Operator, "#666666"),
    (K::OperatorWord, "bold #AA22FF"), (K::NameBuiltin, "#AA22FF"), (K::NameFunction, "#00A000"),
    (K::NameClass, "#0000FF"), (K::NameNamespace, "bold #0000FF"),
    (K::NameException, "bold #D2413A"), (K::NameVariable, "#B8860B"), (K::NameConstant, "#880000"),
    (K::NameLabel, "#A0A000"), (K::NameEntity, "bold #999999"), (K::NameAttribute, "#BB4444"),
    (K::NameTag, "bold #008000"), (K::NameDecorator, "#AA22FF"), (K::String, "#BB4444"),
    (K::StringDoc, "italic"), (K::StringInterpol, "bold #BB6688"),
    (K::StringEscape, "bold #BB6622"), (K::StringRegex, "#BB6688"), (K::StringSymbol, "#B8860B"),
    (K::StringOther, "#008000"), (K::Number, "#666666"), (K::GenericHeading, "bold #000080"),
    (K::GenericSubheading, "bold #800080"), (K::GenericDeleted, "#A00000"),
    (K::GenericInserted, "#00A000"), (K::GenericError, "#FF0000"), (K::GenericEmph, "italic"),
    (K::GenericStrong, "bold"), (K::GenericEmphStrong, "bold italic"),
    (K::GenericPrompt, "bold #000080"), (K::GenericOutput, "#888"), (K::GenericTraceback, "#04D"),
    (K::Error, "border:#FF0000"),
];

#[rustfmt::skip]
const FRIENDLY_RULES: &[StyleRule] = &[
    (K::Whitespace, "#bbbbbb"), (K::Comment, "italic #60a0b0"),
    (K::CommentPreproc, "noitalic #007020"), (K::CommentSpecial, "noitalic bg:#fff0f0"),
    (K::Keyword, "bold #007020"), (K::KeywordPseudo, "nobold"), (K::KeywordType, "nobold #902000"),
    (K::Operator, "#666666"), (K::OperatorWord, "bold #007020"), (K::NameBuiltin, "#007020"),
    (K::NameFunction, "#06287e"), (K::NameClass, "bold #0e84b5"),
    (K::NameNamespace, "bold #0e84b5"), (K::NameException, "#007020"),
    (K::NameVariable, "#bb60d5"), (K::NameConstant, "#60add5"), (K::NameLabel, "bold #002070"),
    (K::NameEntity, "bold #d55537"), (K::NameAttribute, "#4070a0"), (K::NameTag, "bold #062873"),
    (K::NameDecorator, "bold #555555"), (K::String, "#4070a0"), (K::StringDoc, "italic"),
    (K::StringInterpol, "italic #70a0d0"), (K::StringEscape, "bold #4070a0"),
    (K::StringRegex, "#235388"), (K::StringSymbol, "#517918"), (K::StringOther, "#c65d09"),
    (K::Number, "#40a070"), (K::GenericHeading, "bold #000080"),
    (K::GenericSubheading, "bold #800080"), (K::GenericDeleted, "#A00000"),
    (K::GenericInserted, "#00A000"), (K::GenericError, "#FF0000"), (K::GenericEmph, "italic"),
    (K::GenericStrong, "bold"), (K::GenericEmphStrong, "bold italic"),
    (K::GenericPrompt, "bold #c65d09"), (K::GenericOutput, "#888"), (K::GenericTraceback, "#04D"),
    (K::Error, "border:#FF0000"),
];

#[rustfmt::skip]
const FRIENDLY_GRAYSCALE_RULES: &[StyleRule] = &[
    (K::Whitespace, "#bbbbbb"), (K::Comment, "italic #959595"),
    (K::CommentPreproc, "noitalic #575757"), (K::CommentSpecial, "noitalic bg:#F4F4F4"),
    (K::Keyword, "bold #575757"), (K::KeywordPseudo, "nobold"), (K::KeywordType, "nobold #4F4F4F"),
    (K::Operator, "#666666"), (K::OperatorWord, "bold #575757"), (K::NameBuiltin, "#575757"),
    (K::NameFunction, "#3F3F3F"), (K::NameClass, "bold #7E7E7E"),
    (K::NameNamespace, "bold #7E7E7E"), (K::NameException, "#575757"),
    (K::NameVariable, "#9A9A9A"), (K::NameConstant, "#A5A5A5"), (K::NameLabel, "bold #363636"),
    (K::NameEntity, "bold #848484"), (K::NameAttribute, "#707070"), (K::NameTag, "bold #3B3B3B"),
    (K::NameDecorator, "bold #555555"), (K::String, "#717171"), (K::StringDoc, "italic"),
    (K::StringInterpol, "italic #9F9F9F"), (K::StringEscape, "bold #717171"),
    (K::StringRegex, "#575757"), (K::StringSymbol, "#676767"), (K::StringOther, "#7E7E7E"),
    (K::Number, "#888888"), (K::GenericHeading, "bold #373737"),
    (K::GenericSubheading, "bold #5A5A5A"), (K::GenericDeleted, "#545454"),
    (K::GenericInserted, "#7D7D7D"), (K::GenericError, "#898989"), (K::GenericEmph, "italic"),
    (K::GenericStrong, "bold"), (K::GenericEmphStrong, "bold italic"),
    (K::GenericPrompt, "bold #7E7E7E"), (K::GenericOutput, "#888888"),
    (K::GenericTraceback, "#6D6D6D"), (K::Error, "border:#898989"),
];

#[rustfmt::skip]
const FRUITY_RULES: &[StyleRule] = &[
    (K::Whitespace, "#888888"), (K::GenericOutput, "#444444 bg:#222222"),
    (K::Keyword, "#fb660a bold"), (K::KeywordPseudo, "nobold"), (K::Number, "#0086f7 bold"),
    (K::NameTag, "#fb660a bold"), (K::NameVariable, "#fb660a"),
    (K::Comment, "#008800 bg:#0f140f italic"), (K::NameAttribute, "#ff0086 bold"),
    (K::String, "#0086d2"), (K::NameFunction, "#ff0086 bold"), (K::GenericHeading, "#ffffff bold"),
    (K::KeywordType, "#cdcaa9 bold"), (K::GenericSubheading, "#ffffff bold"),
    (K::NameConstant, "#0086d2"), (K::CommentPreproc, "#ff0007 bold"),
];

#[rustfmt::skip]
const GITHUB_DARK_RULES: &[StyleRule] = &[
    (K::Error, "#f85149"), (K::Keyword, "#ff7b72"), (K::KeywordConstant, "#79c0ff"),
    (K::KeywordPseudo, "#79c0ff"), (K::Name, "#e6edf3"), (K::NameClass, "bold #f0883e"),
    (K::NameConstant, "bold #79c0ff"), (K::NameDecorator, "bold #d2a8ff"),
    (K::NameEntity, "#ffa657"), (K::NameException, "bold #f0883e"),
    (K::NameFunction, "bold #d2a8ff"), (K::NameLabel, "bold #79c0ff"),
    (K::NameNamespace, "#ff7b72"), (K::NameProperty, "#79c0ff"), (K::NameTag, "#7ee787"),
    (K::NameVariable, "#79c0ff"), (K::Literal, "#a5d6ff"), (K::LiteralDate, "#79c0ff"),
    (K::String, "#a5d6ff"), (K::StringAffix, "#79c0ff"), (K::StringDelimiter, "#79c0ff"),
    (K::StringEscape, "#79c0ff"), (K::StringHeredoc, "#79c0ff"), (K::StringRegex, "#79c0ff"),
    (K::Number, "#a5d6ff"), (K::Comment, "italic #8b949e"), (K::CommentPreproc, "bold #8b949e"),
    (K::CommentSpecial, "bold italic #8b949e"), (K::Operator, "bold #ff7b72"),
    (K::Generic, "#e6edf3"), (K::GenericDeleted, "bg:#490202 #ffa198"), (K::GenericEmph, "italic"),
    (K::GenericError, "#ffa198"), (K::GenericHeading, "bold #79c0ff"),
    (K::GenericInserted, "bg:#0f5323 #56d364"), (K::GenericOutput, "#8b949e"),
    (K::GenericPrompt, "#8b949e"), (K::GenericStrong, "bold"),
    (K::GenericEmphStrong, "bold italic"), (K::GenericSubheading, "#79c0ff"),
    (K::GenericTraceback, "#ff7b72"), (K::GenericUnderline, "underline"),
    (K::Whitespace, "#6e7681"),
];

#[rustfmt::skip]
const GRUVBOX_DARK_RULES: &[StyleRule] = &[
    (K::Comment, "italic #928374"), (K::CommentPreProc, "#8ec07c"),
    (K::CommentSpecial, "bold italic #ebdbb2"), (K::Keyword, "#fb4934"),
    (K::OperatorWord, "#fb4934"), (K::String, "#b8bb26"), (K::StringEscape, "#fe8019"),
    (K::Number, "#d3869b"), (K::NameBuiltin, "#fe8019"), (K::NameVariable, "#83a598"),
    (K::NameConstant, "#d3869b"), (K::NameClass, "#8ec07c"), (K::NameFunction, "#8ec07c"),
    (K::NameNamespace, "#8ec07c"), (K::NameException, "#fb4934"), (K::NameTag, "#8ec07c"),
    (K::NameAttribute, "#fabd2f"), (K::NameDecorator, "#fb4934"),
    (K::GenericHeading, "bold #ebdbb2"), (K::GenericSubheading, "underline #ebdbb2"),
    (K::GenericDeleted, "bg:#fb4934 #282828"), (K::GenericInserted, "bg:#b8bb26 #282828"),
    (K::GenericError, "#fb4934"), (K::GenericEmph, "italic"), (K::GenericStrong, "bold"),
    (K::GenericEmphStrong, "bold italic"), (K::GenericPrompt, "#a89984"),
    (K::GenericOutput, "#f2e5bc"), (K::GenericTraceback, "#fb4934"),
    (K::Error, "bg:#fb4934 #282828"),
];

#[rustfmt::skip]
const GRUVBOX_LIGHT_RULES: &[StyleRule] = &[
    (K::Comment, "italic #928374"), (K::CommentPreProc, "#427b58"),
    (K::CommentSpecial, "bold italic #3c3836"), (K::Keyword, "#9d0006"),
    (K::OperatorWord, "#9d0006"), (K::String, "#79740e"), (K::StringEscape, "#af3a03"),
    (K::Number, "#8f3f71"), (K::NameBuiltin, "#af3a03"), (K::NameVariable, "#076678"),
    (K::NameConstant, "#8f3f71"), (K::NameClass, "#427b58"), (K::NameFunction, "#427b58"),
    (K::NameNamespace, "#427b58"), (K::NameException, "#9d0006"), (K::NameTag, "#427b58"),
    (K::NameAttribute, "#b57614"), (K::NameDecorator, "#9d0006"),
    (K::GenericHeading, "bold #3c3836"), (K::GenericSubheading, "underline #3c3836"),
    (K::GenericDeleted, "bg:#9d0006 #fbf1c7"), (K::GenericInserted, "bg:#79740e #fbf1c7"),
    (K::GenericError, "#9d0006"), (K::GenericEmph, "italic"), (K::GenericStrong, "bold"),
    (K::GenericPrompt, "#7c6f64"), (K::GenericOutput, "#32302f"), (K::GenericTraceback, "#9d0006"),
    (K::Error, "bg:#9d0006 #fbf1c7"),
];

#[rustfmt::skip]
const IGOR_RULES: &[StyleRule] = &[
    (K::Comment, "italic #FF0000"), (K::Keyword, "#0000FF"), (K::NameFunction, "#C34E00"),
    (K::NameDecorator, "#CC00A3"), (K::NameClass, "#007575"), (K::String, "#009C00"),
];

#[rustfmt::skip]
const INKPOT_RULES: &[StyleRule] = &[
    (K::Text, "#cfbfad"), (K::Other, "#cfbfad"), (K::Whitespace, "#434357"),
    (K::Comment, "#cd8b00"), (K::CommentPreproc, "#409090"),
    (K::CommentPreprocFile, "bg:#404040 #ffcd8b"), (K::CommentSpecial, "#808bed"),
    (K::Keyword, "#808bed"), (K::KeywordPseudo, "nobold"), (K::KeywordType, "#ff8bff"),
    (K::Operator, "#666666"), (K::Punctuation, "#cfbfad"), (K::Name, "#cfbfad"),
    (K::NameAttribute, "#cfbfad"), (K::NameBuiltinPseudo, "#ffff00"), (K::NameBuiltin, "#808bed"),
    (K::NameClass, "#ff8bff"), (K::NameConstant, "#409090"), (K::NameDecorator, "#409090"),
    (K::NameException, "#ff0000"), (K::NameFunction, "#c080d0"), (K::NameLabel, "#808bed"),
    (K::NameNamespace, "#ff0000"), (K::NameVariable, "#cfbfad"), (K::String, "bg:#404040 #ffcd8b"),
    (K::StringDoc, "#808bed"), (K::Number, "#f0ad6d"), (K::GenericHeading, "bold #000080"),
    (K::GenericSubheading, "bold #800080"), (K::GenericDeleted, "#A00000"),
    (K::GenericInserted, "#00A000"), (K::GenericError, "#FF0000"), (K::GenericEmph, "italic"),
    (K::GenericStrong, "bold"), (K::GenericEmphStrong, "bold italic"),
    (K::GenericPrompt, "bold #000080"), (K::GenericOutput, "#888"), (K::GenericTraceback, "#04D"),
    (K::Error, "bg:#6e2e2e #ffffff"),
];

#[rustfmt::skip]
const LIGHTBULB_RULES: &[StyleRule] = &[
    (K::Comment, "#7e8aa1"), (K::CommentHashbang, "italic #f88f7f"),
    (K::CommentPreproc, "bold #FFAD66"), (K::CommentSpecial, "italic #7e8aa1"),
    (K::Error, "#f88f7f"), (K::GenericDeleted, "bg:#3d1e20 #f88f7f"), (K::GenericEmph, "italic"),
    (K::GenericError, "#f88f7f"), (K::GenericInserted, "bg:#19362c #6ad4af"),
    (K::GenericOutput, "#7e8aa1"), (K::GenericStrong, "bold"), (K::GenericTraceback, "#f88f7f"),
    (K::Keyword, "#FFAD66"), (K::KeywordConstant, "#FFAD66"), (K::KeywordDeclaration, "#FFAD66"),
    (K::KeywordNamespace, "#FFAD66"), (K::KeywordReserved, "#FFAD66"), (K::KeywordType, "#73D0FF"),
    (K::Literal, "#D5FF80"), (K::Name, "#d4d2c8"), (K::NameAttribute, "#FFD173"),
    (K::NameBuiltin, "#FFD173"), (K::NameBuiltinPseudo, "#5CCFE6"), (K::NameClass, "#73D0FF"),
    (K::NameConstant, "#FFD173"), (K::NameDecorator, "bold italic #7e8aa1"),
    (K::NameEntity, "#95E6CB"), (K::NameException, "#73D0FF"), (K::NameFunction, "#FFD173"),
    (K::NameFunctionMagic, "#FFD173"), (K::NameOther, "#d4d2c8"), (K::NameProperty, "#FFD173"),
    (K::NameTag, "#5CCFE6"), (K::NameVariable, "#d4d2c8"), (K::Number, "#DFBFFF"),
    (K::Operator, "#FFAD66"), (K::OperatorWord, "#FFAD66"), (K::Punctuation, "#d4d2c8"),
    (K::String, "#D5FF80"), (K::StringAffix, "#F29E74"), (K::StringDoc, "#7e8aa1"),
    (K::StringEscape, "#95E6CB"), (K::StringInterpol, "#95E6CB"), (K::StringOther, "#95E6CB"),
    (K::StringRegex, "#95E6CB"), (K::StringSymbol, "#DFBFFF"),
];

#[rustfmt::skip]
const LILYPOND_RULES: &[StyleRule] = &[
    (K::Keyword, "bold"), (K::Comment, "italic #A3AAB2"), (K::TokenString, "#AB0909"),
    (K::TokenStringEscape, "#C46C6C"), (K::TokenStringSymbol, "noinherit"),
    (K::TokenNumber, "#976806"), (K::ChordModifier, "#976806"), (K::NameLvalue, "#08547A"),
    (K::NameBackslashReference, "#08547A"), (K::NameBuiltinMusicCommand, "bold #08547A"),
    (K::NameBuiltinPaperVariable, "bold #6C5A05"), (K::NameBuiltinHeaderVariable, "bold #6C5A05"),
    (K::NameBuiltinMusicFunction, "bold #08547A"), (K::NameBuiltinClef, "bold #08547A"),
    (K::NameBuiltinScale, "bold #08547A"), (K::NameBuiltinRepeatType, "#08547A"),
    (K::NameBuiltinDynamic, "#68175A"), (K::NameBuiltinArticulation, "#68175A"),
    (K::NameBuiltinSchemeFunction, "bold #A83401"), (K::NameBuiltinSchemeBuiltin, "bold"),
    (K::NameBuiltinMarkupCommand, "bold #831E71"), (K::NameBuiltinContext, "bold #038B8B"),
    (K::NameBuiltinContextProperty, "#038B8B"), (K::NameBuiltinGrob, "bold #0C7441"),
    (K::NameBuiltinGrobProperty, "#0C7441"), (K::NameBuiltinTranslator, "bold #6200A4"),
];

#[rustfmt::skip]
const LOVELACE_RULES: &[StyleRule] = &[
    (K::Whitespace, "#a89028"), (K::Comment, "italic #888888"), (K::CommentHashbang, "#287088"),
    (K::CommentMultiline, "#888888"), (K::CommentPreproc, "noitalic #289870"),
    (K::Keyword, "#2838b0"), (K::KeywordConstant, "italic #444444"),
    (K::KeywordDeclaration, "italic"), (K::KeywordType, "italic"), (K::Operator, "#666666"),
    (K::OperatorWord, "#a848a8"), (K::Punctuation, "#888888"), (K::NameAttribute, "#388038"),
    (K::NameBuiltin, "#388038"), (K::NameBuiltinPseudo, "italic"), (K::NameClass, "#287088"),
    (K::NameConstant, "#b85820"), (K::NameDecorator, "#287088"), (K::NameEntity, "#709030"),
    (K::NameException, "#908828"), (K::NameFunction, "#785840"), (K::NameFunctionMagic, "#b85820"),
    (K::NameLabel, "#289870"), (K::NameNamespace, "#289870"), (K::NameTag, "#2838b0"),
    (K::NameVariable, "#b04040"), (K::NameVariableGlobal, "#908828"),
    (K::NameVariableMagic, "#b85820"), (K::String, "#b83838"), (K::StringAffix, "#444444"),
    (K::StringChar, "#a848a8"), (K::StringDelimiter, "#b85820"), (K::StringDoc, "italic #b85820"),
    (K::StringEscape, "#709030"), (K::StringInterpol, "underline"), (K::StringOther, "#a848a8"),
    (K::StringRegex, "#a848a8"), (K::Number, "#444444"), (K::GenericDeleted, "#c02828"),
    (K::GenericEmph, "italic"), (K::GenericError, "#c02828"), (K::GenericHeading, "#666666"),
    (K::GenericSubheading, "#444444"), (K::GenericInserted, "#388038"),
    (K::GenericOutput, "#666666"), (K::GenericPrompt, "#444444"), (K::GenericStrong, "bold"),
    (K::GenericEmphStrong, "bold italic"), (K::GenericTraceback, "#2838b0"),
    (K::Error, "bg:#a848a8"),
];

#[rustfmt::skip]
const MANNI_RULES: &[StyleRule] = &[
    (K::Whitespace, "#bbbbbb"), (K::Comment, "italic #0099FF"),
    (K::CommentPreproc, "noitalic #009999"), (K::CommentSpecial, "bold"),
    (K::Keyword, "bold #006699"), (K::KeywordPseudo, "nobold"), (K::KeywordType, "#007788"),
    (K::Operator, "#555555"), (K::OperatorWord, "bold #000000"), (K::NameBuiltin, "#336666"),
    (K::NameFunction, "#CC00FF"), (K::NameClass, "bold #00AA88"),
    (K::NameNamespace, "bold #00CCFF"), (K::NameException, "bold #CC0000"),
    (K::NameVariable, "#003333"), (K::NameConstant, "#336600"), (K::NameLabel, "#9999FF"),
    (K::NameEntity, "bold #999999"), (K::NameAttribute, "#330099"), (K::NameTag, "bold #330099"),
    (K::NameDecorator, "#9999FF"), (K::String, "#CC3300"), (K::StringDoc, "italic"),
    (K::StringInterpol, "#AA0000"), (K::StringEscape, "bold #CC3300"), (K::StringRegex, "#33AAAA"),
    (K::StringSymbol, "#FFCC33"), (K::StringOther, "#CC3300"), (K::Number, "#FF6600"),
    (K::GenericHeading, "bold #003300"), (K::GenericSubheading, "bold #003300"),
    (K::GenericDeleted, "border:#CC0000 bg:#FFCCCC"),
    (K::GenericInserted, "border:#00CC00 bg:#CCFFCC"), (K::GenericError, "#FF0000"),
    (K::GenericEmph, "italic"), (K::GenericStrong, "bold"), (K::GenericEmphStrong, "bold italic"),
    (K::GenericPrompt, "bold #000099"), (K::GenericOutput, "#AAAAAA"),
    (K::GenericTraceback, "#99CC66"), (K::Error, "bg:#FFAAAA #AA0000"),
];

#[rustfmt::skip]
const MATERIAL_RULES: &[StyleRule] = &[
    (K::Text, "#EEFFFF"), (K::Escape, "#89DDFF"), (K::Error, "#FF5370"), (K::Keyword, "#BB80B3"),
    (K::KeywordConstant, "#89DDFF"), (K::KeywordDeclaration, "#BB80B3"),
    (K::KeywordNamespace, "italic #89DDFF"), (K::KeywordPseudo, "#89DDFF"),
    (K::KeywordType, "#BB80B3"), (K::Name, "#EEFFFF"), (K::NameAttribute, "#BB80B3"),
    (K::NameBuiltin, "#82AAFF"), (K::NameBuiltinPseudo, "#89DDFF"), (K::NameClass, "#FFCB6B"),
    (K::NameConstant, "#EEFFFF"), (K::NameDecorator, "#82AAFF"), (K::NameEntity, "#89DDFF"),
    (K::NameException, "#FFCB6B"), (K::NameFunction, "#82AAFF"), (K::NameFunctionMagic, "#82AAFF"),
    (K::NameLabel, "#82AAFF"), (K::NameProperty, "#FFCB6B"), (K::NameNamespace, "#FFCB6B"),
    (K::NameOther, "#EEFFFF"), (K::NameTag, "#FF5370"), (K::NameVariable, "#89DDFF"),
    (K::NameVariableClass, "#89DDFF"), (K::NameVariableGlobal, "#89DDFF"),
    (K::NameVariableInstance, "#89DDFF"), (K::NameVariableMagic, "#82AAFF"),
    (K::Literal, "#C3E88D"), (K::LiteralDate, "#C3E88D"), (K::String, "#C3E88D"),
    (K::StringAffix, "#BB80B3"), (K::StringBacktick, "#C3E88D"), (K::StringChar, "#C3E88D"),
    (K::StringDelimiter, "#EEFFFF"), (K::StringDoc, "italic #546E7A"),
    (K::StringDouble, "#C3E88D"), (K::StringEscape, "#EEFFFF"), (K::StringHeredoc, "#C3E88D"),
    (K::StringInterpol, "#89DDFF"), (K::StringOther, "#C3E88D"), (K::StringRegex, "#89DDFF"),
    (K::StringSingle, "#C3E88D"), (K::StringSymbol, "#89DDFF"), (K::Number, "#F78C6C"),
    (K::Operator, "#89DDFF"), (K::OperatorWord, "italic #89DDFF"), (K::Punctuation, "#89DDFF"),
    (K::Comment, "italic #546E7A"), (K::Generic, "#EEFFFF"), (K::GenericDeleted, "#FF5370"),
    (K::GenericEmph, "#89DDFF"), (K::GenericError, "#FF5370"), (K::GenericHeading, "#C3E88D"),
    (K::GenericInserted, "#C3E88D"), (K::GenericOutput, "#546E7A"), (K::GenericPrompt, "#FFCB6B"),
    (K::GenericStrong, "#FF5370"), (K::GenericEmphStrong, "#FFCB6B"),
    (K::GenericSubheading, "#89DDFF"), (K::GenericTraceback, "#FF5370"),
];

#[rustfmt::skip]
const MONOKAI_RULES: &[StyleRule] = &[
    (K::Error, "#ed007e bg:#1e0010"), (K::Comment, "#959077"), (K::Keyword, "#66d9ef"),
    (K::KeywordNamespace, "#ff4689"), (K::Operator, "#ff4689"), (K::Punctuation, "#f8f8f2"),
    (K::Name, "#f8f8f2"), (K::NameAttribute, "#a6e22e"), (K::NameClass, "#a6e22e"),
    (K::NameConstant, "#66d9ef"), (K::NameDecorator, "#a6e22e"), (K::NameException, "#a6e22e"),
    (K::NameFunction, "#a6e22e"), (K::NameOther, "#a6e22e"), (K::NameTag, "#ff4689"),
    (K::Number, "#ae81ff"), (K::Literal, "#ae81ff"), (K::LiteralDate, "#e6db74"),
    (K::String, "#e6db74"), (K::StringEscape, "#ae81ff"), (K::GenericDeleted, "#ff4689"),
    (K::GenericEmph, "italic"), (K::GenericInserted, "#a6e22e"), (K::GenericOutput, "#66d9ef"),
    (K::GenericPrompt, "bold #ff4689"), (K::GenericStrong, "bold"),
    (K::GenericEmphStrong, "bold italic"), (K::GenericSubheading, "#959077"),
];

#[rustfmt::skip]
const MURPHY_RULES: &[StyleRule] = &[
    (K::Whitespace, "#bbbbbb"), (K::Comment, "#666 italic"), (K::CommentPreproc, "#579 noitalic"),
    (K::CommentSpecial, "#c00 bold"), (K::Keyword, "bold #289"), (K::KeywordPseudo, "#08f"),
    (K::KeywordType, "#66f"), (K::Operator, "#333"), (K::OperatorWord, "bold #000"),
    (K::NameBuiltin, "#072"), (K::NameFunction, "bold #5ed"), (K::NameClass, "bold #e9e"),
    (K::NameNamespace, "bold #0e84b5"), (K::NameException, "bold #F00"), (K::NameVariable, "#036"),
    (K::NameVariableInstance, "#aaf"), (K::NameVariableClass, "#ccf"),
    (K::NameVariableGlobal, "#f84"), (K::NameConstant, "bold #5ed"), (K::NameLabel, "bold #970"),
    (K::NameEntity, "#800"), (K::NameAttribute, "#007"), (K::NameTag, "#070"),
    (K::NameDecorator, "bold #555"), (K::String, "bg:#e0e0ff"), (K::StringChar, "#88F bg:"),
    (K::StringDoc, "#D42 bg:"), (K::StringInterpol, "bg:#eee"), (K::StringEscape, "bold #666"),
    (K::StringRegex, "bg:#e0e0ff #000"), (K::StringSymbol, "#fc8 bg:"), (K::StringOther, "#f88"),
    (K::Number, "bold #60E"), (K::NumberInteger, "bold #66f"), (K::NumberFloat, "bold #60E"),
    (K::NumberHex, "bold #058"), (K::NumberOct, "bold #40E"), (K::GenericHeading, "bold #000080"),
    (K::GenericSubheading, "bold #800080"), (K::GenericDeleted, "#A00000"),
    (K::GenericInserted, "#00A000"), (K::GenericError, "#FF0000"), (K::GenericEmph, "italic"),
    (K::GenericStrong, "bold"), (K::GenericEmphStrong, "bold italic"),
    (K::GenericPrompt, "bold #c65d09"), (K::GenericOutput, "#888"), (K::GenericTraceback, "#04D"),
    (K::Error, "#F00 bg:#FAA"),
];

#[rustfmt::skip]
const NATIVE_RULES: &[StyleRule] = &[
    (K::Whitespace, "#666666"), (K::Comment, "italic #ababab"),
    (K::CommentPreproc, "noitalic bold #ff3a3a"),
    (K::CommentSpecial, "noitalic bold #e50808 bg:#520000"), (K::Keyword, "bold #6ebf26"),
    (K::KeywordPseudo, "nobold"), (K::OperatorWord, "bold #6ebf26"), (K::String, "#ed9d13"),
    (K::StringOther, "#ffa500"), (K::Number, "#51b2fd"), (K::NameBuiltin, "#2fbccd"),
    (K::NameVariable, "#40ffff"), (K::NameConstant, "#40ffff"),
    (K::NameClass, "underline #71adff"), (K::NameFunction, "#71adff"),
    (K::NameNamespace, "underline #71adff"), (K::NameException, "#bbbbbb"),
    (K::NameTag, "bold #6ebf26"), (K::NameAttribute, "#bbbbbb"), (K::NameDecorator, "#ffa500"),
    (K::GenericHeading, "bold #ffffff"), (K::GenericSubheading, "underline #ffffff"),
    (K::GenericDeleted, "#ff3a3a"), (K::GenericInserted, "#589819"), (K::GenericError, "#ff3a3a"),
    (K::GenericEmph, "italic"), (K::GenericStrong, "bold"), (K::GenericEmphStrong, "bold italic"),
    (K::GenericPrompt, "#aaaaaa"), (K::GenericOutput, "#cccccc"), (K::GenericTraceback, "#ff3a3a"),
    (K::Error, "bg:#e3d2d2 #a61717"),
];

#[rustfmt::skip]
const NONE_RULES: &[StyleRule] = &[
];

#[rustfmt::skip]
const NORD_RULES: &[StyleRule] = &[
    (K::Whitespace, "#d8dee9"), (K::Punctuation, "#eceff4"), (K::Comment, "italic #616e87"),
    (K::CommentPreproc, "#5e81ac"), (K::Keyword, "bold #81a1c1"),
    (K::KeywordPseudo, "nobold #81a1c1"), (K::KeywordType, "nobold #81a1c1"),
    (K::Operator, "bold #81a1c1"), (K::OperatorWord, "bold #81a1c1"), (K::Name, "#d8dee9"),
    (K::NameBuiltin, "#81a1c1"), (K::NameFunction, "#88c0d0"), (K::NameClass, "#8fbcbb"),
    (K::NameNamespace, "#8fbcbb"), (K::NameException, "#bf616a"), (K::NameVariable, "#d8dee9"),
    (K::NameConstant, "#8fbcbb"), (K::NameEntity, "#d08770"), (K::NameAttribute, "#8fbcbb"),
    (K::NameTag, "#81a1c1"), (K::NameDecorator, "#d08770"), (K::String, "#a3be8c"),
    (K::StringDoc, "#616e87"), (K::StringInterpol, "#a3be8c"), (K::StringEscape, "#ebcb8b"),
    (K::StringRegex, "#ebcb8b"), (K::StringSymbol, "#a3be8c"), (K::StringOther, "#a3be8c"),
    (K::Number, "#b48ead"), (K::GenericHeading, "bold #88c0d0"),
    (K::GenericSubheading, "bold #88c0d0"), (K::GenericDeleted, "#bf616a"),
    (K::GenericInserted, "#a3be8c"), (K::GenericError, "#bf616a"), (K::GenericEmph, "italic"),
    (K::GenericStrong, "bold"), (K::GenericEmphStrong, "bold italic"),
    (K::GenericPrompt, "bold #616e88"), (K::GenericOutput, "#d8dee9"),
    (K::GenericTraceback, "#bf616a"), (K::Error, "#bf616a"), (K::Text, "#d8dee9"),
];

#[rustfmt::skip]
const NORD_DARKER_RULES: &[StyleRule] = &[
    (K::Whitespace, "#d8dee9"), (K::Punctuation, "#eceff4"), (K::Comment, "italic #616e87"),
    (K::CommentPreproc, "#5e81ac"), (K::Keyword, "bold #81a1c1"),
    (K::KeywordPseudo, "nobold #81a1c1"), (K::KeywordType, "nobold #81a1c1"),
    (K::Operator, "bold #81a1c1"), (K::OperatorWord, "bold #81a1c1"), (K::Name, "#d8dee9"),
    (K::NameBuiltin, "#81a1c1"), (K::NameFunction, "#88c0d0"), (K::NameClass, "#8fbcbb"),
    (K::NameNamespace, "#8fbcbb"), (K::NameException, "#bf616a"), (K::NameVariable, "#d8dee9"),
    (K::NameConstant, "#8fbcbb"), (K::NameEntity, "#d08770"), (K::NameAttribute, "#8fbcbb"),
    (K::NameTag, "#81a1c1"), (K::NameDecorator, "#d08770"), (K::String, "#a3be8c"),
    (K::StringDoc, "#616e87"), (K::StringInterpol, "#a3be8c"), (K::StringEscape, "#ebcb8b"),
    (K::StringRegex, "#ebcb8b"), (K::StringSymbol, "#a3be8c"), (K::StringOther, "#a3be8c"),
    (K::Number, "#b48ead"), (K::GenericHeading, "bold #88c0d0"),
    (K::GenericSubheading, "bold #88c0d0"), (K::GenericDeleted, "#bf616a"),
    (K::GenericInserted, "#a3be8c"), (K::GenericError, "#bf616a"), (K::GenericEmph, "italic"),
    (K::GenericStrong, "bold"), (K::GenericPrompt, "bold #616e88"), (K::GenericOutput, "#d8dee9"),
    (K::GenericTraceback, "#bf616a"), (K::Error, "#bf616a"), (K::Text, "#d8dee9"),
];

#[rustfmt::skip]
const ONE_DARK_RULES: &[StyleRule] = &[
    (K::Punctuation, "#ABB2BF"), (K::PunctuationMarker, "#ABB2BF"), (K::Keyword, "#C678DD"),
    (K::KeywordConstant, "#E5C07B"), (K::KeywordDeclaration, "#C678DD"),
    (K::KeywordNamespace, "#C678DD"), (K::KeywordReserved, "#C678DD"), (K::KeywordType, "#E5C07B"),
    (K::Name, "#E06C75"), (K::NameAttribute, "#E06C75"), (K::NameBuiltin, "#E5C07B"),
    (K::NameClass, "#E5C07B"), (K::NameFunction, "bold #61AFEF"),
    (K::NameFunctionMagic, "bold #56B6C2"), (K::NameOther, "#E06C75"), (K::NameTag, "#E06C75"),
    (K::NameDecorator, "#61AFEF"), (K::String, "#98C379"), (K::Number, "#D19A66"),
    (K::Operator, "#56B6C2"), (K::Comment, "#7F848E"),
];

#[rustfmt::skip]
const PARAISO_DARK_RULES: &[StyleRule] = &[
    (K::Text, "#e7e9db"), (K::Error, "#ef6155"), (K::Comment, "#776e71"), (K::Keyword, "#815ba4"),
    (K::KeywordNamespace, "#5bc4bf"), (K::KeywordType, "#fec418"), (K::Operator, "#5bc4bf"),
    (K::Punctuation, "#e7e9db"), (K::Name, "#e7e9db"), (K::NameAttribute, "#06b6ef"),
    (K::NameClass, "#fec418"), (K::NameConstant, "#ef6155"), (K::NameDecorator, "#5bc4bf"),
    (K::NameException, "#ef6155"), (K::NameFunction, "#06b6ef"), (K::NameNamespace, "#fec418"),
    (K::NameOther, "#06b6ef"), (K::NameTag, "#5bc4bf"), (K::NameVariable, "#ef6155"),
    (K::Number, "#f99b15"), (K::Literal, "#f99b15"), (K::LiteralDate, "#48b685"),
    (K::String, "#48b685"), (K::StringChar, "#e7e9db"), (K::StringDoc, "#776e71"),
    (K::StringEscape, "#f99b15"), (K::StringInterpol, "#f99b15"), (K::GenericDeleted, "#ef6155"),
    (K::GenericEmph, "italic"), (K::GenericHeading, "bold #e7e9db"),
    (K::GenericInserted, "#48b685"), (K::GenericPrompt, "bold #776e71"),
    (K::GenericStrong, "bold"), (K::GenericEmphStrong, "bold italic"),
    (K::GenericSubheading, "bold #5bc4bf"),
];

#[rustfmt::skip]
const PARAISO_LIGHT_RULES: &[StyleRule] = &[
    (K::Text, "#2f1e2e"), (K::Error, "#ef6155"), (K::Comment, "#8d8687"), (K::Keyword, "#815ba4"),
    (K::KeywordNamespace, "#5bc4bf"), (K::KeywordType, "#fec418"), (K::Operator, "#5bc4bf"),
    (K::Punctuation, "#2f1e2e"), (K::Name, "#2f1e2e"), (K::NameAttribute, "#06b6ef"),
    (K::NameClass, "#fec418"), (K::NameConstant, "#ef6155"), (K::NameDecorator, "#5bc4bf"),
    (K::NameException, "#ef6155"), (K::NameFunction, "#06b6ef"), (K::NameNamespace, "#fec418"),
    (K::NameOther, "#06b6ef"), (K::NameTag, "#5bc4bf"), (K::NameVariable, "#ef6155"),
    (K::Number, "#f99b15"), (K::Literal, "#f99b15"), (K::LiteralDate, "#48b685"),
    (K::String, "#48b685"), (K::StringChar, "#2f1e2e"), (K::StringDoc, "#8d8687"),
    (K::StringEscape, "#f99b15"), (K::StringInterpol, "#f99b15"), (K::GenericDeleted, "#ef6155"),
    (K::GenericEmph, "italic"), (K::GenericHeading, "bold #2f1e2e"),
    (K::GenericInserted, "#48b685"), (K::GenericPrompt, "bold #8d8687"),
    (K::GenericStrong, "bold"), (K::GenericEmphStrong, "bold italic"),
    (K::GenericSubheading, "bold #5bc4bf"),
];

#[rustfmt::skip]
const PASTIE_RULES: &[StyleRule] = &[
    (K::Whitespace, "#bbbbbb"), (K::Comment, "#888888"), (K::CommentPreproc, "bold #cc0000"),
    (K::CommentSpecial, "bg:#fff0f0 bold #cc0000"), (K::String, "bg:#fff0f0 #dd2200"),
    (K::StringRegex, "bg:#fff0ff #008800"), (K::StringOther, "bg:#f0fff0 #22bb22"),
    (K::StringSymbol, "#aa6600"), (K::StringInterpol, "#3333bb"), (K::StringEscape, "#0044dd"),
    (K::OperatorWord, "#008800"), (K::Keyword, "bold #008800"), (K::KeywordPseudo, "nobold"),
    (K::KeywordType, "#888888"), (K::NameClass, "bold #bb0066"),
    (K::NameException, "bold #bb0066"), (K::NameFunction, "bold #0066bb"),
    (K::NameProperty, "bold #336699"), (K::NameNamespace, "bold #bb0066"),
    (K::NameBuiltin, "#003388"), (K::NameVariable, "#336699"), (K::NameVariableClass, "#336699"),
    (K::NameVariableInstance, "#3333bb"), (K::NameVariableGlobal, "#dd7700"),
    (K::NameConstant, "bold #003366"), (K::NameTag, "bold #bb0066"), (K::NameAttribute, "#336699"),
    (K::NameDecorator, "#555555"), (K::NameLabel, "italic #336699"), (K::Number, "bold #0000DD"),
    (K::GenericHeading, "#333"), (K::GenericSubheading, "#666"),
    (K::GenericDeleted, "bg:#ffdddd #000000"), (K::GenericInserted, "bg:#ddffdd #000000"),
    (K::GenericError, "#aa0000"), (K::GenericEmph, "italic"), (K::GenericStrong, "bold"),
    (K::GenericEmphStrong, "bold italic"), (K::GenericPrompt, "#555555"),
    (K::GenericOutput, "#888888"), (K::GenericTraceback, "#aa0000"),
    (K::Error, "bg:#e3d2d2 #a61717"),
];

#[rustfmt::skip]
const PERLDOC_RULES: &[StyleRule] = &[
    (K::Whitespace, "#bbbbbb"), (K::Comment, "#228B22"), (K::CommentPreproc, "#1e889b"),
    (K::CommentSpecial, "#8B008B bold"), (K::String, "#CD5555"),
    (K::StringHeredoc, "#1c7e71 italic"), (K::StringRegex, "#1c7e71"), (K::StringOther, "#cb6c20"),
    (K::Number, "#B452CD"), (K::OperatorWord, "#8B008B"), (K::Keyword, "#8B008B bold"),
    (K::KeywordType, "#00688B"), (K::NameClass, "#008b45 bold"),
    (K::NameException, "#008b45 bold"), (K::NameFunction, "#008b45"),
    (K::NameNamespace, "#008b45 underline"), (K::NameVariable, "#00688B"),
    (K::NameConstant, "#00688B"), (K::NameDecorator, "#707a7c"), (K::NameTag, "#8B008B bold"),
    (K::NameAttribute, "#658b00"), (K::NameBuiltin, "#658b00"),
    (K::GenericHeading, "bold #000080"), (K::GenericSubheading, "bold #800080"),
    (K::GenericDeleted, "#aa0000"), (K::GenericInserted, "#00aa00"), (K::GenericError, "#aa0000"),
    (K::GenericEmph, "italic"), (K::GenericStrong, "bold"), (K::GenericEmphStrong, "bold italic"),
    (K::GenericPrompt, "#555555"), (K::GenericOutput, "#888888"), (K::GenericTraceback, "#aa0000"),
    (K::Error, "bg:#e3d2d2 #a61717"),
];

#[rustfmt::skip]
const RAINBOW_DASH_RULES: &[StyleRule] = &[
    (K::Comment, "italic #0080ff"), (K::CommentPreproc, "noitalic"), (K::CommentSpecial, "bold"),
    (K::Error, "bg:#cc0000 #ffffff"), (K::GenericDeleted, "border:#c5060b bg:#ffcccc"),
    (K::GenericEmph, "italic"), (K::GenericError, "#ff0000"), (K::GenericHeading, "bold #2c5dcd"),
    (K::GenericInserted, "border:#00cc00 bg:#ccffcc"), (K::GenericOutput, "#aaaaaa"),
    (K::GenericPrompt, "bold #2c5dcd"), (K::GenericStrong, "bold"),
    (K::GenericEmphStrong, "bold italic"), (K::GenericSubheading, "bold #2c5dcd"),
    (K::GenericTraceback, "#c5060b"), (K::Keyword, "bold #2c5dcd"), (K::KeywordPseudo, "nobold"),
    (K::KeywordType, "#5918bb"), (K::NameAttribute, "italic #2c5dcd"),
    (K::NameBuiltin, "bold #5918bb"), (K::NameClass, "underline"), (K::NameConstant, "#318495"),
    (K::NameDecorator, "bold #ff8000"), (K::NameEntity, "bold #5918bb"),
    (K::NameException, "bold #5918bb"), (K::NameFunction, "bold #ff8000"),
    (K::NameTag, "bold #2c5dcd"), (K::Number, "bold #5918bb"), (K::Operator, "#2c5dcd"),
    (K::OperatorWord, "bold"), (K::String, "#00cc66"), (K::StringDoc, "italic"),
    (K::StringEscape, "bold #c5060b"), (K::StringOther, "#318495"),
    (K::StringSymbol, "bold #c5060b"), (K::Text, "#4d4d4d"), (K::Whitespace, "#cbcbcb"),
];

#[rustfmt::skip]
const RRT_RULES: &[StyleRule] = &[
    (K::Comment, "#00ff00"), (K::NameFunction, "#ffff00"), (K::NameVariable, "#eedd82"),
    (K::NameConstant, "#7fffd4"), (K::Keyword, "#ff0000"), (K::OperatorWord, "#ff0000"),
    (K::CommentPreproc, "#e5e5e5"), (K::String, "#87ceeb"), (K::KeywordType, "#ee82ee"),
    (K::Number, "#ff00ff"),
];

#[rustfmt::skip]
const SAS_RULES: &[StyleRule] = &[
    (K::Whitespace, "#bbbbbb"), (K::Comment, "italic #008800"), (K::String, "#800080"),
    (K::Number, "bold #2c8553"), (K::Other, "bg:#ffffe0"), (K::Keyword, "#2c2cff"),
    (K::KeywordReserved, "bold #353580"), (K::KeywordConstant, "bold"),
    (K::NameBuiltin, "#2c2cff"), (K::NameFunction, "bold italic"),
    (K::NameVariable, "bold #2c2cff"), (K::Generic, "#2c2cff"), (K::GenericEmph, "#008800"),
    (K::GenericError, "#d30202"), (K::Error, "bg:#e3d2d2 #a61717"),
];

#[rustfmt::skip]
const SOLARIZED_DARK_RULES: &[StyleRule] = &[
    (K::Comment, "italic #586e75"), (K::CommentHashbang, "#586e75"),
    (K::CommentMultiline, "#586e75"), (K::CommentPreproc, "noitalic #d33682"),
    (K::CommentPreprocFile, "noitalic #586e75"), (K::Keyword, "#859900"),
    (K::KeywordConstant, "#2aa198"), (K::KeywordDeclaration, "#2aa198"),
    (K::KeywordNamespace, "#cb4b16"), (K::KeywordType, "#b58900"), (K::Operator, "#586e75"),
    (K::OperatorWord, "#859900"), (K::NameBuiltin, "#268bd2"), (K::NameBuiltinPseudo, "#268bd2"),
    (K::NameClass, "#268bd2"), (K::NameConstant, "#268bd2"), (K::NameDecorator, "#268bd2"),
    (K::NameEntity, "#268bd2"), (K::NameException, "#268bd2"), (K::NameFunction, "#268bd2"),
    (K::NameFunctionMagic, "#268bd2"), (K::NameLabel, "#268bd2"), (K::NameNamespace, "#268bd2"),
    (K::NameTag, "#268bd2"), (K::NameVariable, "#268bd2"), (K::NameVariableGlobal, "#268bd2"),
    (K::NameVariableMagic, "#268bd2"), (K::String, "#2aa198"), (K::StringDoc, "#586e75"),
    (K::StringRegex, "#cb4b16"), (K::Number, "#2aa198"), (K::Generic, "#839496"),
    (K::GenericDeleted, "#dc322f"), (K::GenericEmph, "italic"), (K::GenericError, "#dc322f"),
    (K::GenericHeading, "bold"), (K::GenericSubheading, "underline"),
    (K::GenericInserted, "#859900"), (K::GenericOutput, "#839496"),
    (K::GenericPrompt, "bold #268bd2"), (K::GenericStrong, "bold"),
    (K::GenericEmphStrong, "bold italic"), (K::GenericTraceback, "#268bd2"),
    (K::Error, "bg:#dc322f"),
];

#[rustfmt::skip]
const SOLARIZED_LIGHT_RULES: &[StyleRule] = &[
    (K::Comment, "italic #93a1a1"), (K::CommentHashbang, "#93a1a1"),
    (K::CommentMultiline, "#93a1a1"), (K::CommentPreproc, "noitalic #d33682"),
    (K::CommentPreprocFile, "noitalic #93a1a1"), (K::Keyword, "#859900"),
    (K::KeywordConstant, "#2aa198"), (K::KeywordDeclaration, "#2aa198"),
    (K::KeywordNamespace, "#cb4b16"), (K::KeywordType, "#b58900"), (K::Operator, "#93a1a1"),
    (K::OperatorWord, "#859900"), (K::NameBuiltin, "#268bd2"), (K::NameBuiltinPseudo, "#268bd2"),
    (K::NameClass, "#268bd2"), (K::NameConstant, "#268bd2"), (K::NameDecorator, "#268bd2"),
    (K::NameEntity, "#268bd2"), (K::NameException, "#268bd2"), (K::NameFunction, "#268bd2"),
    (K::NameFunctionMagic, "#268bd2"), (K::NameLabel, "#268bd2"), (K::NameNamespace, "#268bd2"),
    (K::NameTag, "#268bd2"), (K::NameVariable, "#268bd2"), (K::NameVariableGlobal, "#268bd2"),
    (K::NameVariableMagic, "#268bd2"), (K::String, "#2aa198"), (K::StringDoc, "#93a1a1"),
    (K::StringRegex, "#cb4b16"), (K::Number, "#2aa198"), (K::Generic, "#657b83"),
    (K::GenericDeleted, "#dc322f"), (K::GenericEmph, "italic"), (K::GenericError, "#dc322f"),
    (K::GenericHeading, "bold"), (K::GenericSubheading, "underline"),
    (K::GenericInserted, "#859900"), (K::GenericOutput, "#657b83"),
    (K::GenericPrompt, "bold #268bd2"), (K::GenericStrong, "bold"),
    (K::GenericEmphStrong, "bold italic"), (K::GenericTraceback, "#268bd2"),
    (K::Error, "bg:#dc322f"),
];

#[rustfmt::skip]
const SPHINX_RULES: &[StyleRule] = &[
    (K::Whitespace, "#bbbbbb"), (K::Comment, "italic #408090"),
    (K::CommentPreproc, "noitalic #007020"), (K::CommentSpecial, "noitalic bg:#fff0f0"),
    (K::Keyword, "bold #007020"), (K::KeywordPseudo, "nobold"), (K::KeywordType, "nobold #902000"),
    (K::Operator, "#666666"), (K::OperatorWord, "bold #007020"), (K::NameBuiltin, "#007020"),
    (K::NameFunction, "#06287e"), (K::NameClass, "bold #0e84b5"),
    (K::NameNamespace, "bold #0e84b5"), (K::NameException, "#007020"),
    (K::NameVariable, "#bb60d5"), (K::NameConstant, "#60add5"), (K::NameLabel, "bold #002070"),
    (K::NameEntity, "bold #d55537"), (K::NameAttribute, "#4070a0"), (K::NameTag, "bold #062873"),
    (K::NameDecorator, "bold #555555"), (K::String, "#4070a0"), (K::StringDoc, "italic"),
    (K::StringInterpol, "italic #70a0d0"), (K::StringEscape, "bold #4070a0"),
    (K::StringRegex, "#235388"), (K::StringSymbol, "#517918"), (K::StringOther, "#c65d09"),
    (K::Number, "#208050"), (K::GenericHeading, "bold #000080"),
    (K::GenericSubheading, "bold #800080"), (K::GenericDeleted, "#A00000"),
    (K::GenericInserted, "#00A000"), (K::GenericError, "#FF0000"), (K::GenericEmph, "italic"),
    (K::GenericStrong, "bold"), (K::GenericEmphStrong, "bold italic"),
    (K::GenericPrompt, "bold #c65d09"), (K::GenericOutput, "#333"), (K::GenericTraceback, "#04D"),
    (K::Error, "border:#FF0000"),
];

#[rustfmt::skip]
const STAROFFICE_RULES: &[StyleRule] = &[
    (K::Comment, "#696969"), (K::Error, "#800000"), (K::Literal, "#EE0000"), (K::Name, "#008000"),
];

#[rustfmt::skip]
const STATA_DARK_RULES: &[StyleRule] = &[
    (K::Whitespace, "#bbbbbb"), (K::Error, "bg:#e3d2d2 #a61717"), (K::String, "#51cc99"),
    (K::Number, "#4FB8CC"), (K::NameFunction, "#6a6aff"), (K::NameOther, "#e2828e"),
    (K::Keyword, "bold #7686bb"), (K::Comment, "italic #777777"),
    (K::NameVariable, "bold #7AB4DB"), (K::NameVariableGlobal, "bold #BE646C"),
    (K::GenericPrompt, "#ffffff"),
];

#[rustfmt::skip]
const STATA_LIGHT_RULES: &[StyleRule] = &[
    (K::Text, "#111111"), (K::Whitespace, "#bbbbbb"), (K::Error, "bg:#e3d2d2 #a61717"),
    (K::String, "#7a2424"), (K::Number, "#2c2cff"), (K::NameFunction, "#2c2cff"),
    (K::NameOther, "#be646c"), (K::Keyword, "bold #353580"), (K::Comment, "italic #008800"),
    (K::NameVariable, "bold #35baba"), (K::NameVariableGlobal, "bold #b5565e"),
];

#[rustfmt::skip]
const TANGO_RULES: &[StyleRule] = &[
    (K::Whitespace, "#f8f8f8"), (K::Error, "#a40000 border:#ef2929"), (K::Other, "#000000"),
    (K::Comment, "italic #8f5902"), (K::CommentMultiline, "italic #8f5902"),
    (K::CommentPreproc, "italic #8f5902"), (K::CommentSingle, "italic #8f5902"),
    (K::CommentSpecial, "italic #8f5902"), (K::Keyword, "bold #204a87"),
    (K::KeywordConstant, "bold #204a87"), (K::KeywordDeclaration, "bold #204a87"),
    (K::KeywordNamespace, "bold #204a87"), (K::KeywordPseudo, "bold #204a87"),
    (K::KeywordReserved, "bold #204a87"), (K::KeywordType, "bold #204a87"),
    (K::Operator, "bold #ce5c00"), (K::OperatorWord, "bold #204a87"),
    (K::Punctuation, "bold #000000"), (K::Name, "#000000"), (K::NameAttribute, "#c4a000"),
    (K::NameBuiltin, "#204a87"), (K::NameBuiltinPseudo, "#3465a4"), (K::NameClass, "#000000"),
    (K::NameConstant, "#000000"), (K::NameDecorator, "bold #5c35cc"), (K::NameEntity, "#ce5c00"),
    (K::NameException, "bold #cc0000"), (K::NameFunction, "#000000"), (K::NameProperty, "#000000"),
    (K::NameLabel, "#f57900"), (K::NameNamespace, "#000000"), (K::NameOther, "#000000"),
    (K::NameTag, "bold #204a87"), (K::NameVariable, "#000000"), (K::NameVariableClass, "#000000"),
    (K::NameVariableGlobal, "#000000"), (K::NameVariableInstance, "#000000"),
    (K::Number, "bold #0000cf"), (K::NumberFloat, "bold #0000cf"), (K::NumberHex, "bold #0000cf"),
    (K::NumberInteger, "bold #0000cf"), (K::NumberIntegerLong, "bold #0000cf"),
    (K::NumberOct, "bold #0000cf"), (K::Literal, "#000000"), (K::LiteralDate, "#000000"),
    (K::String, "#4e9a06"), (K::StringBacktick, "#4e9a06"), (K::StringChar, "#4e9a06"),
    (K::StringDoc, "italic #8f5902"), (K::StringDouble, "#4e9a06"), (K::StringEscape, "#4e9a06"),
    (K::StringHeredoc, "#4e9a06"), (K::StringInterpol, "#4e9a06"), (K::StringOther, "#4e9a06"),
    (K::StringRegex, "#4e9a06"), (K::StringSingle, "#4e9a06"), (K::StringSymbol, "#4e9a06"),
    (K::Generic, "#000000"), (K::GenericDeleted, "#a40000"), (K::GenericEmph, "italic #000000"),
    (K::GenericError, "#ef2929"), (K::GenericHeading, "bold #000080"),
    (K::GenericInserted, "#00A000"), (K::GenericOutput, "italic #000000"),
    (K::GenericPrompt, "#8f5902"), (K::GenericStrong, "bold #000000"),
    (K::GenericEmphStrong, "bold italic #000000"), (K::GenericSubheading, "bold #800080"),
    (K::GenericTraceback, "bold #a40000"),
];

#[rustfmt::skip]
const TRAC_RULES: &[StyleRule] = &[
    (K::Whitespace, "#bbbbbb"), (K::Comment, "italic #999988"),
    (K::CommentPreproc, "bold noitalic #999999"), (K::CommentSpecial, "bold #999999"),
    (K::Operator, "bold"), (K::String, "#bb8844"), (K::StringRegex, "#808000"),
    (K::Number, "#009999"), (K::Keyword, "bold"), (K::KeywordType, "#445588"),
    (K::NameBuiltin, "#999999"), (K::NameFunction, "bold #990000"), (K::NameClass, "bold #445588"),
    (K::NameException, "bold #990000"), (K::NameNamespace, "#555555"),
    (K::NameVariable, "#008080"), (K::NameConstant, "#008080"), (K::NameTag, "#000080"),
    (K::NameAttribute, "#008080"), (K::NameEntity, "#800080"), (K::GenericHeading, "#999999"),
    (K::GenericSubheading, "#aaaaaa"), (K::GenericDeleted, "bg:#ffdddd #000000"),
    (K::GenericInserted, "bg:#ddffdd #000000"), (K::GenericError, "#aa0000"),
    (K::GenericEmph, "italic"), (K::GenericStrong, "bold"), (K::GenericEmphStrong, "bold italic"),
    (K::GenericPrompt, "#555555"), (K::GenericOutput, "#888888"), (K::GenericTraceback, "#aa0000"),
    (K::Error, "bg:#e3d2d2 #a61717"),
];

#[rustfmt::skip]
const VIM_RULES: &[StyleRule] = &[
    (K::Comment, "#000080"), (K::CommentSpecial, "bold #cd0000"), (K::Keyword, "#cdcd00"),
    (K::KeywordDeclaration, "#00cd00"), (K::KeywordNamespace, "#cd00cd"),
    (K::KeywordType, "#00cd00"), (K::Operator, "#3399cc"), (K::OperatorWord, "#cdcd00"),
    (K::NameClass, "#00cdcd"), (K::NameBuiltin, "#cd00cd"), (K::NameException, "bold #666699"),
    (K::NameVariable, "#00cdcd"), (K::String, "#cd0000"), (K::Number, "#cd00cd"),
    (K::GenericHeading, "bold #000080"), (K::GenericSubheading, "bold #800080"),
    (K::GenericDeleted, "#cd0000"), (K::GenericInserted, "#00cd00"), (K::GenericError, "#FF0000"),
    (K::GenericEmph, "italic"), (K::GenericStrong, "bold"), (K::GenericEmphStrong, "bold italic"),
    (K::GenericPrompt, "bold #000080"), (K::GenericOutput, "#888"), (K::GenericTraceback, "#04D"),
    (K::Error, "border:#FF0000"),
];

#[rustfmt::skip]
const VS_RULES: &[StyleRule] = &[
    (K::Comment, "#008000"), (K::CommentPreproc, "#0000ff"), (K::Keyword, "#0000ff"),
    (K::OperatorWord, "#0000ff"), (K::KeywordType, "#2b91af"), (K::NameClass, "#2b91af"),
    (K::String, "#a31515"), (K::GenericHeading, "bold"), (K::GenericSubheading, "bold"),
    (K::GenericEmph, "italic"), (K::GenericStrong, "bold"), (K::GenericEmphStrong, "bold italic"),
    (K::GenericPrompt, "bold"), (K::Error, "border:#FF0000"),
];

#[rustfmt::skip]
const XCODE_RULES: &[StyleRule] = &[
    (K::Comment, "#177500"), (K::CommentPreproc, "#633820"), (K::String, "#C41A16"),
    (K::StringChar, "#2300CE"), (K::Operator, "#000000"), (K::Keyword, "#A90D91"),
    (K::Name, "#000000"), (K::NameAttribute, "#836C28"), (K::NameClass, "#3F6E75"),
    (K::NameFunction, "#000000"), (K::NameBuiltin, "#A90D91"), (K::NameBuiltinPseudo, "#5B269A"),
    (K::NameVariable, "#000000"), (K::NameTag, "#000000"), (K::NameDecorator, "#000000"),
    (K::NameLabel, "#000000"), (K::Literal, "#1C01CE"), (K::Number, "#1C01CE"),
    (K::Error, "#000000"),
];

#[rustfmt::skip]
const ZENBURN_RULES: &[StyleRule] = &[
    (K::Error, "#e37170 bold"), (K::Keyword, "#efdcbc"), (K::KeywordType, "#dfdfbf bold"),
    (K::KeywordConstant, "#dca3a3"), (K::KeywordDeclaration, "#f0dfaf"),
    (K::KeywordNamespace, "#f0dfaf"), (K::Name, "#dcdccc"), (K::NameTag, "#e89393 bold"),
    (K::NameEntity, "#cfbfaf"), (K::NameConstant, "#dca3a3"), (K::NameClass, "#efef8f"),
    (K::NameFunction, "#efef8f"), (K::NameBuiltin, "#efef8f"), (K::NameBuiltinPseudo, "#dcdccc"),
    (K::NameAttribute, "#efef8f"), (K::NameException, "#c3bf9f bold"), (K::Literal, "#9fafaf"),
    (K::String, "#cc9393"), (K::StringDoc, "#7f9f7f"), (K::StringInterpol, "#dca3a3 bold"),
    (K::Number, "#8cd0d3"), (K::NumberFloat, "#c0bed1"), (K::Operator, "#f0efd0"),
    (K::Punctuation, "#f0efd0"), (K::Comment, "#7f9f7f italic"),
    (K::CommentPreproc, "#dfaf8f bold"), (K::CommentPreprocFile, "#cc9393"),
    (K::CommentSpecial, "#dfdfdf bold"), (K::Generic, "#ecbcbc bold"),
    (K::GenericEmph, "#ffffff bold"), (K::GenericOutput, "#5b605e bold"),
    (K::GenericHeading, "#efefef bold"), (K::GenericDeleted, "#c3bf9f bg:#313c36"),
    (K::GenericInserted, "#709080 bg:#313c36 bold"),
    (K::GenericTraceback, "#80d4aa bg:#2f2f2f bold"), (K::GenericSubheading, "#efefef bold"),
];
