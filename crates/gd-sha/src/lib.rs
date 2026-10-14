//! Hashes Guidedog needs: SHA-256 (content digests), SHA-1 (Git object ids) and FNV-1a
//! (cheap non-cryptographic keys).
//!
//! Each hash has a one-call form ([`sha256`], [`sha1`], [`fnv1a_64`]) and a streaming
//! hasher ([`Sha256`], [`Sha1`]) that also implements [`std::io::Write`], so a large file
//! can be hashed with `std::io::copy` without loading it whole.
//!
//! ```
//! let digest = gd_sha::sha256(b"abc");
//! assert!(gd_sha::to_hex(&digest).starts_with("ba7816bf"));
//! ```

mod sha1;
mod sha256;

pub use sha1::{Sha1, sha1};
pub use sha256::{Sha256, sha256};

/// Writes `bytes` as lower-case hexadecimal into a new string.
pub fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    write_hex(&mut out, bytes);
    out
}

/// Appends `bytes` as lower-case hexadecimal to `out`.
pub fn write_hex(out: &mut String, bytes: &[u8]) {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    for &b in bytes {
        out.push(DIGITS[usize::from(b >> 4)] as char);
        out.push(DIGITS[usize::from(b & 0xf)] as char);
    }
}

/// Reads hexadecimal `text` (either case) into bytes; `None` when its length is odd or it
/// holds a character that is not a hex digit.
pub fn from_hex(text: &str) -> Option<Vec<u8>> {
    let digits = text.as_bytes();
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    let value = |d: u8| (d as char).to_digit(16);
    let mut out = Vec::with_capacity(digits.len() / 2);
    for pair in digits.as_chunks::<2>().0 {
        let byte = value(pair[0])? * 16 + value(pair[1])?;
        out.push(byte as u8);
    }
    Some(out)
}

/// The 64-bit FNV-1a hash of `bytes`: fast and stable across runs and platforms, for
/// cache keys and change detection, never for security.
pub fn fnv1a_64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    bytes.iter().fold(OFFSET, |hash, &b| (hash ^ u64::from(b)).wrapping_mul(PRIME))
}

/// Feeds `data` through a block function in 64-byte blocks, keeping a partial block in
/// `buffer`. Shared by the SHA-1 and SHA-256 hashers, which use the same padding.
#[derive(Debug, Clone)]
pub(crate) struct Blocks {
    buffer: [u8; 64],
    filled: usize,
    /// Total bytes fed, for the length in the final padding.
    length: u64,
}

impl Blocks {
    pub(crate) fn new() -> Blocks {
        Blocks { buffer: [0; 64], filled: 0, length: 0 }
    }

    pub(crate) fn update(&mut self, mut data: &[u8], mut block: impl FnMut(&[u8; 64])) {
        self.length = self.length.wrapping_add(data.len() as u64);
        if self.filled > 0 {
            let take = (64 - self.filled).min(data.len());
            self.buffer[self.filled..self.filled + take].copy_from_slice(&data[..take]);
            self.filled += take;
            data = &data[take..];
            if self.filled < 64 {
                return;
            }
            block(&self.buffer);
            self.filled = 0;
        }
        let (full_blocks, rest) = data.as_chunks::<64>();
        for full in full_blocks {
            block(full);
        }
        self.buffer[..rest.len()].copy_from_slice(rest);
        self.filled = rest.len();
    }

    /// Pads with 0x80, zeros and the big-endian bit length, running the last blocks.
    pub(crate) fn finish(&mut self, mut block: impl FnMut(&[u8; 64])) {
        let bits = self.length.wrapping_mul(8);
        let mut pad = [0u8; 72];
        pad[0] = 0x80;
        let zeros = (119 - self.filled) % 64;
        pad[1 + zeros..9 + zeros].copy_from_slice(&bits.to_be_bytes());
        let length = self.length;
        self.update(&pad[..9 + zeros], &mut block);
        self.length = length;
    }
}

#[cfg(test)]
mod tests;
