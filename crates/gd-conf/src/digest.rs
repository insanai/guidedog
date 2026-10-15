//! `Digest`: the SHA-256 that names what changed, for the settings, the environment and
//! the outputs (docs/manual/internals/incremental.rst lists every input).

use std::fmt;

use gd_sha::Sha256;

/// A SHA-256 digest. It displays as 64 lowercase hexadecimal digits, the form saved in
/// the environment and the manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Digest(pub [u8; 32]);

impl Digest {
    /// The digest of `parts`, each followed by a zero byte, so `["ab", "c"]` and
    /// `["a", "bc"]` differ. The same as a [`Digester`] fed each part.
    pub fn of_parts(parts: &[&[u8]]) -> Digest {
        let mut d = Digester::new();
        for part in parts {
            d.part(part);
        }
        d.finish()
    }

    /// The digest of one text, as `of_parts(&[text])`.
    pub fn of_text(text: &str) -> Digest {
        Digest::of_parts(&[text.as_bytes()])
    }

    /// Reads 64 hexadecimal digits back; `None` for anything else.
    pub fn from_hex(text: &str) -> Option<Digest> {
        if text.len() != 64 {
            return None;
        }
        let bytes = gd_sha::from_hex(text)?;
        let mut out = [0u8; 32];
        out.copy_from_slice(&bytes);
        Some(Digest(out))
    }

    /// The first 16 hexadecimal digits, which name cache files
    /// (`objects/<docname>.<short>.gdo`).
    pub fn short(&self) -> String {
        let mut text = String::with_capacity(16);
        gd_sha::write_hex(&mut text, &self.0[..8]);
        text
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut text = String::with_capacity(64);
        gd_sha::write_hex(&mut text, &self.0);
        f.write_str(&text)
    }
}

/// The digest of parts fed one at a time, for digests made of many inputs. Each part is
/// followed by a zero byte, so the boundaries between parts count.
#[derive(Debug, Clone, Default)]
pub struct Digester {
    hash: Sha256,
}

impl Digester {
    /// Starts an empty digest.
    pub fn new() -> Digester {
        Digester { hash: Sha256::new() }
    }

    /// Adds one part.
    pub fn part(&mut self, bytes: &[u8]) -> &mut Digester {
        self.hash.update(bytes);
        self.hash.update(&[0]);
        self
    }

    /// Adds one text part.
    pub fn text(&mut self, text: &str) -> &mut Digester {
        self.part(text.as_bytes())
    }

    /// Adds a list of texts as one part per item, then the count, so `["a", "b"]` and
    /// `["a"], ["b"]` in two calls differ.
    pub fn texts<S: AsRef<str>>(&mut self, items: &[S]) -> &mut Digester {
        for item in items {
            self.text(item.as_ref());
        }
        self.text(&items.len().to_string())
    }

    /// Adds a flag as `"1"` or `"0"`.
    pub fn flag(&mut self, on: bool) -> &mut Digester {
        self.text(if on { "1" } else { "0" })
    }

    /// Adds a number in decimal.
    pub fn number(&mut self, n: i64) -> &mut Digester {
        self.text(&n.to_string())
    }

    /// Adds another digest's bytes as one part.
    pub fn digest(&mut self, other: &Digest) -> &mut Digester {
        self.part(&other.0)
    }

    /// The digest of every part added.
    pub fn finish(&self) -> Digest {
        Digest(self.hash.clone().finish())
    }
}
