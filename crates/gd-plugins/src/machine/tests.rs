use super::*;
use crate::testutil::TempDir;

fn entry(path: &str) -> MachineEntry {
    MachineEntry {
        id: "test".into(),
        name: "Test".into(),
        version: "1".into(),
        path: path.into(),
        sha256: "a".repeat(64),
        protocol: crate::PROTOCOL_VERSION,
        capabilities: Capabilities { status: true, recover: false },
        registered: "2026-10-13".into(),
        ..MachineEntry::default()
    }
}

#[test]
fn separate_opens_conflict_even_in_one_process() {
    let root = TempDir::new("registry-lock");
    let path = root.path().join("plugins.toml");
    let first = machine_lock(&path).unwrap();
    assert_eq!(machine_lock(&path).unwrap_err().code, "plugins.busy");
    drop(first);
    machine_lock(&path).unwrap();
}

#[test]
fn registrations_round_trip_and_refuse_repeats() {
    let root = TempDir::new("registry");
    let path = root.path().join("state/plugins.toml");
    assert_eq!(read_machine(&path).unwrap(), Machine::default());
    let program = root.path().join("program").to_string_lossy().into_owned();
    let mut build = entry(&program);
    build.id = "badge".into();
    build.protocol_name = BUILD_PROTOCOL_NAME.into();
    build.hooks = vec!["source".into(), "assets".into()];
    register(&path, &entry(&program)).unwrap();
    register(&path, &build).unwrap();
    assert_eq!(register(&path, &entry(&program)).unwrap_err().code, "plugins.registered");
    let machine = read_machine(&path).unwrap();
    assert_eq!(machine.entries, vec![entry(&program), build]);
    unregister(&path, "test").unwrap();
    assert_eq!(unregister(&path, "test").unwrap_err().code, "plugins.unregistered");
    assert_eq!(read_machine(&path).unwrap().entries.len(), 1);
    let incomplete = MachineEntry { sha256: "abc".into(), ..entry(&program) };
    assert_eq!(register(&path, &incomplete).unwrap_err().code, "plugins.entry");
}

#[test]
fn malformed_registries_are_explained() {
    let root = TempDir::new("registry");
    let path = root.path().join("plugins.toml");
    for (text, needle) in [
        ("schema = 2\n", "schema 1"),
        ("schema = 1\nplugins = 3\n", "array of tables"),
        ("schema = 1\n[[plugins]]\nid = \"x\"\n", "malformed"),
        ("schema = 1\n[[plugins\n", ""),
    ] {
        std::fs::write(&path, text).unwrap();
        let p = read_machine(&path).unwrap_err();
        assert_eq!(p.code, "plugins.machine");
        assert!(p.message.contains(needle), "{text}: {}", p.message);
    }
}

#[test]
fn programs_must_be_native_and_absolute() {
    let root = TempDir::with_files("inspect", &["script.sh"]);
    let script = root.path().join("script.sh").to_string_lossy().into_owned();
    assert_eq!(inspect_entry(&script).unwrap_err().code, "plugins.native");
    assert_eq!(inspect_entry("relative-program").unwrap_err().code, "plugins.path");
    let missing = root.path().join("missing").to_string_lossy().into_owned();
    assert_eq!(inspect_entry(&missing).unwrap_err().code, "plugins.missing");
    assert!(is_native(b"\x7fELF...") && is_native(b"MZ\x90\0") && !is_native(b"#!/b"));
}

#[test]
fn dates_are_civil() {
    assert_eq!(civil_from_days(0), (1970, 1, 1));
    assert_eq!(civil_from_days(20_740), (2026, 10, 14));
    assert_eq!(civil_from_days(11_016), (2000, 2, 29));
}
