//! Standard, padded Base64 (RFC 4648): how JSON carries bytes, such as the asset files
//! of the plugin protocol, and how data URLs and HTTP Basic credentials hold them.
//!
//! ```
//! assert_eq!(gd_json::base64::encode(b"foo"), "Zm9v");
//! assert_eq!(gd_json::base64::decode("Zm8=").as_deref(), Some(&b"fo"[..]));
//! ```

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// The Base64 form of `data`, padded with `=`; [`encode_into`] appends to a buffer the
/// caller reuses instead.
pub fn encode(data: &[u8]) -> String {
    let mut out = String::new();
    encode_into(&mut out, data);
    out
}

/// Appends the Base64 form of `data`, padded with `=`, to `out`.
pub fn encode_into(out: &mut String, data: &[u8]) {
    out.reserve(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[(n >> (18 - 6 * i)) as usize & 63]));
            } else {
                out.push('=');
            }
        }
    }
}

/// The number of bytes `text` decodes to, without checking it, so a caller can refuse
/// data that is too large before decoding it.
pub fn decoded_len(text: &str) -> usize {
    let padding = text.bytes().rev().take_while(|&c| c == b'=').count().min(2);
    (text.len() / 4 * 3).saturating_sub(padding)
}

/// Decodes padded Base64 into the bytes it holds; `None` when `text` is not padded
/// Base64 (a length that is no multiple of 4, padding inside, another character), or
/// when the bytes cannot be allocated.
pub fn decode(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::new();
    out.try_reserve(decoded_len(text)).ok()?;
    let groups = bytes.len() / 4;
    for (index, group) in bytes.chunks(4).enumerate() {
        let last = index + 1 == groups;
        decode_group(group, last, &mut out)?;
    }
    Some(out)
}

fn decode_group(group: &[u8], last: bool, out: &mut Vec<u8>) -> Option<()> {
    let padding = group.iter().rev().take_while(|&&c| c == b'=').count();
    if padding > 2 || (padding > 0 && !last) {
        return None;
    }
    let mut n = 0u32;
    for &c in &group[..4 - padding] {
        n = (n << 6) | u32::from(value(c)?);
    }
    n <<= 6 * padding as u32;
    let decoded = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
    out.extend_from_slice(&decoded[..3 - padding]);
    Some(())
}

fn value(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_length() {
        let cases = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (plain, coded) in cases {
            let mut out = String::new();
            encode_into(&mut out, plain.as_bytes());
            assert_eq!(out, coded);
            assert_eq!(encode(plain.as_bytes()), coded);
            assert_eq!(decode(coded).as_deref(), Some(plain.as_bytes()));
            assert_eq!(decoded_len(coded), plain.len());
        }
        for bad in ["Zg", "Z===", "Zg==Zg==", "Zm9!"] {
            assert_eq!(decode(bad), None, "{bad}");
        }
    }
}
