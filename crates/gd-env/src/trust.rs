//! What a build trusts (GDS 0018), in two independent parts. Its confinement says what
//! the build may reach on the machine: which folders it reads, whether it uses the
//! network, which plugins start, and whether graphs and music may name files. Its
//! content rules (`gd_conf::ContentRules`) say what reaches the output: raw content
//! and unsafe URLs. `BuildOptions::untrusted` sets both to their strict values.

use std::borrow::Cow;
use std::path::PathBuf;

use gd_conf::{Config, ContentRules, Digest, Digester};

use crate::builder::BuildOptions;

/// The folders a build reads.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum ReadScope {
    /// The source folder and every folder conf.toml names: a trusted build.
    #[default]
    Project,
    /// The source folder and these folders only (absolute, or relative to the source
    /// folder; an empty list is the source folder alone). The build confines its copy
    /// of the configuration with `gd_conf::confine` before it reads anything, so a
    /// setting that leads elsewhere is dropped with a warning, `typst` is the embedded
    /// compiler, and no Typst package is downloaded.
    Within(Vec<PathBuf>),
}

/// Which build plugins a build may start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PluginAccess {
    /// The plugins linked into the program, then those of the machine registry.
    #[default]
    Any,
    /// Only the plugins linked into the program; any other the project lists is
    /// dropped with a `build.untrusted` warning.
    Linked,
    /// None: a project that lists any fails with `plugins.untrusted` (exit 2).
    None,
}

/// What a build may reach beyond its documents. `Default` is a trusted build's;
/// [`Confinement::STRICT`] is an `--untrusted` build's. Fill it with struct-update
/// syntax: `Confinement { network: false, ..Default::default() }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Confinement {
    /// The folders the build reads.
    pub reads: ReadScope,
    /// Fetch over the network: intersphinx inventories and Typst packages. Without
    /// it, inventories come from the cache and the readable folders, and Typst
    /// packages only from the machine's package folders.
    pub network: bool,
    /// The build plugins that may start. A confined read scope starts no plugin from
    /// the machine registry, whatever this says.
    pub plugins: PluginAccess,
    /// Read the files a graph names (images, shape files), within the readable folders.
    pub graph_files: bool,
    /// Read music files (`lilypond` file arguments and `\include`), within the
    /// readable folders.
    pub music_files: bool,
}

impl Default for Confinement {
    fn default() -> Confinement {
        Confinement::TRUSTED
    }
}

impl Confinement {
    /// A trusted build's: the project's folders, the network, any plugin, and the
    /// files graphs and music name.
    pub const TRUSTED: Confinement = Confinement {
        reads: ReadScope::Project,
        network: true,
        plugins: PluginAccess::Any,
        graph_files: true,
        music_files: true,
    };

    /// An `--untrusted` build's: the source folder alone, no network, no plugin, and
    /// no file a graph or music names.
    pub const STRICT: Confinement = Confinement {
        reads: ReadScope::Within(Vec::new()),
        network: false,
        plugins: PluginAccess::None,
        graph_files: false,
        music_files: false,
    };

    /// Whether the build reads only below given folders, and so runs only the
    /// embedded Typst compiler and downloads no Typst package.
    pub fn is_confined(&self) -> bool {
        matches!(self.reads, ReadScope::Within(_))
    }
}

impl BuildOptions {
    /// The confinement the build applies: [`Confinement::STRICT`] when `untrusted`,
    /// `confinement` otherwise. Borrows `self`.
    pub fn effective_confinement(&self) -> Cow<'_, Confinement> {
        if self.untrusted {
            Cow::Owned(Confinement::STRICT)
        } else {
            Cow::Borrowed(&self.confinement)
        }
    }

    /// The content rules the build applies to `config`'s project, each stated:
    /// [`ContentRules::OMIT`] when `untrusted`; otherwise each rule of `content`, or
    /// else conf.toml's (`raw_content`, `unsafe_urls`), or else `content_defaults`',
    /// or else `allow`, as in Sphinx. Borrows both for the call.
    pub fn content_rules(&self, config: &Config) -> ContentRules {
        if self.untrusted {
            return ContentRules::OMIT;
        }
        let rules = self.content.or(config.content).or(self.content_defaults);
        ContentRules { raw: Some(rules.raw_rule()), unsafe_urls: Some(rules.unsafe_url_rule()) }
    }
}

/// The digest of what a build trusts, which the reading and output settings join: a
/// build with other trust reads and writes everything again.
pub fn trust_digest(confinement: &Confinement, content: ContentRules) -> Digest {
    let mut d = Digester::new();
    d.text(&format!("{confinement:?}")).text(&format!("{content:?}"));
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gd_conf::ContentRule;

    #[test]
    fn untrusted_sets_the_strict_confinement_and_omits_content() {
        let mut config = Config::new("/p/doc", "/p/doc/_build");
        config.content.raw = Some(ContentRule::Allow);
        let untrusted = BuildOptions { untrusted: true, ..Default::default() };
        assert_eq!(*untrusted.effective_confinement(), Confinement::STRICT);
        assert_eq!(untrusted.content_rules(&config), ContentRules::OMIT);
        let trusted = BuildOptions::default();
        assert_eq!(*trusted.effective_confinement(), Confinement::TRUSTED);
        assert!(!trusted.effective_confinement().is_confined());
        assert_eq!(trusted.content_rules(&config), ContentRules::ALLOW);
    }

    #[test]
    fn the_build_imposes_rules_or_supplies_those_the_project_leaves_unset() {
        let mut config = Config::new("/p/doc", "/p/doc/_build");
        config.content.unsafe_urls = Some(ContentRule::Allow);
        let defaults = BuildOptions { content_defaults: ContentRules::OMIT, ..Default::default() };
        let rules = defaults.content_rules(&config);
        assert_eq!(rules.raw, Some(ContentRule::Omit), "the project leaves raw_content unset");
        assert_eq!(rules.unsafe_urls, Some(ContentRule::Allow), "the project states this one");
        let imposed = BuildOptions { content: ContentRules::OMIT, ..Default::default() };
        assert_eq!(imposed.content_rules(&config), ContentRules::OMIT);
    }

    #[test]
    fn trust_changes_the_digest() {
        let open = trust_digest(&Confinement::TRUSTED, ContentRules::ALLOW);
        assert_ne!(open, trust_digest(&Confinement::STRICT, ContentRules::ALLOW));
        assert_ne!(open, trust_digest(&Confinement::TRUSTED, ContentRules::OMIT));
    }
}
