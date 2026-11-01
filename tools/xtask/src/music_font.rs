//! Checks the original Bravura files against their pinned upstream digests.

use crate::args::{self, Spec};
use crate::fsutil::{read_text, sha256_file};
use crate::{Result, fail, repository_root};

const HELP: &str = "\
Checks the unmodified Bravura music font, metadata and license against SHA256SUMS.

Usage: cargo xtask check-music-font
";

pub(crate) fn main(args: &[String]) -> Result<()> {
    let spec = Spec { command: "check-music-font", flags: &[], options: &[], help: HELP };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(0, &spec)?;
    let root = repository_root().join("third_party/bravura");
    let manifest = read_text(&root.join("SHA256SUMS"))?;
    let expected_names = ["Bravura.otf", "bravura_metadata.json", "LICENSE.txt"];
    for (line, name) in manifest.lines().zip(expected_names) {
        let Some((expected, actual_name)) = line.split_once("  ") else {
            return Err(invalid("A Bravura checksum record is malformed."));
        };
        if name != actual_name || sha256_file(&root.join(name))? != expected {
            return Err(invalid(&format!(
                "Bravura asset {name} differs from its upstream digest."
            )));
        }
    }
    if manifest.lines().count() != expected_names.len() {
        return Err(invalid("The Bravura manifest must name exactly three original files."));
    }
    println!("Verified Bravura 1.482: font, SMuFL metadata and original license.");
    Ok(())
}

fn invalid(message: &str) -> crate::Failure {
    fail(
        "music-font",
        "MUSIC FONT DIFFERS",
        message,
        "Restore the unmodified files named in third_party/bravura/PROVENANCE.txt.",
    )
}
