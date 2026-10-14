//! Canonical Huffman codes: decoding tables and length-limited code construction.

use crate::bits::BitReader;
use crate::{Error, ErrorKind};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// The longest code DEFLATE allows.
pub(crate) const MAX_BITS: usize = 15;

/// A canonical Huffman decoder: how many codes have each length, and the symbols in
/// code order. Decoding walks one bit at a time, as zlib's `puff` reference does.
pub(crate) struct Decoder {
    count: [u16; MAX_BITS + 1],
    symbol: Vec<u16>,
}

impl Decoder {
    /// Builds a decoder from code lengths per symbol (0 for unused). An over-subscribed
    /// set of lengths is an error; an incomplete one is accepted, and its unused codes
    /// fail when met.
    pub(crate) fn new(lengths: &[u8], offset: usize) -> Result<Decoder, Error> {
        let mut count = [0u16; MAX_BITS + 1];
        for &len in lengths {
            count[usize::from(len)] += 1;
        }
        let mut left: i32 = 1;
        for &n in &count[1..] {
            left = left * 2 - i32::from(n);
            if left < 0 {
                return Err(Error { kind: ErrorKind::Block, offset });
            }
        }
        let mut start = [0u16; MAX_BITS + 2];
        for len in 1..=MAX_BITS {
            start[len + 1] = start[len] + count[len];
        }
        let mut symbol = vec![0u16; usize::from(start[MAX_BITS + 1])];
        for (sym, &len) in lengths.iter().enumerate() {
            if len > 0 {
                let slot = &mut start[usize::from(len)];
                symbol[usize::from(*slot)] = sym as u16;
                *slot += 1;
            }
        }
        Ok(Decoder { count, symbol })
    }

    /// Reads one symbol from `bits`.
    pub(crate) fn decode(&self, bits: &mut BitReader<'_>) -> Result<u16, Error> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for &count in &self.count[1..] {
            code |= bits.bits(1)? as i32;
            let count = i32::from(count);
            if code - count < first {
                return Ok(self.symbol[(index + code - first) as usize]);
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err(Error { kind: ErrorKind::Code, offset: bits.pos })
    }
}

/// Code lengths (at most `max_len` bits) for symbols with the given frequencies, forming
/// a complete prefix code. Unused symbols get 0; when fewer than two symbols are used,
/// symbols 0 and 1 are given codes so the code stays complete, which some decoders need.
pub(crate) fn lengths(freqs: &[u32], max_len: u8) -> Vec<u8> {
    let mut used: Vec<usize> = (0..freqs.len()).filter(|&s| freqs[s] > 0).collect();
    for filler in [0, 1] {
        if used.len() < 2 && !used.contains(&filler) {
            used.push(filler);
        }
    }
    let weight = |s: usize| u64::from(freqs[s].max(1));
    let depths = tree_depths(&used, weight);
    let mut lengths = vec![0u8; freqs.len()];
    for (&s, &d) in used.iter().zip(&depths) {
        lengths[s] = d.min(u32::from(max_len)) as u8;
    }
    if depths.iter().any(|&d| d > u32::from(max_len)) {
        make_complete(&mut lengths, &used, freqs, max_len);
    }
    lengths
}

/// The depth of each leaf in a Huffman tree over `leaves`.
fn tree_depths(leaves: &[usize], weight: impl Fn(usize) -> u64) -> Vec<u32> {
    // Nodes are numbered leaves first, then internal nodes in creation order, so every
    // parent has a larger number than its children.
    let mut parent: Vec<usize> = vec![usize::MAX; leaves.len()];
    let mut heap: BinaryHeap<Reverse<(u64, usize)>> =
        leaves.iter().enumerate().map(|(i, &s)| Reverse((weight(s), i))).collect();
    while let (Some(Reverse((wa, a))), Some(Reverse((wb, b)))) = (heap.pop(), heap.pop()) {
        let node = parent.len();
        parent.push(usize::MAX);
        parent[a] = node;
        parent[b] = node;
        heap.push(Reverse((wa + wb, node)));
    }
    let mut depth = vec![0u32; parent.len()];
    for node in (0..parent.len()).rev() {
        if parent[node] != usize::MAX {
            depth[node] = depth[parent[node]] + 1;
        }
    }
    depth.truncate(leaves.len());
    depth
}

/// Adjusts clamped lengths until their Kraft sum is exactly one: first lengthening rare
/// symbols while the code is over-full, then shortening frequent ones while it has room.
fn make_complete(lengths: &mut [u8], used: &[usize], freqs: &[u32], max_len: u8) {
    let unit = |len: u8| 1u32 << (max_len - len);
    let full = 1u32 << max_len;
    let mut kraft: u32 = used.iter().map(|&s| unit(lengths[s])).sum();
    let mut by_rarity = used.to_vec();
    by_rarity.sort_by_key(|&s| (freqs[s], s));
    while kraft > full {
        // Lengthen the longest code that can still grow: it costs the least.
        let Some(&s) =
            by_rarity.iter().filter(|&&s| lengths[s] < max_len).max_by_key(|&&s| lengths[s])
        else {
            return;
        };
        lengths[s] += 1;
        kraft -= unit(lengths[s]);
    }
    while kraft < full {
        // Shorten the most frequent of the longest codes that fit in the room left.
        let room = full - kraft;
        let pick = by_rarity
            .iter()
            .rev()
            .filter(|&&s| lengths[s] > 1 && unit(lengths[s]) <= room)
            .max_by_key(|&&s| lengths[s]);
        let Some(&s) = pick else { return };
        kraft += unit(lengths[s]);
        lengths[s] -= 1;
    }
}

/// The canonical codes for `lengths`, bit-reversed so they can be written LSB first.
pub(crate) fn codes(lengths: &[u8]) -> Vec<u16> {
    let mut count = [0u16; MAX_BITS + 1];
    for &len in lengths {
        count[usize::from(len)] += 1;
    }
    count[0] = 0;
    let mut next = [0u16; MAX_BITS + 1];
    let mut code = 0u16;
    for len in 1..=MAX_BITS {
        code = (code + count[len - 1]) << 1;
        next[len] = code;
    }
    lengths
        .iter()
        .map(|&len| {
            if len == 0 {
                return 0;
            }
            let c = next[usize::from(len)];
            next[usize::from(len)] += 1;
            c.reverse_bits() >> (16 - u16::from(len))
        })
        .collect()
}
