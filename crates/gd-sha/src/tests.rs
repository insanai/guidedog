use super::*;
use std::io::Write;

#[test]
fn sha256_matches_known_vectors() {
    let cases: [(&[u8], &str); 3] = [
        (b"", "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
        (b"abc", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
        (
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        ),
    ];
    for (input, digest) in cases {
        assert_eq!(to_hex(&sha256(input)), digest);
    }
    let million = vec![b'a'; 1_000_000];
    assert_eq!(
        to_hex(&sha256(&million)),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
}

#[test]
fn sha1_matches_known_vectors() {
    assert_eq!(to_hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    assert_eq!(to_hex(&sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
    // A Git blob id: "blob <size>\0" followed by the content.
    assert_eq!(to_hex(&sha1(b"blob 6\0hello\n")), "ce013625030ba8dba906f756967f9e9ca394464a");
}

#[test]
fn streaming_matches_one_shot_at_every_split() {
    let data: Vec<u8> = (0..300u32).map(|i| (i * 7) as u8).collect();
    for split in [0, 1, 55, 56, 63, 64, 65, 128, 299, 300] {
        let mut a = Sha256::new();
        a.update(&data[..split]);
        let _ = a.write_all(&data[split..]);
        assert_eq!(a.finish(), sha256(&data), "split {split}");
        let mut b = Sha1::new();
        b.update(&data[..split]);
        b.update(&data[split..]);
        assert_eq!(b.finish(), sha1(&data), "split {split}");
    }
    for len in 50..70 {
        let mut h = Sha256::new();
        for byte in &data[..len] {
            h.update(std::slice::from_ref(byte));
        }
        assert_eq!(h.finish(), sha256(&data[..len]), "length {len}");
    }
}

#[test]
fn hex_and_fnv() {
    assert_eq!(from_hex("00ffA0"), Some(vec![0, 255, 160]));
    assert_eq!(from_hex("abc"), None);
    assert_eq!(from_hex("zz"), None);
    assert_eq!(fnv1a_64(b""), 0xcbf29ce484222325);
    assert_eq!(fnv1a_64(b"a"), 0xaf63dc4c8601ec8c);
}
