use std::sync::Arc;
use std::sync::atomic::AtomicU8;

use super::*;
use crate::testutil::TempDir;
use crate::{
    API_VERSION, Capabilities, Change, ChangeKind, Loaded, SnapshotMode, SnapshotOptions, Version,
    snapshot,
};

const HONEST: u8 = 0;
const NO_OUTCOME: u8 = 1;
const NO_OPERATION: u8 = 2;
const OTHER_GENERATION: u8 = 3;

#[derive(Clone, Default)]
struct Test {
    behaviour: Arc<AtomicU8>,
    info: Option<PublisherInfo>,
}

impl Publisher for Test {
    fn info(&self) -> PublisherInfo {
        let caps = Capabilities { status: true, recover: false };
        self.info.clone().unwrap_or_else(|| PublisherInfo::new("test", "Test", "1.0.0", caps))
    }

    fn load(&mut self, options: &LoadOptions) -> Result<Loaded, Problem> {
        let source = Source { site: options.config.clone(), ..Source::default() };
        Ok(Loaded { source, target: "memory".into() })
    }

    fn plan(&mut self, snapshot: &Snapshot, plan: &mut Plan) -> Result<(), Problem> {
        let changes = snapshot.entries().iter();
        plan.changes =
            changes.map(|e| Change { path: e.path.clone(), kind: ChangeKind::Added }).collect();
        Ok(())
    }

    fn publish(&mut self, _: &Snapshot, plan: &Plan) -> Outcome {
        let live = Outcome {
            state: State::Live,
            operation: "op-1".into(),
            generation: plan.generation_hex(),
            ..Outcome::default()
        };
        match self.behaviour.load(Ordering::SeqCst) {
            HONEST => live,
            NO_OUTCOME => Outcome::default(),
            NO_OPERATION => Outcome { operation: String::new(), ..live },
            _ => Outcome { generation: "beef".into(), ..live },
        }
    }

    fn status(&mut self, operation: &str) -> Outcome {
        Outcome { state: State::Pending, operation: operation.into(), ..Outcome::default() }
    }
}

#[test]
fn registries_are_checked() {
    let good = Test::default();
    Registry::new(vec![Plugin::publisher(good.clone())]).unwrap();
    let twice =
        Registry::new(vec![Plugin::publisher(good.clone()), Plugin::publisher(good.clone())]);
    assert!(twice.is_err());
    let caps = Capabilities::default();
    let bad_id = PublisherInfo::new("Pages!", "Test", "1", caps);
    let newer = PublisherInfo {
        api: Version::new(API_VERSION.major, API_VERSION.minor + 1),
        ..PublisherInfo::new("test", "T", "1", caps)
    };
    let unnamed = PublisherInfo::new("test", "", "1", caps);
    for info in [bad_id, newer, unnamed] {
        let p = Plugin::publisher(Test { info: Some(info.clone()), ..good.clone() });
        let problem = Registry::new(vec![p]).unwrap_err();
        assert_eq!((problem.code.as_str(), problem.exit), ("plugins.registry", 2), "{info:?}");
    }
}

// The wrappers hold publishers to their contract: no outcome, an outcome without its
// operation, or another generation is never success.
#[test]
fn outcomes_follow_the_contract() {
    let dir = TempDir::with_files("publish", &["index.html"]);
    let test = Test::default();
    let registry = Registry::new(vec![Plugin::publisher(test.clone())]).unwrap();
    let options =
        LoadOptions { config: dir.path().to_string_lossy().into(), ..LoadOptions::default() };
    let mut s = load_publisher(&registry, "test", &options).unwrap();
    let borrow = SnapshotOptions { mode: SnapshotMode::Borrow, ..SnapshotOptions::default() };
    let snap = snapshot(&s.source().site, &borrow).unwrap();
    for (behaviour, state, exit) in [
        (HONEST, State::Live, 0),
        (NO_OUTCOME, State::Unknown, 7),
        (NO_OPERATION, State::Unknown, 7),
        (OTHER_GENERATION, State::Unknown, 7),
    ] {
        test.behaviour.store(behaviour, Ordering::SeqCst);
        let plan = s.plan(&snap).unwrap();
        let r = s.publish(&snap, &plan);
        assert_eq!((r.state, r.exit_status()), (state, exit), "behaviour {behaviour}");
    }
    test.behaviour.store(HONEST, Ordering::SeqCst);
    // A plan is published only by its session.
    let mut other = load_publisher(&registry, "test", &options).unwrap();
    let foreign = other.plan(&snap).unwrap();
    assert_eq!(s.publish(&snap, &foreign).problem.unwrap().code, "plugins.plan");
    let dry = LoadOptions { dry_run: true, ..options.clone() };
    let mut dry = load_publisher(&registry, "test", &dry).unwrap();
    let plan = dry.plan(&snap).unwrap();
    assert_eq!(dry.publish(&snap, &plan).problem.unwrap().code, "plugins.dry-run");
    assert_eq!(s.status("op-9").state, State::Pending);
    assert_eq!(s.recover().problem.unwrap().code, "plugins.unsupported");
    let missing = load_publisher(&registry, "pages", &LoadOptions::default()).unwrap_err();
    assert_eq!(missing.code, "plugins.publisher");
}

#[test]
fn a_changed_snapshot_is_not_published() {
    let dir = TempDir::with_files("publish", &["index.html"]);
    let registry = Registry::new(vec![Plugin::publisher(Test::default())]).unwrap();
    let mut s = load_publisher(&registry, "test", &LoadOptions::default()).unwrap();
    let borrow = SnapshotOptions { mode: SnapshotMode::Borrow, ..SnapshotOptions::default() };
    let snap = snapshot(dir.path(), &borrow).unwrap();
    let plan = s.plan(&snap).unwrap();
    std::fs::write(dir.path().join("index.html"), "changed").unwrap();
    let r = s.publish(&snap, &plan);
    assert_eq!(r.problem.unwrap().code, "plugins.snapshot-changed");
}
