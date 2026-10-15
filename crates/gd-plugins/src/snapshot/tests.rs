use super::*;
use crate::testutil::TempDir;

fn entries(paths: &[&str]) -> Vec<Entry> {
    paths.iter().map(|p| Entry { path: p.to_string(), size: 0, digest: [0; 32] }).collect()
}

// A snapshot is the site's files, checked and digested; the digest names the
// generation, so equal sites have equal digests and any change changes it.
#[test]
fn snapshots_freeze_and_name_a_site() {
    let dir = TempDir::with_files("snap", &["index.html", "_static/a.css", "docs/b.html"]);
    let copy = snapshot(dir.path(), &SnapshotOptions::default()).unwrap();
    assert_eq!(copy.entries().len(), 3);
    assert_eq!(copy.entries()[0].path, "_static/a.css");
    assert_ne!(copy.root(), dir.path());
    let borrow = SnapshotOptions { mode: SnapshotMode::Borrow, ..SnapshotOptions::default() };
    let borrowed = snapshot(dir.path(), &borrow).unwrap();
    assert_eq!(borrowed.digest(), copy.digest());
    copy.verify().unwrap();
    // The copy is private: changing the source does not change it.
    std::fs::write(dir.path().join("index.html"), "changed").unwrap();
    copy.verify().unwrap();
    assert_eq!(borrowed.verify().unwrap_err().code, "plugins.snapshot-changed");
    let private = copy.root().to_path_buf();
    drop(copy);
    drop(borrowed);
    assert!(!private.exists() && dir.path().exists());
}

#[test]
fn unsafe_sites_are_refused() {
    let options = SnapshotOptions::default();
    let no_index = TempDir::with_files("snap", &["page.html"]);
    assert_eq!(snapshot(no_index.path(), &options).unwrap_err().code, "plugins.index");
    #[cfg(unix)]
    {
        let slashed = TempDir::with_files("snap", &["index.html", "a\\b.html"]);
        assert_eq!(snapshot(slashed.path(), &options).unwrap_err().code, "plugins.path");
        let linked = TempDir::with_files("snap", &["index.html"]);
        let link = linked.path().join("link.html");
        std::os::unix::fs::symlink(linked.path().join("index.html"), link).unwrap();
        assert_eq!(snapshot(linked.path(), &options).unwrap_err().code, "plugins.file-type");
    }
    let small = TempDir::with_files("snap", &["index.html"]);
    let tight = SnapshotOptions { max_bytes: 3, ..SnapshotOptions::default() };
    assert_eq!(snapshot(small.path(), &tight).unwrap_err().code, "plugins.limit");
    // Names that differ only in case cannot be made on every system; check them as names.
    for names in [&["Index.html", "index.html"][..], &["A", "a/x.html"], &["a/X", "A/x"]] {
        assert_eq!(check_names(&entries(names)).unwrap_err().code, "plugins.case", "{names:?}");
    }
    check_names(&entries(&["a/x", "a/y", "b"])).unwrap();
}

#[test]
fn a_refused_copy_leaves_no_private_folder() {
    let parent = TempDir::new("snap-parent");
    let site = TempDir::with_files("snap", &["page.html"]);
    let options = SnapshotOptions {
        temp_dir: Some(parent.path().to_path_buf()),
        ..SnapshotOptions::default()
    };
    assert!(snapshot(site.path(), &options).is_err());
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
}

// A generic publisher may accept a file larger than GitHub's default. Rechecking the
// accepted manifest keeps that choice, while still refusing a later change.
#[test]
fn accepted_files_keep_their_bounds() {
    let dir = TempDir::with_files("snap", &["index.html", "large.bin"]);
    let large = dir.path().join("large.bin");
    std::fs::write(&large, vec![7u8; 4096]).unwrap();
    let options = SnapshotOptions {
        mode: SnapshotMode::Borrow,
        max_file_bytes: 4096,
        ..SnapshotOptions::default()
    };
    let s = snapshot(dir.path(), &options).unwrap();
    assert_eq!(s.bounds().max_file_bytes, 4096);
    s.verify().unwrap();
    std::fs::write(&large, vec![7u8; 4097]).unwrap();
    assert_eq!(s.verify().unwrap_err().code, "plugins.limit");
}

// The same, beyond GitHub's 100 MiB default, as the original regression test did.
#[test]
#[ignore = "hashes a 100 MiB file; run with --ignored"]
fn accepted_large_files_keep_their_bounds() {
    let dir = TempDir::with_files("snap", &["index.html"]);
    let file = std::fs::File::create(dir.path().join("large.bin")).unwrap();
    let size = DEFAULT_MAX_FILE_BYTES + 1;
    file.set_len(size).unwrap();
    let options =
        SnapshotOptions { mode: SnapshotMode::Borrow, max_file_bytes: size, ..Default::default() };
    let s = snapshot(dir.path(), &options).unwrap();
    s.verify().unwrap();
    file.set_len(size + 1).unwrap();
    assert!(s.verify().is_err());
}

#[test]
fn manifests_are_length_prefixed() {
    let a = entries(&["ab", "c"]);
    let b = entries(&["a", "bc"]);
    assert_ne!(manifest_digest(&a), manifest_digest(&b));
    assert_eq!(digest_hex(&[0xab; 32]).len(), 64);
}
