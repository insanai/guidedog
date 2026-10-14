//! A port of Odin's tokenizer (core:odin/tokenizer) with semicolon insertion: the parser
//! reads the same token stream, with the same positions, that Odin's own parser reads.

/// The kind of a token, in the groups Odin's tokenizer names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Tk {
    #[default]
    Invalid,
    Eof,
    Comment,
    FileTag,
    Ident,
    Integer,
    Float,
    Imag,
    Rune,
    String,
    Eq,
    Not,
    Hash,
    At,
    Dollar,
    Pointer,
    Question,
    Add,
    Sub,
    Mul,
    Quo,
    Mod,
    ModMod,
    And,
    Or,
    Xor,
    AndNot,
    Shl,
    Shr,
    CmpAnd,
    CmpOr,
    MulMul,
    AddEq,
    SubEq,
    MulEq,
    QuoEq,
    ModEq,
    ModModEq,
    AndEq,
    OrEq,
    XorEq,
    AndNotEq,
    ShlEq,
    ShrEq,
    CmpAndEq,
    CmpOrEq,
    Increment,
    Decrement,
    ArrowRight,
    Undef,
    CmpEq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    OpenBrace,
    CloseBrace,
    Colon,
    Semicolon,
    Period,
    Comma,
    Ellipsis,
    RangeHalf,
    RangeFull,
    Import,
    Foreign,
    Package,
    Typeid,
    When,
    Where,
    If,
    Else,
    For,
    Switch,
    In,
    NotIn,
    Do,
    Case,
    Break,
    Continue,
    Fallthrough,
    Defer,
    Return,
    Proc,
    Struct,
    Union,
    Enum,
    BitSet,
    BitField,
    Map,
    Dynamic,
    AutoCast,
    Cast,
    Transmute,
    Distinct,
    Using,
    Context,
    OrElse,
    OrReturn,
    OrBreak,
    OrContinue,
    Asm,
    Matrix,
}

const KEYWORDS: [(&str, Tk); 40] = [
    ("import", Tk::Import),
    ("foreign", Tk::Foreign),
    ("package", Tk::Package),
    ("typeid", Tk::Typeid),
    ("when", Tk::When),
    ("where", Tk::Where),
    ("if", Tk::If),
    ("else", Tk::Else),
    ("for", Tk::For),
    ("switch", Tk::Switch),
    ("in", Tk::In),
    ("not_in", Tk::NotIn),
    ("do", Tk::Do),
    ("case", Tk::Case),
    ("break", Tk::Break),
    ("continue", Tk::Continue),
    ("fallthrough", Tk::Fallthrough),
    ("defer", Tk::Defer),
    ("return", Tk::Return),
    ("proc", Tk::Proc),
    ("struct", Tk::Struct),
    ("union", Tk::Union),
    ("enum", Tk::Enum),
    ("bit_set", Tk::BitSet),
    ("bit_field", Tk::BitField),
    ("map", Tk::Map),
    ("dynamic", Tk::Dynamic),
    ("auto_cast", Tk::AutoCast),
    ("cast", Tk::Cast),
    ("transmute", Tk::Transmute),
    ("distinct", Tk::Distinct),
    ("using", Tk::Using),
    ("context", Tk::Context),
    ("or_else", Tk::OrElse),
    ("or_return", Tk::OrReturn),
    ("or_break", Tk::OrBreak),
    ("or_continue", Tk::OrContinue),
    ("asm", Tk::Asm),
    ("matrix", Tk::Matrix),
    ("", Tk::Invalid),
];

impl Tk {
    /// Whether the kind is an assignment operator (`=`, `+=`, ...).
    pub(crate) fn is_assignment(self) -> bool {
        use Tk::*;
        matches!(
            self,
            Eq | AddEq
                | SubEq
                | MulEq
                | QuoEq
                | ModEq
                | ModModEq
                | AndEq
                | OrEq
                | XorEq
                | AndNotEq
                | ShlEq
                | ShrEq
                | CmpAndEq
                | CmpOrEq
        )
    }

    /// Whether the kind is an operator, as `expect_operator` accepts one.
    pub(crate) fn is_operator(self) -> bool {
        let n = self as u8;
        (n >= Tk::Eq as u8 && n <= Tk::RangeFull as u8) || matches!(self, Tk::In | Tk::NotIn)
    }

    /// Whether a newline after a token of this kind ends a statement.
    fn inserts_semicolon(self) -> bool {
        use Tk::*;
        matches!(
            self,
            Ident
                | Context
                | Typeid
                | Break
                | Continue
                | Fallthrough
                | Return
                | Integer
                | Float
                | Imag
                | Rune
                | String
                | Undef
                | Question
                | Pointer
                | CloseParen
                | CloseBracket
                | CloseBrace
                | Increment
                | Decrement
                | OrReturn
                | OrBreak
                | OrContinue
        )
    }
}

/// A token: its kind, its text's byte span, and the line it starts on. A newline that
/// ends a statement is a `Semicolon` whose text is "\n" (`nl`), one byte long.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Token {
    pub kind: Tk,
    pub start: u32,
    pub end: u32,
    pub line: u32,
    pub nl: bool,
}

impl Token {
    /// Whether the token is a newline that ends a statement.
    pub(crate) fn is_newline(&self) -> bool {
        self.kind == Tk::Semicolon && self.nl
    }
}

/// The tokenizer's state; it is `Copy`, so a parser peeks ahead by copying it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Tokenizer<'a> {
    src: &'a [u8],
    ch: i32,
    offset: usize,
    read_offset: usize,
    line_count: u32,
    insert_semicolon: bool,
}

const EOF_CH: i32 = -1;

impl<'a> Tokenizer<'a> {
    /// A tokenizer over `src`, past a leading byte order mark.
    pub(crate) fn new(src: &'a str) -> Self {
        let mut t = Tokenizer {
            src: src.as_bytes(),
            ch: ' ' as i32,
            offset: 0,
            read_offset: 0,
            line_count: u32::from(!src.is_empty()),
            insert_semicolon: false,
        };
        t.advance();
        if t.ch == 0xFEFF {
            t.advance();
        }
        t
    }

    fn advance(&mut self) {
        if self.read_offset < self.src.len() {
            self.offset = self.read_offset;
            if self.ch == '\n' as i32 {
                self.line_count += 1;
            }
            let b = self.src[self.read_offset];
            let (r, w) =
                if b < 0x80 { (i32::from(b), 1) } else { decode(&self.src[self.read_offset..]) };
            self.read_offset += w;
            self.ch = r;
        } else {
            self.offset = self.src.len();
            if self.ch == '\n' as i32 {
                self.line_count += 1;
            }
            self.ch = EOF_CH;
        }
    }

    fn peek_byte(&self) -> u8 {
        self.src.get(self.read_offset).copied().unwrap_or(0)
    }

    fn skip_whitespace(&mut self) {
        loop {
            match self.ch {
                0x20 | 0x09 | 0x0D => self.advance(),
                0x0A if !self.insert_semicolon => self.advance(),
                _ => return,
            }
        }
    }

    fn is_ch(&self, c: u8) -> bool {
        self.ch == i32::from(c)
    }

    /// The next token.
    pub(crate) fn scan(&mut self) -> Token {
        self.skip_whitespace();
        let offset = self.offset;
        let line = self.line_count;
        let ch = self.ch;
        let mut nl = false;
        let mut end_override = None;
        let kind = if is_letter(ch) {
            self.scan_identifier(offset)
        } else if (0x30..=0x39).contains(&ch) {
            self.scan_number(false)
        } else {
            self.advance();
            match self.scan_symbol(ch) {
                Scanned::Kind(k) => k,
                Scanned::Newline => {
                    nl = true;
                    end_override = Some(offset + 1);
                    Tk::Semicolon
                }
                Scanned::EofSemicolon => {
                    let end = offset as u32 + 1;
                    return Token {
                        kind: Tk::Semicolon,
                        start: offset as u32,
                        end,
                        line,
                        nl: true,
                    };
                }
                Scanned::Comment(end) => {
                    end_override = Some(end);
                    Tk::Comment
                }
                Scanned::Backslash => {
                    self.insert_semicolon = false;
                    return self.scan();
                }
            }
        };
        match kind {
            Tk::Invalid | Tk::Comment => {}
            k => self.insert_semicolon = k.inserts_semicolon(),
        }
        let end = end_override.unwrap_or(self.offset) as u32;
        Token { kind, start: offset as u32, end, line, nl }
    }

    fn scan_identifier(&mut self, offset: usize) -> Tk {
        while is_letter(self.ch) || is_digit(self.ch) {
            self.advance();
        }
        let text = &self.src[offset..self.offset];
        if text.len() > 1 && text.len() < 16 {
            for (name, kind) in KEYWORDS {
                if name.as_bytes() == text {
                    return kind;
                }
            }
        }
        Tk::Ident
    }

    fn scan_symbol(&mut self, ch: i32) -> Scanned {
        let Ok(c) = u8::try_from(ch) else {
            if ch != EOF_CH {
                return Scanned::Kind(Tk::Invalid);
            }
            if self.insert_semicolon {
                self.insert_semicolon = false;
                return Scanned::EofSemicolon;
            }
            return Scanned::Kind(Tk::Eof);
        };
        let k = match c {
            b'\n' => {
                self.insert_semicolon = false;
                return Scanned::Newline;
            }
            b'\\' => return Scanned::Backslash,
            b'\'' => {
                self.scan_rune();
                Tk::Rune
            }
            b'"' => {
                if self.is_ch(b'"') && self.peek_byte() == b'"' {
                    self.scan_multiline_string();
                } else {
                    self.scan_string();
                }
                Tk::String
            }
            b'`' => {
                self.scan_raw_string();
                Tk::String
            }
            b'.' => self.scan_period(),
            b'@' => Tk::At,
            b'$' => Tk::Dollar,
            b'?' => Tk::Question,
            b'^' => Tk::Pointer,
            b';' => Tk::Semicolon,
            b',' => Tk::Comma,
            b':' => Tk::Colon,
            b'(' => Tk::OpenParen,
            b')' => Tk::CloseParen,
            b'[' => Tk::OpenBracket,
            b']' => Tk::CloseBracket,
            b'{' => Tk::OpenBrace,
            b'}' => Tk::CloseBrace,
            b'#' => return self.scan_hash(),
            b'/' => return self.scan_slash(),
            _ => self.scan_operator(c),
        };
        Scanned::Kind(k)
    }

    fn scan_period(&mut self) -> Tk {
        if (0x30..=0x39).contains(&self.ch) {
            return self.scan_number(true);
        }
        if !self.is_ch(b'.') {
            return Tk::Period;
        }
        self.advance();
        if self.is_ch(b'<') {
            self.advance();
            return Tk::RangeHalf;
        }
        if self.is_ch(b'=') {
            self.advance();
            return Tk::RangeFull;
        }
        Tk::Ellipsis
    }

    fn scan_hash(&mut self) -> Scanned {
        if self.is_ch(b'!') {
            return Scanned::Comment(self.scan_comment());
        }
        if self.is_ch(b'+') {
            while !self.is_ch(b'\n') && self.ch != EOF_CH {
                if self.is_ch(b'/') && matches!(self.peek_byte(), b'/' | b'*') {
                    break;
                }
                self.advance();
            }
            return Scanned::Kind(Tk::FileTag);
        }
        Scanned::Kind(Tk::Hash)
    }

    fn scan_slash(&mut self) -> Scanned {
        if self.is_ch(b'/') || self.is_ch(b'*') {
            return Scanned::Comment(self.scan_comment());
        }
        Scanned::Kind(self.with_eq(Tk::Quo, Tk::QuoEq))
    }

    fn with_eq(&mut self, plain: Tk, eq: Tk) -> Tk {
        if self.is_ch(b'=') {
            self.advance();
            return eq;
        }
        plain
    }

    /// `c`, then `c` again (`double`), then `=` after either (`eq`, `double_eq`).
    fn doubled(&mut self, c: u8, kinds: [Tk; 4]) -> Tk {
        let [plain, eq, double, double_eq] = kinds;
        if self.is_ch(c) {
            self.advance();
            return self.with_eq(double, double_eq);
        }
        self.with_eq(plain, eq)
    }

    fn scan_operator(&mut self, c: u8) -> Tk {
        use Tk::*;
        match c {
            b'%' => self.doubled(b'%', [Mod, ModEq, ModMod, ModModEq]),
            b'*' => {
                if self.is_ch(b'*') {
                    self.advance();
                    return MulMul;
                }
                self.with_eq(Mul, MulEq)
            }
            b'=' => self.with_eq(Eq, CmpEq),
            b'~' => self.with_eq(Xor, XorEq),
            b'!' => self.with_eq(Not, NotEq),
            b'+' => {
                if self.is_ch(b'+') {
                    self.advance();
                    return Increment;
                }
                self.with_eq(Add, AddEq)
            }
            b'-' => self.scan_minus(),
            b'<' => self.doubled(b'<', [Lt, LtEq, Shl, ShlEq]),
            b'>' => self.doubled(b'>', [Gt, GtEq, Shr, ShrEq]),
            b'&' => {
                if self.is_ch(b'~') {
                    self.advance();
                    return self.with_eq(AndNot, AndNotEq);
                }
                self.doubled(b'&', [And, AndEq, CmpAnd, CmpAndEq])
            }
            b'|' => self.doubled(b'|', [Or, OrEq, CmpOr, CmpOrEq]),
            _ => Invalid,
        }
    }

    fn scan_minus(&mut self) -> Tk {
        if self.is_ch(b'-') {
            self.advance();
            if self.is_ch(b'-') {
                self.advance();
                return Tk::Undef;
            }
            return Tk::Decrement;
        }
        if self.is_ch(b'>') {
            self.advance();
            return Tk::ArrowRight;
        }
        self.with_eq(Tk::Sub, Tk::SubEq)
    }

    /// Scans a comment whose first character was read; returns its end, without a
    /// trailing carriage return on a line comment.
    fn scan_comment(&mut self) -> usize {
        let start = self.offset - 1;
        if self.is_ch(b'/') || self.is_ch(b'!') {
            while !self.is_ch(b'\n') && self.ch >= 0 {
                self.advance();
            }
        } else {
            self.advance();
            let mut nest = 1;
            while self.ch >= 0 && nest > 0 {
                let c = self.ch;
                self.advance();
                if c == '/' as i32 && self.is_ch(b'*') {
                    nest += 1;
                }
                if c == '*' as i32 && self.is_ch(b'/') {
                    nest -= 1;
                    self.advance();
                }
            }
        }
        let mut end = self.offset;
        if self.src.get(start + 1) == Some(&b'/') {
            while end > start + 2 && self.src[end - 1] == b'\r' {
                end -= 1;
            }
        }
        end
    }

    fn scan_string(&mut self) {
        loop {
            let c = self.ch;
            if c == '\n' as i32 || c < 0 {
                break;
            }
            self.advance();
            if c == '"' as i32 {
                break;
            }
            if c == '\\' as i32 {
                self.scan_escape();
            }
        }
    }

    /// A `"""` string, which may span lines, up to the next `"""`. Odin's own
    /// tokenizer in core:odin predates them; the compiler reads them.
    fn scan_multiline_string(&mut self) {
        self.advance();
        self.advance();
        let mut quotes = 0;
        while self.ch != EOF_CH && quotes < 3 {
            let c = self.ch;
            self.advance();
            if c == '\\' as i32 {
                self.advance();
                quotes = 0;
            } else if c == '"' as i32 {
                quotes += 1;
            } else {
                quotes = 0;
            }
        }
    }

    fn scan_raw_string(&mut self) {
        loop {
            let c = self.ch;
            if c == EOF_CH {
                break;
            }
            self.advance();
            if c == '`' as i32 {
                break;
            }
        }
    }

    fn scan_rune(&mut self) {
        loop {
            let c = self.ch;
            if c == '\n' as i32 || c < 0 {
                break;
            }
            self.advance();
            if c == '\'' as i32 {
                break;
            }
            if c == '\\' as i32 {
                self.scan_escape();
            }
        }
    }

    fn scan_escape(&mut self) {
        let n = match u8::try_from(self.ch).unwrap_or(0) {
            b'a' | b'b' | b'e' | b'f' | b'n' | b't' | b'v' | b'r' | b'\\' | b'\'' | b'"' => {
                self.advance();
                return;
            }
            b'0'..=b'7' => (3, 8),
            b'x' => (2, 16),
            b'u' => (4, 16),
            b'U' => (8, 16),
            _ => return,
        };
        if n.1 == 16 {
            self.advance();
        }
        for _ in 0..n.0 {
            if digit_val(self.ch) >= n.1 {
                return;
            }
            self.advance();
        }
    }

    fn scan_mantissa(&mut self, base: u32) {
        while digit_val(self.ch) < base || self.is_ch(b'_') {
            self.advance();
        }
    }

    fn scan_exponent(&mut self, kind: &mut Tk) {
        if self.is_ch(b'e') || self.is_ch(b'E') {
            *kind = Tk::Float;
            self.advance();
            if self.is_ch(b'-') || self.is_ch(b'+') {
                self.advance();
            }
            if digit_val(self.ch) < 10 {
                self.scan_mantissa(10);
            }
        }
        if self.is_ch(b'i') || self.is_ch(b'j') || self.is_ch(b'k') {
            *kind = Tk::Imag;
            self.advance();
        }
    }

    /// Reads a fraction; true when `..` follows the number instead (a range).
    fn scan_fraction(&mut self, kind: &mut Tk) -> bool {
        if self.is_ch(b'.') && self.peek_byte() == b'.' {
            return true;
        }
        if self.is_ch(b'.') {
            *kind = Tk::Float;
            self.advance();
            self.scan_mantissa(10);
        }
        false
    }

    fn scan_number(&mut self, seen_point: bool) -> Tk {
        let mut kind = Tk::Integer;
        if seen_point {
            kind = Tk::Float;
            self.scan_mantissa(10);
            self.scan_exponent(&mut kind);
            return kind;
        }
        if self.is_ch(b'0') {
            self.advance();
            let base = match u8::try_from(self.ch).unwrap_or(0) {
                b'b' => Some(2),
                b'o' => Some(8),
                b'd' => Some(10),
                b'z' => Some(12),
                b'x' => Some(16),
                b'h' => {
                    self.advance();
                    self.scan_mantissa(16);
                    kind = Tk::Float;
                    None
                }
                _ => {
                    self.scan_mantissa(10);
                    if self.is_ch(b'.') && self.scan_fraction(&mut kind) {
                        return kind;
                    }
                    self.scan_exponent(&mut kind);
                    return kind;
                }
            };
            if let Some(b) = base {
                self.advance();
                self.scan_mantissa(b);
            }
        }
        self.scan_mantissa(10);
        if self.scan_fraction(&mut kind) {
            return kind;
        }
        self.scan_exponent(&mut kind);
        kind
    }
}

enum Scanned {
    Kind(Tk),
    Newline,
    EofSemicolon,
    Comment(usize),
    Backslash,
}

fn digit_val(c: i32) -> u32 {
    match u8::try_from(c).unwrap_or(0xFF) {
        c @ b'0'..=b'9' => u32::from(c - b'0'),
        c @ b'a'..=b'f' => u32::from(c - b'a' + 10),
        c @ b'A'..=b'F' => u32::from(c - b'A' + 10),
        _ => 16,
    }
}

fn is_letter(c: i32) -> bool {
    match u8::try_from(c) {
        Ok(b) if b < 0x80 => b == b'_' || b.is_ascii_alphabetic(),
        _ => c >= 0x80 && char::from_u32(c as u32).is_some_and(char::is_alphabetic),
    }
}

fn is_digit(c: i32) -> bool {
    match u8::try_from(c) {
        Ok(b) if b < 0x80 => b.is_ascii_digit(),
        _ => c >= 0x80 && char::from_u32(c as u32).is_some_and(char::is_numeric),
    }
}

/// Decodes the UTF-8 sequence at the start of `s`: the character and its width, or
/// U+FFFD and one byte for an invalid sequence.
fn decode(s: &[u8]) -> (i32, usize) {
    let width = match s[0] {
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => return (0xFFFD, 1),
    };
    match s.get(..width).and_then(|b| std::str::from_utf8(b).ok()) {
        Some(text) => (text.chars().next().map_or(0xFFFD, |c| c as i32), width),
        None => (0xFFFD, 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<Tk> {
        let mut t = Tokenizer::new(src);
        let mut out = Vec::new();
        loop {
            let tok = t.scan();
            if tok.kind == Tk::Eof {
                break;
            }
            out.push(tok.kind);
        }
        out
    }

    #[test]
    fn newlines_end_statements_after_operands() {
        use Tk::*;
        assert_eq!(kinds("x := 1\n"), [Ident, Colon, Eq, Integer, Semicolon]);
        assert_eq!(kinds("a +\nb"), [Ident, Add, Ident, Semicolon]);
        assert_eq!(kinds("x // c\ny"), [Ident, Comment, Semicolon, Ident, Semicolon]);
        assert_eq!(kinds("a ---\n..<"), [Ident, Undef, Semicolon, RangeHalf]);
        assert_eq!(kinds("#+build linux\n"), [FileTag]);
        assert_eq!(
            kinds("1..<2 0x1F 1.5e3i"),
            [Integer, RangeHalf, Integer, Integer, Imag, Semicolon]
        );
    }
}
