//! The previous release the plugin protocol is qualified against: its immutable assets on
//! GitHub, each pinned by SHA-256, downloaded with gd-fetch and checked with gd-sha.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::fsutil::{rename, sha256_file, write};
use crate::{Result, fail};

/// The release whose host and SDK the protocol must keep working with.
pub(super) const OLD_TAG: &str = "v0.6.0";

/// Where that release's assets are downloaded from.
const BASE: &str = "https://github.com/insanai/guidedog_odin/releases/download/v0.6.0";

/// One release asset and the SHA-256 it must have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Pin {
    /// The asset's file name, as the release lists it.
    pub name: &'static str,
    /// Its SHA-256 in lowercase hexadecimal.
    pub sha256: &'static str,
}

/// The source archive: it holds the 0.6.0 publisher example, built from that source.
pub(super) const SOURCE: Pin = Pin {
    name: "guidedog-0.6.0-source.tar.gz",
    sha256: "0335f7082dc7e69c88b651f961612f99c7c640f64f272c4564d8023b95ad05d8",
};

/// The program archives of the platforms the qualification runs on, by platform name.
const PROGRAMS: [(&str, Pin); 2] = [
    (
        "linux-amd64",
        Pin {
            name: "guidedog-linux-amd64.tar.gz",
            sha256: "cf76ea2ad244333f068aec6f20e383d9fadce2fa21a9a4ee2cd9792840d30495",
        },
    ),
    (
        "macos-arm64",
        Pin {
            name: "guidedog-macos-arm64.tar.gz",
            sha256: "ca58f49fc6b720ccc7b11035f3f9b2a4fdca8bc7400c49c40fc7e4ebb4a7dce6",
        },
    ),
];

/// The release's platform name for an operating system and architecture as Rust names
/// them (`std::env::consts`), with the pin of its program archive; `None` elsewhere.
pub(super) fn program_pin(os: &str, arch: &str) -> Option<(&'static str, Pin)> {
    let platform = match (os, arch) {
        ("linux", "x86_64") => "linux-amd64",
        ("macos", "aarch64") => "macos-arm64",
        _ => return None,
    };
    PROGRAMS.iter().find(|(name, _)| *name == platform).copied()
}

/// Fails unless `actual`, a lowercase hex SHA-256, is the one `pin` names.
pub(super) fn check_sum(pin: &Pin, actual: &str) -> Result<()> {
    if actual == pin.sha256 {
        return Ok(());
    }
    Err(fail(
        "compat-sum",
        "WRONG CHECKSUM",
        &format!(
            "{} has SHA-256 {actual}, but the pinned release asset has {}.",
            pin.name, pin.sha256
        ),
        "Release assets never change, so the download was damaged or replaced. Delete it \
         from the work folder and run the step again; if it still differs, do not trust it.",
    ))
}

/// Downloads `pin` into `dir` unless a file with the pinned sum is already there, and
/// returns its path. The download goes to a `.part` file renamed once its sum matches.
pub(super) fn fetch(pin: &Pin, dir: &Path) -> Result<PathBuf> {
    let file = dir.join(pin.name);
    if file.is_file() && sha256_file(&file)? == pin.sha256 {
        return Ok(file);
    }
    println!("fetching {}", pin.name);
    let url = format!("{BASE}/{}", pin.name);
    let data = download(&url)?;
    check_sum(pin, &gd_sha::to_hex(&gd_sha::sha256(&data)))?;
    let part = dir.join(format!("{}.part", pin.name));
    write(&part, &data)?;
    rename(&part, &file)?;
    Ok(file)
}

/// Reads `url` with up to three attempts, as the script's `curl --retry 3` did.
fn download(url: &str) -> Result<Vec<u8>> {
    let options = gd_fetch::Options {
        timeout: Some(Duration::from_secs(600)),
        max_bytes: 512 * 1024 * 1024,
        ..gd_fetch::Options::default()
    };
    let mut last = None;
    for _ in 0..3 {
        match gd_fetch::get(url, &options) {
            Ok(fetched) => return Ok(fetched.data),
            Err(err) => last = Some(err),
        }
    }
    let detail = last.map_or_else(String::new, |err| err.detail.into_owned());
    Err(fail(
        "compat-fetch",
        "DOWNLOAD FAILED",
        &format!("I could not download {url}: {detail}"),
        "Check the network connection (and https_proxy, if one is needed), then run the \
         step again.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platforms_have_pins_and_others_do_not() {
        let Some((platform, pin)) = program_pin("linux", "x86_64") else { panic!("no pin") };
        assert_eq!(platform, "linux-amd64");
        assert_eq!(pin.name, "guidedog-linux-amd64.tar.gz");
        let Some((platform, _)) = program_pin("macos", "aarch64") else { panic!("no pin") };
        assert_eq!(platform, "macos-arm64");
        assert!(program_pin("linux", "aarch64").is_none());
        assert!(program_pin("windows", "x86_64").is_none());
    }

    #[test]
    fn pins_are_sha256_hex() {
        let pins = PROGRAMS.iter().map(|(_, pin)| pin).chain([&SOURCE]);
        for pin in pins {
            assert_eq!(pin.sha256.len(), 64, "{}", pin.name);
            assert!(pin.sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
            assert!(BASE.ends_with(OLD_TAG));
        }
    }

    #[test]
    fn a_wrong_sum_is_refused() {
        assert!(check_sum(&SOURCE, SOURCE.sha256).is_ok());
        let Err(failure) = check_sum(&SOURCE, &"0".repeat(64)) else { panic!("accepted") };
        assert!(format!("{failure:?}").contains("WRONG CHECKSUM"));
    }

    #[test]
    fn a_file_with_the_pinned_sum_is_not_downloaded_again() {
        let Ok(temp) = crate::fsutil::TempDir::new("pins") else { panic!("no folder") };
        // The SHA-256 of no bytes, so the file is empty; no URL is ever read.
        let pin = Pin {
            name: "asset.tar.gz",
            sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        };
        let Ok(()) = write(&temp.path().join(pin.name), b"") else { panic!("no write") };
        let Ok(path) = fetch(&pin, temp.path()) else { panic!("fetched again") };
        assert_eq!(path, temp.path().join("asset.tar.gz"));
    }
}
