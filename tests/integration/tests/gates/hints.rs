//! Every diagnostic Guidedog writes in its code is Elm-style: besides a title and a
//! message, its hint tells the reader what to do. This gate finds the hints written as
//! text in the sources and checks that each one gives directions:
//!
//! - struct literals and constants with a `hint:` field (`guidedoc::Message`,
//!   `gd_report::Report`, the crates' `Problem`s), whose hint is a string literal, a
//!   `concat!` of literals, or a `format!` (checked through its format string);
//! - calls of helpers that take a parameter named `hint`, whose argument is such text.
//!
//! A hint is directive when it is not empty and does not repeat its message or title,
//! has a clause starting with an imperative verb (`IMPERATIVES`), possibly after an
//! "If ..., " / "To ..., " / "When ..., " lead, and names something to act on: a flag, an
//! identifier or setting (a word holding `_` or `.`), quoted text, a placeholder, or a
//! word of `CONCRETE`. A hint whose only direction is to report a bug says what to report
//! it with. "Did you mean ...?" and "Nothing to do: <why>" pass by themselves.
//!
//! Hints computed at run time (a variable passed on) are checked where their text is
//! written. This is the Odin suite's hints test, re-read for Rust's token shapes.

use std::collections::BTreeMap;

use super::source::{Located, Token, tokenize, workspace_sources};

/// The verbs a hint's clause may start with.
#[rustfmt::skip]
const IMPERATIVES: &[&str] = &[
    "add", "adjust", "align", "allow", "ask", "avoid", "begin", "break", "build", "call",
    "change", "check", "choose", "clean", "close", "compare", "compile", "configure",
    "convert", "copy", "correct", "create", "declare", "define", "delete", "describe",
    "disable", "do", "download", "document", "drop", "edit", "enable", "enclose", "end",
    "escape", "exclude", "extend", "fix", "flatten", "follow", "free", "give", "grant",
    "increase", "indent", "insert", "install", "join", "keep", "leave", "let", "limit", "link",
    "list", "load", "look", "lower", "make", "mark", "match", "merge", "move", "name", "nest",
    "number", "omit", "open", "pass", "pick", "place", "point", "prefix", "provide", "put",
    "quote", "raise", "re-run", "read", "rebuild", "reduce", "refer", "register", "reinstall",
    "release", "remove", "rename", "reorder", "repeat", "replace", "report", "restore",
    "retry", "reuse", "rewrite", "run", "save", "see", "select", "separate", "set", "shorten",
    "simplify", "spell", "split", "start", "state", "stop", "supply", "surround", "swap",
    "take", "tell", "test", "generate", "lengthen", "translate", "try", "turn", "type",
    "unset", "update", "upgrade", "use", "verify", "wait", "wrap", "write", "paste", "publish",
    "approve", "sign", "bind", "unbind", "inspect", "unregister", "recover", "unlock",
    "revoke", "return", "encode", "send", "invoke", "unload",
];

/// The words that name something to act on.
#[rustfmt::skip]
const CONCRETE: &[&str] = &[
    "adornment", "anchor", "argument", "attribute", "backslash", "blank", "block", "asset",
    "executable", "hook", "plugin", "session", "branch", "budget", "caption", "catalog",
    "cell", "character", "citation", "class", "code", "colon", "column", "command", "comment",
    "conf", "content", "declaration", "delimiter", "dependency", "directive", "directory",
    "document", "domain", "encoding", "entry", "extension", "fence", "field", "figure", "file",
    "flag", "folder", "font", "footnote", "format", "function", "glossary", "graph", "header",
    "heading", "image", "include", "indentation", "index", "input", "installation",
    "inventory", "issue", "item", "key", "keyword", "label", "language", "line", "link",
    "list", "locale", "markup", "marker", "math", "member", "message", "method", "module",
    "msgid", "msgstr", "name", "nesting", "node", "number", "object", "option", "output",
    "package", "page", "parameter", "path", "pattern", "program", "project", "quote", "record",
    "reference", "registry", "role", "row", "rule", "section", "separator", "setting",
    "signature", "source", "space", "spelling", "stack", "statement", "style", "substitution",
    "syntax", "table", "tag", "target", "template", "term", "text", "theme", "thread", "title",
    "toctree", "type", "underline", "overline", "url", "value", "variable", "version", "width",
    "word", "typst", "graphviz", "python", "dot", "pdf", "html", "odin", "po", "mo", "json",
    "toml", "utf-8", "sphinx", "guidedog", "docstring", "adapter", "reader", "renderer", "id",
    "string", "filter", "test", "macro", "expression", "operator", "bracket", "parenthesis",
    "parenthese", "tab", "indent", "numbering", "numfig", "journal", "lock", "stage",
    "permission", "disk", "network", "proxy", "certificate", "server", "port", "address",
    "browser", "plural", "plural-forms", "charset", "date", "definition", "transition",
    "element", "workspace", "region", "array", "probe", "gds", "repository", "token",
    "profile", "commit", "plan", "snapshot", "publisher", "account", "app", "site", "store",
    "rust", "markdown", "myst", "resource", "limit", "crate", "remote", "git", "github", "dsc",
    "state", "reason", "evidence", "author", "slug", "prefix",
];

/// One hint as the code writes it.
#[derive(Debug)]
struct Site {
    file: String,
    line: usize,
    hint: String,
    /// The message and title beside it, when it is a struct literal's field.
    message: Option<String>,
    title: Option<String>,
}

/// The text of a literal expression starting at `tokens[at]` (code tokens only): a string,
/// `concat!` of strings, or `format!` / `format_args!` of a format string.
fn literal_text(tokens: &[&Located], at: usize) -> Option<String> {
    let token = |i: usize| tokens.get(i).map(|t| &t.token);
    match token(at)? {
        Token::Str(text) => Some(unescape(text)),
        Token::Ident(name) if name == "concat" && token(at + 1) == Some(&Token::Punct('!')) => {
            let mut text = String::new();
            let mut i = at + 3;
            while let Some(t) = token(i) {
                match t {
                    Token::Str(part) => text.push_str(&unescape(part)),
                    Token::Punct(',') => {}
                    Token::Punct(')') => return Some(text),
                    _ => return None,
                }
                i += 1;
            }
            None
        }
        Token::Ident(name) if matches!(name.as_str(), "format" | "format_args") => {
            match token(at + 3) {
                Some(Token::Str(text)) => Some(unescape(text)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// A literal's text with line continuations joined and quotes unescaped.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('\n') => {
                while chars.peek().is_some_and(|c| c.is_whitespace()) {
                    chars.next();
                }
            }
            Some('n') => out.push('\n'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// The named field `name: <literal>` within the struct literal around `at`: searched
/// between the nearest unbalanced `{` before `at` and its closing `}`.
fn sibling_field(tokens: &[&Located], at: usize, name: &str) -> Option<String> {
    let (mut depth, mut start) = (0i32, at);
    while start > 0 {
        start -= 1;
        match tokens[start].token {
            Token::Punct('}') => depth += 1,
            Token::Punct('{') if depth == 0 => break,
            Token::Punct('{') => depth -= 1,
            _ => {}
        }
    }
    let mut depth = 0i32;
    for i in start + 1..tokens.len() {
        match &tokens[i].token {
            Token::Punct('{') => depth += 1,
            Token::Punct('}') if depth == 0 => return None,
            Token::Punct('}') => depth -= 1,
            Token::Ident(field) if depth == 0 && field == name && is_field(tokens, i) => {
                return literal_text(tokens, i + 2);
            }
            _ => {}
        }
    }
    None
}

/// Whether `tokens[i]` is a field name: followed by one colon, not a path's `::`.
fn is_field(tokens: &[&Located], i: usize) -> bool {
    tokens.get(i + 1).map(|t| &t.token) == Some(&Token::Punct(':'))
        && tokens.get(i + 2).map(|t| &t.token) != Some(&Token::Punct(':'))
        && (i == 0 || tokens[i - 1].token != Token::Punct(':'))
}

/// The helpers that take a hint, by crate folder and function name: the hint
/// parameter's position (without `self`). A name the crate also defines without a hint
/// parameter, or with it elsewhere, is left out: its calls cannot be told apart here.
fn hint_helpers(files: &[(String, Vec<Located>)]) -> BTreeMap<(String, String), usize> {
    let mut found: BTreeMap<(String, String), Option<usize>> = BTreeMap::new();
    for (rel, tokens) in files {
        let code = code_tokens(tokens);
        for i in 0..code.len().saturating_sub(2) {
            if code[i].token != Token::Ident("fn".into()) {
                continue;
            }
            let Token::Ident(name) = &code[i + 1].token else { continue };
            let position = hint_parameter(&code, i + 2);
            let key = (crate_dir(rel).to_owned(), name.clone());
            match found.get(&key) {
                Some(seen) if *seen != position => {
                    found.insert(key, None);
                }
                Some(_) => {}
                None => {
                    found.insert(key, position);
                }
            }
        }
    }
    found
        .into_iter()
        .filter(|((dir, name), _)| !NOT_DIAGNOSTICS.iter().any(|n| n.0 == dir && n.1 == name))
        .filter_map(|(key, p)| p.map(|p| (key, p)))
        .collect()
}

/// The folder of the crate a source file belongs to (`crates/gd-cli/`).
fn crate_dir(rel: &str) -> &str {
    let mut slashes = rel.match_indices('/').map(|(i, _)| i);
    let end = slashes.nth(1).map_or(rel.len(), |i| i + 1);
    &rel[..end]
}

/// The position of the parameter named `hint` in the parameter list at `at` (skipping
/// generics), not counting `self`.
fn hint_parameter(code: &[&Located], mut at: usize) -> Option<usize> {
    if code.get(at)?.token == Token::Punct('<') {
        let mut depth = 0;
        while let Some(t) = code.get(at) {
            match t.token {
                Token::Punct('<') => depth += 1,
                Token::Punct('>') => depth -= 1,
                _ => {}
            }
            at += 1;
            if depth == 0 {
                break;
            }
        }
    }
    if code.get(at)?.token != Token::Punct('(') {
        return None;
    }
    let args = split_arguments(code, at)?;
    let named = |arg: &[&Located], word: &str| {
        arg.iter().position(|t| t.token == Token::Ident(word.into())).is_some_and(|p| {
            arg[p + 1..].first().map(|t| &t.token) == Some(&Token::Punct(':')) || word == "self"
        })
    };
    let params: Vec<_> = args.into_iter().filter(|a| !named(a, "self")).collect();
    params.iter().position(|a| named(a, "hint"))
}

/// The arguments of the parenthesized list opening at `code[open]`, split at top-level
/// commas.
fn split_arguments<'a, 'b>(
    code: &'b [&'a Located],
    open: usize,
) -> Option<Vec<&'b [&'a Located]>> {
    let mut depth = 0;
    let mut start = open + 1;
    let mut args = Vec::new();
    for i in open..code.len() {
        match code[i].token {
            Token::Punct('(' | '[' | '{') => depth += 1,
            Token::Punct(')' | ']' | '}') => {
                depth -= 1;
                if depth == 0 {
                    if i > start {
                        args.push(&code[start..i]);
                    }
                    return Some(args);
                }
            }
            Token::Punct(',') if depth == 1 => {
                args.push(&code[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    None
}

/// Helpers whose `hint` parameter is not a diagnostic's hint, and why.
const NOT_DIAGNOSTICS: &[(&str, &str, &str)] = &[(
    "crates/gd-jinja/",
    "undefined_with",
    "Jinja's Undefined carries Jinja's own hint text, shown as Jinja shows it",
)];

/// The code tokens of a file: comments and `#[cfg(test)]` modules left out.
fn code_tokens(tokens: &[Located]) -> Vec<&Located> {
    let code: Vec<&Located> =
        tokens.iter().filter(|t| !matches!(t.token, Token::Comment(_))).collect();
    let mut out = Vec::with_capacity(code.len());
    let mut i = 0;
    while i < code.len() {
        if let Some(end) = test_module_end(&code, i) {
            i = end;
            continue;
        }
        out.push(code[i]);
        i += 1;
    }
    out
}

/// When `code[at..]` starts `#[cfg(test)] mod name {`, the index after its closing brace.
fn test_module_end(code: &[&Located], at: usize) -> Option<usize> {
    let words = ["#", "[", "cfg", "(", "test", ")", "]", "mod"];
    let matches = words.iter().enumerate().all(|(k, w)| {
        code.get(at + k).is_some_and(|t| match &t.token {
            Token::Ident(name) => name == w,
            Token::Punct(c) => w.len() == 1 && w.starts_with(*c),
            _ => false,
        })
    });
    if !matches || code.get(at + 9).map(|t| &t.token) != Some(&Token::Punct('{')) {
        return None;
    }
    let mut depth = 0;
    for (i, t) in code.iter().enumerate().skip(at + 9) {
        match t.token {
            Token::Punct('{') => depth += 1,
            Token::Punct('}') => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    Some(code.len())
}

/// The hints written in one file's code.
fn sites(
    file: &str,
    tokens: &[Located],
    helpers: &BTreeMap<(String, String), usize>,
) -> Vec<Site> {
    let code = code_tokens(tokens);
    let mut out = Vec::new();
    for i in 0..code.len() {
        let Token::Ident(word) = &code[i].token else { continue };
        if word == "hint" && is_field(&code, i) {
            if let Some(hint) = literal_text(&code, i + 2) {
                out.push(Site {
                    file: file.into(),
                    line: code[i].line,
                    hint,
                    message: sibling_field(&code, i, "message"),
                    title: sibling_field(&code, i, "title"),
                });
            }
            continue;
        }
        let key = (crate_dir(file).to_owned(), word.clone());
        let Some(&position) = helpers.get(&key) else { continue };
        let is_call = code.get(i + 1).map(|t| &t.token) == Some(&Token::Punct('('))
            && (i == 0 || code[i - 1].token != Token::Ident("fn".into()));
        if !is_call {
            continue;
        }
        let Some(args) = split_arguments(&code, i + 1) else { continue };
        let Some(arg) = args.get(position) else { continue };
        if let Some(hint) = literal_text(arg, 0).filter(|_| arg_is_literal(arg)) {
            out.push(Site {
                file: file.into(),
                line: code[i].line,
                hint,
                message: None,
                title: None,
            });
        }
    }
    out
}

/// Whether an argument is only a literal (not `"a".to_string() + x`): a string, possibly
/// converted, or a `concat!`/`format!` call.
fn arg_is_literal(arg: &[&Located]) -> bool {
    match &arg.first().map(|t| &t.token) {
        Some(Token::Str(_)) => {
            arg.len() == 1 || matches!(arg.get(1).map(|t| &t.token), Some(Token::Punct('.')))
        }
        Some(Token::Ident(_)) => true,
        _ => false,
    }
}

/// Why `site`'s hint gives no directions, or `None` when it does.
fn undirected(site: &Site) -> Option<&'static str> {
    let hint = site.hint.trim();
    if hint.is_empty() {
        return Some("the hint is empty");
    }
    if site.message.as_deref() == Some(hint) || site.title.as_deref() == Some(hint) {
        return Some("the hint repeats the message");
    }
    if hint.contains("Did you mean") || hint.starts_with("Nothing to do: ") {
        return None;
    }
    if hint.starts_with('{') && hint.ends_with('}') && hint.len() <= 4 {
        return None; // the whole hint is an argument the caller writes.
    }
    let verbs = imperatives(hint);
    if verbs.is_empty() {
        return Some("no clause starts with an imperative verb");
    }
    if !names_something(hint) {
        return Some("the hint names nothing to act on");
    }
    let only_report = verbs.iter().all(|v| *v == "report");
    let with = ["version", "command", "input", "document", "--"];
    if only_report && !with.iter().any(|w| hint.contains(w)) {
        return Some("a bug report must say what to report it with");
    }
    None
}

/// The imperative verbs that start the hint's clauses.
fn imperatives(hint: &str) -> Vec<&'static str> {
    let mut clauses = vec![hint];
    for separator in [". ", "; ", ": ", ", or ", " or ", ", then ", ", and ", "\n"] {
        clauses = clauses.iter().flat_map(|c| c.split(separator)).collect();
    }
    let mut verbs = Vec::new();
    for clause in clauses {
        let mut clause = clause.trim().trim_start_matches(['(', '"', '`']);
        // A leading condition: "If ..., do", "To ..., do", "When ..., do".
        let lead = ["If ", "To ", "When ", "Unless ", "Otherwise, ", "Then "];
        if lead.iter().any(|l| clause.starts_with(l)) {
            clause = clause.split_once(", ").map_or(clause, |(_, rest)| rest);
            clause = clause.trim_start_matches("Otherwise, ");
        }
        let word = clause.split(|c: char| !(c.is_alphanumeric() || c == '-')).next();
        let word = word.unwrap_or_default().to_lowercase();
        if let Some(verb) = IMPERATIVES.iter().find(|v| **v == word) {
            verbs.push(*verb);
        }
    }
    verbs
}

/// Whether the hint names something concrete to act on.
fn names_something(hint: &str) -> bool {
    if hint.contains(['"', '`', '{', '\'']) || hint.contains("--") {
        return true;
    }
    hint.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '(' | ')'))
        .map(|w| w.trim_end_matches(['.', '?', '!']))
        .any(|word| {
            if word.len() > 1 && word.starts_with('-') {
                return true;
            }
            let inner = word.trim_matches(|c: char| !c.is_alphanumeric());
            if inner.contains(['_', '.', '/', ':']) {
                return true;
            }
            let lower = inner.to_lowercase();
            let singulars = [
                lower.strip_suffix("ies").map(|s| format!("{s}y")),
                lower.strip_suffix("es").map(str::to_owned),
                lower.strip_suffix('s').map(str::to_owned),
            ];
            CONCRETE.iter().any(|c| *c == lower || singulars.iter().flatten().any(|s| s == c))
        })
}

fn all_sites() -> Vec<Site> {
    let files: Vec<(String, Vec<Located>)> = workspace_sources()
        .into_iter()
        .filter(|(rel, _)| !rel.contains("/tests/") && !rel.starts_with("tests/"))
        .filter(|(rel, _)| !rel.ends_with("/tests.rs") && !rel.contains("/tests_"))
        .map(|(rel, text)| {
            let tokens = tokenize(&text);
            (rel, tokens)
        })
        .collect();
    let helpers = hint_helpers(&files);
    files.iter().flat_map(|(rel, tokens)| sites(rel, tokens, &helpers)).collect()
}

#[test]
fn every_hint_gives_directions() {
    let sites = all_sites();
    assert!(sites.len() > 500, "found only {} hints: is the scan broken?", sites.len());
    let problems: Vec<String> = sites
        .iter()
        .filter_map(|s| {
            undirected(s).map(|why| format!("{}:{}: {why}: {:?}", s.file, s.line, s.hint))
        })
        .collect();
    assert!(
        problems.is_empty(),
        "{} hint(s) give no directions:\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn the_hint_check_rejects_hints_without_directions() {
    let site = |hint: &str| Site {
        file: String::new(),
        line: 0,
        hint: hint.into(),
        message: Some("The value is wrong.".into()),
        title: None,
    };
    for bad in ["", "The value is wrong.", "Report this.", "Free some memory.", "It failed."] {
        assert!(undirected(&site(bad)).is_some(), "{bad:?} passed");
    }
    for good in [
        "Set html_theme in conf.toml.",
        "Did you mean numfig?",
        "Nothing to do: the page is unchanged.",
        "If the file moved, update the path in the toctree.",
        "Report this with the output of guidedog --version.",
        "Run the build again with --budget=512.",
    ] {
        assert_eq!(undirected(&site(good)), None, "{good:?} failed");
    }
}

#[test]
fn hints_are_found_in_fields_and_helper_calls() {
    let source = r#"
        const M: Message = Message { code: "x", title: "T", message: "Bad thing.",
            hint: concat!("Fix the ", "line.") };
        fn problem(code: &str, title: &str, message: String, hint: &str) -> Problem { todo!() }
        fn other(&self, hint: impl Into<String>) {}
        fn go() {
            let p = problem("c", "T", msg, "Check the path.");
            x.other(format!("Set {0}.", y));
        }
    "#;
    let files = vec![("a.rs".to_string(), tokenize(source))];
    let helpers = hint_helpers(&files);
    let key = |name: &str| ("a.rs".to_string(), name.to_string());
    assert_eq!(helpers.get(&key("problem")), Some(&3));
    assert_eq!(helpers.get(&key("other")), Some(&0));
    let found = sites("a.rs", &files[0].1, &helpers);
    let hints: Vec<&str> = found.iter().map(|s| s.hint.as_str()).collect();
    assert_eq!(hints, ["Fix the line.", "Check the path.", "Set {0}."]);
    assert_eq!(found[0].message.as_deref(), Some("Bad thing."));
}
