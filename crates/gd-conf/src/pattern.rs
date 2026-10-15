//! `Pattern`: the regular expressions of `nitpick_ignore_regex`, matched against whole
//! names as Sphinx's `re.fullmatch` does.
//!
//! The standard library has no regular expressions, and these patterns are small, so
//! this is a small backtracking matcher for the subset conf.py patterns use: literals,
//! `.`, classes (`[a-z]`, `[^...]`), `\d \w \s` and their negations, escapes, groups
//! (`(...)`, `(?:...)`), alternation, `^`, `$`, and the quantifiers `* + ? {m} {m,}
//! {m,n}` (lazy forms accepted). Work is bounded: a pattern is at most
//! `MAX_PATTERN_CHARS` long, and a match gives up (no match) after `MAX_STEPS` steps.

use std::fmt;

/// The longest pattern accepted, in characters.
const MAX_PATTERN_CHARS: usize = 1024;
/// The deepest group nesting accepted.
const MAX_GROUP_DEPTH: usize = 32;
/// The most steps one match may take; past it the name counts as not matching.
const MAX_STEPS: usize = 200_000;
/// The longest name matched; longer names count as not matching.
const MAX_NAME_CHARS: usize = 4096;
/// The largest count a `{m,n}` quantifier may give.
const MAX_REPEAT: u32 = 1000;

/// A compiled pattern. Two patterns are equal when their sources are.
#[derive(Clone)]
pub struct Pattern {
    source: String,
    root: Alternatives,
}

impl fmt::Debug for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Pattern({:?})", self.source)
    }
}

impl PartialEq for Pattern {
    fn eq(&self, other: &Pattern) -> bool {
        self.source == other.source
    }
}

/// Why a pattern does not compile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternError {
    /// What is wrong, as a sentence fragment ("an unclosed group").
    pub message: String,
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PatternError {}

type Alternatives = Vec<Vec<Piece>>;

#[derive(Debug, Clone)]
struct Piece {
    atom: Atom,
    min: u32,
    max: Option<u32>,
}

#[derive(Debug, Clone)]
enum Atom {
    Char(char),
    Any,
    Class(Class),
    Start,
    End,
    Group(Alternatives),
}

#[derive(Debug, Clone)]
struct Class {
    negated: bool,
    ranges: Vec<(char, char)>,
    named: Vec<char>,
}

impl Class {
    fn matches(&self, c: char) -> bool {
        let inside = self.ranges.iter().any(|&(lo, hi)| lo <= c && c <= hi)
            || self.named.iter().any(|&n| named_class(n, c));
        inside != self.negated
    }
}

fn named_class(name: char, c: char) -> bool {
    match name {
        'd' => c.is_numeric(),
        'D' => !c.is_numeric(),
        'w' => c.is_alphanumeric() || c == '_',
        'W' => !(c.is_alphanumeric() || c == '_'),
        's' => c.is_whitespace(),
        'S' => !c.is_whitespace(),
        _ => false,
    }
}

impl Pattern {
    /// Compiles `source`; fails with what is wrong when it is not a pattern this
    /// matcher reads.
    pub fn new(source: &str) -> Result<Pattern, PatternError> {
        let chars: Vec<char> = source.chars().collect();
        if chars.len() > MAX_PATTERN_CHARS {
            return Err(error(&format!("longer than {MAX_PATTERN_CHARS} characters")));
        }
        let mut parser = Parser { chars: &chars, at: 0, depth: 0 };
        let root = parser.alternatives()?;
        if parser.at < chars.len() {
            return Err(error("a ) that closes no group"));
        }
        Ok(Pattern { source: source.to_string(), root })
    }

    /// The pattern as written.
    pub fn as_str(&self) -> &str {
        &self.source
    }

    /// Whether the pattern matches the whole of `name`, as Python's `re.fullmatch`.
    pub fn full_match(&self, name: &str) -> bool {
        let text: Vec<char> = name.chars().collect();
        if text.len() > MAX_NAME_CHARS {
            return false;
        }
        let mut m = Matcher { text: &text, steps: 0 };
        m.alternatives(&self.root, 0, &mut |end, _| end == text.len())
    }
}

fn error(message: &str) -> PatternError {
    PatternError { message: message.to_string() }
}

struct Parser<'a> {
    chars: &'a [char],
    at: usize,
    depth: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.at += 1;
            return true;
        }
        false
    }

    fn alternatives(&mut self) -> Result<Alternatives, PatternError> {
        let mut out = vec![self.sequence()?];
        while self.eat('|') {
            out.push(self.sequence()?);
        }
        Ok(out)
    }

    fn sequence(&mut self) -> Result<Vec<Piece>, PatternError> {
        let mut pieces = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom()?;
            let (min, max) = self.quantifier()?;
            pieces.push(Piece { atom, min, max });
        }
        Ok(pieces)
    }

    fn atom(&mut self) -> Result<Atom, PatternError> {
        let Some(c) = self.peek() else { return Err(error("an unexpected end")) };
        self.at += 1;
        match c {
            '.' => Ok(Atom::Any),
            '^' => Ok(Atom::Start),
            '$' => Ok(Atom::End),
            '[' => self.class().map(Atom::Class),
            '(' => self.group(),
            '\\' => self.escape(),
            '*' | '+' | '?' | '{' => Err(error(&format!("nothing to repeat before {c}"))),
            _ => Ok(Atom::Char(c)),
        }
    }

    fn group(&mut self) -> Result<Atom, PatternError> {
        self.depth += 1;
        if self.depth > MAX_GROUP_DEPTH {
            return Err(error("groups nested too deep"));
        }
        if self.eat('?') && !self.eat(':') {
            return Err(error("a group extension other than (?:...)"));
        }
        let inner = self.alternatives()?;
        if !self.eat(')') {
            return Err(error("an unclosed group"));
        }
        self.depth -= 1;
        Ok(Atom::Group(inner))
    }

    fn escape(&mut self) -> Result<Atom, PatternError> {
        let Some(c) = self.peek() else { return Err(error("a trailing backslash")) };
        self.at += 1;
        Ok(match c {
            'd' | 'D' | 'w' | 'W' | 's' | 'S' => {
                Atom::Class(Class { negated: false, ranges: Vec::new(), named: vec![c] })
            }
            'n' => Atom::Char('\n'),
            't' => Atom::Char('\t'),
            c if c.is_ascii_alphanumeric() => {
                return Err(error(&format!("the escape \\{c}")));
            }
            c => Atom::Char(c),
        })
    }

    fn class(&mut self) -> Result<Class, PatternError> {
        let mut class = Class { negated: self.eat('^'), ranges: Vec::new(), named: Vec::new() };
        let mut first = true;
        loop {
            let Some(c) = self.peek() else { return Err(error("an unclosed [")) };
            self.at += 1;
            if c == ']' && !first {
                return Ok(class);
            }
            first = false;
            let lo = match c {
                '\\' => match self.peek() {
                    Some(n @ ('d' | 'D' | 'w' | 'W' | 's' | 'S')) => {
                        self.at += 1;
                        class.named.push(n);
                        continue;
                    }
                    Some(n) => {
                        self.at += 1;
                        n
                    }
                    None => return Err(error("an unclosed [")),
                },
                c => c,
            };
            let hi = self.range_end(lo)?;
            class.ranges.push((lo, hi));
        }
    }

    fn range_end(&mut self, lo: char) -> Result<char, PatternError> {
        let dash = self.peek() == Some('-');
        let next = self.chars.get(self.at + 1).copied();
        if !dash || next.is_none() || next == Some(']') {
            return Ok(lo);
        }
        self.at += 1;
        let mut hi = self.chars[self.at];
        self.at += 1;
        if hi == '\\' {
            hi = self.peek().ok_or_else(|| error("an unclosed ["))?;
            self.at += 1;
        }
        if hi < lo {
            return Err(error(&format!("the reversed range {lo}-{hi}")));
        }
        Ok(hi)
    }

    fn quantifier(&mut self) -> Result<(u32, Option<u32>), PatternError> {
        let bounds = match self.peek() {
            Some('*') => (0, None),
            Some('+') => (1, None),
            Some('?') => (0, Some(1)),
            Some('{') => return self.braces(),
            _ => return Ok((1, Some(1))),
        };
        self.at += 1;
        self.eat('?'); // lazy: the same answer for a whole-name match.
        Ok(bounds)
    }

    fn braces(&mut self) -> Result<(u32, Option<u32>), PatternError> {
        self.at += 1;
        let min = self.number().ok_or_else(|| error("a { without a count"))?;
        let max = if self.eat(',') { self.number() } else { Some(min) };
        if !self.eat('}') {
            return Err(error("an unclosed {"));
        }
        if max.is_some_and(|m| m < min) {
            return Err(error("a {m,n} with n below m"));
        }
        self.eat('?');
        Ok((min, max))
    }

    fn number(&mut self) -> Option<u32> {
        let start = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
        }
        let digits: String = self.chars[start..self.at].iter().collect();
        digits.parse().ok().filter(|&n| n <= MAX_REPEAT)
    }
}

/// The continuation a match calls with where it ended; it says whether the rest of the
/// pattern matched from there.
type Next<'k> = dyn FnMut(usize, &mut Matcher<'_>) -> bool + 'k;

struct Matcher<'t> {
    text: &'t [char],
    steps: usize,
}

impl Matcher<'_> {
    fn spend(&mut self) -> bool {
        self.steps += 1;
        self.steps <= MAX_STEPS
    }

    fn alternatives(&mut self, alts: &Alternatives, at: usize, next: &mut Next<'_>) -> bool {
        alts.iter().any(|seq| self.sequence(seq, at, next))
    }

    fn sequence(&mut self, seq: &[Piece], at: usize, next: &mut Next<'_>) -> bool {
        let Some((first, rest)) = seq.split_first() else { return next(at, self) };
        self.piece(first, 0, at, &mut |end, m: &mut Matcher<'_>| m.sequence(rest, end, next))
    }

    /// Matches `piece` repeated, `done` times so far, greedily, then the rest.
    fn piece(&mut self, piece: &Piece, done: u32, at: usize, next: &mut Next<'_>) -> bool {
        if !self.spend() {
            return false;
        }
        let more = piece.max.is_none_or(|max| done < max);
        if more {
            let again = &mut |end, m: &mut Matcher<'_>| {
                // An empty repetition would loop for ever; it adds nothing anyway. One
                // the minimum needs counts: `^`, `$` and empty groups match nothing.
                (end != at || done < piece.min) && m.piece(piece, done + 1, end, next)
            };
            if self.atom(&piece.atom, at, again) {
                return true;
            }
        }
        done >= piece.min && next(at, self)
    }

    fn atom(&mut self, atom: &Atom, at: usize, next: &mut Next<'_>) -> bool {
        let here = self.text.get(at).copied();
        match atom {
            Atom::Char(c) => here == Some(*c) && next(at + 1, self),
            Atom::Any => here.is_some_and(|c| c != '\n') && next(at + 1, self),
            Atom::Class(class) => here.is_some_and(|c| class.matches(c)) && next(at + 1, self),
            Atom::Start => at == 0 && next(at, self),
            Atom::End => at == self.text.len() && next(at, self),
            Atom::Group(alts) => self.alternatives(alts, at, next),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(pattern: &str, name: &str) -> bool {
        Pattern::new(pattern).unwrap().full_match(name)
    }

    #[test]
    fn patterns_match_whole_names() {
        assert!(matches(r"py:class", "py:class"));
        assert!(!matches(r"py:class", "py:classes"));
        assert!(matches(r"py:.*", "py:func"));
        assert!(matches(r"mod\.Name", "mod.Name"));
        assert!(!matches(r"mod\.Name", "modxName"));
        assert!(matches(r"(foo|bar)\d+", "bar12"));
        assert!(matches(r"(?:a|b)*c", "ababc"));
        assert!(matches(r"[a-c]{2,3}", "abc"));
        assert!(!matches(r"[a-c]{2,3}", "abcd"));
        assert!(matches(r"[^x]+", "abc"));
        assert!(matches(r"\w+\s?\w*", "hello world"));
        assert!(matches(r"x?y*", ""));
        assert!(matches(r"(a*)*b", "aaab"));
        assert!(matches(r"^a.*b$", "axb") && matches(r"(?:)x", "x") && !matches(r"a$b", "ab"));
    }

    #[test]
    fn bad_patterns_say_why() {
        assert!(Pattern::new("(abc").is_err());
        assert!(Pattern::new("abc)").is_err());
        assert!(Pattern::new("*a").is_err());
        assert!(Pattern::new("[abc").is_err());
        assert!(Pattern::new("a{3,1}").is_err());
        assert!(Pattern::new("\\q").is_err());
    }

    #[test]
    fn catastrophic_patterns_give_up() {
        let text = "a".repeat(40);
        assert!(!matches("(a*)*b", &text));
    }
}
