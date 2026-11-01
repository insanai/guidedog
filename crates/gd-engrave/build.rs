//! Verify the unmodified embedded font and metadata before compiling their bytes.

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?).join("../../third_party/bravura");
    let manifest = root.join("SHA256SUMS");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let text = std::fs::read_to_string(&manifest)?;
    let names = ["Bravura.otf", "bravura_metadata.json", "LICENSE.txt"];
    if text.lines().count() != names.len() {
        return Err("Incomplete Bravura manifest.".into());
    }
    for (line, required) in text.lines().zip(names) {
        let (expected, name) =
            line.split_once("  ").ok_or("Malformed Bravura checksum record.")?;
        if name != required {
            return Err("Unexpected Bravura asset name.".into());
        }
        let path = root.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        let mut file = std::fs::File::open(&path)?;
        let mut digest = gd_sha::Sha256::new();
        std::io::copy(&mut file, &mut digest)?;
        if gd_sha::to_hex(&digest.finish()) != expected {
            return Err(format!(
                "Bravura asset {name} differs from its recorded upstream digest."
            )
            .into());
        }
    }
    Ok(())
}
