//! `sphinx.ext.intersphinx`: other projects' inventories.
//!
//! Inventories are fetched when a build starts and kept in
//! `<build>/.doctrees/__intersphinx_cache__/<name>_objects.inv`, reused for
//! `intersphinx_cache_limit` days; local inventory files are always read again. An
//! `--untrusted` build fetches nothing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use gd_conf::{Config, IntersphinxProject};
use gd_env::{Digest, Digester};
use gd_host::{Budget, Ledger, Problem};
use gd_inventory::{Inventory, Item};
use gd_report::Report;

/// The loaded inventories: each by its `intersphinx_mapping` name, and all merged, a
/// later name replacing an earlier one's entries, as Sphinx merges them.
#[derive(Debug, Clone, Default)]
pub struct Intersphinx {
    /// Each project's inventory, by name.
    pub named: BTreeMap<String, Inventory>,
    /// Every inventory merged.
    pub main: Inventory,
    /// The digest of every inventory loaded (name, URI, base and bytes), for the page
    /// digests.
    pub digest: Digest,
}

/// How inventories are loaded.
#[derive(Debug, Clone, Copy)]
pub struct IntersphinxOptions<'a> {
    /// Fetch remote inventories (false in an `--untrusted` build: only cached and local
    /// ones are read).
    pub network: bool,
    /// The budget the inventories are held within for the build.
    pub budget: &'a Budget,
}

impl Intersphinx {
    /// The inventory an item of the merged inventory came from: the last project (by
    /// name) that lists it, as later names replace earlier ones' entries.
    pub fn origin(&self, item: &Item) -> Option<&Inventory> {
        self.named.values().rev().find(|inv| inv.find(&item.kind, &item.name) == Some(item))
    }
}

/// Loads every project's inventory as Sphinx's `load_mappings` does: each location in
/// turn, a fresh cached copy of a remote inventory first. A project none of whose
/// locations works gets a warning in `reports` and is left out. Nothing is loaded
/// unless `sphinx.ext.intersphinx` is on. Fails only when the budget cannot hold an
/// inventory.
pub fn load_intersphinx(
    config: &Config,
    options: IntersphinxOptions<'_>,
    reports: &mut Vec<Report>,
) -> Result<Intersphinx, Problem> {
    let mut it = Intersphinx::default();
    let mapping = &config.intersphinx.mapping;
    if !config.has_extension("sphinx.ext.intersphinx") || mapping.is_empty() {
        return Ok(it);
    }
    // The inventories are held while they load; what they load into lives as long as
    // the build, about four times the bytes read.
    let ledger = Ledger::new(options.budget);
    let loader = Loader { config, network: options.network, roots: loader_roots(config, options) };
    let mut sums = Digester::new();
    let mut names: Vec<&IntersphinxProject> = mapping.iter().collect();
    names.sort_by(|a, b| a.name.cmp(&b.name));
    for p in names {
        let Some((inventory, data, base)) = loader.load(p, reports) else { continue };
        if !ledger.charge(data.len().saturating_mul(4)) {
            let need = ledger.refused();
            return Err(gd_host::budget_problem(
                "Loading this inventory",
                &p.uri,
                need,
                Some(options.budget),
            ));
        }
        sums.text(&p.name).text(&p.uri).text(&base).digest(&Digest::of_parts(&[&data]));
        it.named.insert(p.name.clone(), inventory);
    }
    // Sorted names, later ones replacing earlier entries in the merged inventory.
    for inventory in it.named.values() {
        for item in inventory.items() {
            it.main.add(item.clone());
        }
    }
    it.digest = sums.finish();
    Ok(it)
}

/// The folders an `--untrusted` build may read local inventories from.
fn loader_roots(config: &Config, options: IntersphinxOptions<'_>) -> Vec<PathBuf> {
    if options.network {
        return Vec::new();
    }
    gd_host::canonical_roots(&gd_conf::readable_roots(config))
}

/// What loading the inventories reads.
struct Loader<'a> {
    config: &'a Config,
    /// Fetch remote inventories; false in an `--untrusted` build, which reads remote
    /// inventories only from the cache, at any age, and local ones only in `roots`.
    network: bool,
    roots: Vec<PathBuf>,
}

/// A remote inventory's cache keeps, beside the inventory, a file naming the target
/// URI it was fetched for (as a digest, since the URI may hold a password) and the base
/// its links start with. The base differs from the target URI when the inventory had
/// moved, so a build that reads the cache links as the build that fetched it did. A
/// cache for another target URI is not used.
const CACHE_BASE_SUFFIX: &str = ".base";

impl Loader<'_> {
    /// Sphinx's `_fetch_inventory_group`: each location in turn, a fresh cached copy of
    /// a remote inventory first; a warning only when none works. Returns the inventory,
    /// its bytes, and the base URL its links start with.
    fn load(
        &self,
        p: &IntersphinxProject,
        reports: &mut Vec<Report>,
    ) -> Option<(Inventory, Vec<u8>, String)> {
        let cache_dir = gd_env::doctrees_dir(self.config).join("__intersphinx_cache__");
        let cache_path = cache_dir.join(format!("{}_objects.inv", p.name));
        let mut failures = Vec::new();
        for location in &p.locations {
            let remote = location.is_empty() || location.contains("://");
            let mut source = if location.is_empty() {
                url_join(&p.uri, "objects.inv")
            } else {
                location.clone()
            };
            if remote {
                if let Some(cached) = self.cached(p, &cache_path) {
                    return Some(cached);
                }
            } else if !Path::new(&source).is_absolute() {
                source = self.config.source_dir.join(&source).to_string_lossy().into_owned();
            }
            if let Some(refusal) = self.refusal(&source, remote) {
                failures.push(refusal);
                continue;
            }
            let (data, base) = match self.fetch(p, &source) {
                Ok(fetched) => fetched,
                Err(failure) => {
                    failures.push(failure);
                    continue;
                }
            };
            match gd_inventory::load_inventory(&data, &base) {
                Ok(inventory) => {
                    if remote {
                        cache_inventory(&cache_dir, &cache_path, p, &data, &base);
                    }
                    return Some((inventory, data, base));
                }
                Err(e) => failures.push(format!(
                    "intersphinx inventory {source:?} is not an inventory: {}",
                    e.message()
                )),
            }
        }
        if !failures.is_empty() {
            reports.push(fetch_warning(self.config, p, &failures));
        }
        None
    }

    /// Why an `--untrusted` build does not read an inventory location: it reaches no
    /// network, and it reads no local file outside the roots.
    fn refusal(&self, source: &str, remote: bool) -> Option<String> {
        if self.network {
            return None;
        }
        if remote {
            return Some(format!(
                "{source} was not fetched: an --untrusted build uses no network"
            ));
        }
        if !gd_host::confined(&self.roots, Path::new(source)) {
            return Some(format!("{source} is outside the source folder and the include roots"));
        }
        None
    }

    /// Reads one inventory location. A remote inventory that moved makes its new folder
    /// the base of the links when the base was the old folder, as in Sphinx.
    fn fetch(&self, p: &IntersphinxProject, source: &str) -> Result<(Vec<u8>, String), String> {
        let timeout = self.config.intersphinx.timeout;
        let options = gd_fetch::Options {
            timeout: (timeout > 0).then(|| Duration::from_secs(timeout as u64)),
            user_agent: "Guidedog intersphinx".into(),
            ..gd_fetch::Options::default()
        };
        let fetched = gd_fetch::get(source, &options).map_err(|e| {
            let detail = match e.status {
                Some(status) if e.kind == gd_fetch::ErrorKind::Status => {
                    format!("HTTP status {status}")
                }
                _ => e.detail.to_string(),
            };
            format!("intersphinx inventory {source:?} not fetchable due to {}: {detail}", e.code())
        })?;
        let mut base = strip_auth(&p.uri);
        if !fetched.url.is_empty() && fetched.url != source && source.contains("://") {
            let folder = &source[..source.rfind('/').map_or(0, |i| i + 1)];
            if base == source || base == folder || base == folder.trim_end_matches('/') {
                base = fetched.url[..fetched.url.rfind('/').unwrap_or(0)].to_string();
            }
        }
        Ok((fetched.data, base))
    }

    /// The cached copy of a remote inventory, when it is fresh (at any age for an
    /// `--untrusted` build) and was fetched for this target URI.
    fn cached(&self, p: &IntersphinxProject, path: &Path) -> Option<(Inventory, Vec<u8>, String)> {
        let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
        let limit = self.config.intersphinx.cache_limit;
        if self.network && limit >= 0 {
            let age = SystemTime::now().duration_since(modified).unwrap_or_default();
            if age > Duration::from_secs((limit as u64).saturating_mul(86_400)) {
                return None;
            }
        }
        let meta = std::fs::read_to_string(cache_meta_path(path)).ok()?;
        let (target, rest) = meta.split_once('\n')?;
        if target != Digest::of_text(&p.uri).to_string() {
            return None;
        }
        let base = rest.trim_end_matches('\n').to_string();
        let data = std::fs::read(path).ok()?;
        let inventory = gd_inventory::load_inventory(&data, &base).ok()?;
        Some((inventory, data, base))
    }
}

fn cache_meta_path(path: &Path) -> PathBuf {
    let mut text = path.as_os_str().to_owned();
    text.push(CACHE_BASE_SUFFIX);
    PathBuf::from(text)
}

/// Keeps a fetched inventory for the next builds; a cache that cannot be written is
/// only a slower next build.
fn cache_inventory(dir: &Path, path: &Path, p: &IntersphinxProject, data: &[u8], base: &str) {
    if std::fs::create_dir_all(dir).is_err() || std::fs::write(path, data).is_err() {
        return;
    }
    let meta = format!("{}\n{base}\n", Digest::of_text(&p.uri));
    let _ = std::fs::write(cache_meta_path(path), meta);
}

fn fetch_warning(config: &Config, p: &IntersphinxProject, failures: &[String]) -> Report {
    let message = format!(
        "failed to reach any of the inventories of {} with the following issues:\n{}",
        p.name,
        failures.join("\n")
    );
    let report = Report::warning("build.intersphinx.fetch", "INVENTORY NOT LOADED", &message)
        .hint("Check the URL and the network, or give a local copy as a location.");
    let path = config.config_path().to_string_lossy().into_owned();
    Report { path, ..report }
}

/// `posixpath.join`: a second part starting with `/` replaces the first.
fn url_join(base: &str, part: &str) -> String {
    if part.starts_with('/') {
        return part.to_string();
    }
    if base.is_empty() || base.ends_with('/') {
        return format!("{base}{part}");
    }
    format!("{base}/{part}")
}

/// Removes `user:password@` from a URL, as Sphinx does before making links.
fn strip_auth(url: &str) -> String {
    let Some(scheme) = url.find("://") else { return url.to_string() };
    let rest = &url[scheme + 3..];
    let host = rest.find(['/', '?', '#']).map_or(rest, |end| &rest[..end]);
    match host.rfind('@') {
        Some(at) => format!("{}{}", &url[..scheme + 3], &rest[at + 1..]),
        None => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_join_and_lose_their_credentials() {
        assert_eq!(
            url_join("https://a.org/docs", "objects.inv"),
            "https://a.org/docs/objects.inv"
        );
        assert_eq!(url_join("https://a.org/", "objects.inv"), "https://a.org/objects.inv");
        assert_eq!(url_join("https://a.org", "/x.inv"), "/x.inv");
        assert_eq!(strip_auth("https://u:p@a.org/x@y"), "https://a.org/x@y");
        assert_eq!(strip_auth("https://a.org/"), "https://a.org/");
    }
}
