//! SHA-1 (FIPS 180-4), for Git object ids only: it is broken for security uses.

use crate::Blocks;

const INITIAL: [u32; 5] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0];

/// A streaming SHA-1 hasher: feed it with [`Sha1::update`] (or `std::io::Write`) and read
/// the 20-byte digest with [`Sha1::finish`]. Use it where a format demands SHA-1, such as
/// Git object ids; it is not collision-resistant.
#[derive(Debug, Clone)]
pub struct Sha1 {
    state: [u32; 5],
    blocks: Blocks,
}

impl Default for Sha1 {
    fn default() -> Sha1 {
        Sha1::new()
    }
}

impl Sha1 {
    /// A hasher that has seen no bytes.
    pub fn new() -> Sha1 {
        Sha1 { state: INITIAL, blocks: Blocks::new() }
    }

    /// Feeds `data`; it may be split across calls in any way.
    pub fn update(&mut self, data: &[u8]) {
        let state = &mut self.state;
        self.blocks.update(data, |block| compress(state, block));
    }

    /// Finishes the hash and returns the digest, consuming the hasher.
    pub fn finish(mut self) -> [u8; 20] {
        let state = &mut self.state;
        self.blocks.finish(|block| compress(state, block));
        let mut out = [0u8; 20];
        for (chunk, word) in out.as_chunks_mut::<4>().0.iter_mut().zip(self.state) {
            *chunk = word.to_be_bytes();
        }
        out
    }
}

impl std::io::Write for Sha1 {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.update(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The SHA-1 digest of `data`.
pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut hasher = Sha1::new();
    hasher.update(data);
    hasher.finish()
}

fn compress(state: &mut [u32; 5], block: &[u8; 64]) {
    let mut w = [0u32; 80];
    for (i, word) in block.as_chunks::<4>().0.iter().enumerate() {
        w[i] = u32::from_be_bytes(*word);
    }
    for i in 16..80 {
        w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
    }
    let [mut a, mut b, mut c, mut d, mut e] = *state;
    for (i, &word) in w.iter().enumerate() {
        let (f, k) = match i {
            0..20 => ((b & c) | (!b & d), 0x5a827999),
            20..40 => (b ^ c ^ d, 0x6ed9eba1),
            40..60 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdc),
            _ => (b ^ c ^ d, 0xca62c1d6),
        };
        let t =
            a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(word);
        (e, d, c, b, a) = (d, c, b.rotate_left(30), a, t);
    }
    for (s, v) in state.iter_mut().zip([a, b, c, d, e]) {
        *s = s.wrapping_add(v);
    }
}
