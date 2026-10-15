//! The build-plugin process adapter: a `BuildPlugin` that runs an installed program
//! and speaks `guidedog-build-plugin` 1.0 with it, one program per session.

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use gd_json::{Object, Value, base64};

use crate::child::{Child, ChildKind};
use crate::paths::is_bounded;
use crate::problem::{self, Problem};
use crate::process_publish::{HANDSHAKE_TIMEOUT, unload_child};
use crate::wire::{self, Handshake, object};
use crate::{
    Asset, BUILD_PROTOCOL_NAME, BuildPlugin, Hooks, HtmlInput, LoadContext, MAX_PATH,
    MAX_PLUGIN_ASSET_BYTES, MAX_PLUGIN_ASSETS, MAX_PLUGIN_TEXT, MachineEntry, Plugin, PluginInfo,
    SourceInput, Version, select_version,
};

/// Bounds a build-plugin request when the options leave it zero.
const DEFAULT_BUILD_TIMEOUT: Duration = Duration::from_secs(60);

/// A build plugin that runs the registered program `entry`, one program a session:
/// load starts it and checks its identity and hooks against the entry, dropping the
/// session ends it. Verify the entry ([`crate::verify_entry`]) before offering it.
/// Fails for a publisher's entry or undeclared hook names.
pub fn process_plugin(entry: &MachineEntry) -> Result<Plugin, Problem> {
    let handshake = Handshake {
        protocol: entry.protocol_name.clone(),
        id: entry.id.clone(),
        name: entry.name.clone(),
        version: entry.version.clone(),
        api: crate::PLUGIN_API_VERSION,
        hooks: entry.hooks.clone(),
        ..Handshake::default()
    };
    Ok(build_plugin_of(&handshake, &entry.path)?.with_program_sha256(&entry.sha256))
}

/// The plugin a build program's handshake describes, running `executable`.
pub(crate) fn build_plugin_of(h: &Handshake, executable: &str) -> Result<Plugin, Problem> {
    if !h.is_build() {
        return Err(problem::contract(
            &h.id,
            "Choose a build plugin; this program is a publisher for guidedog publish.",
        ));
    }
    let hooks = hooks_of(&h.hooks, &h.id)?;
    let info = PluginInfo {
        id: h.id.clone(),
        name: h.name.clone(),
        version: h.version.clone(),
        api: h.api,
        hooks,
    };
    let (prototype, program) = (info.clone(), executable.to_string());
    let make = Arc::new(move || -> Box<dyn BuildPlugin> {
        Box::new(ProcessPlugin {
            info: prototype.clone(),
            executable: program.clone(),
            child: None,
        })
    });
    let plugin = Plugin::from_build_maker(info, executable, make);
    plugin.validate()?;
    Ok(plugin)
}

/// The hooks `names` declares; refuses a repeated or unknown name.
fn hooks_of(names: &[String], id: &str) -> Result<Hooks, Problem> {
    let mut hooks = Hooks::NONE;
    for name in names {
        let slot = match name.as_str() {
            "source" => &mut hooks.source,
            "html" => &mut hooks.html,
            "assets" => &mut hooks.assets,
            _ => {
                return Err(problem::contract(
                    id,
                    "Use supported hook names: source, html, and assets; update Guidedog for \
                     newer hooks.",
                ));
            }
        };
        if *slot {
            return Err(problem::contract(
                id,
                "Declare every hook once in the handshake; remove the repeated name.",
            ));
        }
        *slot = true;
    }
    Ok(hooks)
}

struct ProcessPlugin {
    info: PluginInfo,
    executable: String,
    child: Option<Child>,
}

impl ProcessPlugin {
    fn exchange(&mut self, request: Value) -> Result<Object, Problem> {
        match self.child.as_mut() {
            Some(child) => child.exchange(request),
            None => Err(problem::protocol("The build plugin program is not loaded.")),
        }
    }

    fn next_id(&mut self) -> i64 {
        self.child.as_mut().map_or(0, Child::next_id)
    }

    fn check_handshake(&self, h: &Handshake) -> Result<Version, Problem> {
        let id = &self.info.id;
        if h.id != self.info.id
            || h.version != self.info.version
            || h.protocol != BUILD_PROTOCOL_NAME
        {
            return Err(problem::contract(
                id,
                "Inspect the executable and register it again if its identity changed \
                 intentionally.",
            ));
        }
        if hooks_of(&h.hooks, id)? != self.info.hooks {
            return Err(problem::contract(
                id,
                "Register the executable again: its hook declarations changed.",
            ));
        }
        select_version(h).ok_or_else(|| {
            problem::contract(
                id,
                "Install a build plugin that supports BUILD_PROTOCOL_VERSION 1.0.",
            )
        })
    }

    fn text_reply(&self, o: &Object, stage: &str) -> Result<String, Problem> {
        match wire::str_field(o, "text") {
            Some(text) => Ok(text.to_string()),
            None => Err(problem::contract(
                &self.info.id,
                &format!("Return a string text field for every successful {stage} response."),
            )),
        }
    }
}

impl BuildPlugin for ProcessPlugin {
    fn info(&self) -> PluginInfo {
        self.info.clone()
    }

    fn load(&mut self, ctx: &mut LoadContext<'_>) -> Result<(), Problem> {
        let o = ctx.options();
        let timeout =
            if o.request_timeout.is_zero() { DEFAULT_BUILD_TIMEOUT } else { o.request_timeout };
        let child = Child::start(&self.executable, ChildKind::Build, timeout, o.cancel.clone())?;
        let handshake = self.child.insert(child).handshake(HANDSHAKE_TIMEOUT)?;
        let version = self.check_handshake(&handshake)?;
        let id = self.next_id();
        self.exchange(wire::select_request(id, version))?;
        let options = object([
            ("config", o.config.as_str().into()),
            ("project", o.project.as_str().into()),
            ("builder", o.builder.as_str().into()),
            ("language", o.language.as_str().into()),
        ]);
        let id = self.next_id();
        let reply = self.exchange(object([
            ("id", id.into()),
            ("operation", "load".into()),
            ("options", options),
        ]))?;
        load_metadata(&self.info.id, &reply, ctx)
    }

    fn process_source<'a>(&mut self, input: &SourceInput<'a>) -> Result<Cow<'a, str>, Problem> {
        let wire_input = object([
            ("path", input.path.into()),
            ("docname", input.docname.into()),
            ("format", input.format.name().into()),
            ("language", input.language.into()),
            ("text", input.text.into()),
        ]);
        let id = self.next_id();
        let o = self.exchange(object([
            ("id", id.into()),
            ("operation", "source".into()),
            ("input", wire_input),
        ]))?;
        Ok(Cow::Owned(self.text_reply(&o, "source")?))
    }

    fn process_html<'a>(&mut self, input: &HtmlInput<'a>) -> Result<Cow<'a, str>, Problem> {
        let wire_input = object([
            ("path", input.path.into()),
            ("docname", input.docname.into()),
            ("language", input.language.into()),
            ("text", input.text.into()),
            ("assets_url", input.assets_url.into()),
        ]);
        let id = self.next_id();
        let o = self.exchange(object([
            ("id", id.into()),
            ("operation", "html".into()),
            ("input", wire_input),
        ]))?;
        Ok(Cow::Owned(self.text_reply(&o, "HTML")?))
    }

    fn collect_assets(&mut self) -> Result<Vec<Asset>, Problem> {
        let id = self.next_id();
        let o = self.exchange(object([("id", id.into()), ("operation", "assets".into())]))?;
        assets_of(&self.info.id, &o)
    }
}

impl Drop for ProcessPlugin {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            unload_child(child);
        }
    }
}

/// Records the dependencies and cache key a program's load reply declares.
fn load_metadata(id: &str, reply: &Object, ctx: &mut LoadContext<'_>) -> Result<(), Problem> {
    let items = wire::arr_field(reply, "dependencies");
    let key = wire::str_field(reply, "cache_key");
    let (Some(items), Some(key)) = (items, key) else {
        return Err(bad_metadata(id));
    };
    if items.len() > 256 || key.len() > MAX_PLUGIN_TEXT {
        return Err(bad_metadata(id));
    }
    for item in items {
        let path = item.as_str().filter(|p| is_bounded(p, MAX_PATH));
        let Some(path) = path else {
            return Err(problem::contract(id, "Declare dependency files as valid bounded paths."));
        };
        ctx.declare_dependency(path)?;
    }
    ctx.add_cache_key(key);
    Ok(())
}

fn bad_metadata(id: &str) -> Problem {
    problem::contract(
        id,
        "Return a cache_key string and a dependencies array of at most 256 file paths.",
    )
}

/// The assets an `assets` reply carries, decoded within the 256-file, 16 MiB limits.
fn assets_of(id: &str, o: &Object) -> Result<Vec<Asset>, Problem> {
    let items = wire::arr_field(o, "assets").filter(|items| items.len() <= MAX_PLUGIN_ASSETS);
    let Some(items) = items else {
        return Err(problem::contract(
            id,
            "Return an assets array with at most MAX_PLUGIN_ASSETS (256) entries.",
        ));
    };
    let mut assets = Vec::with_capacity(items.len());
    let mut total = 0usize;
    for item in items {
        let fields = item
            .as_object()
            .and_then(|a| Some((wire::str_field(a, "path")?, wire::str_field(a, "base64")?)));
        let Some((path, data)) =
            fields.filter(|(_, d)| base64::decoded_len(d) <= MAX_PLUGIN_ASSET_BYTES - total)
        else {
            return Err(problem::contract(
                id,
                "Return path and base64 strings within the 16 MiB generated-asset limit.",
            ));
        };
        let Some(data) = base64::decode(data) else {
            return Err(problem::contract(id, "Encode asset bytes as valid, padded Base64."));
        };
        total += data.len();
        assets.push(Asset { path: path.to_string(), data });
    }
    Ok(assets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_names_are_known_and_unique() {
        let names = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            hooks_of(&names(&["html", "source"]), "x").unwrap(),
            Hooks { source: true, html: true, assets: false }
        );
        assert!(hooks_of(&names(&["source", "source"]), "x").is_err());
        assert!(hooks_of(&names(&["directive"]), "x").is_err());
        let publisher =
            Handshake { protocol: crate::PROTOCOL_NAME.into(), ..Handshake::default() };
        assert!(build_plugin_of(&publisher, "/bin/x").is_err());
    }

    #[test]
    fn asset_replies_are_decoded_and_bounded() {
        let reply = |data: &str| {
            let asset = object([("path", "a.css".into()), ("base64", data.into())]);
            let o = object([("assets", Value::Array(vec![asset]))]);
            o.as_object().cloned().unwrap()
        };
        assert_eq!(assets_of("x", &reply("Ym9keXt9")).unwrap()[0].data, b"body{}");
        assert!(assets_of("x", &reply("Ym9ke")).is_err());
    }
}
