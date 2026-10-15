//! Checks on the generated files a build plugin returns.

use crate::paths::is_portable_asset;
use crate::problem::{self, Problem};
use crate::{Asset, MAX_PLUGIN_ASSET_BYTES, MAX_PLUGIN_ASSETS};

/// Refuses more than 256 assets or 16 MiB, a path that is not portable, and two
/// paths that are equal ignoring case or where one is the other's folder.
pub(crate) fn check_assets(id: &str, assets: &[Asset]) -> Result<(), Problem> {
    if assets.len() > MAX_PLUGIN_ASSETS {
        return Err(problem::contract(
            id,
            "Return no more than MAX_PLUGIN_ASSETS (256) generated files per build.",
        ));
    }
    let mut total = 0usize;
    let mut folded: Vec<String> = Vec::with_capacity(assets.len());
    for asset in assets {
        if !is_portable_asset(&asset.path) {
            return Err(problem::contract(
                id,
                "Use portable relative asset paths, such as css/badge.css; avoid '..' and \
                 Windows device names, forbidden characters, and trailing dots or spaces.",
            ));
        }
        total = total.saturating_add(asset.data.len());
        if total > MAX_PLUGIN_ASSET_BYTES {
            return Err(problem::contract(
                id,
                "Keep generated assets within MAX_PLUGIN_ASSET_BYTES (16 MiB) per plugin.",
            ));
        }
        let path = asset.path.to_lowercase();
        if folded.iter().any(|p| conflicts(p, &path)) {
            return Err(problem::contract(
                id,
                "Use distinct paths; files cannot share names or be another file's folder.",
            ));
        }
        folded.push(path);
    }
    Ok(())
}

/// Whether two lowercased paths name one file, or one is the other's folder.
fn conflicts(a: &str, b: &str) -> bool {
    a == b || is_parent(a, b) || is_parent(b, a)
}

fn is_parent(parent: &str, child: &str) -> bool {
    child.len() > parent.len()
        && child.starts_with(parent)
        && child.as_bytes()[parent.len()] == b'/'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(paths: &[&str]) -> Vec<Asset> {
        paths.iter().map(|p| Asset::new(p, Vec::new())).collect()
    }

    #[test]
    fn assets_refuse_traversal_and_portable_name_collisions() {
        let bad = [
            "../bad", "C:/bad", "/bad", "a\\b", "a/../b", ".git/a", "CON.css", "a/aux",
            "com1.txt", "LPT²", "a.", "a ", "a?b", "a|b",
        ];
        for path in bad {
            let p = check_assets("test-build", &named(&[path])).unwrap_err();
            assert!(!p.hint.is_empty(), "accepted {path}");
        }
        for pair in [["A.css", "a.css"], ["a", "a/b"], ["a/b", "A"]] {
            assert!(check_assets("test-build", &named(&pair)).is_err(), "{pair:?}");
        }
        check_assets("test-build", &named(&["css/badge.css", "css/badge.js"])).unwrap();
    }

    #[test]
    fn assets_are_bounded() {
        let many: Vec<Asset> = (0..257).map(|i| Asset::new(&format!("a{i}"), [])).collect();
        assert!(check_assets("x", &many).is_err());
        let big = vec![Asset::new("a", vec![0; MAX_PLUGIN_ASSET_BYTES + 1])];
        assert!(check_assets("x", &big).is_err());
    }
}
