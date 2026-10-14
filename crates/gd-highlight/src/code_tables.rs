//! Rule tables for the languages the code engine lexes. Word lists are the languages'
//! own reserved words and standard names, grouped by the kind Pygments gives them.

use crate::code::{CodeSpec, Interp, Lead, Quote, WordGroup, feature as F, quote as Q};
use crate::lexers::Lexer;
use crate::token::TokenKind as K;

/// The rule table of a lexer built on the code engine.
pub(crate) fn code_spec(lexer: Lexer) -> Option<&'static CodeSpec> {
    Some(match lexer {
        Lexer::C => &C_SPEC,
        Lexer::Cpp => &CPP_SPEC,
        Lexer::Rust => &RUST_SPEC,
        Lexer::Go => &GO_SPEC,
        Lexer::Java => &JAVA_SPEC,
        Lexer::Kotlin => &KOTLIN_SPEC,
        Lexer::Csharp => &CSHARP_SPEC,
        Lexer::Swift => &SWIFT_SPEC,
        Lexer::Zig => &ZIG_SPEC,
        Lexer::Odin => &ODIN_SPEC,
        Lexer::Javascript => &JS_SPEC,
        Lexer::Typescript => &TS_SPEC,
        Lexer::Powershell => &POWERSHELL_SPEC,
        Lexer::Sql => &SQL_SPEC,
        Lexer::Dot => &DOT_SPEC,
        _ => return None,
    })
}

const fn group(kind: K, words: &'static [&'static str]) -> WordGroup {
    WordGroup { kind, words }
}

const fn q(open: &'static str, close: &'static str, kind: K, flags: u8, interp: Interp) -> Quote {
    Quote { open, close, kind, flags, interp }
}

#[rustfmt::skip]
const C_KEYWORDS: &[&str] = &[
    "auto", "break", "case", "const", "continue", "default", "do", "else", "enum", "extern", "for",
    "goto", "if", "inline", "register", "restrict", "return", "sizeof", "static", "struct",
    "switch", "typedef", "union", "volatile", "while", "_Alignas", "_Alignof", "_Atomic",
    "_Generic", "_Noreturn", "_Static_assert", "_Thread_local", "asm", "__asm__", "__attribute__",
    "__inline", "typeof", "alignas", "alignof", "static_assert", "thread_local", "constexpr",
];

#[rustfmt::skip]
const C_TYPES: &[&str] = &[
    "bool", "char", "double", "float", "int", "long", "short", "signed", "unsigned", "void",
    "_Bool", "_Complex", "size_t", "ssize_t", "ptrdiff_t", "intptr_t", "uintptr_t", "int8_t",
    "int16_t", "int32_t", "int64_t", "uint8_t", "uint16_t", "uint32_t", "uint64_t", "wchar_t",
    "char16_t", "char32_t", "char8_t",
];

#[rustfmt::skip]
const C_QUOTES: &[Quote] = &[
        q("\"", "\"", K::String, Q::ESCAPES, Interp::None),
    ];

const C_OPERATORS: &str = "~!%^&*+=|?:<>/-";

const C_PUNCTUATION: &str = "()[]{},.;";

#[rustfmt::skip]
pub(crate) const C_SPEC: CodeSpec = CodeSpec {
    features: F::PREPROCESSOR | F::CHAR_LITERALS | F::C_FUNCTIONS | F::C_OCTAL,
    words: &[
        group(K::KeywordType, C_TYPES),
        group(K::Keyword, C_KEYWORDS),
        group(K::NameBuiltin, &["NULL", "true", "false", "nullptr"]),
    ],
    line_comments: &["//"],
    block_open: "/*",
    block_close: "*/",
    quotes: C_QUOTES,
    string_prefixes: &["L", "u", "U", "u8"],
    operators: C_OPERATORS,
    punctuation: C_PUNCTUATION,
    ..CodeSpec::BASE
};

#[rustfmt::skip]
const CPP_KEYWORDS: &[&str] = &[
    "alignas", "alignof", "and", "and_eq", "asm", "bitand", "bitor", "catch", "class", "co_await",
    "co_return", "co_yield", "compl", "concept", "const_cast", "consteval", "constexpr",
    "constinit", "decltype", "delete", "dynamic_cast", "explicit", "export", "friend", "mutable",
    "namespace", "new", "noexcept", "not", "not_eq", "operator", "or", "or_eq", "private",
    "protected", "public", "reinterpret_cast", "requires", "static_assert", "static_cast",
    "template", "this", "throw", "try", "typeid", "typename", "using", "virtual", "xor", "xor_eq",
    "override", "final", "import", "module",
];

#[rustfmt::skip]
pub(crate) const CPP_SPEC: CodeSpec = CodeSpec {
    features: F::PREPROCESSOR | F::CHAR_LITERALS | F::C_FUNCTIONS | F::C_OCTAL | F::CPP_RAW_STRINGS
        | F::DIGIT_QUOTES,
    words: &[
        group(K::KeywordType, C_TYPES),
        group(K::Keyword, C_KEYWORDS),
        group(K::Keyword, CPP_KEYWORDS),
        group(K::NameBuiltin, &["NULL", "nullptr"]),
        group(K::KeywordConstant, &["true", "false"]),
    ],
    leads: &[
        Lead { word: "class", kind: K::NameClass },
        Lead { word: "struct", kind: K::NameClass },
        Lead { word: "union", kind: K::NameClass },
        Lead { word: "enum", kind: K::NameClass },
        Lead { word: "namespace", kind: K::NameNamespace },
        Lead { word: "concept", kind: K::NameClass },
    ],
    line_comments: &["//"],
    block_open: "/*",
    block_close: "*/",
    quotes: C_QUOTES,
    string_prefixes: &["L", "u", "U", "u8"],
    operators: C_OPERATORS,
    punctuation: C_PUNCTUATION,
    function_depth: 1,
    ..CodeSpec::BASE
};

#[rustfmt::skip]
pub(crate) const RUST_SPEC: CodeSpec = CodeSpec {
    features: F::LIFETIMES | F::CHAR_LITERALS | F::RUST_RAW_STRINGS | F::RUST_ATTRIBUTES
        | F::MACRO_BANG | F::NESTED_COMMENTS,
    words: &[
        group(K::Keyword, &[
            "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
            "extern", "fn", "for", "if", "impl", "in", "loop", "match", "mod", "move", "mut",
            "pub", "ref", "return", "static", "struct", "super", "trait", "type", "union",
            "unsafe", "use", "where", "while", "yield", "macro_rules", "box", "try",
        ]),
        group(K::KeywordDeclaration, &["let"]),
        group(K::KeywordConstant, &["true", "false"]),
        group(K::NameBuiltinPseudo, &["self", "Self"]),
        group(K::KeywordType, &[
            "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
            "f32", "f64", "bool", "char", "str",
        ]),
        group(K::NameBuiltin, &[
            "String", "Vec", "Option", "Result", "Box", "Some", "None", "Ok", "Err", "Rc", "Arc",
            "Default", "Clone", "Copy", "Send", "Sync", "Drop", "Iterator", "Into", "From",
        ]),
    ],
    leads: &[
        Lead { word: "fn", kind: K::NameFunction },
        Lead { word: "struct", kind: K::NameClass },
        Lead { word: "enum", kind: K::NameClass },
        Lead { word: "trait", kind: K::NameClass },
        Lead { word: "union", kind: K::NameClass },
        Lead { word: "type", kind: K::NameClass },
        Lead { word: "mod", kind: K::NameNamespace },
    ],
    line_comments: &["//"],
    doc_comments: &["///", "//!"],
    block_open: "/*",
    block_close: "*/",
    quotes: &[
        q("\"", "\"", K::String, Q::ESCAPES | Q::MULTILINE, Interp::None),
    ],
    string_prefixes: &["b", "c"],
    operators: "+-*/%&|<>^!~@=:?$",
    punctuation: "{}()[],.;",
    ..CodeSpec::BASE
};

#[rustfmt::skip]
pub(crate) const GO_SPEC: CodeSpec = CodeSpec {
    features: F::CHAR_LITERALS | F::GO_RECEIVERS,
    words: &[
        group(K::Keyword, &[
            "break", "case", "continue", "default", "defer", "else", "fallthrough", "for", "go",
            "goto", "if", "range", "return", "select", "switch",
        ]),
        group(K::KeywordDeclaration, &[
            "chan", "const", "func", "interface", "map", "struct", "type", "var",
        ]),
        group(K::KeywordNamespace, &["import", "package"]),
        group(K::KeywordConstant, &["true", "false", "iota", "nil"]),
        group(K::KeywordType, &[
            "bool", "byte", "complex64", "complex128", "error", "float32", "float64", "int",
            "int8", "int16", "int32", "int64", "rune", "string", "uint", "uint8", "uint16",
            "uint32", "uint64", "uintptr", "any", "comparable",
        ]),
        group(K::NameBuiltin, &[
            "append", "cap", "clear", "close", "complex", "copy", "delete", "imag", "len", "make",
            "max", "min", "new", "panic", "print", "println", "real", "recover",
        ]),
    ],
    leads: &[
        Lead { word: "func", kind: K::NameFunction },
        Lead { word: "package", kind: K::NameNamespace },
    ],
    line_comments: &["//"],
    block_open: "/*",
    block_close: "*/",
    quotes: &[
        q("\"", "\"", K::String, Q::ESCAPES, Interp::None),
        q("`", "`", K::String, Q::MULTILINE, Interp::None),
    ],
    operators: "+-*/%&|^<>=!:~",
    punctuation: "{}()[],.;",
    name_kind: K::NameOther,
    ..CodeSpec::BASE
};

#[rustfmt::skip]
pub(crate) const JAVA_SPEC: CodeSpec = CodeSpec {
    features: F::ANNOTATIONS | F::CHAR_LITERALS | F::C_FUNCTIONS | F::MEMBER_ATTRIBUTES,
    words: &[
        group(K::Keyword, &[
            "abstract", "assert", "break", "case", "catch", "continue", "default", "do", "else",
            "extends", "final", "finally", "for", "goto", "if", "implements", "instanceof",
            "native", "new", "permits", "private", "protected", "public", "return", "sealed",
            "static", "strictfp", "super", "switch", "synchronized", "this", "throw", "throws",
            "transient", "try", "volatile", "while", "yield", "non-sealed", "exports", "module",
            "requires",
        ]),
        group(K::KeywordDeclaration, &["class", "interface", "enum", "record"]),
        group(K::KeywordNamespace, &["import", "package"]),
        group(K::KeywordType, &[
            "boolean", "byte", "char", "double", "float", "int", "long", "short", "void", "var",
        ]),
        group(K::KeywordConstant, &["true", "false", "null"]),
    ],
    leads: &[
        Lead { word: "class", kind: K::NameClass },
        Lead { word: "interface", kind: K::NameClass },
        Lead { word: "enum", kind: K::NameClass },
        Lead { word: "record", kind: K::NameClass },
        Lead { word: "package", kind: K::NameNamespace },
        Lead { word: "import", kind: K::NameNamespace },
    ],
    line_comments: &["//"],
    block_open: "/*",
    block_close: "*/",
    quotes: &[
        q("\"\"\"", "\"\"\"", K::String, Q::ESCAPES | Q::MULTILINE, Interp::None),
        q("\"", "\"", K::String, Q::ESCAPES, Interp::None),
    ],
    operators: "~^*!%&<>|+=:/?-",
    punctuation: "{}()[],.;@",
    function_depth: 1,
    ..CodeSpec::BASE
};

#[rustfmt::skip]
pub(crate) const KOTLIN_SPEC: CodeSpec = CodeSpec {
    features: F::ANNOTATIONS | F::CHAR_LITERALS | F::NESTED_COMMENTS,
    words: &[
        group(K::Keyword, &[
            "abstract", "actual", "annotation", "as", "break", "by", "catch", "companion", "const",
            "constructor", "continue", "crossinline", "data", "do", "dynamic", "else", "enum",
            "expect", "external", "final", "finally", "for", "get", "if", "in", "infix", "init",
            "inline", "inner", "internal", "is", "lateinit", "noinline", "open", "operator", "out",
            "override", "private", "protected", "public", "reified", "return", "sealed", "set",
            "super", "suspend", "tailrec", "this", "throw", "try", "typealias", "vararg", "when",
            "where", "while", "value",
        ]),
        group(K::KeywordDeclaration, &["fun", "val", "var", "class", "interface", "object"]),
        group(K::KeywordNamespace, &["import", "package"]),
        group(K::KeywordConstant, &["true", "false", "null"]),
        group(K::KeywordType, &[
            "Int", "Long", "Short", "Byte", "Float", "Double", "Boolean", "Char", "String", "Unit",
            "Any", "Nothing", "Array", "List", "Map", "Set", "MutableList", "MutableMap",
        ]),
    ],
    leads: &[
        Lead { word: "fun", kind: K::NameFunction },
        Lead { word: "class", kind: K::NameClass },
        Lead { word: "interface", kind: K::NameClass },
        Lead { word: "object", kind: K::NameClass },
        Lead { word: "typealias", kind: K::NameClass },
        Lead { word: "package", kind: K::NameNamespace },
        Lead { word: "import", kind: K::NameNamespace },
    ],
    line_comments: &["//"],
    block_open: "/*",
    block_close: "*/",
    quotes: &[
        q("\"\"\"", "\"\"\"", K::String, Q::MULTILINE, Interp::DollarName),
        q("\"", "\"", K::String, Q::ESCAPES, Interp::DollarName),
    ],
    operators: "+-*/%&|<>=!?:~^.",
    punctuation: "{}()[],;@",
    ..CodeSpec::BASE
};

#[rustfmt::skip]
pub(crate) const CSHARP_SPEC: CodeSpec = CodeSpec {
    features: F::PREPROCESSOR | F::CHAR_LITERALS | F::C_FUNCTIONS,
    words: &[
        group(K::Keyword, &[
            "abstract", "as", "base", "break", "case", "catch", "checked", "continue", "default",
            "delegate", "do", "else", "event", "explicit", "extern", "finally", "fixed", "for",
            "foreach", "goto", "if", "implicit", "in", "is", "lock", "new", "operator", "out",
            "override", "params", "private", "protected", "public", "readonly", "ref", "return",
            "sealed", "sizeof", "stackalloc", "static", "switch", "this", "throw", "try", "typeof",
            "unchecked", "unsafe", "using", "virtual", "volatile", "while", "async", "await",
            "get", "set", "add", "remove", "var", "yield", "where", "when", "init", "required",
            "with", "nameof", "partial", "global", "class", "struct", "interface", "enum",
            "record", "namespace", "const", "internal", "select", "from", "orderby", "let", "into",
        ]),
        group(K::KeywordType, &[
            "bool", "byte", "char", "decimal", "double", "dynamic", "float", "int", "long",
            "object", "sbyte", "short", "string", "uint", "ulong", "ushort", "void", "nint",
            "nuint",
        ]),
        group(K::KeywordConstant, &["true", "false", "null"]),
    ],
    leads: &[
        Lead { word: "class", kind: K::NameClass },
        Lead { word: "struct", kind: K::NameClass },
        Lead { word: "interface", kind: K::NameClass },
        Lead { word: "enum", kind: K::NameClass },
        Lead { word: "record", kind: K::NameClass },
        Lead { word: "namespace", kind: K::NameNamespace },
    ],
    line_comments: &["//"],
    block_open: "/*",
    block_close: "*/",
    quotes: &[
        q("$@\"", "\"", K::String, Q::MULTILINE | Q::DOUBLING, Interp::Brace),
        q("@$\"", "\"", K::String, Q::MULTILINE | Q::DOUBLING, Interp::Brace),
        q("@\"", "\"", K::String, Q::MULTILINE | Q::DOUBLING, Interp::None),
        q("$\"", "\"", K::String, Q::ESCAPES, Interp::Brace),
        q("\"\"\"", "\"\"\"", K::String, Q::MULTILINE, Interp::None),
        q("\"", "\"", K::String, Q::ESCAPES, Interp::None),
    ],
    operators: "!%^&*+=|?:<>/-~",
    punctuation: "()[]{},.;",
    function_depth: 2,
    ..CodeSpec::BASE
};

#[rustfmt::skip]
pub(crate) const SWIFT_SPEC: CodeSpec = CodeSpec {
    features: F::ANNOTATIONS | F::PREPROCESSOR | F::NESTED_COMMENTS,
    words: &[
        group(K::Keyword, &[
            "associatedtype", "break", "case", "catch", "continue", "default", "defer", "deinit",
            "do", "else", "fallthrough", "fileprivate", "for", "guard", "if", "in", "init",
            "inout", "internal", "is", "open", "operator", "private", "public", "repeat",
            "rethrows", "return", "static", "subscript", "super", "switch", "throw", "throws",
            "try", "where", "while", "as", "async", "await", "some", "any", "mutating",
            "nonmutating", "override", "convenience", "required", "lazy", "weak", "unowned",
            "final", "get", "set", "willSet", "didSet",
        ]),
        group(K::KeywordDeclaration, &[
            "func", "let", "var", "class", "struct", "enum", "extension", "protocol", "typealias",
            "import", "actor",
        ]),
        group(K::KeywordConstant, &["true", "false", "nil"]),
        group(K::NameBuiltinPseudo, &["self", "Self"]),
        group(K::NameBuiltin, &[
            "Int", "Double", "String", "Bool", "Float", "Character", "Array", "Dictionary", "Set",
            "Optional", "Void", "Any", "AnyObject", "UInt", "Int8", "Int16", "Int32", "Int64",
            "UInt8", "UInt16", "UInt32", "UInt64", "print",
        ]),
    ],
    leads: &[
        Lead { word: "func", kind: K::NameFunction },
        Lead { word: "class", kind: K::NameClass },
        Lead { word: "struct", kind: K::NameClass },
        Lead { word: "enum", kind: K::NameClass },
        Lead { word: "protocol", kind: K::NameClass },
        Lead { word: "extension", kind: K::NameClass },
        Lead { word: "actor", kind: K::NameClass },
        Lead { word: "import", kind: K::NameNamespace },
    ],
    line_comments: &["//"],
    block_open: "/*",
    block_close: "*/",
    quotes: &[
        q("\"\"\"", "\"\"\"", K::String, Q::ESCAPES | Q::MULTILINE, Interp::BackslashParen),
        q("\"", "\"", K::String, Q::ESCAPES, Interp::BackslashParen),
    ],
    operators: "+-*/%&|<>=!?~^:.",
    punctuation: "{}()[],;",
    ..CodeSpec::BASE
};

#[rustfmt::skip]
pub(crate) const ZIG_SPEC: CodeSpec = CodeSpec {
    features: F::CHAR_LITERALS | F::ZIG_LINE_STRINGS | F::ZIG_BUILTINS,
    words: &[
        group(K::Keyword, &[
            "addrspace", "align", "allowzero", "and", "anyframe", "anytype", "asm", "async",
            "await", "break", "callconv", "catch", "comptime", "const", "continue", "defer",
            "else", "enum", "errdefer", "error", "export", "extern", "fn", "for", "if", "inline",
            "linksection", "noalias", "noinline", "nosuspend", "opaque", "or", "orelse", "packed",
            "pub", "resume", "return", "struct", "suspend", "switch", "test", "threadlocal", "try",
            "union", "unreachable", "usingnamespace", "var", "volatile", "while",
        ]),
        group(K::KeywordType, &[
            "bool", "f16", "f32", "f64", "f80", "f128", "i8", "i16", "i32", "i64", "i128", "isize",
            "u8", "u16", "u32", "u64", "u128", "usize", "void", "noreturn", "type", "anyerror",
            "anyopaque", "comptime_int", "comptime_float", "c_int", "c_uint", "c_long", "c_ulong",
            "c_char", "c_short", "c_ushort", "c_longlong", "c_ulonglong",
        ]),
        group(K::KeywordConstant, &["true", "false", "null", "undefined"]),
    ],
    leads: &[
        Lead { word: "fn", kind: K::NameFunction },
    ],
    line_comments: &["//"],
    quotes: &[
        q("\"", "\"", K::String, Q::ESCAPES, Interp::None),
    ],
    operators: "+-*/%&|<>=!?~^:.",
    punctuation: "{}()[],;",
    ..CodeSpec::BASE
};

#[rustfmt::skip]
pub(crate) const ODIN_SPEC: CodeSpec = CodeSpec {
    features: F::CHAR_LITERALS | F::HASH_DIRECTIVES | F::ANNOTATIONS | F::ODIN_DECLARATIONS
        | F::NESTED_COMMENTS,
    words: &[
        group(K::Keyword, &[
            "asm", "auto_cast", "bit_field", "bit_set", "break", "case", "cast", "context",
            "continue", "defer", "distinct", "do", "dynamic", "else", "enum", "fallthrough", "for",
            "foreign", "if", "in", "map", "matrix", "not_in", "or_break", "or_continue", "or_else",
            "or_return", "proc", "return", "struct", "switch", "transmute", "union", "using",
            "when", "where",
        ]),
        group(K::KeywordNamespace, &["package", "import"]),
        group(K::KeywordType, &[
            "bool", "b8", "b16", "b32", "b64", "byte", "i8", "i16", "i32", "i64", "i128", "int",
            "u8", "u16", "u32", "u64", "u128", "uint", "uintptr", "f16", "f32", "f64", "complex32",
            "complex64", "complex128", "quaternion64", "quaternion128", "quaternion256", "rune",
            "string", "cstring", "rawptr", "typeid", "any",
        ]),
        group(K::KeywordConstant, &["true", "false", "nil"]),
        group(K::NameBuiltin, &[
            "len", "cap", "size_of", "align_of", "offset_of", "type_of", "typeid_of",
            "type_info_of", "swizzle", "min", "max", "abs", "clamp", "make", "new", "delete",
            "append", "free", "copy", "panic", "assert", "raw_data", "soa_zip", "soa_unzip",
            "clear", "resize", "reserve", "inject_at", "pop",
        ]),
    ],
    leads: &[
        Lead { word: "package", kind: K::NameNamespace },
    ],
    line_comments: &["//"],
    block_open: "/*",
    block_close: "*/",
    quotes: &[
        q("\"", "\"", K::String, Q::ESCAPES, Interp::None),
        q("`", "`", K::String, Q::MULTILINE, Interp::None),
    ],
    operators: "+-*/%&|<>=!?~^:$",
    punctuation: "{}()[],.;",
    ..CodeSpec::BASE
};

#[rustfmt::skip]
const JS_WORDS: &[WordGroup] = &[
        group(K::Keyword, &[
            "break", "case", "catch", "continue", "debugger", "default", "do", "else", "finally",
            "for", "if", "in", "return", "switch", "this", "throw", "try", "while", "with",
            "yield", "await", "async", "of", "export", "import", "from", "as", "static", "get",
            "set", "extends", "super",
        ]),
        group(K::KeywordDeclaration, &["var", "let", "const", "function", "class"]),
        group(K::OperatorWord, &["new", "typeof", "instanceof", "delete", "void"]),
        group(K::KeywordConstant, &["true", "false", "null", "NaN", "Infinity", "undefined"]),
        group(K::NameBuiltin, &[
            "Array", "Boolean", "Date", "BigInt", "Error", "Function", "Math", "Number", "Object",
            "RegExp", "String", "Promise", "Proxy", "Symbol", "Map", "Set", "WeakMap", "WeakSet",
            "Reflect", "JSON", "Intl", "console", "window", "document", "globalThis", "parseInt",
            "parseFloat", "isNaN", "isFinite", "eval",
        ]),
    ];

#[rustfmt::skip]
const JS_QUOTES: &[Quote] = &[
        q("\"", "\"", K::StringDouble, Q::ESCAPES, Interp::None),
        q("'", "'", K::StringSingle, Q::ESCAPES, Interp::None),
        q("`", "`", K::StringBacktick, Q::ESCAPES | Q::MULTILINE, Interp::DollarBrace),
    ];

#[rustfmt::skip]
pub(crate) const JS_SPEC: CodeSpec = CodeSpec {
    features: F::REGEX_LITERALS,
    words: JS_WORDS,
    line_comments: &["//"],
    block_open: "/*",
    block_close: "*/",
    quotes: JS_QUOTES,
    operators: "+-*/%&|^!~=<>?:",
    punctuation: "{}()[];,.",
    name_kind: K::NameOther,
    ident_extra: "$",
    ..CodeSpec::BASE
};

#[rustfmt::skip]
pub(crate) const TS_SPEC: CodeSpec = CodeSpec {
    features: F::REGEX_LITERALS | F::ANNOTATIONS,
    words: &[
        JS_WORDS[0],
        JS_WORDS[1],
        JS_WORDS[2],
        JS_WORDS[3],
        JS_WORDS[4],
        group(K::KeywordReserved, &[
            "abstract", "declare", "enum", "implements", "interface", "namespace", "private",
            "protected", "public", "readonly", "type", "keyof", "infer", "is", "asserts",
            "satisfies", "module", "override", "unique",
        ]),
        group(K::KeywordType, &[
            "string", "number", "boolean", "any", "void", "never", "unknown", "object", "symbol",
            "bigint",
        ]),
    ],
    line_comments: &["//"],
    block_open: "/*",
    block_close: "*/",
    quotes: JS_QUOTES,
    operators: "+-*/%&|^!~=<>?:",
    punctuation: "{}()[];,.",
    name_kind: K::NameOther,
    ident_extra: "$",
    ..CodeSpec::BASE
};

#[rustfmt::skip]
pub(crate) const POWERSHELL_SPEC: CodeSpec = CodeSpec {
    features: F::DOLLAR_VARIABLES | F::DASH_OPERATORS | F::CASE_INSENSITIVE,
    words: &[
        group(K::Keyword, &[
            "begin", "break", "catch", "class", "continue", "data", "define", "do", "dynamicparam",
            "else", "elseif", "end", "exit", "filter", "finally", "for", "foreach", "from",
            "function", "if", "in", "param", "process", "return", "switch", "throw", "trap", "try",
            "until", "using", "var", "while", "workflow", "parallel", "sequence", "inlinescript",
            "configuration", "enum", "hidden", "static",
        ]),
    ],
    line_comments: &["#"],
    block_open: "<#",
    block_close: "#>",
    quotes: &[
        q("@\"", "\"@", K::StringHeredoc, Q::ESCAPES | Q::MULTILINE, Interp::DollarParen),
        q("@'", "'@", K::StringHeredoc, Q::MULTILINE, Interp::None),
        q("\"", "\"", K::StringDouble, Q::ESCAPES | Q::MULTILINE | Q::DOUBLING,
            Interp::DollarParen),
        q("'", "'", K::StringSingle, Q::MULTILINE | Q::DOUBLING, Interp::None),
    ],
    operators: "+-*/%=<>!&|.:",
    punctuation: "{}()[];,@",
    ident_extra: "-",
    escape: b'`',
    ..CodeSpec::BASE
};

#[rustfmt::skip]
pub(crate) const SQL_SPEC: CodeSpec = CodeSpec {
    features: F::CASE_INSENSITIVE,
    words: &[
        group(K::Keyword, &[
            "select", "from", "where", "and", "or", "not", "insert", "into", "values", "update",
            "set", "delete", "create", "table", "drop", "alter", "add", "column", "primary", "key",
            "foreign", "references", "index", "view", "as", "join", "inner", "left", "right",
            "outer", "full", "cross", "on", "group", "by", "order", "having", "limit", "offset",
            "union", "all", "distinct", "case", "when", "then", "else", "end", "is", "null",
            "like", "ilike", "in", "between", "exists", "default", "constraint", "unique", "check",
            "begin", "commit", "rollback", "transaction", "with", "recursive", "returning", "if",
            "replace", "grant", "revoke", "to", "asc", "desc", "cascade", "database", "schema",
            "trigger", "procedure", "function", "return", "returns", "declare", "natural", "using",
            "over", "partition", "window", "fetch", "next", "rows", "only", "true", "false",
            "count", "sum", "avg", "min", "max", "coalesce", "cast", "language", "temporary",
            "explain", "analyze", "vacuum", "truncate", "merge", "matched", "except", "intersect",
            "lateral", "any", "some", "do", "nothing", "conflict",
        ]),
        group(K::NameBuiltin, &[
            "int", "integer", "smallint", "bigint", "decimal", "numeric", "real", "float",
            "double", "precision", "char", "character", "varchar", "varying", "text", "date",
            "time", "timestamp", "timestamptz", "boolean", "bool", "blob", "serial", "bigserial",
            "bytea", "json", "jsonb", "uuid", "interval", "money",
        ]),
    ],
    line_comments: &["--"],
    block_open: "/*",
    block_close: "*/",
    quotes: &[
        q("'", "'", K::StringSingle, Q::MULTILINE | Q::DOUBLING, Interp::None),
        q("\"", "\"", K::StringSymbol, Q::DOUBLING, Interp::None),
    ],
    operators: "+-*/%=<>!|&~^:",
    punctuation: "(),;.",
    ..CodeSpec::BASE
};

#[rustfmt::skip]
pub(crate) const DOT_SPEC: CodeSpec = CodeSpec {
    features: F::CASE_INSENSITIVE | F::DOT_ATTRIBUTES | F::HTML_LABELS,
    words: &[
        group(K::Keyword, &["graph", "digraph", "subgraph", "node", "edge", "strict"]),
    ],
    line_comments: &["//", "#"],
    block_open: "/*",
    block_close: "*/",
    quotes: &[
        q("\"", "\"", K::String, Q::ESCAPES | Q::MULTILINE, Interp::None),
    ],
    operators: "-=>",
    punctuation: "{}[];,:",
    ..CodeSpec::BASE
};
