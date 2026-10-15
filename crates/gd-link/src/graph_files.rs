//! Files a graph names (graph_files.odin).
//!
//! A graph can name files for Graphviz to open: a node's image or shapefile, an `<IMG>`
//! in an HTML-like label, and the folders imagepath and fontpath. A build reads a file
//! a graph names as it reads an image a document shows: relative to the document's
//! folder, and only below the source folder and the include roots
//! (`gd_host::confined`); imagepath and fontpath are left out, since Guidedog resolves
//! names itself. An `--untrusted` build reads none. The drawing then carries each image
//! as a `data:` URL, so it shows in pages and books alike, and no path of the machine
//! that built it reaches the output.

use std::path::{Path, PathBuf};

use gd_graphdog::FileDecision;
use gd_host::Ledger;

/// The images a graph may show, with their media types.
const GRAPH_IMAGE_TYPES: [(&str, &str); 5] = [
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("svg", "image/svg+xml"),
];

/// What one graph's drawing has read: the paths given to Graphviz, which the drawing
/// names until [`GraphFiles::embed`] replaces them, every path looked at (the page's
/// trace notes them), and why a name was refused.
#[derive(Debug)]
pub(crate) struct GraphFiles<'r> {
    /// An `--untrusted` build reads no file.
    pub untrusted: bool,
    /// The folder of the document the graph is in.
    pub base: PathBuf,
    /// The canonical folders the build may read.
    pub roots: &'r [PathBuf],
    /// The files given to Graphviz.
    pub named: Vec<PathBuf>,
    /// Every file looked at, for the page's trace.
    pub looked: Vec<PathBuf>,
    /// Why a file was refused; empty when none was.
    pub problem: String,
}

impl GraphFiles<'_> {
    /// Graphviz's file access for a build: see the module comment.
    pub fn decide(&mut self, attribute: &str, name: &str) -> FileDecision {
        if self.untrusted {
            self.problem =
                "An --untrusted build reads no file a graph names; leave the attribute out."
                    .into();
            return FileDecision::Refuse;
        }
        if attribute == "imagepath" || attribute == "fontpath" {
            return FileDecision::Omit;
        }
        let path = gd_conf::clean_path(&self.base.join(name));
        self.looked.push(path.clone());
        let text = path.to_string_lossy();
        let problem = if media_type(&path).is_none() {
            "A graph shows PNG, JPEG, GIF, or SVG images."
        } else if !path.is_file() {
            "Name an existing file, relative to the document."
        } else if !gd_host::confined(self.roots, &path) {
            "A graph reads files only below the source folder and the include roots; add \
             the file's folder to include_roots."
        } else if text.contains(['"', '<', '>', '&', '\'']) {
            "Rename the file: its path holds a character a drawing cannot name."
        } else {
            let open = text.into_owned();
            self.named.push(path);
            return FileDecision::Open(open);
        };
        self.problem = problem.to_string();
        FileDecision::Refuse
    }

    /// Replaces each path the drawing names with the file as a `data:` URL, reading and
    /// encoding within `ledger`. A drawing that would still show a path is refused, so
    /// a path never reaches a page or a book. Fails with the message to report.
    pub fn embed(&mut self, svg: Vec<u8>, ledger: &Ledger<'_>) -> Result<Vec<u8>, String> {
        if self.named.is_empty() {
            return Ok(svg);
        }
        let mut out =
            String::from_utf8(svg).map_err(|_| self.refuse("Graphviz wrote no text."))?;
        for path in &self.named {
            let href = format!("xlink:href=\"{}\"", path.display());
            if !out.contains(&href) {
                continue;
            }
            let data = gd_host::read_file(path, Some(ledger), gd_host::MAX_SOURCE_BYTES)
                .map_err(|p| p.message)?;
            let encoded_size = 4 * data.len().div_ceil(3);
            if !ledger.charge(encoded_size + out.len()) {
                return Err("The drawing's images need more memory than the build's budget \
                            has left."
                    .into());
            }
            let media = media_type(path).unwrap_or("application/octet-stream");
            let url = format!("xlink:href=\"data:{media};base64,{}\"", base64(&data));
            out = out.replace(&href, &url);
        }
        let leaked = self.named.iter().any(|p| out.contains(&*p.to_string_lossy()));
        if leaked {
            return Err(self.refuse(
                "Graphviz wrote a file's path where Guidedog does not \
                                    replace it.",
            ));
        }
        Ok(out.into_bytes())
    }

    fn refuse(&mut self, message: &str) -> String {
        self.problem = "The graph is shown as code.".into();
        message.to_string()
    }
}

/// Whether a graph's source or the dot arguments could name a file, so a drawing kept
/// from an earlier build may be out of date.
pub(crate) fn may_name_files(source: &str, args: &str) -> bool {
    let text = format!("{source}\0{args}").to_lowercase();
    ["image", "shapefile", "fontpath", "<img"].iter().any(|word| text.contains(word))
}

/// The media type of an image a graph may show, by its suffix.
fn media_type(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    GRAPH_IMAGE_TYPES.iter().find(|(e, _)| *e == ext).map(|(_, media)| *media)
}

/// Standard base64 with padding.
fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(4 * data.len().div_ceil(3));
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_pads_like_the_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn graphs_that_may_name_files_are_drawn_again() {
        assert!(may_name_files("a [image=\"x.png\"]", ""));
        assert!(may_name_files("a [label=<<IMG SRC=\"x.png\"/>>]", ""));
        assert!(may_name_files("a -> b", "-Gfontpath=/x"));
        assert!(!may_name_files("a -> b", "-Grankdir=LR"));
    }

    #[test]
    fn untrusted_builds_and_unknown_types_are_refused() {
        let roots = [std::env::temp_dir()];
        let mut files = GraphFiles {
            untrusted: true,
            base: std::env::temp_dir(),
            roots: &roots,
            named: Vec::new(),
            looked: Vec::new(),
            problem: String::new(),
        };
        assert_eq!(files.decide("image", "x.png"), FileDecision::Refuse);
        assert!(files.problem.contains("--untrusted"));
        files.untrusted = false;
        assert_eq!(files.decide("imagepath", "/etc"), FileDecision::Omit);
        assert_eq!(files.decide("image", "notes.txt"), FileDecision::Refuse);
        assert!(files.problem.contains("PNG"));
        assert_eq!(files.decide("image", "missing-graph-image.png"), FileDecision::Refuse);
        assert!(files.problem.contains("existing file"));
    }
}
