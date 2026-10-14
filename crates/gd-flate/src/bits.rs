//! Reading and writing DEFLATE's least-significant-bit-first bit stream.

use crate::{Error, ErrorKind};

/// Reads bits from a byte slice, loading one byte at a time so that after
/// [`BitReader::align`] no input byte is held in the buffer.
pub(crate) struct BitReader<'a> {
    data: &'a [u8],
    /// The next byte to load.
    pub(crate) pos: usize,
    buffer: u32,
    count: u32,
}

impl<'a> BitReader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> BitReader<'a> {
        BitReader { data, pos: 0, buffer: 0, count: 0 }
    }

    pub(crate) fn truncated(&self) -> Error {
        Error { kind: ErrorKind::Truncated, offset: self.data.len() }
    }

    /// Reads `n` bits (at most 16), first bit lowest.
    pub(crate) fn bits(&mut self, n: u32) -> Result<u32, Error> {
        while self.count < n {
            let byte = *self.data.get(self.pos).ok_or_else(|| self.truncated())?;
            self.buffer |= u32::from(byte) << self.count;
            self.pos += 1;
            self.count += 8;
        }
        let value = self.buffer & ((1u32 << n) - 1);
        self.buffer >>= n;
        self.count -= n;
        Ok(value)
    }

    /// Drops the bits left in the current byte.
    pub(crate) fn align(&mut self) {
        self.buffer = 0;
        self.count = 0;
    }

    /// Borrows the next `n` whole bytes after [`BitReader::align`].
    pub(crate) fn take_bytes(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let bytes = self.data.get(self.pos..self.pos + n).ok_or_else(|| self.truncated())?;
        self.pos += n;
        Ok(bytes)
    }
}

/// Appends bits to a byte vector, first bit lowest.
pub(crate) struct BitWriter<'a> {
    pub(crate) out: &'a mut Vec<u8>,
    buffer: u64,
    count: u32,
}

impl<'a> BitWriter<'a> {
    pub(crate) fn new(out: &'a mut Vec<u8>) -> BitWriter<'a> {
        BitWriter { out, buffer: 0, count: 0 }
    }

    /// Writes the low `n` bits of `value` (n at most 32).
    pub(crate) fn bits(&mut self, value: u32, n: u32) {
        self.buffer |= u64::from(value) << self.count;
        self.count += n;
        while self.count >= 8 {
            self.out.push(self.buffer as u8);
            self.buffer >>= 8;
            self.count -= 8;
        }
    }

    /// Pads with zero bits to the next byte boundary.
    pub(crate) fn align(&mut self) {
        if self.count > 0 {
            self.out.push(self.buffer as u8);
            self.buffer = 0;
            self.count = 0;
        }
    }
}
