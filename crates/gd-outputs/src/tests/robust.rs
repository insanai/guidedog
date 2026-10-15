//! Robustness, for the records this crate reads: a manifest or a journal, mutated, is
//! read or refused, never a crash. (gd-conf and gd-migrate test conf.toml and conf.py.)

use std::collections::BTreeMap;
use std::path::PathBuf;

use gd_env::{Asset, Digest, Lookup, LookupKind};
use gd_report::Report;

use crate::journal::{Journal, PendingFile};
use crate::{Manifest, OutputRecord};

/// A small deterministic generator (xorshift), so a failure repeats.
struct Random(u64);

impl Random {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n.max(1) as u64) as usize
    }
}

fn mutate(seed: &str, random: &mut Random, alphabet: &[u8]) -> Vec<u8> {
    let mut buf = seed.as_bytes().to_vec();
    for _ in 0..1 + random.below(6) {
        match random.below(3) {
            0 if !buf.is_empty() => {
                let at = random.below(buf.len());
                buf[at] = alphabet[random.below(alphabet.len())];
            }
            1 if buf.len() > 1 => {
                let at = random.below(buf.len());
                let k = (random.below(8) + 1).min(buf.len() - at);
                buf.drain(at..at + k);
            }
            _ => {
                let at = random.below(buf.len() + 1);
                buf.insert(at, alphabet[random.below(alphabet.len())]);
            }
        }
    }
    buf
}

fn sample_manifest() -> Manifest {
    let record = OutputRecord {
        inputs: Digest::of_text("inputs"),
        files: vec!["guide/intro.html".into(), "_images/a.png".into()],
        assets: vec![Asset {
            source: PathBuf::from("/src/a.png"),
            target: "_images/a.png".into(),
        }],
        lookups: vec![
            (Lookup::new(LookupKind::Label, "intro"), Digest::of_text("a")),
            (Lookup::suffix("py:", ".open", "py:func"), Digest::of_text("b")),
        ],
        reports: vec![Report::error("ref.x", "A REPORT", "message").hint("a hint")],
    };
    let mut units = BTreeMap::new();
    units.insert("guide/intro".to_string(), record);
    units.insert(String::new(), OutputRecord::default());
    Manifest { units }
}

fn sample_journal() -> Journal {
    Journal {
        out_dir: "/p/_build/html".into(),
        next: Some("/p/_build/.html.next".into()),
        old: "/p/_build/.html.old".into(),
        next_id: Some(gd_host::Identity { device: 7, inode: 42 }),
        records: vec![PendingFile {
            from: "/p/_build/.doctrees/html.outputs.json.next".into(),
            to: "/p/_build/.doctrees/html.outputs.json".into(),
            previous: None,
        }],
    }
}

#[test]
fn records_round_trip() {
    let manifest = sample_manifest();
    assert_eq!(Manifest::from_json(&manifest.to_json()), Some(manifest));
    let journal = sample_journal();
    assert_eq!(Journal::from_json(&journal.to_json().unwrap()), Some(journal));
    let older = "{\"version\": 1, \"units\": {}}";
    assert_eq!(Manifest::from_json(older), None);
}

#[test]
fn mutated_records_never_crash() {
    let mut random = Random(20_260_930);
    let alphabet = b"\"'\\\n[]{}:,.-0123456789 aeTZx_\xff\xc3null";
    let manifest = sample_manifest().to_json();
    let journal = sample_journal().to_json().unwrap();
    for _ in 0..2000 {
        let text = mutate(&manifest, &mut random, alphabet);
        if let Ok(text) = std::str::from_utf8(&text) {
            let _ = Manifest::from_json(text);
        }
        let text = mutate(&journal, &mut random, alphabet);
        if let Ok(text) = std::str::from_utf8(&text) {
            let _ = Journal::from_json(text);
        }
    }
}
