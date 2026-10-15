//! Choosing the build plugin a project selects: linked plugins first, then the
//! programs registered on this machine.

use std::path::Path;

use crate::problem::{self, Problem};
use crate::{Plugin, Registry, process_plugin, read_machine, verify_entry};

/// The most build plugins one build may select.
pub const MAX_BUILD_PLUGINS: usize = 64;

/// Finds the build plugin `id` as a build does: in `linked`, the plugins compiled into
/// this program, and otherwise in the machine registry at `machine_file`, which is
/// read only then. A registered program is verified against its SHA-256 before it is
/// offered. Fails when neither has the ID, when the ID names a publisher, or when the
/// registry or the program cannot be trusted.
pub fn select_build_plugin(
    linked: &Registry,
    id: &str,
    machine_file: &Path,
) -> Result<Plugin, Problem> {
    if let Some(plugin) = linked.find(id) {
        if plugin.is_publisher() {
            return Err(problem::contract(
                id,
                "Select a build plugin here; run this publisher with guidedog publish instead.",
            ));
        }
        return Ok(plugin.clone());
    }
    let machine = read_machine(machine_file)?;
    let Some(entry) = machine.find(id) else {
        return Err(Problem::new(
            "plugins.not-registered",
            "The plugin is not available",
            &format!("No plugin {id} is linked into this program or registered on this machine."),
        )
        .hint(&format!("Compile it and run guidedog plugin register {id} /path/to/program."))
        .path(id)
        .exit(2));
    };
    verify_entry(entry)?;
    process_plugin(entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use crate::{BuildPlugin, Hooks, PluginInfo};

    #[derive(Clone)]
    struct Linked;

    impl BuildPlugin for Linked {
        fn info(&self) -> PluginInfo {
            PluginInfo::new("linked", "Linked", "1", Hooks { source: true, ..Hooks::NONE })
        }
    }

    #[test]
    fn linked_plugins_come_first_and_others_must_be_registered() {
        let dir = TempDir::new("select");
        let machine = dir.path().join("plugins.toml");
        let linked = Registry::new(vec![Plugin::build(Linked)]).unwrap();
        let plugin = select_build_plugin(&linked, "linked", &machine).unwrap();
        assert_eq!(plugin.id(), "linked");
        assert_eq!(plugin.identity("host-1"), plugin.identity("host-1"));
        assert_ne!(plugin.identity("host-1"), plugin.identity("host-2"));
        let p = select_build_plugin(&linked, "other", &machine).unwrap_err();
        assert_eq!(p.code, "plugins.not-registered");
    }
}
