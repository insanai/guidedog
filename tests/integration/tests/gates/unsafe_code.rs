//! Memory safety: unsafe code lives only in the FFI modules (and in the counting
//! allocators of two allocation tests), and every unsafe block and impl says why it is
//! sound in a `// SAFETY:` comment just above it. The compiler enforces the first part
//! too (`unsafe_code = "deny"` for the workspace, `#![forbid(unsafe_code)]` in every crate
//! without FFI); this gate keeps the list of exceptions short and visible, and checks
//! the comments, which the compiler cannot. It replaces the Odin suite's scan for
//! `#no_bounds_check`, multi-pointers and `uintptr`.

use super::source::{Located, Token, tokenize, workspace_sources};

/// Files (or folders, ending in `/`) that may contain unsafe code, and why.
const UNSAFE_ALLOWED: &[(&str, &str)] = &[
    ("crates/gd-graphdog/src/", "the binding to the Graphviz C library"),
    ("crates/gd-host/src/sys.rs", "the Ctrl+C handler and the atomic path swap"),
    ("crates/gd-pages/src/credential/", "the system credential stores"),
    ("crates/gd-pages/src/command/windows_console.rs", "Windows console modes"),
    ("cmd/guidedog-pages/src/interrupt.rs", "signal(2) for Ctrl+C"),
    ("crates/gd-clock/src/offset.rs", "the local time zone's offset from UTC"),
    ("crates/guidedoc/tests/services.rs", "a counting global allocator"),
    ("crates/gd-gettext/tests/large.rs", "a counting global allocator"),
    ("tests/integration/tests/allocations.rs", "a counting global allocator"),
];

/// Files that may open an FFI module (or themselves) with `allow(unsafe_code)`.
const ALLOW_SITES: &[&str] = &[
    "crates/gd-graphdog/src/lib.rs",
    "crates/gd-host/src/lib.rs",
    "crates/gd-pages/src/lib.rs",
    "crates/gd-pages/src/command/mod.rs",
    "cmd/guidedog-pages/src/main.rs",
    "crates/gd-clock/src/lib.rs",
    "crates/guidedoc/tests/services.rs",
    "crates/gd-gettext/tests/large.rs",
    "tests/integration/tests/allocations.rs",
];

/// What an `unsafe` keyword introduces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Use {
    /// `unsafe { ... }`: needs a SAFETY comment.
    Block,
    /// `unsafe impl`: needs a SAFETY comment.
    Impl,
    /// `unsafe fn` or `unsafe extern`: a declaration, whose callers justify their calls.
    Declaration,
}

/// The unsafe uses of `tokens`: each one's line, kind, and whether a SAFETY comment
/// stands just above it (or on its line).
fn unsafe_uses(tokens: &[Located]) -> Vec<(usize, Use, bool)> {
    let code: Vec<usize> =
        (0..tokens.len()).filter(|&i| !matches!(tokens[i].token, Token::Comment(_))).collect();
    let mut out = Vec::new();
    for (n, &i) in code.iter().enumerate() {
        if tokens[i].token != Token::Ident("unsafe".into()) {
            continue;
        }
        let next = code.get(n + 1).map(|&j| &tokens[j].token);
        let kind = match next {
            Some(Token::Punct('{')) => Use::Block,
            Some(Token::Ident(word)) if word == "impl" => Use::Impl,
            _ => Use::Declaration,
        };
        out.push((tokens[i].line, kind, has_safety_comment(tokens, i)));
    }
    out
}

/// Whether a `SAFETY:` comment justifies the token `at`: a comment inside the statement
/// that holds it, before it, or the comment lines directly above that statement.
fn has_safety_comment(tokens: &[Located], at: usize) -> bool {
    let mut i = at;
    // Back to the start of the statement (`let x =\n    unsafe {` included).
    while i > 0 {
        match &tokens[i - 1].token {
            Token::Comment(text) if text.contains("SAFETY:") => return true,
            Token::Punct(';' | '{' | '}') => break,
            _ => i -= 1,
        }
    }
    // The comment lines directly above, contiguous.
    let mut expected = tokens[i].line;
    while i > 0 {
        i -= 1;
        let Token::Comment(text) = &tokens[i].token else { return false };
        if tokens[i].line + 1 < expected {
            return false;
        }
        if text.contains("SAFETY:") {
            return true;
        }
        expected = tokens[i].line;
    }
    false
}

fn unsafe_allowed(rel: &str) -> bool {
    UNSAFE_ALLOWED
        .iter()
        .any(|(p, _)| if p.ends_with('/') { rel.starts_with(p) } else { rel == *p })
}

/// Whether `tokens` hold the attribute `allow(unsafe_code)`.
fn allows_unsafe(tokens: &[Located]) -> bool {
    let code: Vec<&Token> =
        tokens.iter().map(|t| &t.token).filter(|t| !matches!(t, Token::Comment(_))).collect();
    code.windows(3).any(|w| {
        w[0] == &Token::Ident("allow".into())
            && w[1] == &Token::Punct('(')
            && w[2] == &Token::Ident("unsafe_code".into())
    })
}

#[test]
fn unsafe_code_stays_in_ffi_modules_and_says_why_it_is_sound() {
    let mut problems = Vec::new();
    for (rel, text) in workspace_sources() {
        let tokens = tokenize(&text);
        if allows_unsafe(&tokens) && !ALLOW_SITES.contains(&rel.as_str()) {
            problems.push(format!("{rel} allows unsafe code but is not an FFI site"));
        }
        for (line, kind, documented) in unsafe_uses(&tokens) {
            if !unsafe_allowed(&rel) {
                problems.push(format!("{rel}:{line}: unsafe code outside the FFI modules"));
            } else if kind != Use::Declaration && !documented {
                problems
                    .push(format!("{rel}:{line}: unsafe {kind:?} without a // SAFETY: comment"));
            }
        }
    }
    assert!(problems.is_empty(), "unsafe code:\n{}", problems.join("\n"));
}

#[test]
fn crates_without_ffi_forbid_unsafe_code() {
    let mut problems = Vec::new();
    for (rel, text) in workspace_sources() {
        let root = rel.ends_with("/src/lib.rs") || rel.ends_with("/src/main.rs");
        if !root || rel.contains("/tests/") {
            continue;
        }
        let crate_dir = rel.trim_end_matches("lib.rs").trim_end_matches("main.rs");
        let has_ffi = UNSAFE_ALLOWED.iter().any(|(p, _)| p.starts_with(crate_dir));
        let forbids = text.contains("#![forbid(unsafe_code)]");
        if !has_ffi && !forbids {
            problems.push(format!("{rel}: add #![forbid(unsafe_code)] (the crate has no FFI)"));
        }
    }
    assert!(problems.is_empty(), "crate roots:\n{}", problems.join("\n"));
}

#[test]
fn the_scan_finds_undocumented_blocks_and_ignores_declarations() {
    let source = "// SAFETY: fine.\nlet a = unsafe { f() };\nlet b = unsafe { g() };\n\
        unsafe extern \"C\" { fn h(); }\n// SAFETY: forwards.\nunsafe impl Send for X {}\n\
        /* SAFETY: a block comment. */ let c = unsafe { k() };\n// SAFETY: the statement.\n\
        let d =\n    unsafe { m() };\n";
    let uses = unsafe_uses(&tokenize(source));
    assert_eq!(
        uses,
        [
            (2, Use::Block, true),
            (3, Use::Block, false),
            (4, Use::Declaration, false),
            (6, Use::Impl, true),
            (7, Use::Block, true),
            (10, Use::Block, true),
        ]
    );
}
