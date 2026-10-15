//! `PluginSession`: one loaded build plugin, with checked hook calls.

use std::borrow::Cow;

use crate::assets::check_assets;
use crate::build::MAX_DEPENDENCIES;
use crate::paths::is_bounded;
use crate::problem::{self, Problem};
use crate::registry::validate_build;
use crate::{
    Asset, BuildPlugin, HtmlInput, LoadContext, MAX_PATH, MAX_PLUGIN_TEXT, MAX_REQUEST, Plugin,
    PluginInfo, PluginOptions, Registry, SourceInput,
};

/// One loaded build plugin, private to one build. It owns copied options, the
/// plugin's state, and the dependency files and cache key that identify what was
/// loaded. Every hook call checks its input and the plugin's reply. Dropping the
/// session unloads the plugin (and ends its program, for an installed one).
pub struct PluginSession {
    plugin: Box<dyn BuildPlugin>,
    info: PluginInfo,
    options: PluginOptions,
    dependencies: Vec<String>,
    cache_key: String,
}

impl std::fmt::Debug for PluginSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginSession")
            .field("info", &self.info)
            .field("options", &self.options)
            .field("dependencies", &self.dependencies)
            .field("cache_key", &self.cache_key)
            .finish_non_exhaustive()
    }
}

/// Loads the build plugin `id` of `registry` with `options`: the one-call path to
/// [`PluginSession::load`]. Fails when the registry has no such build plugin.
pub fn load_plugin(
    registry: &Registry,
    id: &str,
    options: &PluginOptions,
) -> Result<PluginSession, Problem> {
    let Some(plugin) = registry.find(id) else {
        return Err(problem::contract(
            id,
            "Register this plugin, or add it to the application's Registry.",
        ));
    };
    PluginSession::load(plugin, options)
}

impl PluginSession {
    /// Starts a fresh plugin value and loads it with a copy of `options`. Fails for a
    /// publisher, invalid options, a load problem, or cancellation; a failed load is
    /// unloaded before this returns.
    pub fn load(plugin: &Plugin, options: &PluginOptions) -> Result<PluginSession, Problem> {
        plugin.validate()?;
        let Some(value) = plugin.make_build() else {
            return Err(problem::contract(
                plugin.id(),
                "Select a build plugin here; run this publisher with guidedog publish instead.",
            ));
        };
        PluginSession::load_value(value, options)
    }

    /// Loads one plugin value, checking its metadata first.
    pub(crate) fn load_value(
        mut value: Box<dyn BuildPlugin>,
        options: &PluginOptions,
    ) -> Result<PluginSession, Problem> {
        let info = value.info();
        validate_build(&info)?;
        let id = info.id.as_str();
        check_options(id, options)?;
        let mut ctx = LoadContext::new(id, options);
        value.load(&mut ctx).map_err(|p| problem::from_plugin(p, id, "load", &options.config))?;
        let (dependencies, cache_key) = (ctx.dependencies().to_vec(), ctx.cache_key().to_string());
        check_metadata(id, &dependencies, &cache_key)?;
        if options.cancel.is_cancelled() {
            return Err(problem::build_cancelled(id));
        }
        Ok(PluginSession {
            plugin: value,
            info,
            options: options.clone(),
            dependencies,
            cache_key,
        })
    }

    /// The plugin's identity and hooks, as read before load.
    pub fn info(&self) -> &PluginInfo {
        &self.info
    }

    /// The options the session was loaded with.
    pub fn options(&self) -> &PluginOptions {
        &self.options
    }

    /// The files load read; the host hashes them before reusing output.
    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    /// The fingerprint of everything else load used.
    pub fn cache_key(&self) -> &str {
        &self.cache_key
    }

    /// Runs the source hook on one primary document. Without a source hook the text
    /// is returned unchanged. Fails on input or output over 8 MiB, overlong names, a
    /// hook problem (with the plugin and stage named), or cancellation.
    pub fn process_source<'a>(
        &mut self,
        input: &SourceInput<'a>,
    ) -> Result<Cow<'a, str>, Problem> {
        let id = self.info.id.as_str();
        self.check_text(input.text)?;
        if !is_bounded(input.path, MAX_PATH)
            || !is_bounded(input.docname, MAX_PATH)
            || !is_bounded(input.language, 64)
        {
            return Err(problem::contract(
                id,
                "Pass bounded UTF-8 paths, docname, language, and format rst or markdown.",
            ));
        }
        if !self.info.hooks.source {
            return Ok(Cow::Borrowed(input.text));
        }
        let text = self.plugin.process_source(input);
        let text = text.map_err(|p| problem::from_plugin(p, id, "source", input.path))?;
        self.finish_text(text)
    }

    /// Runs the HTML hook on one complete rendered page, as
    /// [`PluginSession::process_source`] does for source text.
    pub fn process_html<'a>(&mut self, input: &HtmlInput<'a>) -> Result<Cow<'a, str>, Problem> {
        let id = self.info.id.as_str();
        self.check_text(input.text)?;
        if !is_bounded(input.path, MAX_PATH)
            || !is_bounded(input.docname, MAX_PATH)
            || !is_bounded(input.language, 64)
            || !is_bounded(input.assets_url, MAX_PATH)
        {
            return Err(problem::contract(
                id,
                "Pass bounded UTF-8 paths, docname, language, and the relative assets_url.",
            ));
        }
        if !self.info.hooks.html {
            return Ok(Cow::Borrowed(input.text));
        }
        let text = self.plugin.process_html(input);
        let text = text.map_err(|p| problem::from_plugin(p, id, "html", input.path))?;
        self.finish_text(text)
    }

    /// Collects the plugin's generated files, checked: at most 256 files and 16 MiB,
    /// portable relative paths, no duplicates ignoring case, no file used as a folder.
    /// The host stages them under `_static/plugins/ID/`.
    pub fn collect_assets(&mut self) -> Result<Vec<Asset>, Problem> {
        let id = self.info.id.as_str();
        if self.options.cancel.is_cancelled() {
            return Err(problem::build_cancelled(id));
        }
        if !self.info.hooks.assets {
            return Ok(Vec::new());
        }
        let assets = self.plugin.collect_assets();
        let assets =
            assets.map_err(|p| problem::from_plugin(p, id, "assets", &self.options.config))?;
        check_assets(id, &assets)?;
        Ok(assets)
    }

    fn check_text(&self, text: &str) -> Result<(), Problem> {
        if text.len() > MAX_PLUGIN_TEXT {
            return Err(problem::contract(
                &self.info.id,
                "Pass valid UTF-8 text no larger than MAX_PLUGIN_TEXT (8 MiB).",
            ));
        }
        if self.options.cancel.is_cancelled() {
            return Err(problem::build_cancelled(&self.info.id));
        }
        Ok(())
    }

    fn finish_text<'a>(&self, text: Cow<'a, str>) -> Result<Cow<'a, str>, Problem> {
        if self.options.cancel.is_cancelled() {
            return Err(problem::build_cancelled(&self.info.id));
        }
        if text.len() > MAX_PLUGIN_TEXT {
            return Err(problem::contract(
                &self.info.id,
                "Return valid UTF-8 text no larger than MAX_PLUGIN_TEXT (8 MiB).",
            ));
        }
        Ok(text)
    }
}

fn check_options(id: &str, o: &PluginOptions) -> Result<(), Problem> {
    if !is_bounded(&o.config, MAX_PATH)
        || !is_bounded(&o.project, MAX_PATH)
        || !is_bounded(&o.builder, 64)
        || !is_bounded(&o.language, 64)
    {
        return Err(problem::contract(
            id,
            "Pass UTF-8 paths up to MAX_PATH and builder/language names up to 64 bytes; omit NUL.",
        ));
    }
    if o.cancel.is_cancelled() {
        return Err(problem::build_cancelled(id));
    }
    Ok(())
}

/// Checks what load declared: at most 256 bounded dependency paths and a cache key of
/// at most 1 MiB.
pub(crate) fn check_metadata(id: &str, dependencies: &[String], key: &str) -> Result<(), Problem> {
    if dependencies.len() > MAX_DEPENDENCIES {
        return Err(problem::contract(
            id,
            "Declare at most 256 dependency files in the load context.",
        ));
    }
    if key.len() > MAX_REQUEST {
        return Err(problem::contract(id, "Return a UTF-8 cache_key of at most 1 MiB."));
    }
    if dependencies.iter().any(|p| p.is_empty() || !is_bounded(p, MAX_PATH)) {
        return Err(problem::contract(
            id,
            "Declare nonempty UTF-8 dependency paths of at most MAX_PATH, without NUL bytes.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
