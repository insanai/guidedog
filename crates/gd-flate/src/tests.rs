use super::*;

const LEVELS: [Level; 4] = [Level::Store, Level::Fast, Level::Default, Level::Best];

/// Pseudo-random bytes from a fixed seed, so tests are repeatable.
fn noise(len: usize, seed: u32) -> Vec<u8> {
    let mut x = seed;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect()
}

fn samples() -> Vec<Vec<u8>> {
    let text = "Guidedog builds HTML sites and PDF books from reStructuredText. ".repeat(500);
    let mut mixed = noise(70_000, 7);
    mixed.extend_from_slice(text.as_bytes());
    mixed.extend(std::iter::repeat_n(b'z', 100_000));
    vec![
        Vec::new(),
        b"a".to_vec(),
        b"abcabcabcabcabcabc".to_vec(),
        text.into_bytes(),
        noise(100_000, 1),
        vec![0; 300_000],
        mixed,
    ]
}

#[test]
fn round_trips_at_every_level() {
    for data in samples() {
        for level in LEVELS {
            let packed = zlib_compress(&data, level);
            let unpacked = zlib_decompress(&packed, data.len());
            assert_eq!(unpacked.as_deref(), Ok(&data[..]), "{level:?}, {} bytes", data.len());
            let raw = deflate(&data, level);
            assert_eq!(inflate(&raw, usize::MAX).as_deref(), Ok(&data[..]));
        }
    }
}

#[test]
fn compresses_repetitive_text() {
    let text = "objects.inv lists every target: py:function, std:label, std:doc. ".repeat(200);
    let packed = zlib_compress(text.as_bytes(), Level::Default);
    assert!(packed.len() * 20 < text.len(), "{} of {}", packed.len(), text.len());
    let random = noise(10_000, 3);
    // Incompressible data falls back to stored blocks: barely larger than the input.
    assert!(zlib_compress(&random, Level::Best).len() <= random.len() + 16);
}

#[test]
fn decodes_streams_from_zlib() {
    // `python3 -c "import zlib; print(zlib.compress(b'hello, hello, hello world\n'))"`
    let stream = b"x\x9c\xcbH\xcd\xc9\xc9\xd7Q\xc8@\xa2\x14\xca\xf3\x8brR\xb8\x00}\xae\t'";
    let text = zlib_decompress(stream, 1024);
    assert_eq!(text.as_deref(), Ok(&b"hello, hello, hello world\n"[..]));
    // A fixed-Huffman stream of "a" and a stored block of "abc".
    assert_eq!(inflate(&[0x4b, 0x04, 0x00], 10).as_deref(), Ok(&b"a"[..]));
    assert_eq!(inflate(&[1, 3, 0, 0xfc, 0xff, b'a', b'b', b'c'], 10).as_deref(), Ok(&b"abc"[..]));
}

#[test]
fn limits_output_against_bombs() {
    let zeros = vec![0u8; 1 << 20];
    let packed = zlib_compress(&zeros, Level::Default);
    assert!(packed.len() < 2048);
    let e = zlib_decompress(&packed, 1 << 16);
    assert_eq!(e.map_err(|e| e.kind), Err(ErrorKind::Limit));
    assert_eq!(zlib_decompress(&packed, 1 << 20).map(|v| v.len()), Ok(1 << 20));
}

#[test]
fn refuses_damaged_streams() {
    let packed = zlib_compress(b"some text to protect", Level::Default);
    let kind = |data: &[u8]| zlib_decompress(data, 1024).map_err(|e| e.kind);
    assert_eq!(kind(&packed[..packed.len() - 1]), Err(ErrorKind::Truncated));
    assert_eq!(kind(&packed[..4]), Err(ErrorKind::Truncated));
    let mut bad_sum = packed.clone();
    let last = bad_sum.len() - 1;
    bad_sum[last] ^= 1;
    assert_eq!(kind(&bad_sum), Err(ErrorKind::Checksum));
    assert_eq!(kind(&[0x78, 0x00]), Err(ErrorKind::Header));
    assert_eq!(kind(&[0x78, 0xbb, 0, 0, 0, 0]), Err(ErrorKind::Header));
    assert_eq!(inflate(&[0x07], 10).map_err(|e| e.kind), Err(ErrorKind::Block));
    assert_eq!(inflate(&[1, 3, 0, 0xfc, 0xfe], 10).map_err(|e| e.kind), Err(ErrorKind::Block));
    // A fixed block whose first symbol is a match reaching before the start.
    assert_eq!(inflate(&[0x03, 0x02], 10).map_err(|e| e.kind), Err(ErrorKind::Distance));
    for seed in 0..200 {
        // Random input must fail cleanly or decode within the limit, never panic.
        let _ = inflate(&noise(64, seed), 4096);
    }
}

#[test]
fn reads_consecutive_streams() {
    let mut joined = zlib_compress(b"first", Level::Fast);
    let first_len = joined.len();
    joined.extend(zlib_compress(b"second", Level::Default));
    let mut out = Vec::new();
    assert_eq!(zlib_decompress_into(&mut out, &joined, 100), Ok(first_len));
    let used = zlib_decompress_into(&mut out, &joined[first_len..], 100);
    assert_eq!(used, Ok(joined.len() - first_len));
    assert_eq!(out, b"firstsecond");
}

#[test]
fn adler32_matches_known_values() {
    assert_eq!(adler32(b""), 1);
    assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
    let big = vec![0xffu8; 100_000];
    let split = adler32_update(adler32(&big[..12_345]), &big[12_345..]);
    assert_eq!(split, adler32(&big));
}
