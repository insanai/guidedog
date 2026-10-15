//! `PluginRun`: the build plugins one build runs (GDS 0010).
//!
//! A build starts one private session per selected plugin, never shared with another
//! build, and stops them when it ends (`Drop`). Reading threads serialize access to a
//! session through its `Mutex`, so a plugin need not be thread-safe. The run's digest
//! (identity, cache key, configuration and dependency files, assets) joins the reading
//! and output settings, so a changed plugin rebuilds what it touched.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use gd_conf::{CONFIG_FILE, Config, Digest, Digester};
use gd_host::Problem;
use gd_plugins::{
    Asset as PluginAsset, Cancel, HtmlInput, MAX_BUILD_PLUGINS, Plugin, PluginOptions,
    PluginSession, Registry, SourceFormat, SourceInput,
};

use crate::builder::BuilderKind;
use crate::discover::{SourceFile, SourceKind};

/// One build's plugin sessions, started in the order `plugins` lists them.
#[derive(Debug, Default)]
pub struct PluginRun {
    sessions: Vec<Mutex<PluginSession>>,
    ids: Vec<String>,
    assets: Vec<Vec<PluginAsset>>,
    digest: Digest,
}

/// The facts a run is started with.
#[derive(Debug, Clone, Copy)]
pub struct PluginStart<'a> {
    /// The build's configuration (`plugins`, `plugin_configs`, `plugin_timeout`).
    pub config: &'a Config,
    /// The plugins linked into this program, looked up before the machine registry.
    pub linked: &'a Registry,
    /// The builder: HTML builders also collect assets.
    pub kind: BuilderKind,
    /// An `--untrusted` build starts no native code.
    pub untrusted: bool,
}

impl PluginRun {
    /// Starts the plugins `config.plugins.ids` selects: each from the linked registry,
    /// or else from the machine registry (verified against its SHA-256), loaded with
    /// its configuration file, and for HTML builders with its assets collected.
    ///
    /// Fails with `plugins.untrusted` when an `--untrusted` build selects any,
    /// `plugins.config` for more than [`MAX_BUILD_PLUGINS`], an invalid or repeated ID,
    /// or an unreadable dependency, and with the plugin's own problem when it cannot be
    /// found or loaded. Sessions started before a failure are stopped.
    pub fn start(start: PluginStart<'_>) -> Result<PluginRun, Problem> {
        let config = start.config;
        let ids = &config.plugins.ids;
        let mut run = PluginRun::default();
        if ids.is_empty() {
            return Ok(run);
        }
        if start.untrusted {
            return Err(Problem::new(
                "plugins.untrusted",
                "NATIVE PLUGINS ARE DISABLED",
                &config.config_dir.join(CONFIG_FILE).display().to_string(),
                "An untrusted build cannot start native code.",
                "Remove plugins from this build, or review the project and build without \
                 --untrusted.",
                2,
            ));
        }
        if ids.len() > MAX_BUILD_PLUGINS {
            let message = format!("This build lists more than {MAX_BUILD_PLUGINS} plugins.");
            return Err(configuration(config, "", &message, "Keep plugins to at most 64 IDs."));
        }
        let mut digest = Digester::new();
        for (i, id) in ids.iter().enumerate() {
            check_id(config, id, &ids[..i])?;
            let plugin = select(start.linked, id)?;
            run.start_one(start, &plugin, &mut digest)?;
        }
        run.digest = digest.finish();
        Ok(run)
    }

    fn start_one(
        &mut self,
        start: PluginStart<'_>,
        plugin: &Plugin,
        digest: &mut Digester,
    ) -> Result<(), Problem> {
        let config = start.config;
        let id = plugin.id().to_string();
        let file = config.plugins.configs.get(&id).map(|f| resolve(&config.config_dir, f));
        let file_text = file.as_deref().map(|f| f.display().to_string()).unwrap_or_default();
        let source = config.source_dir.display().to_string();
        let mut options =
            PluginOptions::new(&file_text, &source, start.kind.name(), &config.language);
        options.request_timeout = Duration::from_secs(config.plugins.timeout.max(1) as u64);
        options.cancel = Cancel::from_fn(gd_host::was_interrupted);
        let mut session = PluginSession::load(plugin, &options).map_err(|p| host_problem(&p))?;
        let identity = plugin.identity(crate::executable_stamp());
        digest.text(&id).text(&identity).text(session.cache_key());
        digest.text(&config.language).text(start.kind.name());
        let paths = file
            .into_iter()
            .chain(session.dependencies().iter().map(|d| resolve(&config.source_dir, d)));
        for path in paths {
            let Some(sum) = crate::file_digest(&path) else {
                let message = format!("Its dependency {} could not be read.", path.display());
                let hint = "Restore that file or fix its path in the plugin configuration, \
                            then build again.";
                return Err(configuration(config, &id, &message, hint));
            };
            digest.text(&path.display().to_string()).digest(&sum);
        }
        let mut assets = Vec::new();
        let collects = plugin.build_info().is_some_and(|i| i.hooks.assets);
        if start.kind.is_html() && collects {
            assets = session.collect_assets().map_err(|p| host_problem(&p))?;
            for asset in &assets {
                digest.text(&asset.path).part(&asset.data);
            }
        }
        self.sessions.push(Mutex::new(session));
        self.ids.push(id);
        self.assets.push(assets);
        Ok(())
    }

    /// Whether no plugin runs.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// The digest of every running plugin's identity, cache key, configuration and
    /// dependency files, and assets; the zero digest when none runs.
    pub fn digest(&self) -> Digest {
        self.digest
    }

    /// The IDs of the running plugins, in order.
    pub fn ids(&self) -> &[String] {
        &self.ids
    }

    /// Passes a document's source through every plugin's source hook, in order; the
    /// text is borrowed back unchanged when no hook changes it. Called by reading
    /// threads; each session is locked while it works. Fails with the first hook's
    /// problem.
    pub fn process_source<'t>(
        &self,
        file: &SourceFile,
        language: &str,
        text: &'t str,
    ) -> Result<Cow<'t, str>, Problem> {
        let path = file.path.display().to_string();
        let format = match file.kind {
            SourceKind::Rst => SourceFormat::Rst,
            SourceKind::Markdown => SourceFormat::Markdown,
        };
        let mut current = Cow::Borrowed(text);
        for session in &self.sessions {
            let mut session = lock(session);
            let input = SourceInput::new(&path, &file.docname, format, language, &current);
            let next = session.process_source(&input).map_err(|p| host_problem(&p))?;
            if let Cow::Owned(changed) = next {
                current = Cow::Owned(changed);
            }
        }
        Ok(current)
    }

    /// Passes a complete HTML page through every plugin's HTML hook, in order. `page`
    /// is the page's path below the output folder (`guide/intro.html`), from which each
    /// plugin's assets URL (`../_static/plugins/<id>/`) is made.
    pub fn process_html<'t>(
        &self,
        page: &str,
        docname: &str,
        language: &str,
        text: &'t str,
    ) -> Result<Cow<'t, str>, Problem> {
        let mut current = Cow::Borrowed(text);
        for (session, id) in self.sessions.iter().zip(&self.ids) {
            let url = assets_url(page, id);
            let mut session = lock(session);
            let input = HtmlInput::new(page, docname, language, &current, &url);
            let next = session.process_html(&input).map_err(|p| host_problem(&p))?;
            if let Cow::Owned(changed) = next {
                current = Cow::Owned(changed);
            }
        }
        Ok(current)
    }

    /// The assets the plugins generated, each with its plugin's ID: the builder writes
    /// them to `_static/plugins/<id>/<path>`. Empty for builders other than HTML.
    pub fn assets(&self) -> impl Iterator<Item = (&str, &PluginAsset)> {
        self.ids
            .iter()
            .zip(&self.assets)
            .flat_map(|(id, list)| list.iter().map(move |a| (id.as_str(), a)))
    }
}

fn lock(session: &Mutex<PluginSession>) -> MutexGuard<'_, PluginSession> {
    // A plugin that panicked leaves a session whose state is its own business.
    session.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The URL from the page at `page` (below the output folder) to a plugin's assets.
pub fn assets_url(page: &str, id: &str) -> String {
    let depth = page.matches('/').count();
    format!("{}_static/plugins/{id}/", "../".repeat(depth))
}

fn resolve(base: &Path, path: &str) -> PathBuf {
    gd_conf::clean_path(&base.join(path))
}

fn check_id(config: &Config, id: &str, before: &[String]) -> Result<(), Problem> {
    if !gd_plugins::is_valid_id(id) {
        let hint = "Set plugins to IDs of 2 to 32 lowercase letters, digits and hyphens.";
        return Err(configuration(config, id, "This is not a plugin ID.", hint));
    }
    if before.iter().any(|b| b == id) {
        let hint = "Keep each ID only once in plugins.";
        return Err(configuration(config, id, "This plugin is selected twice.", hint));
    }
    Ok(())
}

/// The linked plugin `id`, or the machine registry's (read only then).
fn select(linked: &Registry, id: &str) -> Result<Plugin, Problem> {
    let machine = if linked.find(id).is_some() {
        PathBuf::new()
    } else {
        gd_plugins::machine_path().map_err(|p| host_problem(&p))?
    };
    gd_plugins::select_build_plugin(linked, id, &machine).map_err(|p| host_problem(&p))
}

fn configuration(config: &Config, id: &str, message: &str, hint: &str) -> Problem {
    Problem::new(
        "plugins.config",
        "BUILD PLUGIN CONFIGURATION",
        &config.config_dir.join(CONFIG_FILE).display().to_string(),
        format!("Plugin {id}: {message}"),
        hint,
        2,
    )
}

/// The codes gd-plugins gives problems a build can meet, kept as they are.
const PLUGIN_CODES: &[&str] = &[
    "plugins.cancelled",
    "plugins.config-read",
    "plugins.config-size",
    "plugins.contract",
    "plugins.failed",
    "plugins.file-type",
    "plugins.io",
    "plugins.limit",
    "plugins.machine",
    "plugins.memory",
    "plugins.missing",
    "plugins.not-registered",
    "plugins.path",
    "plugins.protocol",
    "plugins.protocol-closed",
    "plugins.registry",
    "plugins.start",
    "plugins.timeout",
    "plugins.toml",
    "plugins.unsupported",
    "plugins.version",
    "plugins.changed",
    "plugins.entry",
];

/// A gd-plugins problem as a host problem: its code kept when it is one a build can
/// meet (`plugins.failed` otherwise), its title folded into the message, its hint,
/// path and exit status kept.
pub fn host_problem(p: &gd_plugins::Problem) -> Problem {
    let code = PLUGIN_CODES.iter().find(|c| **c == p.code).copied().unwrap_or("plugins.failed");
    let message = if p.title.is_empty() {
        p.message.clone()
    } else {
        format!("{}: {}", p.title.trim_end_matches('.'), p.message)
    };
    Problem::new(code, "BUILD PLUGIN FAILED", &p.path, message, p.hint.clone(), p.exit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untrusted_builds_start_no_plugins() {
        let mut config = Config::new("/p", "/p/_build");
        let linked = Registry::default();
        let start = |c: &Config, untrusted| {
            PluginRun::start(PluginStart {
                config: c,
                linked: &linked,
                kind: BuilderKind::Html,
                untrusted,
            })
        };
        assert!(start(&config, true).is_ok_and(|r| r.is_empty()));
        config.plugins.ids = vec!["badge".into()];
        assert_eq!(start(&config, true).map(|_| ()).unwrap_err().code, "plugins.untrusted");
        config.plugins.ids = vec!["Bad ID".into()];
        assert_eq!(start(&config, false).map(|_| ()).unwrap_err().code, "plugins.config");
        assert_eq!(assets_url("guide/intro.html", "badge"), "../_static/plugins/badge/");
    }
}
