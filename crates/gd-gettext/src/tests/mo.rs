use crate::mo::{MoReader, hashpjw};
use crate::{Catalog, ErrorKind, WriteOptions, parse_mo, parse_po, write_mo, write_po};

const RU: &str = r#"msgid ""
msgstr ""
"Content-Type: text/plain; charset=UTF-8\n"
"Plural-Forms: nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && "
"n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);\n"

msgid "Next"
msgstr "Далее"

msgctxt "verb"
msgid "Open"
msgstr "Открыть"

msgid "%d file"
msgid_plural "%d files"
msgstr[0] "%d файл"
msgstr[1] "%d файла"
msgstr[2] "%d файлов"

#, fuzzy
msgid "Previous"
msgstr "Назад"

msgid "Untranslated"
msgstr ""
"#;

fn russian() -> Catalog {
    parse_po(RU, "ru.po", false).expect("RU parses")
}

#[test]
fn mo_round_trips() {
    let bytes = write_mo(&russian(), false).expect("RU compiles");
    let mo = parse_mo(&bytes, "ru.mo").expect("the MO file reads");
    assert_eq!(mo.messages.len(), 3); // fuzzy and untranslated are left out.
    assert_eq!(mo.plural.nplurals, 3);
    assert_eq!(mo.text("Next"), "Далее");
    assert_eq!(mo.with_context("verb", "Open"), "Открыть");
    assert_eq!(mo.text("Open"), "Open");
    assert_eq!(mo.plural("%d file", "%d files", 21), "%d файл");
    assert_eq!(mo.plural("%d file", "%d files", 3), "%d файла");
    assert_eq!(mo.plural("%d file", "%d files", 11), "%d файлов");
    assert_eq!(mo.text("Previous"), "Previous");
    let back = write_po(&mo, &WriteOptions::default());
    let again = parse_po(&back, "back.po", false).expect("the written PO parses");
    assert_eq!(write_mo(&again, false), Ok(bytes));
}

#[test]
fn mo_reads_big_endian() {
    let little = write_mo(&russian(), false).expect("RU compiles");
    let mut big = little.clone();
    // Swap the header, the two tables, and the hash table: everything before the strings.
    let count = usize::from(little[8]);
    let hash_size = usize::from(little[20]) | usize::from(little[21]) << 8;
    for w in 0..7 + 4 * count + hash_size {
        big[4 * w..4 * w + 4].reverse();
    }
    let mo = parse_mo(&big, "big.mo").expect("the big-endian file reads");
    assert_eq!(mo.text("Next"), "Далее");
}

#[test]
fn mo_includes_fuzzy_on_request() {
    let bytes = write_mo(&russian(), true).expect("RU compiles");
    let mo = parse_mo(&bytes, "ru.mo").expect("the MO file reads");
    assert_eq!(mo.text("Previous"), "Назад");
}

#[test]
fn mo_hash_table_finds_every_key() {
    let bytes = write_mo(&russian(), false).expect("RU compiles");
    let r = MoReader::new(&bytes);
    let word = |at| r.word(at).expect("a header word") as usize;
    let (count, originals, size, at) = (word(8), word(12), word(20), word(24));
    for i in 0..count {
        let key = r.string(originals, i).expect("a key");
        let h = hashpjw(key) as usize;
        let (mut slot, step) = (h % size, 1 + h % (size - 2));
        let mut found = false;
        for _ in 0..size {
            match word(at + 4 * slot) {
                0 => break,
                v if v == i + 1 => {
                    found = true;
                    break;
                }
                _ => slot = (slot + step) % size,
            }
        }
        assert!(found, "key {key:?} is not in the hash table");
    }
}

#[test]
fn mo_damage_is_an_error() {
    let bytes = write_mo(&russian(), false).expect("RU compiles");
    for cut in [0, 3, 12, 27, 40, bytes.len() - 1] {
        let err = parse_mo(&bytes[..cut], "cut.mo").expect_err("a cut file fails");
        assert_eq!(err.kind, ErrorKind::MoFormat, "cut at {cut}");
        assert!(err.to_string().contains("Hint: "));
    }
    let mut bad = bytes.clone();
    bad[32..35].fill(0xff); // the second original string's offset, low bytes.
    let err = parse_mo(&bad, "bad.mo").expect_err("a bad offset fails");
    assert_eq!(err.kind, ErrorKind::MoFormat);
}
