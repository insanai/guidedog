//! Explicit corpus font ownership; the CJK face is downloaded and pinned, not a host font.

use std::io::Read;
use std::path::Path;

use gd_engrave::{FaceData, FontOptions, Fonts};

const REGULAR: &[u8] =
    include_bytes!("../../../docs/shared/_static/data/fonts/LibertinusSerif-Regular.otf");
const BOLD: &[u8] =
    include_bytes!("../../../docs/shared/_static/data/fonts/LibertinusSerif-Bold.otf");
const ITALIC: &[u8] =
    include_bytes!("../../../docs/shared/_static/data/fonts/LibertinusSerif-Italic.otf");
const DEVANAGARI: &[u8] =
    include_bytes!("../../../docs/shared/fonts/NotoSerifDevanagari-Regular.ttf");
const CJK_SUM: &str = "2a2eae2628df83556c54018c41e20fa532c1b862c5256ae8b3f23feb918d12ca";

pub(crate) struct FontData {
    cjk: Vec<u8>,
}

impl FontData {
    pub(crate) fn fonts(&self) -> Result<Fonts<'_>, String> {
        let face = |data| FaceData { data, ..Default::default() };
        gd_engrave::load_fonts(&FontOptions {
            regular: vec![face(REGULAR), face(DEVANAGARI), face(&self.cjk)],
            bold: vec![face(BOLD), face(DEVANAGARI), face(&self.cjk)],
            italic: vec![face(ITALIC), face(DEVANAGARI), face(&self.cjk)],
            bold_italic: Vec::new(),
        })
        .map_err(|error| error.to_string())
    }
}

pub(crate) fn load(root: &Path) -> Result<FontData, String> {
    let path = root.join("docs/shared/fonts/NotoSerifCJKsc-Regular.otf");
    let hint = "Run cargo xtask fetch-fonts --music to install the pinned CJK music test face.";
    let file = std::fs::File::open(&path).map_err(|error| format!("{error}. {hint}"))?;
    let mut cjk = Vec::new();
    file.take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut cjk)
        .map_err(|error| format!("{error}. {hint}"))?;
    if gd_sha::sha256(&cjk).iter().map(|byte| format!("{byte:02x}")).collect::<String>() != CJK_SUM
    {
        return Err(format!("{} has the wrong checksum. {hint}", path.display()));
    }
    Ok(FontData { cjk })
}
