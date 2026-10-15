//! The build-plugin interface (GDS 0010): the `BuildPlugin` trait, its inputs and the
//! load context that reads configuration and records cache identity.

use std::borrow::Cow;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

use crate::paths::is_bounded;
use crate::problem::{self, Problem};
use crate::{Cancel, MAX_PATH, MAX_PLUGIN_TEXT, MAX_REQUEST, PLUGIN_API_VERSION, Version};

/// The most dependency files one load may declare.
pub(crate) const MAX_DEPENDENCIES: usize = 256;

/// Build hooks: transformations of source text and complete HTML pages, and generated
/// static files. Every hook has a default meaning "service absent" (text passes through
/// unchanged; no assets); [`PluginInfo::hooks`] declares which ones a plugin offers.
///
/// A plugin value is one session's state: [`BuildPlugin::load`] prepares it, the hooks
/// read it, and dropping the value unloads it (close files or children in `Drop`).
/// Hooks run one at a time; a plugin needs no synchronization of its own. Linked
/// plugins are trusted code: a panic or a hang in a hook reaches the host. Run an
/// untrusted plugin as a program instead ([`crate::process_plugin`]).
pub trait BuildPlugin: Send {
    /// What the plugin is and which hooks it offers. It must not change over the value's
    /// life; the SDK reads it once, before load.
    fn info(&self) -> PluginInfo;

    /// Prepares the session: reads configuration through `ctx` (which fingerprints what
    /// it reads) and keeps what the hooks need in `self`. The default does nothing.
    fn load(&mut self, ctx: &mut LoadContext<'_>) -> Result<(), Problem> {
        let _ = ctx;
        Ok(())
    }

    /// Returns the complete replacement of a primary source document, before parsing.
    /// Borrow `input.text` when nothing changes. The default returns it unchanged.
    fn process_source<'a>(&mut self, input: &SourceInput<'a>) -> Result<Cow<'a, str>, Problem> {
        Ok(Cow::Borrowed(input.text))
    }

    /// Returns the complete replacement of a rendered HTML page, before staging. The
    /// default returns it unchanged.
    fn process_html<'a>(&mut self, input: &HtmlInput<'a>) -> Result<Cow<'a, str>, Problem> {
        Ok(Cow::Borrowed(input.text))
    }

    /// Returns generated files, staged under `_static/plugins/ID/`. Paths are relative
    /// and portable; the same loaded settings must give the same files in the same
    /// order. The default returns none.
    fn collect_assets(&mut self) -> Result<Vec<Asset>, Problem> {
        Ok(Vec::new())
    }
}

/// A boxed plugin is a plugin, so registries and servers can hold any kind. Every hook
/// forwards; a new hook must be forwarded here too.
impl BuildPlugin for Box<dyn BuildPlugin> {
    fn info(&self) -> PluginInfo {
        (**self).info()
    }

    fn load(&mut self, ctx: &mut LoadContext<'_>) -> Result<(), Problem> {
        (**self).load(ctx)
    }

    fn process_source<'a>(&mut self, input: &SourceInput<'a>) -> Result<Cow<'a, str>, Problem> {
        (**self).process_source(input)
    }

    fn process_html<'a>(&mut self, input: &HtmlInput<'a>) -> Result<Cow<'a, str>, Problem> {
        (**self).process_html(input)
    }

    fn collect_assets(&mut self) -> Result<Vec<Asset>, Problem> {
        (**self).collect_assets()
    }
}

/// Which build hooks a plugin offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Hooks {
    /// [`BuildPlugin::process_source`].
    pub source: bool,
    /// [`BuildPlugin::process_html`].
    pub html: bool,
    /// [`BuildPlugin::collect_assets`].
    pub assets: bool,
}

impl Hooks {
    /// No hooks.
    pub const NONE: Hooks = Hooks { source: false, html: false, assets: false };
    /// Every hook.
    pub const ALL: Hooks = Hooks { source: true, html: true, assets: true };

    /// Whether at least one hook is offered.
    pub fn is_any(self) -> bool {
        self.source || self.html || self.assets
    }

    /// The protocol names of the offered hooks, in the order source, html, assets.
    pub fn names(self) -> impl Iterator<Item = &'static str> {
        [(self.source, "source"), (self.html, "html"), (self.assets, "assets")]
            .into_iter()
            .filter_map(|(on, name)| on.then_some(name))
    }
}

/// A build plugin's identity: an ID (see [`crate::is_valid_id`]), a display name of
/// at most 256 bytes, a product version of at most 64 bytes, the interface version it
/// was built against, and its hooks.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PluginInfo {
    /// The ID a project selects in `conf.toml`.
    pub id: String,
    /// The name people see.
    pub name: String,
    /// The plugin's own release version.
    pub version: String,
    /// The build-plugin interface version, normally [`PLUGIN_API_VERSION`].
    pub api: Version,
    /// The hooks it offers.
    pub hooks: Hooks,
}

impl PluginInfo {
    /// Describes a plugin built against [`PLUGIN_API_VERSION`].
    pub fn new(id: &str, name: &str, version: &str, hooks: Hooks) -> PluginInfo {
        PluginInfo {
            id: id.to_string(),
            name: name.to_string(),
            version: version.to_string(),
            api: PLUGIN_API_VERSION,
            hooks,
        }
    }
}

/// The markup of a source document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum SourceFormat {
    /// reStructuredText.
    #[default]
    Rst,
    /// CommonMark or MyST Markdown.
    Markdown,
}

impl SourceFormat {
    /// The protocol name: `"rst"` or `"markdown"`.
    pub fn name(self) -> &'static str {
        match self {
            SourceFormat::Rst => "rst",
            SourceFormat::Markdown => "markdown",
        }
    }

    /// The format the protocol calls `name`.
    pub fn from_name(name: &str) -> Option<SourceFormat> {
        match name {
            "rst" => Some(SourceFormat::Rst),
            "markdown" => Some(SourceFormat::Markdown),
            _ => None,
        }
    }
}

/// One complete primary document before parsing, borrowed for the call. Includes are
/// read by the engine later and do not pass through a source hook.
///
/// Hosts make it; it is `#[non_exhaustive]` so later fields are not a breaking change.
/// Use the constructor, or `Default` and field assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct SourceInput<'a> {
    /// The document's file path.
    pub path: &'a str,
    /// The document's name in the project, such as `guide/install`.
    pub docname: &'a str,
    /// Its markup.
    pub format: SourceFormat,
    /// The build's language, such as `en`.
    pub language: &'a str,
    /// The complete text.
    pub text: &'a str,
}

/// One complete rendered HTML page before staging, borrowed for the call.
///
/// Hosts make it; it is `#[non_exhaustive]` so later fields are not a breaking change.
/// Use the constructor, or `Default` and field assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct HtmlInput<'a> {
    /// The output page's path.
    pub path: &'a str,
    /// The document's name in the project.
    pub docname: &'a str,
    /// The build's language.
    pub language: &'a str,
    /// The complete page.
    pub text: &'a str,
    /// The URL of this plugin's assets relative to the page, ending in `/`.
    pub assets_url: &'a str,
}

impl<'a> SourceInput<'a> {
    /// Describes one primary document.
    pub fn new(
        path: &'a str,
        docname: &'a str,
        format: SourceFormat,
        language: &'a str,
        text: &'a str,
    ) -> SourceInput<'a> {
        SourceInput { path, docname, format, language, text }
    }
}

impl<'a> HtmlInput<'a> {
    /// Describes one rendered page; `assets_url` is relative to it and ends in `/`.
    pub fn new(
        path: &'a str,
        docname: &'a str,
        language: &'a str,
        text: &'a str,
        assets_url: &'a str,
    ) -> HtmlInput<'a> {
        HtmlInput { path, docname, language, text, assets_url }
    }
}

/// One generated file: a relative, portable path such as `css/badge.css`, and bytes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Asset {
    /// The path below `_static/plugins/ID/`.
    pub path: String,
    /// The file's contents.
    pub data: Vec<u8>,
}

impl Asset {
    /// Makes an asset from a path and bytes.
    pub fn new(path: &str, data: impl Into<Vec<u8>>) -> Asset {
        Asset { path: path.to_string(), data: data.into() }
    }
}

/// A build session's settings. Paths are at most [`MAX_PATH`] bytes; builder and
/// language names at most 64.
///
/// Hosts make it; it is `#[non_exhaustive]` so later fields are not a breaking change.
/// Use the constructor, or `Default` and field assignment.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct PluginOptions {
    /// The plugin's configuration file, from `[plugin_configs]`; may be empty.
    pub config: String,
    /// The project folder; relative input paths resolve from it.
    pub project: String,
    /// The builder, such as `html`.
    pub builder: String,
    /// The build's language.
    pub language: String,
    /// Bounds each whole request to an installed program; zero means one minute.
    /// Linked plugins have no forced deadline.
    pub request_timeout: Duration,
    /// Stops the build between and during requests.
    pub cancel: Cancel,
}

impl PluginOptions {
    /// Options for one build: the plugin's configuration file, the project folder, the
    /// builder and the language; the default timeout and no cancellation.
    pub fn new(config: &str, project: &str, builder: &str, language: &str) -> PluginOptions {
        PluginOptions {
            config: config.to_string(),
            project: project.to_string(),
            builder: builder.to_string(),
            language: language.to_string(),
            ..PluginOptions::default()
        }
    }
}

/// What [`BuildPlugin::load`] gets: the session's options, and the dependency files
/// and cache key that identify what it loaded. Both only grow: helpers append the
/// fingerprints of the bytes they read, and nothing can erase them.
#[derive(Debug)]
pub struct LoadContext<'a> {
    id: &'a str,
    options: &'a PluginOptions,
    dependencies: Vec<String>,
    cache_key: String,
}

impl<'a> LoadContext<'a> {
    pub(crate) fn new(id: &'a str, options: &'a PluginOptions) -> LoadContext<'a> {
        LoadContext { id, options, dependencies: Vec::new(), cache_key: String::new() }
    }

    /// The session's options.
    pub fn options(&self) -> &PluginOptions {
        self.options
    }

    /// The dependency files declared so far.
    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    /// The cache key so far.
    pub fn cache_key(&self) -> &str {
        &self.cache_key
    }

    /// Appends `text` to the cache key: a fingerprint of anything loaded that is not a
    /// declared file, such as a selected environment value.
    pub fn add_cache_key(&mut self, text: &str) {
        self.cache_key.push_str(text);
    }

    /// Declares a file read by other means; the host hashes it before reusing output.
    /// Fails beyond 256 files or for an empty, overlong or NUL-holding path.
    pub fn declare_dependency(&mut self, path: &str) -> Result<(), Problem> {
        self.check_dependency(path)?;
        self.dependencies.push(path.to_string());
        Ok(())
    }

    fn check_dependency(&self, path: &str) -> Result<(), Problem> {
        if self.dependencies.len() >= MAX_DEPENDENCIES {
            return Err(problem::contract(
                self.id,
                "Read at most 256 load dependency files; combine small inputs when practical.",
            ));
        }
        if path.is_empty() || !is_bounded(path, MAX_PATH) {
            return Err(problem::contract(
                self.id,
                "Pass a nonempty UTF-8 file path of at most MAX_PATH, without NUL bytes.",
            ));
        }
        Ok(())
    }

    /// Reads the configuration file (`options.config`, at most 1 MiB of UTF-8),
    /// declares it and adds its fingerprint to the cache key.
    pub fn read_config(&mut self) -> Result<String, Problem> {
        if self.options.config.is_empty() {
            return Err(problem::contract(
                self.id,
                "Set this plugin's configuration path in [plugin_configs] in conf.toml.",
            ));
        }
        let path = self.options.config.clone();
        self.read_bounded(&path, MAX_REQUEST)
    }

    /// Reads a UTF-8 load input of at most 8 MiB, declares it and adds its fingerprint
    /// to the cache key. A relative path resolves from `options.project`. Fingerprinting
    /// the bytes actually read keeps old state from pairing with a newer file.
    pub fn read_file(&mut self, path: &str) -> Result<String, Problem> {
        self.read_bounded(path, MAX_PLUGIN_TEXT)
    }

    fn read_bounded(&mut self, name: &str, limit: usize) -> Result<String, Problem> {
        self.check_dependency(name)?;
        let path = if Path::new(name).is_absolute() || self.options.project.is_empty() {
            name.to_string()
        } else {
            Path::new(&self.options.project).join(name).to_string_lossy().into_owned()
        };
        let data = read_limited(&path, limit)?;
        let text = String::from_utf8(data).map_err(|_| {
            problem::contract(
                self.id,
                "Save this plugin input as UTF-8 text before reading it with read_file.",
            )
        })?;
        self.declare_dependency(&path)?;
        self.cache_key.push(':');
        gd_sha::write_hex(&mut self.cache_key, &gd_sha::sha256(text.as_bytes()));
        Ok(text)
    }
}

fn read_limited(path: &str, limit: usize) -> Result<Vec<u8>, Problem> {
    let unreadable = |err: std::io::Error, title: &str, hint: &str| {
        Problem::new("plugins.config-read", title, &err.to_string()).hint(hint).path(path).exit(4)
    };
    let file = std::fs::File::open(path).map_err(|e| {
        unreadable(
            e,
            "Cannot read a plugin input",
            "Create this file, or correct its path in the plugin settings.",
        )
    })?;
    let mut data = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut data).map_err(|e| {
        unreadable(
            e,
            "Cannot finish reading a plugin input",
            "Check this file's read permissions, then run the build again.",
        )
    })?;
    if data.len() > limit {
        return Err(Problem::new(
            "plugins.config-size",
            "The plugin input is too large",
            "This input exceeds the bounded text reader's limit.",
        )
        .hint("Keep configuration below 1 MiB and other load text files below 8 MiB.")
        .path(path)
        .exit(2));
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, text: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("gd-plugins-load-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn reads_fingerprint_and_declare_their_files() {
        let path = temp_file("badge.toml", "product = \"Orbit\"\n");
        let options = PluginOptions {
            config: path.to_string_lossy().into_owned(),
            ..PluginOptions::default()
        };
        let mut ctx = LoadContext::new("badge", &options);
        ctx.add_cache_key("env");
        assert_eq!(ctx.read_config().unwrap(), "product = \"Orbit\"\n");
        assert_eq!(ctx.dependencies().len(), 1);
        assert!(ctx.cache_key().starts_with("env:"));
        assert_eq!(ctx.cache_key().len(), 4 + 64);
        let missing = ctx.read_file("/nonexistent/gd-plugins/x").unwrap_err();
        assert_eq!(missing.code, "plugins.config-read");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_missing_configuration_path_is_explained() {
        let options = PluginOptions::default();
        let mut ctx = LoadContext::new("badge", &options);
        let p = ctx.read_config().unwrap_err();
        assert!(p.hint.contains("[plugin_configs]"));
        assert_eq!(Hooks::ALL.names().collect::<Vec<_>>(), ["source", "html", "assets"]);
    }
}
