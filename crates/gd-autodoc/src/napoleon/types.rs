//! `napoleon_preprocess_types`: a NumPy type specification such as
//! `int or None, default: None` becomes cross-references and literals
//! (`` :py:class:`int` or :py:obj:`None`, *default*: :py:obj:`None` ``).

use super::patterns::{is_numeric, is_xref, split_type_tokens, starts_default};

/// What one token of a type specification is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TokenType {
    /// A number, a set of values, or a quoted string: shown as a literal.
    Literal,
    /// `optional` or `default`: emphasised.
    Control,
    /// `, `, ` or `, ...: kept.
    Delimiter,
    /// Already a cross-reference: kept.
    Reference,
    /// A name: linked to its class.
    Obj,
}

/// A malformed token: what is wrong with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TokenProblem {
    pub(crate) what: &'static str,
    pub(crate) token: String,
}

/// `_tokenize_type_spec`.
pub(crate) fn tokenize(spec: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for raw in split_type_tokens(spec) {
        if starts_default(raw) {
            // `default` can be separated by nothing but a single space for now.
            let rest = &raw[7..];
            let other = &rest[rest.chars().next().map_or(0, char::len_utf8)..];
            out.extend([&raw[..7], " ", other].into_iter().filter(|s| !s.is_empty()));
        } else if !raw.is_empty() {
            out.push(raw);
        }
    }
    out
}

/// `_recombine_set_tokens`: `{`, values and `}` become one token.
pub(crate) fn recombine_sets(tokens: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i] == "{" {
            let (set, next) = take_set(tokens, i);
            out.push(set);
            i = next;
        } else {
            out.push(tokens[i].to_owned());
            i += 1;
        }
    }
    out
}

/// `takewhile_set`: the set that starts at `tokens[start]` (a `{`), joined, and where
/// the tokens after it start. A keyword ends an unclosed set, and is kept, with the
/// `, ` before it.
fn take_set(tokens: &[&str], start: usize) -> (String, usize) {
    let mut set = String::new();
    let (mut open, mut i) = (0usize, start);
    let mut comma: Option<usize> = None;
    while i < tokens.len() {
        let token = tokens[i];
        if token == ", " {
            comma = Some(i);
            i += 1;
            continue;
        }
        if token.trim().is_empty() {
            i += 1;
            continue;
        }
        if token == "optional" || token == "default" {
            return (set, comma.unwrap_or(i));
        }
        if comma.take().is_some() {
            set.push_str(", ");
        }
        match token {
            "{" => open += 1,
            "}" => open = open.saturating_sub(1),
            _ => {}
        }
        set.push_str(token);
        i += 1;
        if open == 0 {
            break;
        }
    }
    (set, i)
}

/// `_token_type`: what `token` is, and what is wrong with it when it is malformed.
pub(crate) fn token_type(token: &str) -> (TokenType, Option<TokenProblem>) {
    let problem = |what| Some(TokenProblem { what, token: token.to_owned() });
    if token.starts_with(' ') || token.ends_with(' ') {
        return (TokenType::Delimiter, None);
    }
    let enclosed = |a: char, b: char| token.starts_with(a) && token.ends_with(b);
    if is_numeric(token) || enclosed('{', '}') || enclosed('"', '"') || enclosed('\'', '\'') {
        return (TokenType::Literal, None);
    }
    if token.starts_with('{') {
        return (TokenType::Literal, problem("invalid value set (missing closing brace)"));
    }
    if token.ends_with('}') {
        return (TokenType::Literal, problem("invalid value set (missing opening brace)"));
    }
    if token.starts_with(['\'', '"']) {
        return (TokenType::Literal, problem("malformed string literal (missing closing quote)"));
    }
    if token.ends_with(['\'', '"']) {
        return (TokenType::Literal, problem("malformed string literal (missing opening quote)"));
    }
    if token == "optional" || token == "default" {
        return (TokenType::Control, None);
    }
    if is_xref(token) {
        return (TokenType::Reference, None);
    }
    (TokenType::Obj, None)
}

/// `_convert_type_spec`: the specification with its names linked, and the malformed
/// tokens found.
pub(crate) fn convert_type_spec(
    spec: &str,
    aliases: &[(String, String)],
    problems: &mut Vec<TokenProblem>,
) -> String {
    let tokens = recombine_sets(&tokenize(spec));
    let mut out = String::with_capacity(spec.len() * 2);
    for token in &tokens {
        let (kind, problem) = token_type(token);
        problems.extend(problem);
        match kind {
            TokenType::Literal => {
                out.push_str("``");
                out.push_str(token);
                out.push_str("``");
            }
            TokenType::Obj => out.push_str(&convert_obj(token, aliases)),
            TokenType::Control => {
                out.push('*');
                out.push_str(token);
                out.push('*');
            }
            TokenType::Delimiter | TokenType::Reference => out.push_str(token),
        }
    }
    out
}

/// `_convert_type_spec_obj`: a name, translated by `napoleon_type_aliases`, linked.
fn convert_obj(obj: &str, aliases: &[(String, String)]) -> String {
    let translation = aliases.iter().find(|(k, _)| k == obj).map_or(obj, |(_, v)| v.as_str());
    if is_xref(translation) {
        return translation.to_owned();
    }
    match translation {
        "..." => ":py:obj:`... <Ellipsis>`".to_owned(),
        "None" | "True" | "False" | "Ellipsis" => format!(":py:obj:`{translation}`"),
        _ => format!(":py:class:`{translation}`"),
    }
}
