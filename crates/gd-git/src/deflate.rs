//! A streaming zlib compressor (RFC 1950/1951) for pack objects: LZ77 with hash chains
//! inside each block, coded with the fixed Huffman tables, or stored when that is
//! smaller (already-compressed images and fonts). It replaces the Odin port's zlib.

const MAX_BLOCK: usize = 65_535;
const WINDOW: usize = 32_768;
const HASH_BITS: u32 = 15;
const MAX_CHAIN: usize = 32;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] =
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

#[derive(Debug, Clone, Copy)]
enum Token {
    Literal(u8),
    Match { len: u16, dist: u16 },
}

/// One zlib stream: feed it with [`Deflater::push`], the final piece with `last` set.
#[derive(Debug)]
pub(crate) struct Deflater {
    acc: u64,
    bits: u32,
    adler_a: u32,
    adler_b: u32,
    started: bool,
    head: Vec<i32>,
    prev: Vec<i32>,
    tokens: Vec<Token>,
}

impl Deflater {
    pub(crate) fn new() -> Deflater {
        Deflater {
            acc: 0,
            bits: 0,
            adler_a: 1,
            adler_b: 0,
            started: false,
            head: vec![-1; 1 << HASH_BITS],
            prev: Vec::new(),
            tokens: Vec::new(),
        }
    }

    /// Starts a new stream, keeping the buffers of the last one.
    pub(crate) fn reset(&mut self) {
        (self.acc, self.bits, self.adler_a, self.adler_b) = (0, 0, 1, 0);
        self.started = false;
    }

    /// Compresses `data` and appends the output to `out`; `last` ends the stream with
    /// its checksum.
    pub(crate) fn push(&mut self, data: &[u8], last: bool, out: &mut Vec<u8>) {
        if !self.started {
            out.extend_from_slice(&[0x78, 0x9C]);
            self.started = true;
        }
        self.adler(data);
        let mut blocks = data.chunks(MAX_BLOCK).peekable();
        if data.is_empty() && last {
            self.block(&[], true, out);
        }
        while let Some(block) = blocks.next() {
            self.block(block, last && blocks.peek().is_none(), out);
        }
        if last {
            self.align(out);
            let sum = (self.adler_b << 16) | self.adler_a;
            out.extend_from_slice(&sum.to_be_bytes());
        }
    }

    fn adler(&mut self, data: &[u8]) {
        for piece in data.chunks(5552) {
            for &c in piece {
                self.adler_a += u32::from(c);
                self.adler_b += self.adler_a;
            }
            self.adler_a %= 65_521;
            self.adler_b %= 65_521;
        }
    }

    fn block(&mut self, data: &[u8], last: bool, out: &mut Vec<u8>) {
        self.tokenize(data);
        let fixed: usize = 3 + 7 + self.tokens.iter().map(|&t| token_bits(t)).sum::<usize>();
        let stored = 3 + 7 + 32 + 8 * data.len();
        self.put(u32::from(last), 1, out);
        if fixed <= stored {
            self.put(1, 2, out);
            for i in 0..self.tokens.len() {
                let token = self.tokens[i];
                self.put_token(token, out);
            }
            self.put_code(0, 7, out);
            return;
        }
        self.put(0, 2, out);
        self.align(out);
        let len = data.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(data);
    }

    fn tokenize(&mut self, data: &[u8]) {
        self.tokens.clear();
        self.head.fill(-1);
        self.prev.clear();
        self.prev.resize(data.len(), -1);
        let mut i = 0;
        while i < data.len() {
            let (len, dist) = self.longest(data, i);
            if len >= MIN_MATCH {
                self.tokens.push(Token::Match { len: len as u16, dist: dist as u16 });
                for at in i + 1..i + len {
                    self.insert(data, at);
                }
                i += len;
            } else {
                self.tokens.push(Token::Literal(data[i]));
                i += 1;
            }
        }
    }

    /// Inserts position `at` into the hash chains and returns the previous chain head.
    fn insert(&mut self, data: &[u8], at: usize) -> i32 {
        if at + MIN_MATCH > data.len() {
            return -1;
        }
        let h = hash(&data[at..at + MIN_MATCH]);
        let previous = self.head[h];
        self.prev[at] = previous;
        self.head[h] = at as i32;
        previous
    }

    fn longest(&mut self, data: &[u8], i: usize) -> (usize, usize) {
        let mut candidate = self.insert(data, i);
        let limit = MAX_MATCH.min(data.len() - i);
        let (mut best, mut best_dist) = (0, 0);
        let mut steps = 0;
        while candidate >= 0 && steps < MAX_CHAIN {
            let c = candidate as usize;
            if i - c > WINDOW {
                break;
            }
            let len =
                data[c..].iter().zip(&data[i..i + limit]).take_while(|(a, b)| a == b).count();
            if len > best {
                (best, best_dist) = (len, i - c);
                if len == limit {
                    break;
                }
            }
            candidate = self.prev[c];
            steps += 1;
        }
        (best, best_dist)
    }

    fn put_token(&mut self, token: Token, out: &mut Vec<u8>) {
        match token {
            Token::Literal(c) => {
                let (code, len) = literal_code(u16::from(c));
                self.put_code(code, len, out);
            }
            Token::Match { len, dist } => {
                let l = LEN_BASE.partition_point(|&b| b <= len) - 1;
                let (code, bits) = literal_code(257 + l as u16);
                self.put_code(code, bits, out);
                self.put(u32::from(len - LEN_BASE[l]), u32::from(LEN_EXTRA[l]), out);
                let d = DIST_BASE.partition_point(|&b| b <= dist) - 1;
                self.put_code(d as u32, 5, out);
                self.put(u32::from(dist - DIST_BASE[d]), u32::from(DIST_EXTRA[d]), out);
            }
        }
    }

    /// Writes a Huffman code, which DEFLATE stores with its first bit first.
    fn put_code(&mut self, code: u32, len: u32, out: &mut Vec<u8>) {
        let reversed = code.reverse_bits() >> (32 - len);
        self.put(reversed, len, out);
    }

    fn put(&mut self, value: u32, len: u32, out: &mut Vec<u8>) {
        self.acc |= u64::from(value) << self.bits;
        self.bits += len;
        while self.bits >= 8 {
            out.push(self.acc as u8);
            self.acc >>= 8;
            self.bits -= 8;
        }
    }

    fn align(&mut self, out: &mut Vec<u8>) {
        if self.bits > 0 {
            out.push(self.acc as u8);
        }
        self.acc = 0;
        self.bits = 0;
    }
}

fn hash(three: &[u8]) -> usize {
    let v = u32::from(three[0]) << 16 | u32::from(three[1]) << 8 | u32::from(three[2]);
    (v.wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize
}

/// The fixed Huffman code of a literal/length symbol, and its length in bits.
fn literal_code(symbol: u16) -> (u32, u32) {
    let s = u32::from(symbol);
    match symbol {
        0..=143 => (0x30 + s, 8),
        144..=255 => (0x190 + s - 144, 9),
        256..=279 => (s - 256, 7),
        _ => (0xC0 + s - 280, 8),
    }
}

fn token_bits(token: Token) -> usize {
    match token {
        Token::Literal(c) => literal_code(u16::from(c)).1 as usize,
        Token::Match { len, dist } => {
            let l = LEN_BASE.partition_point(|&b| b <= len) - 1;
            let d = DIST_BASE.partition_point(|&b| b <= dist) - 1;
            let code = literal_code(257 + l as u16).1 as usize;
            code + usize::from(LEN_EXTRA[l]) + 5 + usize::from(DIST_EXTRA[d])
        }
    }
}
