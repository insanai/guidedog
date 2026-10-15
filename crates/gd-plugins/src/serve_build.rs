//! Serving a build plugin over the `guidedog-build-plugin` protocol, and the
//! one-call entry point for a plugin program's `main`.

use std::io::{BufRead, BufReader, Write};
use std::process::ExitCode;

use gd_json::{Object, Value, base64};

use crate::paths::is_bounded;
use crate::problem::{self, Problem};
use crate::wire::{self, Handshake, object};
use crate::{
    BUILD_PROTOCOL_NAME, BUILD_PROTOCOL_VERSION, BuildPlugin, HtmlInput, MAX_PATH, MAX_RESPONSE,
    PLUGIN_API_VERSION, PROTOCOL_FLAG, Plugin, PluginOptions, PluginSession, SourceFormat,
    SourceInput,
};

/// The whole `main` of a build-plugin program: when started with [`PROTOCOL_FLAG`], it
/// serves `plugin` over standard input and output until the host unloads it, and
/// returns the program's exit status; otherwise it writes a one-line usage note to
/// standard error and returns 2. Keep protocol output on stdout: log to stderr only.
///
/// ```no_run
/// # #[derive(Clone)] struct Badge;
/// # impl gd_plugins::BuildPlugin for Badge {
/// #     fn info(&self) -> gd_plugins::PluginInfo { unimplemented!() }
/// # }
/// fn main() -> std::process::ExitCode {
///     gd_plugins::serve(Badge)
/// }
/// ```
pub fn serve<P: BuildPlugin + 'static>(plugin: P) -> ExitCode {
    if !started_by_host() {
        let id = plugin.info().id;
        eprintln!("Run this plugin through guidedog build with plugins = [\"{id}\"].");
        return ExitCode::from(2);
    }
    let input = BufReader::new(std::io::stdin().lock());
    let code = serve_streams(plugin, input, std::io::stdout().lock());
    ExitCode::from(code)
}

/// The whole `main` of a plugin crate's program, serving the same [`Plugin`] a custom
/// host links: build hooks as [`serve`] does, a publisher as
/// [`crate::serve_publisher`] does. One implementation, two deployments:
///
/// ```no_run
/// # mod badge { pub fn plugin() -> gd_plugins::Plugin { unimplemented!() } }
/// // In the plugin crate's src/main.rs; its lib.rs exposes `pub fn plugin() -> Plugin`.
/// fn main() -> std::process::ExitCode {
///     gd_plugins::serve_plugin(&badge::plugin())
/// }
/// ```
pub fn serve_plugin(plugin: &Plugin) -> ExitCode {
    if let Some(build) = plugin.make_build() {
        return serve(build);
    }
    match plugin.make_publisher() {
        Some(publisher) => crate::serve_publisher(publisher),
        None => ExitCode::from(2),
    }
}

/// Whether the program was started with exactly [`PROTOCOL_FLAG`].
pub(crate) fn started_by_host() -> bool {
    let mut args = std::env::args_os().skip(1);
    args.next().is_some_and(|a| a == PROTOCOL_FLAG) && args.next().is_none()
}

/// Serves `plugin` over `input` and `output`: writes the handshake, then answers the
/// host's requests until it unloads the plugin or closes `input`. Returns the exit
/// status: 0 after unload or a closed input, 1 when the host broke the protocol or
/// output failed, 2 for invalid plugin metadata.
pub fn serve_streams<P: BuildPlugin + 'static>(
    plugin: P,
    mut input: impl BufRead,
    output: impl Write,
) -> u8 {
    let info = plugin.info();
    if crate::registry::validate_build(&info).is_err() {
        return 2;
    }
    let handshake = Handshake {
        protocol: BUILD_PROTOCOL_NAME.to_string(),
        versions: vec![BUILD_PROTOCOL_VERSION],
        api: PLUGIN_API_VERSION,
        id: info.id.clone(),
        name: info.name.clone(),
        version: info.version.clone(),
        hooks: info.hooks.names().map(str::to_string).collect(),
        ..Handshake::default()
    };
    let mut server = Server {
        id: info.id,
        hooks: info.hooks,
        plugin: Some(Box::new(plugin)),
        session: None,
        output,
        selected: false,
        broken: false,
    };
    server.send(&handshake.to_json());
    let mut last_id = 0;
    while !server.broken {
        let line = match wire::read_line(&mut input, MAX_RESPONSE) {
            Ok(line) => line,
            Err(p) => return end_status(&p),
        };
        let Some((id, operation, o)) = request_of(&line).filter(|(id, ..)| *id > last_id) else {
            return 1;
        };
        last_id = id;
        if !server.answer(id, &operation, &o) {
            return u8::from(server.broken);
        }
    }
    1
}

/// 0 when the host closed the input (it is done with the plugin), 1 otherwise.
pub(crate) fn end_status(p: &Problem) -> u8 {
    u8::from(p.code != wire::closed().code)
}

/// A request's ID, operation and object; `None` for a line that is not one.
pub(crate) fn request_of(line: &str) -> Option<(i64, String, Object)> {
    let o = wire::parse_message(line).ok()?;
    let id = wire::int_field(&o, "id").filter(|&id| id > 0)?;
    let operation = wire::str_field(&o, "operation")?.to_string();
    Some((id, operation, o))
}

// Correction hints of the build server's refusals.
const SELECT_FIRST: &str =
    "Select BUILD_PROTOCOL_VERSION before loading the plugin or invoking hooks.";
const LOAD_FIRST: &str = "Load the plugin before invoking source, html, or assets.";
const RELOAD: &str =
    "Unload the current session, and start the program again to load another configuration.";
const SOURCE_FIELDS: &str =
    "Send path, docname, format (rst or markdown), language and text to a declared hook.";
const HTML_FIELDS: &str =
    "Send a declared HTML hook's path, docname, language and complete page text.";
const ASSETS_HOOK: &str = "Use the assets operation only when the plugin declares an assets hook.";
const OPERATIONS: &str =
    "Use select, load, source, html, assets, or unload in the build protocol.";
const OPTIONS: &str =
    "Send configuration and project paths, builder and language as bounded strings.";

struct Server<W: Write> {
    id: String,
    hooks: crate::Hooks,
    plugin: Option<Box<dyn BuildPlugin>>,
    session: Option<PluginSession>,
    output: W,
    selected: bool,
    broken: bool,
}

impl<W: Write> Server<W> {
    /// Answers one request; false once the host has unloaded the plugin.
    fn answer(&mut self, id: i64, op: &str, o: &Object) -> bool {
        if !self.selected && op != "select" {
            self.refuse_with(id, SELECT_FIRST);
            return true;
        }
        if !matches!(op, "select" | "load" | "unload") && self.session.is_none() {
            self.refuse_with(id, LOAD_FIRST);
            return true;
        }
        match op {
            "select" => self.select(id, o),
            "load" => self.load(id, o),
            "source" => self.source(id, o),
            "html" => self.html(id, o),
            "assets" => self.assets(id),
            "unload" => {
                self.session = None;
                self.send(&wire::ok_reply(id, []));
                return false;
            }
            _ => self.refuse_with(id, OPERATIONS),
        }
        true
    }

    fn select(&mut self, id: i64, o: &Object) {
        let version = o.get("version").and_then(wire::version_of);
        if version != Some(BUILD_PROTOCOL_VERSION) || self.selected {
            self.refuse_with(id, "Select build protocol 1.0 exactly once per session.");
            return;
        }
        self.selected = true;
        self.send(&wire::ok_reply(id, []));
    }

    fn load(&mut self, id: i64, o: &Object) {
        let Some(plugin) = self.plugin.take().filter(|_| self.session.is_none()) else {
            self.refuse_with(id, RELOAD);
            return;
        };
        let Some(options) = options_of(o) else {
            self.plugin = Some(plugin);
            self.refuse_with(id, OPTIONS);
            return;
        };
        match PluginSession::load_value(plugin, &options) {
            Ok(session) => {
                let deps = session.dependencies().iter().map(|d| d.as_str().into()).collect();
                let reply = wire::ok_reply(
                    id,
                    [
                        ("cache_key", session.cache_key().into()),
                        ("dependencies", Value::Array(deps)),
                    ],
                );
                self.session = Some(session);
                self.send(&reply);
            }
            Err(p) => self.refuse(id, &p),
        }
    }

    fn source(&mut self, id: i64, o: &Object) {
        let input = wire::obj_field(o, "input");
        let field = |key| input.and_then(|i| wire::str_field(i, key));
        let format = field("format").and_then(SourceFormat::from_name);
        let (Some(path), Some(docname), Some(format), Some(language), Some(text)) =
            (field("path"), field("docname"), format, field("language"), field("text"))
        else {
            return self.refuse_with(id, SOURCE_FIELDS);
        };
        if !self.hooks.source {
            return self.refuse_with(id, SOURCE_FIELDS);
        }
        let input = SourceInput { path, docname, format, language, text };
        let result =
            self.session.as_mut().map(|s| s.process_source(&input).map(|t| t.into_owned()));
        self.send_text(id, result);
    }

    fn html(&mut self, id: i64, o: &Object) {
        let input = wire::obj_field(o, "input");
        let field = |key| input.and_then(|i| wire::str_field(i, key));
        let (Some(path), Some(docname), Some(language), Some(text), Some(assets_url)) = (
            field("path"),
            field("docname"),
            field("language"),
            field("text"),
            field("assets_url"),
        ) else {
            return self.refuse_with(id, HTML_FIELDS);
        };
        if !self.hooks.html {
            return self.refuse_with(id, HTML_FIELDS);
        }
        let input = HtmlInput { path, docname, language, text, assets_url };
        let result = self.session.as_mut().map(|s| s.process_html(&input).map(|t| t.into_owned()));
        self.send_text(id, result);
    }

    fn send_text(&mut self, id: i64, result: Option<Result<String, Problem>>) {
        match result {
            Some(Ok(text)) => self.send(&wire::ok_reply(id, [("text", text.into())])),
            Some(Err(p)) => self.refuse(id, &p),
            None => self.refuse_with(id, LOAD_FIRST),
        }
    }

    fn assets(&mut self, id: i64) {
        if !self.hooks.assets {
            return self.refuse_with(id, ASSETS_HOOK);
        }
        let Some(session) = self.session.as_mut() else {
            return self.refuse_with(id, LOAD_FIRST);
        };
        match session.collect_assets() {
            Ok(assets) => {
                let wire_assets = assets.iter().map(|a| {
                    let mut data = String::new();
                    base64::encode_into(&mut data, &a.data);
                    object([("path", a.path.as_str().into()), ("base64", data.into())])
                });
                let reply = wire::ok_reply(id, [("assets", Value::Array(wire_assets.collect()))]);
                self.send(&reply);
            }
            Err(p) => self.refuse(id, &p),
        }
    }

    fn refuse_with(&mut self, id: i64, hint: &str) {
        let p = problem::contract(&self.id, hint);
        self.refuse(id, &p);
    }

    fn refuse(&mut self, id: i64, p: &Problem) {
        self.send(&wire::refusal(id, p));
    }

    fn send(&mut self, value: &Value) {
        let line = wire::message_line(value);
        let written = self.output.write_all(line.as_bytes()).and_then(|()| self.output.flush());
        if written.is_err() {
            self.broken = true;
        }
    }
}

/// The session options a `load` request carries, each a bounded string.
fn options_of(o: &Object) -> Option<PluginOptions> {
    let options = wire::obj_field(o, "options")?;
    let field = |key, limit| {
        wire::str_field(options, key).filter(|s| is_bounded(s, limit)).map(str::to_string)
    };
    Some(PluginOptions {
        config: field("config", MAX_PATH)?,
        project: field("project", MAX_PATH)?,
        builder: field("builder", 64)?,
        language: field("language", 64)?,
        ..PluginOptions::default()
    })
}
