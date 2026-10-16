//! Portable archives have a canonical tree identity independent of encoding order.
#![allow(clippy::unwrap_used)]

use gd_theme::pack::{self, Files, Lock};

#[test]
fn archives_round_trip_and_hash_names_as_well_as_bytes() {
    let files = Files::from([
        ("_templates/page.html".into(), b"{{ body }}".to_vec()),
        ("_static/image.bin".into(), vec![0, 255, 13, 10]),
    ]);
    let archive = pack::encode(&files).unwrap();
    assert_eq!(pack::decode(&archive).unwrap(), files);
    let mut changed = files.clone();
    changed.insert("_static/image.bin".into(), vec![0, 255, 13, 11]);
    assert_ne!(pack::tree_digest(&files), pack::tree_digest(&changed));
    let data = changed.remove("_static/image.bin").unwrap();
    changed.insert("_static/renamed.bin".into(), data);
    assert_ne!(pack::tree_digest(&files), pack::tree_digest(&changed));
    let lock = Lock {
        source: "https://example.org/theme.gdt".into(),
        archive_sha256: pack::archive_digest(&archive),
        tree_sha256: pack::tree_digest(&files),
    };
    assert_eq!(Lock::decode(&lock.encode()).unwrap(), lock);
    assert_eq!(
        pack::archive_digest(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn archives_reject_traversal_platform_aliases_and_file_directory_overlap() {
    for name in [
        "../escape",
        "_static/../escape",
        "_static/a\\b",
        "_static/NUL.txt",
        "_static/trailing.",
        "/_static/x",
        "conf.toml",
    ] {
        let files = Files::from([(name.into(), b"data".to_vec())]);
        assert!(pack::encode(&files).is_err(), "{name}");
    }
    for names in [["_static/A.css", "_static/a.css"], ["_static/dir", "_static/dir/file"]] {
        let files = names.into_iter().map(|s| (s.into(), Vec::new())).collect();
        assert!(pack::encode(&files).is_err());
    }
    for archive in [
        br#"{"format":2,"files":{}}"#.as_slice(),
        br#"{"format":1,"files":{"_static/x":"!bad!"}}"#,
    ] {
        assert!(pack::decode(archive).is_err());
    }
    assert!(
        Lock::decode(
            r#"{"format":1,"source":"x","archive_sha256":"../bad","tree_sha256":"../bad"}"#
        )
        .is_err()
    );
}
