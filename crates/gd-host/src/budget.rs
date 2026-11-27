//! A coarse memory budget for a build: what the host holds of files read, includes, and
//! outputs, reserved before it is allocated. A conversion's own memory is bounded by
//! `guidedoc::Limits` (see `limits_within`), not metered allocation by allocation.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};

use guidedoc::Limits;

use crate::problem::{Problem, human_bytes};

/// The budget `guidedog convert` uses when none is given (256 MiB).
pub const DEFAULT_BUDGET: usize = 256 << 20;

/// The environment variable that states the machine's free memory in MiB, for machines
/// whose own report misleads (`available_memory`).
pub const AVAILABLE_OVERRIDE: &str = "GUIDEDOG_AVAILABLE_MIB";

const MIB: usize = 1 << 20;

/// What a budget tells its policy when it cannot hold a reservation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Refusal {
    /// The bytes asked for.
    pub need: usize,
    /// What the budget holds.
    pub used: usize,
    /// Its limit.
    pub limit: usize,
}

/// What a budget's policy decides about a reservation it cannot hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalChoice {
    /// Refuse it, and every later one without asking again.
    Stop,
    /// Raise the limit to this many bytes (more than `Refusal::limit`) and try again.
    Raise(usize),
}

/// The policy a budget asks (at most one call at a time, while its other reservations
/// wait). It must not reserve from the budget it is asked about.
pub type OnRefusal = dyn Fn(&Refusal) -> RefusalChoice + Send + Sync;

/// Bounds the bytes a build holds at once, shared by its threads (`Budget` is `Sync`).
///
/// Units of work that run at once (`join`) wait for each other: a reservation the budget
/// cannot hold now waits while another joined unit runs, since that one releases what
/// it holds when it ends. When every other unit waits too, nothing would be released,
/// and the reservation is refused instead, so the budget never deadlocks. A reservation
/// larger than the whole limit goes to the policy at once.
pub struct Budget {
    state: Mutex<State>,
    freed: Condvar,
    on_refusal: Option<Box<OnRefusal>>,
}

#[derive(Debug, Default)]
struct State {
    limit: usize,
    used: usize,
    peak: usize,
    active: usize,
    waiting: usize,
    stopped: bool,
    refused_used: usize,
    refused_need: usize,
}

impl std::fmt::Debug for Budget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Budget")
            .field("state", &*self.lock())
            .field("on_refusal", &self.on_refusal.is_some())
            .finish()
    }
}

impl Budget {
    /// Makes a budget of `limit` bytes that refuses what it cannot hold.
    pub fn new(limit: usize) -> Budget {
        Budget {
            state: Mutex::new(State { limit, ..State::default() }),
            freed: Condvar::new(),
            on_refusal: None,
        }
    }

    /// Makes a budget of `limit` bytes that asks `on_refusal` what to do with a
    /// reservation it cannot hold (a terminal build asks the user whether to raise it).
    pub fn with_policy(
        limit: usize,
        on_refusal: impl Fn(&Refusal) -> RefusalChoice + Send + Sync + 'static,
    ) -> Budget {
        Budget { on_refusal: Some(Box::new(on_refusal)), ..Budget::new(limit) }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // A panic while the lock was held leaves plain counters: still usable.
        self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The limit now; a policy may have raised it.
    pub fn limit(&self) -> usize {
        self.lock().limit
    }

    /// Sets the limit; reservations waiting are woken to try again.
    pub fn set_limit(&self, limit: usize) {
        self.lock().limit = limit;
        self.freed.notify_all();
    }

    /// The bytes held now.
    pub fn used(&self) -> usize {
        self.lock().used
    }

    /// The most bytes held at once.
    pub fn peak(&self) -> usize {
        self.lock().peak
    }

    /// How many joined units wait for memory now (tests watch it).
    #[cfg(test)]
    pub(crate) fn waiting_count(&self) -> usize {
        self.lock().waiting
    }

    /// What the budget held and was asked for when it last refused, if it has.
    pub fn last_refusal(&self) -> Option<Refusal> {
        let s = self.lock();
        (s.refused_need > 0).then_some(Refusal {
            need: s.refused_need,
            used: s.refused_used,
            limit: s.limit,
        })
    }

    /// Counts a unit of work that runs at once with others until the returned guard
    /// drops (see [`Budget`]). Borrows the budget for the guard's life.
    pub fn join(&self) -> Unit<'_> {
        self.lock().active += 1;
        Unit { budget: self }
    }

    /// Takes `bytes` from the budget, waiting as [`Budget`] describes; false when the
    /// budget and its policy refuse, which records the refusal (`last_refusal`).
    pub(crate) fn reserve(&self, bytes: usize) -> bool {
        let mut s = self.lock();
        while s.used > s.limit || bytes > s.limit - s.used {
            if bytes > s.limit || s.waiting + 1 >= s.active {
                if self.decide(&mut s, bytes) {
                    continue;
                }
                s.refused_used = s.used;
                s.refused_need = bytes;
                return false;
            }
            s.waiting += 1;
            self.freed.notify_all(); // the others check whether any unit still runs.
            s = self.freed.wait(s).unwrap_or_else(|poisoned| poisoned.into_inner());
            s.waiting -= 1;
        }
        s.used += bytes;
        s.peak = s.peak.max(s.used);
        true
    }

    /// Returns `bytes` to the budget and wakes reservations waiting for them.
    pub(crate) fn release(&self, bytes: usize) {
        if bytes == 0 {
            return;
        }
        let mut s = self.lock();
        s.used = s.used.saturating_sub(bytes);
        drop(s);
        self.freed.notify_all();
    }

    /// Asks the policy, with the lock held; true when the limit was raised.
    fn decide(&self, s: &mut State, bytes: usize) -> bool {
        let Some(policy) = &self.on_refusal else { return false };
        if s.stopped {
            return false;
        }
        match policy(&Refusal { need: bytes, used: s.used, limit: s.limit }) {
            RefusalChoice::Raise(limit) if limit > s.limit => {
                s.limit = limit;
                true
            }
            RefusalChoice::Raise(_) | RefusalChoice::Stop => {
                s.stopped = true;
                false
            }
        }
    }
}

/// A unit of work counted by [`Budget::join`]; dropping it ends the unit.
#[derive(Debug)]
pub struct Unit<'b> {
    budget: &'b Budget,
}

impl Drop for Unit<'_> {
    fn drop(&mut self) {
        let mut s = self.budget.lock();
        s.active = s.active.saturating_sub(1);
        drop(s);
        self.budget.freed.notify_all(); // waiting units may now be the only ones left.
    }
}

/// Counts what one unit of work (a document's read, a page, a conversion) holds of a
/// budget: the files it reads, the outputs it keeps. Each charge is made before the
/// memory is allocated; dropping the ledger (or `settle`) releases them all. A ledger
/// without a budget accepts every charge. Its methods take `&self`, so a resource
/// provider can charge through a shared reference, from several threads.
#[derive(Debug, Default)]
pub struct Ledger<'b> {
    budget: Option<&'b Budget>,
    held: AtomicUsize,
    refused: AtomicUsize,
}

impl<'b> Ledger<'b> {
    /// Makes a ledger charging `budget`, which it borrows.
    pub fn new(budget: &'b Budget) -> Ledger<'b> {
        Ledger { budget: Some(budget), held: AtomicUsize::new(0), refused: AtomicUsize::new(0) }
    }

    /// Makes a ledger that counts nothing and refuses nothing.
    pub fn unbounded() -> Ledger<'static> {
        Ledger::default()
    }

    /// The budget it charges, if any.
    pub fn budget(&self) -> Option<&'b Budget> {
        self.budget
    }

    /// Reserves `bytes`, waiting as [`Budget`] describes. Returns false, reserving
    /// nothing, when the budget refuses; `refused` then tells the bytes asked for.
    pub fn charge(&self, bytes: usize) -> bool {
        let Some(budget) = self.budget else { return true };
        if !budget.reserve(bytes) {
            self.refused.store(bytes, Ordering::Relaxed);
            return false;
        }
        self.held.fetch_add(bytes, Ordering::Relaxed);
        true
    }

    /// Returns `bytes` the unit has freed before it ends (at most what it holds).
    pub fn discharge(&self, bytes: usize) {
        let Some(budget) = self.budget else { return };
        let mut taken = 0;
        let _ = self.held.try_update(Ordering::Relaxed, Ordering::Relaxed, |held| {
            taken = bytes.min(held);
            Some(held - taken)
        });
        budget.release(taken);
    }

    /// Releases everything the ledger holds.
    pub fn settle(&self) {
        self.discharge(usize::MAX);
    }

    /// The bytes held now.
    pub fn held(&self) -> usize {
        self.held.load(Ordering::Relaxed)
    }

    /// The bytes the last refused charge asked for, or 0.
    pub fn refused(&self) -> usize {
        self.refused.load(Ordering::Relaxed)
    }
}

impl Drop for Ledger<'_> {
    fn drop(&mut self) {
        self.settle();
    }
}

/// Explains a charge `budget` refused (`host.budget`, exit 3): what needed `bytes`, and
/// where. The hint names a budget that fits (`budget_hint`).
pub fn budget_problem(what: &str, path: &str, bytes: usize, budget: Option<&Budget>) -> Problem {
    let limit = budget.map_or(0, Budget::limit);
    Problem::new(
        "host.budget",
        "MEMORY BUDGET REACHED",
        path,
        format!(
            "{what} needs {} of memory, more than the budget of {} has left.",
            human_bytes(bytes),
            human_bytes(limit)
        ),
        budget_hint(budget, bytes, "split the work into smaller files"),
        3,
    )
}

/// The hint of a budget's problem: the budget the refused step fits in, from what
/// `budget` held and was asked for when it last refused (`need` when it refused
/// nothing itself), capped by the machine's free memory, and the other way out the
/// caller names (`other`, lower case, without a full stop).
pub fn budget_hint(budget: Option<&Budget>, need: usize, other: &str) -> String {
    let (mut used, mut asked, mut limit) = (0, need, 0);
    if let Some(b) = budget {
        limit = b.limit();
        if let Some(r) = b.last_refusal() {
            (used, asked) = (r.used, r.need);
        }
    }
    budget_advice(used, asked, limit, available_memory(), other)
}

/// The text of `budget_hint`, for a step that asked for `need` bytes while the budget
/// held `used` of `limit`, on a machine with `available` bytes free (`None`: unknown).
pub fn budget_advice(
    used: usize,
    need: usize,
    limit: usize,
    available: Option<usize>,
    other: &str,
) -> String {
    let (budget, fits) = suggest_budget(used, need, limit, available);
    let mib = budget / MIB;
    if fits {
        return format!("Run again with --budget={mib}, or {other}.");
    }
    let free = available.unwrap_or(0) / MIB;
    format!(
        "Free memory and run again with --budget={mib}: the step needs about {mib} MiB, \
         more than the {free} MiB this machine has free. Or {other}."
    )
}

/// The budget in bytes to suggest so that one change is enough: what is held plus the
/// request four times over (a step also holds its output and the copy it writes), or
/// twice the old limit, whichever is larger, in steps of 64 MiB. With `available` known
/// and smaller, the budget is capped to `fitting_budget`, or is what is needed in steps
/// of 16 MiB when even that does not fit; `fits` is then false.
pub fn suggest_budget(
    used: usize,
    need: usize,
    limit: usize,
    available: Option<usize>,
) -> (usize, bool) {
    const STEP_COPIES: usize = 4;
    const CAP: usize = usize::MAX / 8;
    let held = used.min(CAP);
    let request = need.min((CAP - held) / STEP_COPIES);
    let needed = held + request * STEP_COPIES;
    let budget = round_up((limit.min(CAP) * 2).max(needed), 64 * MIB);
    match available {
        Some(free) if budget > free => {
            let budget = fitting_budget(free).max(round_up(needed, 16 * MIB));
            (budget, budget <= free)
        }
        _ => (budget, true),
    }
}

/// A budget that fits in `available` free bytes: nine tenths of it, in steps of
/// 16 MiB; 0 when there is no room for even one step (never offer 0 to the CLI, where
/// it means the default).
pub fn fitting_budget(available: usize) -> usize {
    const STEP: usize = 16 * MIB;
    available / 10 * 9 / STEP * STEP
}

fn round_up(n: usize, step: usize) -> usize {
    n.div_ceil(step).saturating_mul(step)
}

/// The memory this machine has free, in bytes: the value of [`AVAILABLE_OVERRIDE`] in
/// MiB when set, else what the system reports (GDS 0020). On Linux that is
/// `MemAvailable` from `/proc/meminfo`, lowered to what the process's control group may
/// still use (`cgroup_room`); on macOS, the free, speculative and inactive pages that
/// `vm_stat` counts; on Windows, `ullAvailPhys` of `GlobalMemoryStatusEx`. `None` when
/// the system does not say.
pub fn available_memory() -> Option<usize> {
    if let Ok(text) = std::env::var(AVAILABLE_OVERRIDE) {
        return text.trim().parse::<usize>().ok().map(|mib| mib.saturating_mul(MIB));
    }
    system_available()
}

#[cfg(target_os = "linux")]
fn system_available() -> Option<usize> {
    let info = std::fs::read_to_string("/proc/meminfo").ok()?;
    let system = kib_field(&info, "MemAvailable:")?;
    let read = |path: &Path| std::fs::read_to_string(path).ok();
    Some(cgroup_room(&read).map_or(system, |room| system.min(room)))
}

#[cfg(target_os = "macos")]
fn system_available() -> Option<usize> {
    // The program every macOS installs, by its full path, so PATH cannot replace it.
    let output = std::process::Command::new("/usr/bin/vm_stat").output().ok()?;
    if !output.status.success() {
        return None;
    }
    vm_stat_available(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(windows)]
fn system_available() -> Option<usize> {
    crate::sys::available_physical().map(|bytes| usize::try_from(bytes).unwrap_or(usize::MAX))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn system_available() -> Option<usize> {
    None
}

/// The peak resident memory of this process so far, in bytes: `VmHWM` from
/// `/proc/self/status` on Linux, `ru_maxrss` of `getrusage` on macOS, and the peak
/// working set on Windows; `None` elsewhere or when the system does not say.
pub fn peak_memory() -> Option<u64> {
    system_peak()
}

#[cfg(target_os = "linux")]
fn system_peak() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    kib_field(&status, "VmHWM:").map(|bytes| bytes as u64)
}

#[cfg(target_os = "macos")]
fn system_peak() -> Option<u64> {
    crate::sys::peak_resident()
}

#[cfg(windows)]
fn system_peak() -> Option<u64> {
    crate::sys::peak_working_set()
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn system_peak() -> Option<u64> {
    None
}

/// The memory `vm_stat`'s report says new processes can use, in bytes: the free,
/// speculative and inactive pages, times the page size of its first line. Purgeable
/// pages are left out, since `vm_stat` also counts them among the active and inactive
/// ones. `None` when the report lacks the page size or the free pages.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn vm_stat_available(report: &str) -> Option<usize> {
    let first = report.lines().next()?;
    let size_text = first.split("page size of ").nth(1)?.split_whitespace().next()?;
    let page_size = size_text.parse::<usize>().ok()?;
    let pages = |name: &str| {
        let line = report.lines().find(|l| l.starts_with(name))?;
        line[name.len()..].trim().trim_end_matches('.').parse::<usize>().ok()
    };
    let free = pages("Pages free:")?;
    let more = pages("Pages speculative:").unwrap_or(0) + pages("Pages inactive:").unwrap_or(0);
    Some(free.saturating_add(more).saturating_mul(page_size))
}

/// The value of the line `name   N kB` of a `/proc` file, in bytes.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn kib_field(text: &str, name: &str) -> Option<usize> {
    let line = text.lines().find(|l| l.starts_with(name))?;
    let kib = line[name.len()..].split_whitespace().next()?.parse::<usize>().ok()?;
    Some(kib.saturating_mul(1024))
}

/// The root of the cgroup v2 hierarchy.
const CGROUP_ROOT: &str = "/sys/fs/cgroup";

/// What the process's control group (cgroup v2) may still use, in bytes: for the group
/// and each group above it with a limit, `memory.max` less `memory.current`, the
/// smallest. `None` when no group has a limit or the files cannot be read (cgroup v1,
/// other systems). `read` returns a file's text.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn cgroup_room(read: &dyn Fn(&Path) -> Option<String>) -> Option<usize> {
    let groups = read(Path::new("/proc/self/cgroup"))?;
    let group = groups.lines().find_map(|l| l.strip_prefix("0::"))?;
    let root = Path::new(CGROUP_ROOT);
    let mut dir = root.join(group.trim().trim_start_matches('/'));
    let mut room: Option<usize> = None;
    // Each step goes one folder up, so the loop ends at the root.
    loop {
        let number = |name: &str| read(&dir.join(name))?.trim().parse::<usize>().ok();
        // `memory.max` is "max" where there is no limit, which does not parse.
        if let (Some(max), Some(current)) = (number("memory.max"), number("memory.current")) {
            let left = max.saturating_sub(current);
            room = Some(room.map_or(left, |r| r.min(left)));
        }
        if dir == root || !dir.pop() || !dir.starts_with(root) {
            break;
        }
    }
    room
}

/// `limits` with the byte bounds of one conversion (text pool, output, included
/// sources) lowered to `budget` bytes, so no single conversion is allowed more than the
/// whole budget. A `budget` of 0 leaves the limits as they are.
pub fn limits_within(limits: Limits, budget: usize) -> Limits {
    if budget == 0 {
        return limits;
    }
    Limits {
        max_text_bytes: limits.max_text_bytes.min(budget),
        max_output_bytes: limits.max_output_bytes.min(budget),
        max_included_bytes: limits.max_included_bytes.min(budget),
        ..limits
    }
}

#[cfg(test)]
mod memory_tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use super::{cgroup_room, kib_field, vm_stat_available};

    #[test]
    fn proc_fields_are_read_in_bytes() {
        let info = "MemTotal:       16 kB\nMemAvailable:    8 kB\n";
        assert_eq!(kib_field(info, "MemAvailable:"), Some(8 * 1024));
        assert_eq!(kib_field("VmHWM:\t  123 kB\n", "VmHWM:"), Some(123 * 1024));
        assert_eq!(kib_field(info, "VmHWM:"), None);
    }

    fn files(entries: &[(&str, &str)]) -> impl Fn(&Path) -> Option<String> {
        let map: BTreeMap<String, String> =
            entries.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |path: &Path| map.get(path.to_str()?).cloned()
    }

    #[test]
    fn a_vm_stat_report_gives_the_free_speculative_and_inactive_pages() {
        let report = "Mach Virtual Memory Statistics: (page size of 16384 bytes)\n\
                      Pages free:                               10000.\n\
                      Pages active:                            500000.\n\
                      Pages inactive:                           20000.\n\
                      Pages speculative:                         3000.\n\
                      Pages purgeable:                           9999.\n";
        assert_eq!(vm_stat_available(report), Some(33000 * 16384));
        assert_eq!(vm_stat_available("Pages free: 1.\n"), None, "no page size");
        assert_eq!(vm_stat_available("(page size of 4096 bytes)\nPages active: 1.\n"), None);
    }

    #[test]
    fn the_tightest_group_limit_counts() {
        let read = files(&[
            ("/proc/self/cgroup", "0::/a/b\n"),
            ("/sys/fs/cgroup/a/b/memory.max", "max\n"),
            ("/sys/fs/cgroup/a/b/memory.current", "100\n"),
            ("/sys/fs/cgroup/a/memory.max", "1000\n"),
            ("/sys/fs/cgroup/a/memory.current", "400\n"),
        ]);
        assert_eq!(cgroup_room(&read), Some(600));
    }

    #[test]
    fn no_limit_and_no_cgroup_v2_give_none() {
        let unlimited = files(&[
            ("/proc/self/cgroup", "0::/user.slice\n"),
            ("/sys/fs/cgroup/user.slice/memory.max", "max\n"),
            ("/sys/fs/cgroup/user.slice/memory.current", "5\n"),
        ]);
        assert_eq!(cgroup_room(&unlimited), None);
        let v1 = files(&[("/proc/self/cgroup", "4:memory:/docker/x\n")]);
        assert_eq!(cgroup_room(&v1), None);
        let namespaced = files(&[
            ("/proc/self/cgroup", "0::/\n"),
            ("/sys/fs/cgroup/memory.max", "2000\n"),
            ("/sys/fs/cgroup/memory.current", "2500\n"),
        ]);
        assert_eq!(cgroup_room(&namespaced), Some(0));
    }
}
