//! DEFLATE decompression.

use crate::bits::BitReader;
use crate::huffman::Decoder;
use crate::{Error, ErrorKind};

#[rustfmt::skip]
pub(crate) const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
    131, 163, 195, 227, 258,
];
#[rustfmt::skip]
pub(crate) const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
#[rustfmt::skip]
pub(crate) const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
#[rustfmt::skip]
pub(crate) const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
/// The order in which a dynamic block lists code-length code lengths.
pub(crate) const CODE_LENGTH_ORDER: [usize; 19] =
    [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

/// The literal/length code lengths of a fixed Huffman block.
pub(crate) fn fixed_literal_lengths() -> [u8; 288] {
    let mut lengths = [8u8; 288];
    lengths[144..256].fill(9);
    lengths[256..280].fill(7);
    lengths
}

/// Decompresses the raw DEFLATE stream at the start of `data`, refusing output beyond
/// `limit` bytes. Bytes after the final block are ignored.
pub fn inflate(data: &[u8], limit: usize) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    inflate_into(&mut out, data, limit)?;
    Ok(out)
}

/// Appends the decompressed DEFLATE stream at the start of `data` to `out`, producing at
/// most `limit` bytes, and returns how many input bytes the stream used. Matches may not
/// reach into what `out` held before the call.
pub fn inflate_into(out: &mut Vec<u8>, data: &[u8], limit: usize) -> Result<usize, Error> {
    let mut inflater = Inflater { bits: BitReader::new(data), out, start: 0, limit };
    inflater.start = inflater.out.len();
    loop {
        let last = inflater.bits.bits(1)? == 1;
        match inflater.bits.bits(2)? {
            0 => inflater.stored()?,
            1 => {
                let literals = Decoder::new(&fixed_literal_lengths(), 0)?;
                let distances = Decoder::new(&[5u8; 30], 0)?;
                inflater.codes(&literals, &distances)?;
            }
            2 => inflater.dynamic()?,
            _ => return Err(inflater.error(ErrorKind::Block)),
        }
        if last {
            return Ok(inflater.bits.pos);
        }
    }
}

struct Inflater<'a, 'o> {
    bits: BitReader<'a>,
    out: &'o mut Vec<u8>,
    /// Where this stream's output starts in `out`.
    start: usize,
    limit: usize,
}

impl Inflater<'_, '_> {
    fn error(&self, kind: ErrorKind) -> Error {
        Error { kind, offset: self.bits.pos }
    }

    /// Checks that `n` more bytes stay within the limit, and reserves room for them.
    fn make_room(&mut self, n: usize) -> Result<(), Error> {
        let produced = self.out.len() - self.start;
        if n > self.limit.saturating_sub(produced) {
            return Err(self.error(ErrorKind::Limit));
        }
        Ok(())
    }

    fn stored(&mut self) -> Result<(), Error> {
        self.bits.align();
        let len = self.bits.bits(16)?;
        let complement = self.bits.bits(16)?;
        if len != !complement & 0xffff {
            return Err(self.error(ErrorKind::Block));
        }
        let len = len as usize;
        self.make_room(len)?;
        let bytes = self.bits.take_bytes(len)?;
        self.out.extend_from_slice(bytes);
        Ok(())
    }

    fn dynamic(&mut self) -> Result<(), Error> {
        let literal_count = self.bits.bits(5)? as usize + 257;
        let distance_count = self.bits.bits(5)? as usize + 1;
        let code_length_count = self.bits.bits(4)? as usize + 4;
        if literal_count > 286 || distance_count > 30 {
            return Err(self.error(ErrorKind::Block));
        }
        let mut code_lengths = [0u8; 19];
        for &symbol in &CODE_LENGTH_ORDER[..code_length_count] {
            code_lengths[symbol] = self.bits.bits(3)? as u8;
        }
        let code_length_decoder = Decoder::new(&code_lengths, self.bits.pos)?;
        let mut lengths = [0u8; 286 + 30];
        let total = literal_count + distance_count;
        let mut i = 0;
        while i < total {
            let symbol = code_length_decoder.decode(&mut self.bits)?;
            let (value, repeat) = match symbol {
                0..=15 => (symbol as u8, 1),
                16 if i > 0 => (lengths[i - 1], 3 + self.bits.bits(2)? as usize),
                17 => (0, 3 + self.bits.bits(3)? as usize),
                18 => (0, 11 + self.bits.bits(7)? as usize),
                _ => return Err(self.error(ErrorKind::Block)),
            };
            if i + repeat > total {
                return Err(self.error(ErrorKind::Block));
            }
            lengths[i..i + repeat].fill(value);
            i += repeat;
        }
        if lengths[256] == 0 {
            // Without a code for end-of-block the block could never end.
            return Err(self.error(ErrorKind::Block));
        }
        let literals = Decoder::new(&lengths[..literal_count], self.bits.pos)?;
        let distances = Decoder::new(&lengths[literal_count..total], self.bits.pos)?;
        self.codes(&literals, &distances)
    }

    /// Decodes literals and matches until the end-of-block symbol.
    fn codes(&mut self, literals: &Decoder, distances: &Decoder) -> Result<(), Error> {
        loop {
            let symbol = usize::from(literals.decode(&mut self.bits)?);
            if symbol < 256 {
                self.make_room(1)?;
                self.out.push(symbol as u8);
                continue;
            }
            if symbol == 256 {
                return Ok(());
            }
            let index = symbol - 257;
            if index >= LENGTH_BASE.len() {
                return Err(self.error(ErrorKind::Code));
            }
            let extra = self.bits.bits(u32::from(LENGTH_EXTRA[index]))?;
            let length = usize::from(LENGTH_BASE[index]) + extra as usize;
            let index = usize::from(distances.decode(&mut self.bits)?);
            if index >= DIST_BASE.len() {
                return Err(self.error(ErrorKind::Code));
            }
            let extra = self.bits.bits(u32::from(DIST_EXTRA[index]))?;
            let distance = usize::from(DIST_BASE[index]) + extra as usize;
            self.copy_match(length, distance)?;
        }
    }

    /// Appends `length` bytes copied from `distance` bytes back; the source may overlap
    /// the bytes being written, which repeats them.
    fn copy_match(&mut self, length: usize, distance: usize) -> Result<(), Error> {
        if distance > self.out.len() - self.start {
            return Err(self.error(ErrorKind::Distance));
        }
        self.make_room(length)?;
        let from = self.out.len() - distance;
        if distance >= length {
            self.out.extend_from_within(from..from + length);
        } else {
            for i in 0..length {
                let byte = self.out[from + i];
                self.out.push(byte);
            }
        }
        Ok(())
    }
}
