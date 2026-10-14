//! Token kinds with Pygments' names, CSS classes and hierarchy.

/// A run of the input: `code[start..end]` has `kind`.
///
/// The tokens a scanner yields are contiguous, cover every byte exactly once, never split a
/// UTF-8 character, and never put two neighbours of one kind side by side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Token {
    /// Byte offset of the first byte.
    pub start: u32,
    /// Byte offset just past the last byte.
    pub end: u32,
    /// What the run is.
    pub kind: TokenKind,
}

/// Pygments' token hierarchy.
///
/// Each kind has Pygments' dotted name ([`kind_name`]), its short CSS class
/// ([`kind_class`]) and its parent ([`kind_parent`]), so the stylesheets Sphinx themes
/// already ship style the output unchanged. The kinds after
/// [`TokenKind::GenericTraceback`] are subtypes only some lexers or styles use: the YAML
/// lexer's `Punctuation.Indicator` and `Literal.Scalar.Plain`, and the types only Pygments'
/// styles name (github-dark's `Generic.Underline`, gruvbox's `Comment.PreProc`, the LilyPond
/// lexer's types, which hang off the root).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u8)]
pub enum TokenKind {
    /// Pygments' `Text`; written without a span.
    #[default]
    Text,
    /// Pygments' `Text.Whitespace`.
    Whitespace,
    /// Pygments' `Escape`.
    Escape,
    /// Pygments' `Error`.
    Error,
    /// Pygments' `Other`.
    Other,
    /// Pygments' `Keyword`.
    Keyword,
    /// Pygments' `Keyword.Constant`.
    KeywordConstant,
    /// Pygments' `Keyword.Declaration`.
    KeywordDeclaration,
    /// Pygments' `Keyword.Namespace`.
    KeywordNamespace,
    /// Pygments' `Keyword.Pseudo`.
    KeywordPseudo,
    /// Pygments' `Keyword.Reserved`.
    KeywordReserved,
    /// Pygments' `Keyword.Type`.
    KeywordType,
    /// Pygments' `Name`.
    Name,
    /// Pygments' `Name.Attribute`.
    NameAttribute,
    /// Pygments' `Name.Builtin`.
    NameBuiltin,
    /// Pygments' `Name.Builtin.Pseudo`.
    NameBuiltinPseudo,
    /// Pygments' `Name.Class`.
    NameClass,
    /// Pygments' `Name.Constant`.
    NameConstant,
    /// Pygments' `Name.Decorator`.
    NameDecorator,
    /// Pygments' `Name.Entity`.
    NameEntity,
    /// Pygments' `Name.Exception`.
    NameException,
    /// Pygments' `Name.Function`.
    NameFunction,
    /// Pygments' `Name.Function.Magic`.
    NameFunctionMagic,
    /// Pygments' `Name.Property`.
    NameProperty,
    /// Pygments' `Name.Label`.
    NameLabel,
    /// Pygments' `Name.Namespace`.
    NameNamespace,
    /// Pygments' `Name.Other`.
    NameOther,
    /// Pygments' `Name.Tag`.
    NameTag,
    /// Pygments' `Name.Variable`.
    NameVariable,
    /// Pygments' `Name.Variable.Class`.
    NameVariableClass,
    /// Pygments' `Name.Variable.Global`.
    NameVariableGlobal,
    /// Pygments' `Name.Variable.Instance`.
    NameVariableInstance,
    /// Pygments' `Name.Variable.Magic`.
    NameVariableMagic,
    /// Pygments' `Literal`.
    Literal,
    /// Pygments' `Literal.Date`.
    LiteralDate,
    /// Pygments' `Literal.String`.
    String,
    /// Pygments' `Literal.String.Affix`.
    StringAffix,
    /// Pygments' `Literal.String.Backtick`.
    StringBacktick,
    /// Pygments' `Literal.String.Char`.
    StringChar,
    /// Pygments' `Literal.String.Delimiter`.
    StringDelimiter,
    /// Pygments' `Literal.String.Doc`.
    StringDoc,
    /// Pygments' `Literal.String.Double`.
    StringDouble,
    /// Pygments' `Literal.String.Escape`.
    StringEscape,
    /// Pygments' `Literal.String.Heredoc`.
    StringHeredoc,
    /// Pygments' `Literal.String.Interpol`.
    StringInterpol,
    /// Pygments' `Literal.String.Other`.
    StringOther,
    /// Pygments' `Literal.String.Regex`.
    StringRegex,
    /// Pygments' `Literal.String.Single`.
    StringSingle,
    /// Pygments' `Literal.String.Symbol`.
    StringSymbol,
    /// Pygments' `Literal.Number`.
    Number,
    /// Pygments' `Literal.Number.Bin`.
    NumberBin,
    /// Pygments' `Literal.Number.Float`.
    NumberFloat,
    /// Pygments' `Literal.Number.Hex`.
    NumberHex,
    /// Pygments' `Literal.Number.Integer`.
    NumberInteger,
    /// Pygments' `Literal.Number.Integer.Long`.
    NumberIntegerLong,
    /// Pygments' `Literal.Number.Oct`.
    NumberOct,
    /// Pygments' `Operator`.
    Operator,
    /// Pygments' `Operator.Word`.
    OperatorWord,
    /// Pygments' `Punctuation`.
    Punctuation,
    /// Pygments' `Punctuation.Marker`.
    PunctuationMarker,
    /// Pygments' `Comment`.
    Comment,
    /// Pygments' `Comment.Hashbang`.
    CommentHashbang,
    /// Pygments' `Comment.Multiline`.
    CommentMultiline,
    /// Pygments' `Comment.Preproc`.
    CommentPreproc,
    /// Pygments' `Comment.PreprocFile`.
    CommentPreprocFile,
    /// Pygments' `Comment.Single`.
    CommentSingle,
    /// Pygments' `Comment.Special`.
    CommentSpecial,
    /// Pygments' `Generic`.
    Generic,
    /// Pygments' `Generic.Deleted`.
    GenericDeleted,
    /// Pygments' `Generic.Emph`.
    GenericEmph,
    /// Pygments' `Generic.EmphStrong`.
    GenericEmphStrong,
    /// Pygments' `Generic.Error`.
    GenericError,
    /// Pygments' `Generic.Heading`.
    GenericHeading,
    /// Pygments' `Generic.Inserted`.
    GenericInserted,
    /// Pygments' `Generic.Output`.
    GenericOutput,
    /// Pygments' `Generic.Prompt`.
    GenericPrompt,
    /// Pygments' `Generic.Strong`.
    GenericStrong,
    /// Pygments' `Generic.Subheading`.
    GenericSubheading,
    /// Pygments' `Generic.Traceback`.
    GenericTraceback,
    /// Pygments' `Punctuation.Indicator`.
    PunctuationIndicator,
    /// Pygments' `Literal.Scalar.Plain`.
    LiteralScalarPlain,
    /// Pygments' `Generic.Underline`.
    GenericUnderline,
    /// Pygments' `Comment.PreProc`.
    CommentPreProc,
    /// Pygments' `Name.Lvalue`.
    NameLvalue,
    /// Pygments' `Name.BackslashReference`.
    NameBackslashReference,
    /// Pygments' `Name.Builtin.Articulation`.
    NameBuiltinArticulation,
    /// Pygments' `Name.Builtin.Clef`.
    NameBuiltinClef,
    /// Pygments' `Name.Builtin.Context`.
    NameBuiltinContext,
    /// Pygments' `Name.Builtin.ContextProperty`.
    NameBuiltinContextProperty,
    /// Pygments' `Name.Builtin.Dynamic`.
    NameBuiltinDynamic,
    /// Pygments' `Name.Builtin.Grob`.
    NameBuiltinGrob,
    /// Pygments' `Name.Builtin.GrobProperty`.
    NameBuiltinGrobProperty,
    /// Pygments' `Name.Builtin.HeaderVariable`.
    NameBuiltinHeaderVariable,
    /// Pygments' `Name.Builtin.MarkupCommand`.
    NameBuiltinMarkupCommand,
    /// Pygments' `Name.Builtin.MusicCommand`.
    NameBuiltinMusicCommand,
    /// Pygments' `Name.Builtin.MusicFunction`.
    NameBuiltinMusicFunction,
    /// Pygments' `Name.Builtin.PaperVariable`.
    NameBuiltinPaperVariable,
    /// Pygments' `Name.Builtin.RepeatType`.
    NameBuiltinRepeatType,
    /// Pygments' `Name.Builtin.Scale`.
    NameBuiltinScale,
    /// Pygments' `Name.Builtin.SchemeBuiltin`.
    NameBuiltinSchemeBuiltin,
    /// Pygments' `Name.Builtin.SchemeFunction`.
    NameBuiltinSchemeFunction,
    /// Pygments' `Name.Builtin.Translator`.
    NameBuiltinTranslator,
    /// Pygments' `String`.
    TokenString,
    /// Pygments' `String.Escape`.
    TokenStringEscape,
    /// Pygments' `String.Symbol`.
    TokenStringSymbol,
    /// Pygments' `Number`.
    TokenNumber,
    /// Pygments' `Pitch`.
    Pitch,
    /// Pygments' `ChordModifier`.
    ChordModifier,
}

use TokenKind as K;

impl TokenKind {
    /// Every kind, in declaration order.
    pub const ALL: [TokenKind; 108] = [
        K::Text,
        K::Whitespace,
        K::Escape,
        K::Error,
        K::Other,
        K::Keyword,
        K::KeywordConstant,
        K::KeywordDeclaration,
        K::KeywordNamespace,
        K::KeywordPseudo,
        K::KeywordReserved,
        K::KeywordType,
        K::Name,
        K::NameAttribute,
        K::NameBuiltin,
        K::NameBuiltinPseudo,
        K::NameClass,
        K::NameConstant,
        K::NameDecorator,
        K::NameEntity,
        K::NameException,
        K::NameFunction,
        K::NameFunctionMagic,
        K::NameProperty,
        K::NameLabel,
        K::NameNamespace,
        K::NameOther,
        K::NameTag,
        K::NameVariable,
        K::NameVariableClass,
        K::NameVariableGlobal,
        K::NameVariableInstance,
        K::NameVariableMagic,
        K::Literal,
        K::LiteralDate,
        K::String,
        K::StringAffix,
        K::StringBacktick,
        K::StringChar,
        K::StringDelimiter,
        K::StringDoc,
        K::StringDouble,
        K::StringEscape,
        K::StringHeredoc,
        K::StringInterpol,
        K::StringOther,
        K::StringRegex,
        K::StringSingle,
        K::StringSymbol,
        K::Number,
        K::NumberBin,
        K::NumberFloat,
        K::NumberHex,
        K::NumberInteger,
        K::NumberIntegerLong,
        K::NumberOct,
        K::Operator,
        K::OperatorWord,
        K::Punctuation,
        K::PunctuationMarker,
        K::Comment,
        K::CommentHashbang,
        K::CommentMultiline,
        K::CommentPreproc,
        K::CommentPreprocFile,
        K::CommentSingle,
        K::CommentSpecial,
        K::Generic,
        K::GenericDeleted,
        K::GenericEmph,
        K::GenericEmphStrong,
        K::GenericError,
        K::GenericHeading,
        K::GenericInserted,
        K::GenericOutput,
        K::GenericPrompt,
        K::GenericStrong,
        K::GenericSubheading,
        K::GenericTraceback,
        K::PunctuationIndicator,
        K::LiteralScalarPlain,
        K::GenericUnderline,
        K::CommentPreProc,
        K::NameLvalue,
        K::NameBackslashReference,
        K::NameBuiltinArticulation,
        K::NameBuiltinClef,
        K::NameBuiltinContext,
        K::NameBuiltinContextProperty,
        K::NameBuiltinDynamic,
        K::NameBuiltinGrob,
        K::NameBuiltinGrobProperty,
        K::NameBuiltinHeaderVariable,
        K::NameBuiltinMarkupCommand,
        K::NameBuiltinMusicCommand,
        K::NameBuiltinMusicFunction,
        K::NameBuiltinPaperVariable,
        K::NameBuiltinRepeatType,
        K::NameBuiltinScale,
        K::NameBuiltinSchemeBuiltin,
        K::NameBuiltinSchemeFunction,
        K::NameBuiltinTranslator,
        K::TokenString,
        K::TokenStringEscape,
        K::TokenStringSymbol,
        K::TokenNumber,
        K::Pitch,
        K::ChordModifier,
    ];

    /// How many kinds there are.
    pub const COUNT: usize = Self::ALL.len();
}

/// The last kind of Pygments' standard set; kinds after it are subtypes.
pub(crate) const LAST_STANDARD_KIND: TokenKind = TokenKind::GenericTraceback;

/// Each kind with its class, name and parent, in declaration order, so a kind's row is
/// `KIND_INFO[kind as usize]` (a test holds the order to the enum's).
#[rustfmt::skip]
pub(crate) static KIND_INFO: [(TokenKind, &str, &str, Option<TokenKind>); TokenKind::COUNT] = [
    (K::Text, "", "Text", None),
    (K::Whitespace, "w", "Text.Whitespace", Some(K::Text)),
    (K::Escape, "esc", "Escape", None),
    (K::Error, "err", "Error", None),
    (K::Other, "x", "Other", None),
    (K::Keyword, "k", "Keyword", None),
    (K::KeywordConstant, "kc", "Keyword.Constant", Some(K::Keyword)),
    (K::KeywordDeclaration, "kd", "Keyword.Declaration", Some(K::Keyword)),
    (K::KeywordNamespace, "kn", "Keyword.Namespace", Some(K::Keyword)),
    (K::KeywordPseudo, "kp", "Keyword.Pseudo", Some(K::Keyword)),
    (K::KeywordReserved, "kr", "Keyword.Reserved", Some(K::Keyword)),
    (K::KeywordType, "kt", "Keyword.Type", Some(K::Keyword)),
    (K::Name, "n", "Name", None),
    (K::NameAttribute, "na", "Name.Attribute", Some(K::Name)),
    (K::NameBuiltin, "nb", "Name.Builtin", Some(K::Name)),
    (K::NameBuiltinPseudo, "bp", "Name.Builtin.Pseudo", Some(K::NameBuiltin)),
    (K::NameClass, "nc", "Name.Class", Some(K::Name)),
    (K::NameConstant, "no", "Name.Constant", Some(K::Name)),
    (K::NameDecorator, "nd", "Name.Decorator", Some(K::Name)),
    (K::NameEntity, "ni", "Name.Entity", Some(K::Name)),
    (K::NameException, "ne", "Name.Exception", Some(K::Name)),
    (K::NameFunction, "nf", "Name.Function", Some(K::Name)),
    (K::NameFunctionMagic, "fm", "Name.Function.Magic", Some(K::NameFunction)),
    (K::NameProperty, "py", "Name.Property", Some(K::Name)),
    (K::NameLabel, "nl", "Name.Label", Some(K::Name)),
    (K::NameNamespace, "nn", "Name.Namespace", Some(K::Name)),
    (K::NameOther, "nx", "Name.Other", Some(K::Name)),
    (K::NameTag, "nt", "Name.Tag", Some(K::Name)),
    (K::NameVariable, "nv", "Name.Variable", Some(K::Name)),
    (K::NameVariableClass, "vc", "Name.Variable.Class", Some(K::NameVariable)),
    (K::NameVariableGlobal, "vg", "Name.Variable.Global", Some(K::NameVariable)),
    (K::NameVariableInstance, "vi", "Name.Variable.Instance", Some(K::NameVariable)),
    (K::NameVariableMagic, "vm", "Name.Variable.Magic", Some(K::NameVariable)),
    (K::Literal, "l", "Literal", None),
    (K::LiteralDate, "ld", "Literal.Date", Some(K::Literal)),
    (K::String, "s", "Literal.String", Some(K::Literal)),
    (K::StringAffix, "sa", "Literal.String.Affix", Some(K::String)),
    (K::StringBacktick, "sb", "Literal.String.Backtick", Some(K::String)),
    (K::StringChar, "sc", "Literal.String.Char", Some(K::String)),
    (K::StringDelimiter, "dl", "Literal.String.Delimiter", Some(K::String)),
    (K::StringDoc, "sd", "Literal.String.Doc", Some(K::String)),
    (K::StringDouble, "s2", "Literal.String.Double", Some(K::String)),
    (K::StringEscape, "se", "Literal.String.Escape", Some(K::String)),
    (K::StringHeredoc, "sh", "Literal.String.Heredoc", Some(K::String)),
    (K::StringInterpol, "si", "Literal.String.Interpol", Some(K::String)),
    (K::StringOther, "sx", "Literal.String.Other", Some(K::String)),
    (K::StringRegex, "sr", "Literal.String.Regex", Some(K::String)),
    (K::StringSingle, "s1", "Literal.String.Single", Some(K::String)),
    (K::StringSymbol, "ss", "Literal.String.Symbol", Some(K::String)),
    (K::Number, "m", "Literal.Number", Some(K::Literal)),
    (K::NumberBin, "mb", "Literal.Number.Bin", Some(K::Number)),
    (K::NumberFloat, "mf", "Literal.Number.Float", Some(K::Number)),
    (K::NumberHex, "mh", "Literal.Number.Hex", Some(K::Number)),
    (K::NumberInteger, "mi", "Literal.Number.Integer", Some(K::Number)),
    (K::NumberIntegerLong, "il", "Literal.Number.Integer.Long", Some(K::NumberInteger)),
    (K::NumberOct, "mo", "Literal.Number.Oct", Some(K::Number)),
    (K::Operator, "o", "Operator", None),
    (K::OperatorWord, "ow", "Operator.Word", Some(K::Operator)),
    (K::Punctuation, "p", "Punctuation", None),
    (K::PunctuationMarker, "pm", "Punctuation.Marker", Some(K::Punctuation)),
    (K::Comment, "c", "Comment", None),
    (K::CommentHashbang, "ch", "Comment.Hashbang", Some(K::Comment)),
    (K::CommentMultiline, "cm", "Comment.Multiline", Some(K::Comment)),
    (K::CommentPreproc, "cp", "Comment.Preproc", Some(K::Comment)),
    (K::CommentPreprocFile, "cpf", "Comment.PreprocFile", Some(K::Comment)),
    (K::CommentSingle, "c1", "Comment.Single", Some(K::Comment)),
    (K::CommentSpecial, "cs", "Comment.Special", Some(K::Comment)),
    (K::Generic, "g", "Generic", None),
    (K::GenericDeleted, "gd", "Generic.Deleted", Some(K::Generic)),
    (K::GenericEmph, "ge", "Generic.Emph", Some(K::Generic)),
    (K::GenericEmphStrong, "ges", "Generic.EmphStrong", Some(K::Generic)),
    (K::GenericError, "gr", "Generic.Error", Some(K::Generic)),
    (K::GenericHeading, "gh", "Generic.Heading", Some(K::Generic)),
    (K::GenericInserted, "gi", "Generic.Inserted", Some(K::Generic)),
    (K::GenericOutput, "go", "Generic.Output", Some(K::Generic)),
    (K::GenericPrompt, "gp", "Generic.Prompt", Some(K::Generic)),
    (K::GenericStrong, "gs", "Generic.Strong", Some(K::Generic)),
    (K::GenericSubheading, "gu", "Generic.Subheading", Some(K::Generic)),
    (K::GenericTraceback, "gt", "Generic.Traceback", Some(K::Generic)),
    (K::PunctuationIndicator, "p p-Indicator", "Punctuation.Indicator", Some(K::Punctuation)),
    (K::LiteralScalarPlain, "l l-Scalar l-Scalar-Plain", "Literal.Scalar.Plain", Some(K::Literal)),
    (K::GenericUnderline, "g g-Underline", "Generic.Underline", Some(K::Generic)),
    (K::CommentPreProc, "c c-PreProc", "Comment.PreProc", Some(K::Comment)),
    (K::NameLvalue, "n n-Lvalue", "Name.Lvalue", Some(K::Name)),
    (K::NameBackslashReference, "n n-BackslashReference",
        "Name.BackslashReference", Some(K::Name)),
    (K::NameBuiltinArticulation, "nb nb-Articulation",
        "Name.Builtin.Articulation", Some(K::NameBuiltin)),
    (K::NameBuiltinClef, "nb nb-Clef", "Name.Builtin.Clef", Some(K::NameBuiltin)),
    (K::NameBuiltinContext, "nb nb-Context", "Name.Builtin.Context", Some(K::NameBuiltin)),
    (K::NameBuiltinContextProperty, "nb nb-ContextProperty",
        "Name.Builtin.ContextProperty", Some(K::NameBuiltin)),
    (K::NameBuiltinDynamic, "nb nb-Dynamic", "Name.Builtin.Dynamic", Some(K::NameBuiltin)),
    (K::NameBuiltinGrob, "nb nb-Grob", "Name.Builtin.Grob", Some(K::NameBuiltin)),
    (K::NameBuiltinGrobProperty, "nb nb-GrobProperty",
        "Name.Builtin.GrobProperty", Some(K::NameBuiltin)),
    (K::NameBuiltinHeaderVariable, "nb nb-HeaderVariable",
        "Name.Builtin.HeaderVariable", Some(K::NameBuiltin)),
    (K::NameBuiltinMarkupCommand, "nb nb-MarkupCommand",
        "Name.Builtin.MarkupCommand", Some(K::NameBuiltin)),
    (K::NameBuiltinMusicCommand, "nb nb-MusicCommand",
        "Name.Builtin.MusicCommand", Some(K::NameBuiltin)),
    (K::NameBuiltinMusicFunction, "nb nb-MusicFunction",
        "Name.Builtin.MusicFunction", Some(K::NameBuiltin)),
    (K::NameBuiltinPaperVariable, "nb nb-PaperVariable",
        "Name.Builtin.PaperVariable", Some(K::NameBuiltin)),
    (K::NameBuiltinRepeatType, "nb nb-RepeatType",
        "Name.Builtin.RepeatType", Some(K::NameBuiltin)),
    (K::NameBuiltinScale, "nb nb-Scale", "Name.Builtin.Scale", Some(K::NameBuiltin)),
    (K::NameBuiltinSchemeBuiltin, "nb nb-SchemeBuiltin",
        "Name.Builtin.SchemeBuiltin", Some(K::NameBuiltin)),
    (K::NameBuiltinSchemeFunction, "nb nb-SchemeFunction",
        "Name.Builtin.SchemeFunction", Some(K::NameBuiltin)),
    (K::NameBuiltinTranslator, "nb nb-Translator",
        "Name.Builtin.Translator", Some(K::NameBuiltin)),
    (K::TokenString, " -String", "String", None),
    (K::TokenStringEscape, " -String -String-Escape", "String.Escape", Some(K::TokenString)),
    (K::TokenStringSymbol, " -String -String-Symbol", "String.Symbol", Some(K::TokenString)),
    (K::TokenNumber, " -Number", "Number", None),
    (K::Pitch, " -Pitch", "Pitch", None),
    (K::ChordModifier, " -ChordModifier", "ChordModifier", None),
];

fn kind_info(kind: TokenKind) -> (&'static str, &'static str, Option<TokenKind>) {
    let (_, class, name, parent) = KIND_INFO[kind as usize];
    (class, name, parent)
}

/// The CSS class Pygments' HtmlFormatter writes for `kind`: `"k"`, `"s2"`, `"c1"`.
///
/// It is `""` for [`TokenKind::Text`], which HTML writes without a span; subtypes outside
/// the standard set carry their ancestors' classes too (`"p p-Indicator"`).
pub fn kind_class(kind: TokenKind) -> &'static str {
    kind_info(kind).0
}

/// Pygments' dotted name without the `Token.` root: `"Literal.String.Double"`.
pub fn kind_name(kind: TokenKind) -> &'static str {
    kind_info(kind).1
}

/// The kind's parent; `None` for kinds directly below the root.
pub fn kind_parent(kind: TokenKind) -> Option<TokenKind> {
    kind_info(kind).2
}

/// Whether `kind` is `ancestor` or one of its descendants, as Pygments'
/// `ttype in Token.String` tells.
pub fn kind_is(kind: TokenKind, ancestor: TokenKind) -> bool {
    let mut k = kind;
    loop {
        if k == ancestor {
            return true;
        }
        match kind_parent(k) {
            Some(parent) => k = parent,
            None => return false,
        }
    }
}

/// The input a token covers; `""` when the token does not lie within `code`.
pub fn token_text(token: Token, code: &str) -> &str {
    code.get(token.start as usize..token.end as usize).unwrap_or("")
}
