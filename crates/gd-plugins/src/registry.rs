//! `Plugin` (build hooks or a publisher, never both) and `Registry`, a validated list.

use std::fmt;
use std::sync::Arc;

use crate::command::validate_commands;
use crate::paths::is_bounded;
use crate::problem::{self, Problem};
use crate::{
    API_VERSION, BuildPlugin, CommandContext, CommandInfo, PLUGIN_API_VERSION, PluginInfo,
    Publisher, PublisherInfo, Version, is_valid_id,
};

type BuildMaker = Arc<dyn Fn() -> Box<dyn BuildPlugin> + Send + Sync>;
type PublisherMaker = Arc<dyn Fn() -> Box<dyn Publisher> + Send + Sync>;

/// A plugin an application offers: build hooks or a publisher, never both. It makes a
/// fresh plugin value for every session, so sessions never share state.
#[derive(Clone)]
pub struct Plugin {
    kind: Kind,
    executable: String,
    program_sha256: String,
    commands: Vec<CommandInfo>,
    /// The command protocol an installed program's commands are run with.
    command_protocol: Version,
}

#[derive(Clone)]
enum Kind {
    Build { info: PluginInfo, make: BuildMaker },
    Publisher { info: PublisherInfo, make: PublisherMaker },
}

impl Plugin {
    /// Offers build hooks. Each session starts from a clone of `prototype`, the
    /// unloaded plugin, and its `info` is read once, here.
    pub fn build<P: BuildPlugin + Clone + Sync + 'static>(prototype: P) -> Plugin {
        let info = prototype.info();
        let commands = prototype.commands();
        let make: BuildMaker = Arc::new(move || Box::new(prototype.clone()));
        Plugin::of(Kind::Build { info, make }, "").with_commands(commands, Version::default())
    }

    /// Offers a publisher. Each session starts from a clone of `prototype`.
    pub fn publisher<P: Publisher + Clone + Sync + 'static>(prototype: P) -> Plugin {
        let info = prototype.info();
        let commands = prototype.commands();
        let make: PublisherMaker = Arc::new(move || Box::new(prototype.clone()));
        Plugin::of(Kind::Publisher { info, make }, "").with_commands(commands, Version::default())
    }

    pub(crate) fn from_build_maker(
        info: PluginInfo,
        executable: &str,
        make: BuildMaker,
    ) -> Plugin {
        Plugin::of(Kind::Build { info, make }, executable)
    }

    pub(crate) fn from_publisher_maker(
        info: PublisherInfo,
        executable: &str,
        make: PublisherMaker,
    ) -> Plugin {
        Plugin::of(Kind::Publisher { info, make }, executable)
    }

    fn of(kind: Kind, executable: &str) -> Plugin {
        Plugin {
            kind,
            executable: executable.to_string(),
            program_sha256: String::new(),
            commands: Vec::new(),
            command_protocol: Version::default(),
        }
    }

    /// Records the commands the plugin contributes, and for an installed program the
    /// command protocol version it was registered with.
    pub(crate) fn with_commands(
        mut self,
        commands: Vec<CommandInfo>,
        protocol: Version,
    ) -> Plugin {
        self.commands = commands;
        self.command_protocol = protocol;
        self
    }

    /// The commands the plugin adds to the host's command line, in its order.
    pub fn commands(&self) -> &[CommandInfo] {
        &self.commands
    }

    pub(crate) fn command_protocol(&self) -> Version {
        self.command_protocol
    }

    /// Runs the contributed command `name` (its words joined by spaces, such as
    /// `pages workflow`) with `args`, the words after its name, and returns its exit
    /// status. A linked plugin runs it on a fresh value; an installed program is started
    /// with [`crate::COMMAND_FLAG`] on the host's terminal. Fails when the plugin
    /// contributes no such command, or the command or its program fails.
    pub fn run_command(
        &self,
        ctx: &mut CommandContext<'_>,
        name: &str,
        args: &[String],
    ) -> Result<i32, Problem> {
        crate::process_command::run_command(self, ctx, name, args)
    }

    pub(crate) fn run_linked_command(
        &self,
        ctx: &mut CommandContext<'_>,
        name: &str,
        args: &[String],
    ) -> Result<i32, Problem> {
        match &self.kind {
            Kind::Build { make, .. } => make().run_command(ctx, name, args),
            Kind::Publisher { make, .. } => make().run_command(ctx, name, args),
        }
    }

    /// The plugin's ID.
    pub fn id(&self) -> &str {
        match &self.kind {
            Kind::Build { info, .. } => &info.id,
            Kind::Publisher { info, .. } => &info.id,
        }
    }

    /// The build plugin's identity and hooks; `None` for a publisher.
    pub fn build_info(&self) -> Option<&PluginInfo> {
        match &self.kind {
            Kind::Build { info, .. } => Some(info),
            Kind::Publisher { .. } => None,
        }
    }

    /// The publisher's identity and capabilities; `None` for build hooks.
    pub fn publisher_info(&self) -> Option<&PublisherInfo> {
        match &self.kind {
            Kind::Publisher { info, .. } => Some(info),
            Kind::Build { .. } => None,
        }
    }

    /// Whether this plugin is a publisher.
    pub fn is_publisher(&self) -> bool {
        matches!(self.kind, Kind::Publisher { .. })
    }

    /// The installed program it runs, for [`crate::process_plugin`] and
    /// [`crate::process_publisher`]; empty for a linked plugin.
    pub fn executable(&self) -> &str {
        &self.executable
    }

    /// The plugin's identity for build cache keys: what must change for outputs it
    /// produced to be rebuilt. For an installed program it is the registered program's
    /// SHA-256, which pins its exact code. For a linked plugin it is the SHA-256 of its
    /// ID, product version and interface version together with `host_build`, the host
    /// program's own build identity (its version and commit, or its executable's
    /// digest), since the plugin's code is part of the host. Lowercase hexadecimal.
    pub fn identity(&self, host_build: &str) -> String {
        if !self.program_sha256.is_empty() {
            return self.program_sha256.clone();
        }
        let (id, version, api) = match &self.kind {
            Kind::Build { info, .. } => (&info.id, &info.version, info.api),
            Kind::Publisher { info, .. } => (&info.id, &info.version, info.api),
        };
        let kind = if self.is_publisher() { "publisher" } else { "build" };
        let text = format!("guidedog-linked-plugin\n{kind}\n{id}\n{version}\n{api}\n{host_build}");
        gd_sha::to_hex(&gd_sha::sha256(text.as_bytes()))
    }

    /// Records the registered program's SHA-256, which becomes its identity.
    pub(crate) fn with_program_sha256(mut self, sha256: &str) -> Plugin {
        self.program_sha256 = sha256.to_string();
        self
    }

    /// Checks the plugin's metadata without running any of its hooks.
    pub fn validate(&self) -> Result<(), Problem> {
        match &self.kind {
            Kind::Build { info, .. } => validate_build(info)?,
            Kind::Publisher { info, .. } => validate_publisher(info)?,
        }
        validate_commands(self.id(), &self.commands)
    }

    pub(crate) fn make_build(&self) -> Option<Box<dyn BuildPlugin>> {
        match &self.kind {
            Kind::Build { make, .. } => Some(make()),
            Kind::Publisher { .. } => None,
        }
    }

    pub(crate) fn make_publisher(&self) -> Option<Box<dyn Publisher>> {
        match &self.kind {
            Kind::Publisher { make, .. } => Some(make()),
            Kind::Build { .. } => None,
        }
    }
}

impl fmt::Debug for Plugin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("Plugin");
        match &self.kind {
            Kind::Build { info, .. } => d.field("build", info),
            Kind::Publisher { info, .. } => d.field("publisher", info),
        };
        d.field("executable", &self.executable).field("commands", &self.commands).finish()
    }
}

pub(crate) fn validate_build(info: &PluginInfo) -> Result<(), Problem> {
    let id = info.id.as_str();
    if !is_valid_id(id)
        || info.name.is_empty()
        || info.version.is_empty()
        || !is_bounded(&info.name, 256)
        || !is_bounded(&info.version, 64)
    {
        return Err(problem::contract(
            id,
            "Set a valid ID, a display name, and a product version in PluginInfo.",
        ));
    }
    if !info.api.is_compatible_with(PLUGIN_API_VERSION) {
        return Err(problem::contract(id, "Rebuild against PLUGIN_API_VERSION 1.0."));
    }
    if !info.hooks.is_any() {
        return Err(problem::contract(
            id,
            "Offer source, html, or assets hooks, or wrap a publisher with Plugin::publisher.",
        ));
    }
    Ok(())
}

pub(crate) fn validate_publisher(info: &PublisherInfo) -> Result<(), Problem> {
    let id = info.id.as_str();
    if !is_valid_id(id) {
        return Err(registry_problem(
            id,
            "is not a publisher ID",
            "Use 2 to 32 lowercase letters, digits, and hyphens, such as pages.",
        ));
    }
    if info.name.is_empty() || info.version.is_empty() {
        return Err(registry_problem(
            id,
            "has no name or version",
            "Set the publisher's name and version.",
        ));
    }
    if !info.api.is_compatible_with(API_VERSION) {
        let hint = format!(
            "Rebuild the publisher against interface {}.0 to {API_VERSION}.",
            API_VERSION.major
        );
        return Err(registry_problem(
            id,
            &format!("was built against interface {}", info.api),
            &hint,
        ));
    }
    Ok(())
}

fn registry_problem(id: &str, what: &str, hint: &str) -> Problem {
    Problem::new(
        "plugins.registry",
        "The publisher registry is not valid",
        &format!("The publisher {id:?} {what}."),
    )
    .hint(hint)
    .path(id)
    .exit(2)
}

/// The plugins an application offers, checked when it is made: every plugin is valid
/// (its commands included) and has its own ID. Running nothing, it never executes a
/// plugin's hooks.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    plugins: Vec<Plugin>,
}

impl Registry {
    /// Makes a registry of `plugins`; fails on the first invalid or repeated one.
    pub fn new(plugins: Vec<Plugin>) -> Result<Registry, Problem> {
        for (i, p) in plugins.iter().enumerate() {
            p.validate()?;
            if plugins[..i].iter().any(|q| q.id() == p.id()) {
                return Err(problem::contract(
                    p.id(),
                    "Give every plugin a different ID; remove the duplicate registration.",
                ));
            }
        }
        Ok(Registry { plugins })
    }

    /// The plugin with exactly the ID `id`.
    pub fn find(&self, id: &str) -> Option<&Plugin> {
        self.plugins.iter().find(|p| p.id() == id)
    }

    /// Every plugin, in the order given.
    pub fn plugins(&self) -> &[Plugin] {
        &self.plugins
    }

    /// Every contributed command with the plugin that runs it, in the plugins' order.
    pub fn commands(&self) -> impl Iterator<Item = (&Plugin, &CommandInfo)> {
        self.plugins.iter().flat_map(|p| p.commands.iter().map(move |c| (p, c)))
    }
}
