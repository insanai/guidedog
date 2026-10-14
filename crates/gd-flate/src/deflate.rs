//! DEFLATE compression: LZ77 matching over a hash chain, then per block the smallest of
//! stored, fixed Huffman or dynamic Huffman coding.

use crate::Level;
use crate::bits::BitWriter;
use crate::huffman;
use crate::inflate::{
    CODE_LENGTH_ORDER, DIST_BASE, DIST_EXTRA, LENGTH_BASE, LENGTH_EXTRA, fixed_literal_lengths,
};

const WINDOW: usize = 32768;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
const HASH_BITS: u32 = 15;
const NONE: u32 = u32::MAX;
/// Tokens per block: enough for the code tables to pay off, few enough to adapt.
const BLOCK_TOKENS: usize = 16384;
const END_OF_BLOCK: usize = 256;

#[derive(Debug, Clone, Copy)]
enum Token {
    Literal(u8),
    Match { length: u16, distance: u16 },
}

/// Compresses `data` as a raw DEFLATE stream.
pub fn deflate(data: &[u8], level: Level) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() / 2 + 16);
    deflate_into(&mut out, data, level);
    out
}

/// Appends `data` compressed as a raw DEFLATE stream to `out`.
pub fn deflate_into(out: &mut Vec<u8>, data: &[u8], level: Level) {
    let mut writer = BitWriter::new(out);
    let (max_chain, lazy) = match level {
        Level::Store => {
            write_stored(&mut writer, data, true);
            return;
        }
        Level::Fast => (16, false),
        Level::Default => (128, true),
        Level::Best => (1024, true),
    };
    let mut matcher = Matcher::new(data, max_chain);
    let mut tokens = Vec::with_capacity(BLOCK_TOKENS);
    let (mut pos, mut block_start) = (0, 0);
    while pos < data.len() {
        pos = matcher.next_token(pos, lazy, &mut tokens);
        if tokens.len() >= BLOCK_TOKENS {
            write_block(&mut writer, &tokens, &data[block_start..pos], pos == data.len());
            tokens.clear();
            block_start = pos;
        }
    }
    if !tokens.is_empty() || block_start == 0 {
        write_block(&mut writer, &tokens, &data[block_start..], true);
    }
    writer.align();
}

/// Finds earlier occurrences of the text at a position through chains of positions that
/// share a three-byte hash.
struct Matcher<'d> {
    data: &'d [u8],
    head: Vec<u32>,
    prev: Vec<u32>,
    /// Positions below this are in the chains.
    inserted: usize,
    max_chain: usize,
}

impl<'d> Matcher<'d> {
    fn new(data: &'d [u8], max_chain: usize) -> Matcher<'d> {
        let window = WINDOW.min(data.len().max(1));
        Matcher {
            data,
            head: vec![NONE; 1 << HASH_BITS],
            prev: vec![NONE; window],
            inserted: 0,
            max_chain,
        }
    }

    fn hash(&self, pos: usize) -> usize {
        let d = self.data;
        let v = u32::from(d[pos]) << 16 | u32::from(d[pos + 1]) << 8 | u32::from(d[pos + 2]);
        (v.wrapping_mul(2_654_435_761) >> (32 - HASH_BITS)) as usize
    }

    fn insert_until(&mut self, end: usize) {
        while self.inserted < end {
            let pos = self.inserted;
            if pos + MIN_MATCH <= self.data.len() {
                let h = self.hash(pos);
                let slot = pos % self.prev.len();
                self.prev[slot] = self.head[h];
                self.head[h] = pos as u32;
            }
            self.inserted += 1;
        }
    }

    /// The longest earlier match for the text at `pos` as (length, distance), or (0, 0).
    /// Positions before `pos` must be inserted, and `pos` itself not.
    fn find(&self, pos: usize) -> (usize, usize) {
        let data = self.data;
        if pos + MIN_MATCH > data.len() {
            return (0, 0);
        }
        let max_len = MAX_MATCH.min(data.len() - pos);
        let (mut best_len, mut best_dist) = (0, 0);
        let mut candidate = self.head[self.hash(pos)];
        for _ in 0..self.max_chain {
            if candidate == NONE {
                break;
            }
            let at = candidate as usize;
            let distance = pos - at;
            if distance > WINDOW || distance > self.prev.len() {
                break;
            }
            // Only a candidate that also matches the byte just past the best can beat it.
            if data[at + best_len] == data[pos + best_len] {
                let len = data[at..at + max_len]
                    .iter()
                    .zip(&data[pos..])
                    .take_while(|(a, b)| a == b)
                    .count();
                if len > best_len {
                    (best_len, best_dist) = (len, distance);
                    if len == max_len {
                        break;
                    }
                }
            }
            let next = self.prev[at % self.prev.len()];
            if next != NONE && next as usize >= at {
                break;
            }
            candidate = next;
        }
        if best_len >= MIN_MATCH { (best_len, best_dist) } else { (0, 0) }
    }

    /// Emits the token at `pos` and returns the position after it. With `lazy`, a match
    /// is deferred by one byte when the next position has a longer one.
    fn next_token(&mut self, pos: usize, lazy: bool, tokens: &mut Vec<Token>) -> usize {
        self.insert_until(pos);
        let (length, distance) = self.find(pos);
        if length == 0 {
            tokens.push(Token::Literal(self.data[pos]));
            return pos + 1;
        }
        if lazy && length < MAX_MATCH && pos + 1 < self.data.len() {
            self.insert_until(pos + 1);
            if self.find(pos + 1).0 > length {
                tokens.push(Token::Literal(self.data[pos]));
                return pos + 1;
            }
        }
        tokens.push(Token::Match { length: length as u16, distance: distance as u16 });
        pos + length
    }
}

fn length_symbol(length: usize) -> usize {
    LENGTH_BASE.partition_point(|&base| usize::from(base) <= length) - 1
}

fn distance_symbol(distance: usize) -> usize {
    DIST_BASE.partition_point(|&base| usize::from(base) <= distance) - 1
}

/// Symbol frequencies of a block: literal/length symbols (with end-of-block) and
/// distance symbols.
fn frequencies(tokens: &[Token]) -> ([u32; 286], [u32; 30]) {
    let (mut literals, mut distances) = ([0u32; 286], [0u32; 30]);
    for token in tokens {
        match *token {
            Token::Literal(b) => literals[usize::from(b)] += 1,
            Token::Match { length, distance } => {
                literals[257 + length_symbol(usize::from(length))] += 1;
                distances[distance_symbol(usize::from(distance))] += 1;
            }
        }
    }
    literals[END_OF_BLOCK] += 1;
    (literals, distances)
}

/// The bits a block's symbols take with the given code lengths, extra bits included.
fn symbol_bits(
    literals: &[u32],
    literal_lengths: &[u8],
    distances: &[u32],
    distance_lengths: &[u8],
) -> usize {
    let mut bits = 0;
    for (s, &f) in literals.iter().enumerate() {
        let extra = if s > 256 { LENGTH_EXTRA[s - 257] } else { 0 };
        bits += f as usize * usize::from(literal_lengths[s] + extra);
    }
    for (s, &f) in distances.iter().enumerate() {
        bits += f as usize * usize::from(distance_lengths[s] + DIST_EXTRA[s]);
    }
    bits
}

fn stored_bits(len: usize) -> usize {
    // Per 65535-byte piece: 3 header bits padded to a byte, then LEN and NLEN.
    (len / 65535 + 1) * 5 * 8 + len * 8
}

/// Writes one block of `tokens` (which encode `raw`) in whichever coding is smallest.
fn write_block(w: &mut BitWriter<'_>, tokens: &[Token], raw: &[u8], last: bool) {
    let (literals, distances) = frequencies(tokens);
    let literal_lengths = huffman::lengths(&literals, 15);
    let distance_lengths = huffman::lengths(&distances, 15);
    let header = DynamicHeader::new(&literal_lengths, &distance_lengths);
    let dynamic = 3
        + header.bits()
        + symbol_bits(&literals, &literal_lengths, &distances, &distance_lengths);
    let fixed_lengths = fixed_literal_lengths();
    let fixed = 3 + symbol_bits(&literals, &fixed_lengths[..286], &distances, &[5; 30]);
    if stored_bits(raw.len()) <= dynamic.min(fixed) {
        write_stored(w, raw, last);
    } else if fixed <= dynamic {
        w.bits(u32::from(last) | 1 << 1, 3);
        write_tokens(w, tokens, &fixed_lengths, &[5; 30]);
    } else {
        w.bits(u32::from(last) | 2 << 1, 3);
        header.write(w);
        write_tokens(w, tokens, &literal_lengths, &distance_lengths);
    }
}

fn write_stored(w: &mut BitWriter<'_>, raw: &[u8], last: bool) {
    let mut pieces = raw.chunks(65535).peekable();
    if raw.is_empty() {
        w.bits(u32::from(last), 3);
        w.align();
        w.out.extend_from_slice(&[0, 0, 0xff, 0xff]);
        return;
    }
    while let Some(piece) = pieces.next() {
        let final_piece = last && pieces.peek().is_none();
        w.bits(u32::from(final_piece), 3);
        w.align();
        let len = piece.len() as u16;
        w.out.extend_from_slice(&len.to_le_bytes());
        w.out.extend_from_slice(&(!len).to_le_bytes());
        w.out.extend_from_slice(piece);
    }
}

fn write_tokens(
    w: &mut BitWriter<'_>,
    tokens: &[Token],
    literal_lengths: &[u8],
    distance_lengths: &[u8],
) {
    let literal_codes = huffman::codes(literal_lengths);
    let distance_codes = huffman::codes(distance_lengths);
    let symbol = |w: &mut BitWriter<'_>, s: usize| {
        w.bits(u32::from(literal_codes[s]), u32::from(literal_lengths[s]));
    };
    for token in tokens {
        match *token {
            Token::Literal(b) => symbol(w, usize::from(b)),
            Token::Match { length, distance } => {
                let (length, distance) = (usize::from(length), usize::from(distance));
                let l = length_symbol(length);
                symbol(w, 257 + l);
                w.bits((length - usize::from(LENGTH_BASE[l])) as u32, u32::from(LENGTH_EXTRA[l]));
                let d = distance_symbol(distance);
                w.bits(u32::from(distance_codes[d]), u32::from(distance_lengths[d]));
                w.bits((distance - usize::from(DIST_BASE[d])) as u32, u32::from(DIST_EXTRA[d]));
            }
        }
    }
    symbol(w, END_OF_BLOCK);
}

/// The code-length description of a dynamic block's two Huffman codes.
struct DynamicHeader {
    literal_count: usize,
    distance_count: usize,
    /// Run-length coded lengths: (code-length symbol 0..=18, its extra-bit value).
    runs: Vec<(u8, u8)>,
    code_lengths: Vec<u8>,
    code_length_count: usize,
}

impl DynamicHeader {
    fn new(literal_lengths: &[u8], distance_lengths: &[u8]) -> DynamicHeader {
        let used = |lengths: &[u8], min: usize| {
            lengths.iter().rposition(|&l| l > 0).map_or(min, |i| (i + 1).max(min))
        };
        let literal_count = used(literal_lengths, 257);
        let distance_count = used(distance_lengths, 1);
        let all: Vec<u8> = literal_lengths[..literal_count]
            .iter()
            .chain(&distance_lengths[..distance_count])
            .copied()
            .collect();
        let runs = run_lengths(&all);
        let mut freqs = [0u32; 19];
        for &(symbol, _) in &runs {
            freqs[usize::from(symbol)] += 1;
        }
        let code_lengths = huffman::lengths(&freqs, 7);
        let code_length_count = CODE_LENGTH_ORDER
            .iter()
            .rposition(|&s| code_lengths[s] > 0)
            .map_or(4, |i| (i + 1).max(4));
        DynamicHeader { literal_count, distance_count, runs, code_lengths, code_length_count }
    }

    fn bits(&self) -> usize {
        let extra = |s: u8| match s {
            16 => 2,
            17 => 3,
            18 => 7,
            _ => 0,
        };
        let runs: usize = self
            .runs
            .iter()
            .map(|&(s, _)| usize::from(self.code_lengths[usize::from(s)]) + extra(s))
            .sum();
        14 + 3 * self.code_length_count + runs
    }

    fn write(&self, w: &mut BitWriter<'_>) {
        w.bits((self.literal_count - 257) as u32, 5);
        w.bits((self.distance_count - 1) as u32, 5);
        w.bits((self.code_length_count - 4) as u32, 4);
        for &s in &CODE_LENGTH_ORDER[..self.code_length_count] {
            w.bits(u32::from(self.code_lengths[s]), 3);
        }
        let codes = huffman::codes(&self.code_lengths);
        for &(symbol, extra) in &self.runs {
            let s = usize::from(symbol);
            w.bits(u32::from(codes[s]), u32::from(self.code_lengths[s]));
            match symbol {
                16 => w.bits(u32::from(extra), 2),
                17 => w.bits(u32::from(extra), 3),
                18 => w.bits(u32::from(extra), 7),
                _ => {}
            }
        }
    }
}

/// Codes a list of code lengths with DEFLATE's repeat symbols: 16 repeats the previous
/// length 3 to 6 times, 17 and 18 write 3 to 10 and 11 to 138 zeros.
fn run_lengths(lengths: &[u8]) -> Vec<(u8, u8)> {
    let mut runs = Vec::new();
    let mut i = 0;
    while i < lengths.len() {
        let value = lengths[i];
        let run = lengths[i..].iter().take_while(|&&l| l == value).count();
        if value == 0 && run >= 11 {
            let n = run.min(138);
            runs.push((18, (n - 11) as u8));
            i += n;
        } else if value == 0 && run >= 3 {
            let n = run.min(10);
            runs.push((17, (n - 3) as u8));
            i += n;
        } else if value != 0 && run >= 4 {
            runs.push((value, 0));
            let n = (run - 1).min(6);
            runs.push((16, (n - 3) as u8));
            i += 1 + n;
        } else {
            runs.push((value, 0));
            i += 1;
        }
    }
    runs
}
