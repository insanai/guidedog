//! Mutated catalogs must give a catalog or an error, never a panic.

use super::Rng;
use crate::Lookup;
use crate::{
    MoCatalog, WriteOptions, compile_plural, parse_mo, parse_po, parse_po_bytes, plural_index,
};
use crate::{write_mo, write_po};

const SEED: &str = r#"msgid ""
msgstr ""
"Content-Type: text/plain; charset=UTF-8\n"
"Plural-Forms: nplurals=3; plural=(n==1 ? 0 : n%10>=2 && n%10<=4 ? 1 : 2);\n"

# comment
#. note
#: a.rst:1
#, fuzzy, c-format
#| msgid "old"
msgctxt "c"
msgid "a\n"
"b"
msgid_plural "as"
msgstr[0] "x\t\"\\"
msgstr[1] "y"
msgstr[2] "z"

#~ msgid "gone"
#~ msgstr "weg"
"#;

/// Replaces, deletes, or inserts a few random bytes.
fn mutate(rng: &mut Rng, buf: &mut Vec<u8>) {
    let alphabet = b"\"\\\n#~|:.,[]0123456789 msgidstrctxplural_\xff\xc3";
    for _ in 0..1 + rng.below(6) {
        let at = rng.below(buf.len());
        match rng.below(3) {
            0 => buf[at] = *rng.choice(alphabet),
            1 => {
                buf.remove(at);
            }
            _ => buf.insert(at, *rng.choice(alphabet)),
        }
    }
}

/// Whatever parses must survive a write and a reparse, and compile to MO and back.
#[test]
fn fuzz_malformed_po() {
    let mut rng = Rng::new(20260929);
    let mut buf = Vec::with_capacity(SEED.len() + 64);
    let mut parsed = 0;
    for _ in 0..3000 {
        buf.clear();
        buf.extend_from_slice(SEED.as_bytes());
        mutate(&mut rng, &mut buf);
        let cat = match parse_po_bytes(&buf, "fuzz.po") {
            Ok(cat) => cat,
            Err(err) => {
                assert!(!err.to_string().is_empty());
                continue;
            }
        };
        parsed += 1;
        let out = write_po(&cat, &WriteOptions::default());
        if let Err(err) = parse_po(&out, "again.po") {
            panic!("rewritten catalog fails: {err}\n{out}");
        }
        let mo = write_mo(&cat, false).expect("a parsed catalog compiles");
        let back = parse_mo(&mo, "fuzz.mo").expect("a compiled catalog reads");
        for m in &cat.messages {
            let _ = back.text(&m.id);
        }
    }
    assert!(parsed > 100, "only {parsed} parsed");
}

#[test]
fn fuzz_random_mo_bytes() {
    let mut rng = Rng::new(7);
    let cat = parse_po(SEED, "seed.po").expect("SEED parses");
    let good = write_mo(&cat, false).expect("SEED compiles");
    let mut buf = good.clone();
    for _ in 0..3000 {
        buf.copy_from_slice(&good);
        for _ in 0..1 + rng.below(4) {
            let at = rng.below(buf.len());
            buf[at] = rng.below(256) as u8;
        }
        let n = if rng.below(4) > 0 { buf.len() } else { rng.below(buf.len()) };
        if let Ok(mo) = parse_mo(&buf[..n], "fuzz.mo") {
            let _ = mo.plural("a\nb", "as", 3);
        }
        // The zero-copy reader accepts exactly what the copying one does, and then
        // answers every lookup without panicking.
        let zero_copy = MoCatalog::from_bytes(buf[..n].to_vec(), "fuzz.mo");
        assert_eq!(zero_copy.is_ok(), parse_mo(&buf[..n], "fuzz.mo").is_ok());
        if let Ok(mo) = zero_copy {
            for e in mo.entries() {
                assert!(mo.find(e.id, e.ctx).is_some(), "{e:?} is not found");
            }
            let _ = mo.with_context("c", "a\nb");
        }
    }
}

#[test]
fn fuzz_plural_expressions() {
    let mut rng = Rng::new(99);
    let tokens = [
        "n", "1", "0", "10", "(", ")", "?", ":", "&&", "||", "==", "!=", "<", ">", "<=", ">=",
        "+", "-", "*", "/", "%", "!", " ",
    ];
    let mut expr = String::with_capacity(256);
    for _ in 0..5000 {
        expr.clear();
        for _ in 0..rng.below(24) {
            expr.push_str(rng.choice(&tokens));
        }
        let Ok(rule) = compile_plural(&expr, 4) else { continue };
        for n in 0..30 {
            assert!(plural_index(&rule, n) < 4);
        }
    }
}
