//! DEFLATE (RFC 1951) and zlib (RFC 1950) compression and decompression.
//!
//! [`inflate`] reads raw DEFLATE streams with stored, fixed and dynamic Huffman blocks;
//! [`zlib_decompress`] adds the zlib header and Adler-32 check. Both stop with
//! [`ErrorKind::Limit`] before producing more than the caller's `limit`, so a small
//! hostile input (a "zip bomb") cannot exhaust memory.
//!
//! [`deflate`] and [`zlib_compress`] compress with LZ77 over a 32 KiB window and choose,
//! per block, whichever of stored, fixed or dynamic Huffman coding is smallest.
//!
//! The `*_into` forms append to a caller's `Vec<u8>` so buffers can be reused.
//!
//! ```
//! let packed = gd_flate::zlib_compress(b"hello hello hello", gd_flate::Level::Default);
//! let unpacked = gd_flate::zlib_decompress(&packed, 1 << 20).unwrap();
//! assert_eq!(unpacked, b"hello hello hello");
//! ```

#![forbid(unsafe_code)]

mod bits;
mod deflate;
mod huffman;
mod inflate;

use std::fmt;

pub use deflate::{deflate, deflate_into};
pub use inflate::{inflate, inflate_into};

/// How hard [`deflate`] looks for repeated text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Level {
    /// No compression: stored blocks only.
    Store,
    /// Short match searches; fast, compresses less.
    Fast,
    /// A balance of speed and size, like zlib's level 6.
    #[default]
    Default,
    /// Long match searches; slowest, smallest.
    Best,
}

/// What is wrong with compressed data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The data ends in the middle of a stream.
    Truncated,
    /// The zlib header is not one for DEFLATE data, or asks for a preset dictionary.
    Header,
    /// A block has a reserved type, a stored length that does not match its complement,
    /// or a Huffman table that is not a valid code.
    Block,
    /// A Huffman code, length or distance symbol is not defined.
    Code,
    /// A match refers back further than the data produced so far.
    Distance,
    /// The Adler-32 checksum does not match the data.
    Checksum,
    /// The output would exceed the caller's limit.
    Limit,
}

/// Why compressed data could not be read, with the input offset where it was noticed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error {
    /// What is wrong.
    pub kind: ErrorKind,
    /// Byte offset in the compressed input.
    pub offset: usize,
}

impl Error {
    /// A sentence saying what is wrong.
    pub fn message(&self) -> &'static str {
        match self.kind {
            ErrorKind::Truncated => "The compressed data ends too early.",
            ErrorKind::Header => "The data does not start with a zlib header for DEFLATE.",
            ErrorKind::Block => "A compressed block is malformed.",
            ErrorKind::Code => "The compressed data uses an undefined code.",
            ErrorKind::Distance => "The compressed data refers back before its start.",
            ErrorKind::Checksum => "The decompressed data fails its Adler-32 checksum.",
            ErrorKind::Limit => "The decompressed data is larger than allowed.",
        }
    }

    /// A sentence saying what to do about it.
    pub fn hint(&self) -> &'static str {
        match self.kind {
            ErrorKind::Limit => {
                "The file may be a decompression bomb; if it is trusted, raise the size limit."
            }
            _ => "The file is damaged or not in this format; download or generate it again.",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (at byte {})", self.message(), self.offset)
    }
}

impl std::error::Error for Error {}

/// Continues an Adler-32 checksum: pass 1 to start, and the previous result to go on.
pub fn adler32_update(adler: u32, data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    // 5552 is the most bytes that can be summed before b could overflow a u32.
    let (mut a, mut b) = (adler & 0xffff, adler >> 16);
    for chunk in data.chunks(5552) {
        for &byte in chunk {
            a += u32::from(byte);
            b += a;
        }
        a %= MOD;
        b %= MOD;
    }
    (b << 16) | a
}

/// The Adler-32 checksum of `data`, as zlib streams end with.
pub fn adler32(data: &[u8]) -> u32 {
    adler32_update(1, data)
}

/// Compresses `data` as a zlib stream: header, DEFLATE data and Adler-32 checksum.
pub fn zlib_compress(data: &[u8], level: Level) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() / 2 + 16);
    zlib_compress_into(&mut out, data, level);
    out
}

/// Appends `data` compressed as a zlib stream to `out`.
pub fn zlib_compress_into(out: &mut Vec<u8>, data: &[u8], level: Level) {
    // CMF 0x78: DEFLATE with a 32 KiB window. FLG is chosen so the pair divides by 31;
    // its level bits are informational only.
    let flag = match level {
        Level::Store | Level::Fast => 0x01,
        Level::Default => 0x9c,
        Level::Best => 0xda,
    };
    out.extend_from_slice(&[0x78, flag]);
    deflate_into(out, data, level);
    out.extend_from_slice(&adler32(data).to_be_bytes());
}

/// Decompresses the zlib stream at the start of `data`, refusing output beyond `limit`
/// bytes. Bytes after the stream are ignored. Fails on a bad header, malformed data, a
/// checksum mismatch or the limit.
pub fn zlib_decompress(data: &[u8], limit: usize) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    zlib_decompress_into(&mut out, data, limit)?;
    Ok(out)
}

/// Appends the decompressed zlib stream at the start of `data` to `out`, producing at
/// most `limit` bytes, and returns how many input bytes the stream used, so that
/// consecutive streams (as in a Git pack) can be read one after another.
pub fn zlib_decompress_into(out: &mut Vec<u8>, data: &[u8], limit: usize) -> Result<usize, Error> {
    let header = Error { kind: ErrorKind::Header, offset: 0 };
    let [cmf, flg, ..] = *data else {
        return Err(Error { kind: ErrorKind::Truncated, offset: data.len() });
    };
    let valid =
        cmf & 0x0f == 8 && cmf >> 4 <= 7 && (u16::from(cmf) << 8 | u16::from(flg)) % 31 == 0;
    if !valid || flg & 0x20 != 0 {
        return Err(header);
    }
    let start = out.len();
    let used = inflate_into(out, &data[2..], limit)?;
    let end = 2 + used;
    let Some(sum) = data.get(end..end + 4) else {
        return Err(Error { kind: ErrorKind::Truncated, offset: data.len() });
    };
    let expected = u32::from_be_bytes([sum[0], sum[1], sum[2], sum[3]]);
    if adler32(&out[start..]) != expected {
        return Err(Error { kind: ErrorKind::Checksum, offset: end });
    }
    Ok(end + 4)
}

#[cfg(test)]
mod tests;
