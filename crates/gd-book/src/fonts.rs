//! The fonts Guidedog ships for books (fonts.odin).
//!
//! Guidedog ships Libertinus Sans (SIL Open Font License 1.1, assets/fonts/OFL.txt), the
//! sans companion of the Libertinus Serif Typst embeds, so book headings look the same
//! on every machine, with `pdf_fonts = "embedded"` too. The files are written to
//! `<doctrees>/fonts` when a book is built, and Typst searches that folder after the
//! project's `pdf_font_paths`.

use std::path::PathBuf;

use gd_conf::Config;

/// One file of the bundled fonts.
struct BundledFont {
    name: &'static str,
    data: &'static [u8],
}

const BUNDLED_FONTS: [BundledFont; 4] = [
    BundledFont {
        name: "LibertinusSans-Regular.otf",
        data: include_bytes!("../assets/fonts/LibertinusSans-Regular.otf"),
    },
    BundledFont {
        name: "LibertinusSans-Bold.otf",
        data: include_bytes!("../assets/fonts/LibertinusSans-Bold.otf"),
    },
    BundledFont {
        name: "LibertinusSans-Italic.otf",
        data: include_bytes!("../assets/fonts/LibertinusSans-Italic.otf"),
    },
    BundledFont { name: "OFL.txt", data: include_bytes!("../assets/fonts/OFL.txt") },
];

/// The folder holding the bundled fonts (`<doctrees>/fonts`), writing any file that is
/// missing or has another size. `None` when it cannot be written; Typst then uses other
/// fonts.
pub fn bundled_font_dir(config: &Config) -> Option<PathBuf> {
    let dir = gd_env::doctrees_dir(config).join("fonts");
    if std::fs::create_dir_all(&dir).is_err() && !dir.is_dir() {
        return None;
    }
    for font in &BUNDLED_FONTS {
        let path = dir.join(font.name);
        let same_size = std::fs::metadata(&path).is_ok_and(|m| m.len() == font.data.len() as u64);
        if !same_size && std::fs::write(&path, font.data).is_err() {
            return None;
        }
    }
    Some(dir)
}
