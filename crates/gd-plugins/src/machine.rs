//! The machine registry: a user's record of the plugin programs they chose to run.
//!
//! For each program it holds its ID, absolute path, SHA-256, and what it declared when
//! registered. It lives in the user's configuration folder ([`config_dir`]), never in
//! a project; only [`register`] and [`unregister`] change it, under a lock, by
//! replacing the whole file. Running an entry checks its SHA-256 first
//! ([`verify_entry`]), so a program that changed does not run until registered again.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use gd_toml::{Table, Value};

use crate::problem::{self, Problem};
use crate::process_build::build_plugin_of;
use crate::snapshot::create_private;
use crate::{
    BUILD_PROTOCOL_NAME, Capabilities, HANDSHAKE_TIMEOUT, PROTOCOL_NAME, Version, inspect_program,
    is_valid_id, process_plugin, select_version,
};

/// The registry's file name in [`config_dir`].
pub const MACHINE_FILE: &str = "plugins.toml";
/// When set, names the folder of Guidedog's machine state instead of the user's
/// configuration folder, for tests and portable installations.
pub const CONFIG_VARIABLE: &str = "GUIDEDOG_CONFIG_DIR";

const MAX_MACHINE_BYTES: u64 = 1024 * 1024;

/// One registered program, as [`inspect_entry`] describes it and the registry keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct MachineEntry {
    /// The plugin's ID.
    pub id: String,
    /// Its display name.
    pub name: String,
    /// Its product version.
    pub version: String,
    /// The program's absolute path.
    pub path: String,
    /// The program's SHA-256 in hexadecimal.
    pub sha256: String,
    /// The protocol version selected when it was registered.
    pub protocol: Version,
    /// A publisher's optional operations.
    pub capabilities: Capabilities,
    /// The date it was registered, `YYYY-MM-DD`.
    pub registered: String,
    /// [`BUILD_PROTOCOL_NAME`] for a build plugin; empty for a publisher.
    pub protocol_name: String,
    /// A build plugin's hook names; empty for a publisher.
    pub hooks: Vec<String>,
}

impl MachineEntry {
    /// Whether the entry is a build plugin's.
    pub fn is_build(&self) -> bool {
        self.protocol_name == BUILD_PROTOCOL_NAME
    }
}

/// The registry's entries.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Machine {
    /// Every registered program, in registration order.
    pub entries: Vec<MachineEntry>,
}

impl Machine {
    /// The entry with the ID `id`.
    pub fn find(&self, id: &str) -> Option<&MachineEntry> {
        self.entries.iter().find(|e| e.id == id)
    }
}

/// The folder of Guidedog's machine state: `$GUIDEDOG_CONFIG_DIR`, or a `guidedog`
/// folder in the user's configuration folder. Fails when the system names none.
pub fn config_dir() -> Result<PathBuf, Problem> {
    if let Some(dir) = std::env::var_os(CONFIG_VARIABLE).filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    user_config_dir().map(|base| base.join("guidedog")).ok_or_else(|| {
        Problem::new(
            "plugins.config-dir",
            "No configuration folder",
            "The system names no configuration folder for this user.",
        )
        .hint(&format!("Set {CONFIG_VARIABLE} to a private folder for Guidedog's state."))
        .exit(4)
    })
}

fn user_config_dir() -> Option<PathBuf> {
    let var = |name| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(windows) {
        return var("APPDATA");
    }
    if cfg!(target_os = "macos") {
        return var("HOME").map(|h| h.join("Library/Application Support"));
    }
    var("XDG_CONFIG_HOME").or_else(|| var("HOME").map(|h| h.join(".config")))
}

/// The registry's file: [`MACHINE_FILE`] in [`config_dir`].
pub fn machine_path() -> Result<PathBuf, Problem> {
    Ok(config_dir()?.join(MACHINE_FILE))
}

/// Reads the registry at `path`; a missing file is an empty registry. Fails for a
/// file over 1 MiB, invalid TOML, another schema, or a malformed or repeated entry.
pub fn read_machine(path: &Path) -> Result<Machine, Problem> {
    let shown = path.to_string_lossy();
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Machine::default()),
        Err(e) => return Err(problem::io(&shown, &e)),
    };
    let mut text = String::new();
    let read = file.take(MAX_MACHINE_BYTES + 1).read_to_string(&mut text);
    read.map_err(|e| problem::io(&shown, &e))?;
    if text.len() as u64 > MAX_MACHINE_BYTES {
        return Err(machine_problem(&shown, 0, "The registry is over 1 MiB."));
    }
    let root = gd_toml::parse(&text).map_err(|e| machine_problem(&shown, e.line, &e.message))?;
    if root.get("schema").and_then(Value::as_integer) != Some(1) {
        return Err(machine_problem(&shown, 0, "The registry is not of schema 1."));
    }
    let mut machine = Machine::default();
    let Some(list) = root.get("plugins") else {
        return Ok(machine);
    };
    let Some(items) = list.as_array() else {
        let message = "plugins must be an array of tables, written as [[plugins]].";
        return Err(machine_problem(&shown, list.line, message));
    };
    for item in items {
        let entry = item.as_table().and_then(entry_of);
        let Some(entry) = entry else {
            return Err(machine_problem(&shown, item.line, "An entry is malformed."));
        };
        if machine.find(&entry.id).is_some() {
            let message = format!("The ID {} is registered twice.", entry.id);
            return Err(machine_problem(&shown, item.line, &message));
        }
        machine.entries.push(entry);
    }
    Ok(machine)
}

fn entry_of(t: &Table) -> Option<MachineEntry> {
    let text = |key| t.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
    let protocol_name = text("protocol_name");
    if !protocol_name.is_empty()
        && protocol_name != PROTOCOL_NAME
        && protocol_name != BUILD_PROTOCOL_NAME
    {
        return None;
    }
    let mut hooks = Vec::new();
    if protocol_name == BUILD_PROTOCOL_NAME {
        for hook in t.get("hooks")?.as_array()? {
            hooks.push(hook.as_str()?.to_string());
        }
    }
    let protocol = text("protocol");
    let (major, minor) = protocol.split_once('.')?;
    let protocol = Version::new(major.parse().ok()?, minor.parse().ok()?);
    let names = t.get("capabilities").and_then(Value::as_array).unwrap_or_default();
    let capabilities = Capabilities::from_names(names.iter().filter_map(Value::as_str));
    let entry = MachineEntry {
        id: text("id"),
        name: text("name"),
        version: text("version"),
        path: text("path"),
        sha256: text("sha256"),
        protocol,
        capabilities,
        registered: text("registered"),
        protocol_name,
        hooks,
    };
    let valid = is_valid_id(&entry.id)
        && !entry.name.is_empty()
        && !entry.version.is_empty()
        && Path::new(&entry.path).is_absolute()
        && entry.sha256.len() == 64;
    valid.then_some(entry)
}

/// Describes the program at `path` as a registry entry: checks that `path` is an
/// absolute path to a native program (not a script), takes its SHA-256, and runs it in
/// protocol mode to read its handshake. Running it runs its code: call it only for a
/// program the user chose.
pub fn inspect_entry(path: &str) -> Result<MachineEntry, Problem> {
    if !Path::new(path).is_absolute() {
        return Err(Problem::new(
            "plugins.path",
            "The path is not absolute",
            "A registered program is named by its absolute path.",
        )
        .hint("Give the program's full path, such as /usr/local/bin/guidedog-pages.")
        .path(path)
        .exit(2));
    }
    let sha256 = program_digest(path)?;
    let h = inspect_program(path, HANDSHAKE_TIMEOUT)?;
    let Some(protocol) = select_version(&h) else {
        return Err(Problem::new(
            "plugins.version",
            "The program speaks another protocol",
            "It shares no protocol version with this Guidedog.",
        )
        .hint("Install a version of the plugin made for this Guidedog.")
        .path(path)
        .exit(2));
    };
    if h.is_build() {
        build_plugin_of(&h, path)?;
    }
    Ok(MachineEntry {
        capabilities: Capabilities::from_names(h.capabilities.iter().map(String::as_str)),
        protocol_name: if h.is_build() { BUILD_PROTOCOL_NAME.to_string() } else { String::new() },
        hooks: h.hooks,
        id: h.id,
        name: h.name,
        version: h.version,
        path: path.to_string(),
        sha256,
        protocol,
        registered: today(),
    })
}

/// Checks that the program `entry` names is still the one registered: it exists and
/// has the registered SHA-256.
pub fn verify_entry(entry: &MachineEntry) -> Result<(), Problem> {
    if program_digest(&entry.path)? == entry.sha256 {
        return Ok(());
    }
    let id = &entry.id;
    Err(Problem::new(
        "plugins.changed",
        "The registered program changed",
        &format!("The program registered as {id} has changed since it was registered."),
    )
    .hint(&format!(
        "If you trust the new program, run guidedog plugin unregister {id}, then guidedog \
         plugin register {id} {}.",
        entry.path
    ))
    .path(&entry.path)
    .exit(2))
}

/// Adds `entry` to the registry at `path`, which it creates, with its folder, readable
/// only by the user. Refuses an incomplete entry and an ID registered already.
pub fn register(path: &Path, entry: &MachineEntry) -> Result<(), Problem> {
    let complete = is_valid_id(&entry.id)
        && !entry.name.is_empty()
        && !entry.version.is_empty()
        && Path::new(&entry.path).is_absolute()
        && entry.sha256.len() == 64
        && entry.protocol.major != 0;
    if !complete {
        return Err(Problem::new(
            "plugins.entry",
            "The entry is not a plugin program",
            "The entry is missing required metadata.",
        )
        .hint("Use inspect_entry on a trusted native program before registering it.")
        .path(&path.to_string_lossy())
        .exit(2));
    }
    if entry.is_build() {
        process_plugin(entry)?;
    } else if !entry.protocol_name.is_empty() && entry.protocol_name != PROTOCOL_NAME {
        let hint = "Use inspect_entry on a supported native program before registering it.";
        return Err(problem::contract(&entry.id, hint));
    } else if !entry.hooks.is_empty() {
        let hint = "Set hooks only for a build plugin; run inspect_entry on the executable again.";
        return Err(problem::contract(&entry.id, hint));
    }
    let _lock = machine_lock(path)?;
    let mut machine = read_machine(path)?;
    if machine.find(&entry.id).is_some() {
        let id = &entry.id;
        return Err(Problem::new(
            "plugins.registered",
            "The ID is registered already",
            &format!("A program is registered as {id}."),
        )
        .hint(&format!("Run guidedog plugin unregister {id} first to replace it."))
        .path(id)
        .exit(2));
    }
    machine.entries.push(entry.clone());
    write_machine(path, &machine.entries)
}

/// Removes the entry `id` from the registry at `path`. It removes nothing else: not
/// the program, a site, nor a stored credential.
pub fn unregister(path: &Path, id: &str) -> Result<(), Problem> {
    let _lock = machine_lock(path)?;
    let mut machine = read_machine(path)?;
    let before = machine.entries.len();
    machine.entries.retain(|e| e.id != id);
    if machine.entries.len() == before {
        return Err(Problem::new(
            "plugins.unregistered",
            "No program is registered with this ID",
            "There is nothing to unregister.",
        )
        .hint("Run guidedog plugin list to see the registered programs.")
        .path(id)
        .exit(2));
    }
    write_machine(path, &machine.entries)
}

/// Takes the registry's lock file, nonblocking; the lock lasts until the file closes.
/// Separate opens conflict even within one process.
pub(crate) fn machine_lock(path: &Path) -> Result<fs::File, Problem> {
    make_private_dirs(path)?;
    let name = lock_path(path);
    let shown = name.to_string_lossy();
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let file = options.open(&name).map_err(|e| problem::io(&shown, &e))?;
    if file.try_lock().is_err() {
        return Err(Problem::new(
            "plugins.busy",
            "The registry is being changed",
            "Another command holds the registry's lock.",
        )
        .hint("Wait for that command to finish, then register or unregister again.")
        .path(&path.to_string_lossy())
        .exit(4));
    }
    Ok(file)
}

fn lock_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".lock");
    PathBuf::from(name)
}

fn make_private_dirs(path: &Path) -> Result<(), Problem> {
    let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) else {
        return Ok(());
    };
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir).map_err(|e| problem::io(&dir.to_string_lossy(), &e))
}

/// Writes the registry to a file beside `path`, synced, then renamed over it, so a
/// reader sees the old registry or the new one, never a part.
fn write_machine(path: &Path, entries: &[MachineEntry]) -> Result<(), Problem> {
    make_private_dirs(path)?;
    let text = machine_text(entries);
    let mut next = path.as_os_str().to_owned();
    next.push(".next");
    let next = PathBuf::from(next);
    let shown = next.to_string_lossy().into_owned();
    let _ = fs::remove_file(&next);
    let written = create_private(&next).and_then(|mut f| {
        f.write_all(text.as_bytes())?;
        f.sync_all()
    });
    if let Err(e) = written {
        let _ = fs::remove_file(&next);
        return Err(problem::io(&shown, &e));
    }
    fs::rename(&next, path).map_err(|e| {
        let _ = fs::remove_file(&next);
        problem::io(&path.to_string_lossy(), &e)
    })
}

fn machine_text(entries: &[MachineEntry]) -> String {
    let mut out = String::from(
        "# Plugin programs registered on this machine. Change it with\n\
         # guidedog plugin register and guidedog plugin unregister.\nschema = 1\n",
    );
    let string_list = |out: &mut String, items: &mut dyn Iterator<Item = &str>| {
        out.push('[');
        for (i, item) in items.enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            gd_toml::write_string(out, item);
        }
        out.push_str("]\n");
    };
    for e in entries {
        out.push_str("\n[[plugins]]\n");
        for (key, value) in [
            ("id", &e.id),
            ("name", &e.name),
            ("version", &e.version),
            ("path", &e.path),
            ("sha256", &e.sha256),
        ] {
            out.push_str(key);
            out.push_str(" = ");
            gd_toml::write_string(&mut out, value);
            out.push('\n');
        }
        out.push_str(&format!("protocol = \"{}\"\ncapabilities = ", e.protocol));
        string_list(&mut out, &mut e.capabilities.names());
        out.push_str("registered = ");
        gd_toml::write_string(&mut out, &e.registered);
        out.push('\n');
        if e.is_build() {
            out.push_str(&format!("protocol_name = \"{BUILD_PROTOCOL_NAME}\"\nhooks = "));
            string_list(&mut out, &mut e.hooks.iter().map(String::as_str));
        }
    }
    out
}

/// The SHA-256 of the native program at `path`, which must be a regular file that
/// starts as an ELF, Mach-O or PE program does.
fn program_digest(path: &str) -> Result<String, Problem> {
    let meta = fs::symlink_metadata(path).map_err(|e| {
        Problem::new("plugins.missing", "The program is missing", &e.to_string())
            .hint("Install the program again, or register it at its new path.")
            .path(path)
            .exit(4)
    })?;
    if !meta.is_file() {
        return Err(Problem::new(
            "plugins.file-type",
            "Not a program file",
            "A registered program is a regular file, not a link or folder.",
        )
        .hint("Register the program file itself.")
        .path(path)
        .exit(2));
    }
    let mut file = fs::File::open(path).map_err(|e| problem::io(path, &e))?;
    let mut hash = gd_sha::Sha256::new();
    let mut chunk = vec![0u8; 64 * 1024];
    let mut head = Vec::with_capacity(4);
    loop {
        let n = file.read(&mut chunk).map_err(|e| problem::io(path, &e))?;
        if n == 0 {
            break;
        }
        if head.len() < 4 {
            head.extend(chunk[..n].iter().take(4 - head.len()));
        }
        hash.update(&chunk[..n]);
    }
    if !is_native(&head) {
        return Err(Problem::new(
            "plugins.native",
            "Not a native program",
            "Only native programs can be registered, not scripts or other files.",
        )
        .hint("Register the plugin's released native program.")
        .path(path)
        .exit(2));
    }
    Ok(gd_sha::to_hex(&hash.finish()))
}

fn is_native(head: &[u8]) -> bool {
    let &[a, b, c, d, ..] = head else {
        return false;
    };
    let magic = u32::from_be_bytes([a, b, c, d]);
    let elf = magic == 0x7F45_4C46;
    let mach_o =
        [0xCFFA_EDFE, 0xCEFA_EDFE, 0xFEED_FACF, 0xFEED_FACE, 0xCAFE_BABE].contains(&magic);
    elf || mach_o || (a == b'M' && b == b'Z')
}

/// Today's date in UTC, `YYYY-MM-DD`.
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The proleptic Gregorian date of `days` since 1970-01-01 (Howard Hinnant's method).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn machine_problem(path: &str, line: usize, message: &str) -> Problem {
    let place = if line > 0 { format!("{path}:{line}") } else { path.to_string() };
    Problem::new("plugins.machine", "The plugin registry is not valid", message)
        .hint("Fix or remove the file; guidedog plugin register writes a new one.")
        .path(&place)
        .exit(2)
}

#[cfg(test)]
mod tests;
