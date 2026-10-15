//! A build holds at most its budget of memory (`--budget`). Before it starts, the
//! budget is compared with the memory the machine has free; when a build reaches its
//! budget, it stops, or, asked, raises the budget. Asking needs a person at a terminal
//! (standard input and standard error); a build without one never waits for an answer:
//! it follows `--budget`, and stops at it with a problem that names a budget that fits.

use std::sync::{Arc, Mutex};

use gd_host::{Problem, Refusal, RefusalChoice};
use gd_report::{Report, Style};

const MIB: usize = 1 << 20;

/// How the memory questions reach a person: the terminal, or a test's script.
pub(crate) trait Ask: Send {
    /// Shows `text` (a report, the choices, a question).
    fn show(&mut self, text: &str);
    /// Reads one answer; `None` at the end of input.
    fn answer(&mut self) -> Option<String>;
}

/// The terminal: questions on standard error, answers from standard input.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TerminalAsk;

impl Ask for TerminalAsk {
    fn show(&mut self, text: &str) {
        eprint!("{text}");
    }

    fn answer(&mut self) -> Option<String> {
        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim().to_string()),
        }
    }
}

/// What the questions know, and what the answers were, across the builds of one
/// command (serve rebuilds): a raised budget holds for the builds after it.
pub(crate) struct MemoryState {
    pub ask: Box<dyn Ask>,
    pub color: bool,
    /// The memory the machine had free when last measured; `None` when unknown.
    pub available: Option<usize>,
    /// The budget a person raised it to.
    pub raised: Option<usize>,
}

/// The shared state the budget's policy (called from reading threads) asks through.
pub(crate) type Memory = Arc<Mutex<MemoryState>>;

/// The memory state of a command on the terminal.
pub(crate) fn terminal_memory(color: bool) -> Memory {
    Arc::new(Mutex::new(MemoryState {
        ask: Box::new(TerminalAsk),
        color,
        available: None,
        raised: None,
    }))
}

/// What a person chose about a budget larger than the free memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Start {
    /// Go on with the budget.
    Continue,
    /// Use this budget, in bytes.
    Lower(usize),
    /// Do not build.
    Stop,
}

/// Asks what to do with a budget larger than the free memory: continue, lower it to
/// what fits (or to a number of MiB), or stop. The end of input stops.
pub(crate) fn start_question(state: &mut MemoryState, budget: usize) -> Start {
    let available = state.available.unwrap_or(0);
    let fit = gd_host::fitting_budget(available);
    let report = low_memory_report(budget, available, true);
    let text = gd_report::render_text(&report, &Style { color: state.color, width: 80 });
    state.ask.show(&text);
    state.ask.show(&format!("\n  [c] Continue with the budget of {} MiB\n", budget / MIB));
    if fit > 0 {
        state.ask.show(&format!(
            "  [l] Lower the budget to {} MiB, which fits the free memory\n",
            fit / MIB
        ));
    }
    state.ask.show("  [s] Stop\n\n");
    let choices = if fit > 0 { "c/l/s" } else { "c/s" };
    for _ in 0..3 {
        state
            .ask
            .show(&format!("What should the build do? [{choices}, or a budget in MiB] (c): "));
        let Some(line) = state.ask.answer() else {
            state.ask.show("\n");
            return Start::Stop;
        };
        match line.to_lowercase().as_str() {
            "" | "c" => return Start::Continue,
            "l" if fit > 0 => return Start::Lower(fit),
            "s" => return Start::Stop,
            answer => {
                if let Some(bytes) = budget_answer(answer) {
                    return Start::Lower(bytes);
                }
            }
        }
        state.ask.show(&format!("Please answer {choices}, or give a budget in MiB.\n"));
    }
    Start::Stop
}

/// A budget answered in MiB, from 1 MiB to 1 TiB, as bytes.
fn budget_answer(answer: &str) -> Option<usize> {
    answer.parse::<usize>().ok().filter(|n| (1..=1 << 20).contains(n)).map(|n| n * MIB)
}

/// Asks what to do with a reservation the budget cannot hold: raise the budget (to the
/// one every hint names, or to a number of MiB that holds the step), or stop. The end of
/// input stops the build.
pub(crate) fn ask_refusal(state: &mut MemoryState, r: &Refusal) -> RefusalChoice {
    let (raise, fits) = gd_host::suggest_budget(r.used, r.need, r.limit, state.available);
    let needed = r.used.saturating_add(r.need);
    let report = refusal_report(r, state.available);
    let text = gd_report::render_text(&report, &Style { color: state.color, width: 80 });
    state.ask.show(&text);
    let swaps = if fits { "" } else { " (more than the free memory: the system will swap)" };
    state
        .ask
        .show(&format!("\n  [r] Raise the budget to {} MiB, and go on{swaps}\n", raise / MIB));
    state.ask.show("  [s] Stop the build\n\n");
    for _ in 0..3 {
        state.ask.show("What should the build do? [r/s, or a budget in MiB] (r): ");
        let Some(line) = state.ask.answer() else {
            state.ask.show("\n");
            return RefusalChoice::Stop;
        };
        match line.to_lowercase().as_str() {
            "" | "r" => return RefusalChoice::Raise(raise),
            "s" => return RefusalChoice::Stop,
            answer => {
                let bytes = budget_answer(answer).filter(|&b| b >= needed && b > r.limit);
                if let Some(bytes) = bytes {
                    return RefusalChoice::Raise(bytes);
                }
            }
        }
        let least = needed.max(r.limit + 1).div_ceil(MIB);
        state
            .ask
            .show(&format!("Please answer r or s, or give a budget of at least {least} MiB.\n"));
    }
    RefusalChoice::Stop
}

/// Warns of a budget larger than the free memory; the hint names the budget that fits
/// there (`gd_host::fitting_budget`).
pub(crate) fn low_memory_report(budget: usize, available: usize, asking: bool) -> Report {
    let fit = gd_host::fitting_budget(available) / MIB;
    let hint = if fit == 0 {
        "Free memory before building: there is too little free memory to suggest a budget \
         that fits it."
            .to_string()
    } else if asking {
        format!("Next time, choose ahead with --budget={fit}.")
    } else {
        format!("Lower the budget with --budget={fit}, which fits the free memory.")
    };
    let message = format!(
        "The build may hold up to {} MiB of memory (its budget), but this machine has only \
         {} MiB free now. Past that, the system swaps, and everything slows down.",
        budget / MIB,
        available / MIB
    );
    Report::warning("host.memory.low", "LITTLE MEMORY FREE", &message).hint(&hint)
}

/// Explains a refusal as the build's report would, with what it needs.
pub(crate) fn refusal_report(r: &Refusal, available: Option<usize>) -> Report {
    let left = r.limit.saturating_sub(r.used);
    let free = available
        .map(|a| format!(" This machine has {} MiB of memory free.", a / MIB))
        .unwrap_or_default();
    let message = format!(
        "The build needs {} more memory, but the budget of {} MiB has {} left.{free}",
        mib_text(r.need),
        r.limit / MIB,
        mib_text(left)
    );
    Report::warning("host.budget", "MEMORY BUDGET REACHED", &message)
        .hint(&next_time_hint(r, available))
}

/// How to choose ahead of a refusal: the budget the step fits in (`suggest_budget`), the
/// one `[r]` offers.
pub(crate) fn next_time_hint(r: &Refusal, available: Option<usize>) -> String {
    let (budget, fits) = gd_host::suggest_budget(r.used, r.need, r.limit, available);
    let mib = budget / MIB;
    if fits {
        format!("Next time, choose ahead with --budget={mib}.")
    } else {
        format!(
            "Next time, free memory first and choose ahead with --budget={mib}: that is more \
             than the memory this machine has free."
        )
    }
}

/// A size in MiB with a tenth, rounded up, so a need never reads as less.
pub(crate) fn mib_text(n: usize) -> String {
    if n > usize::MAX / 16 {
        return format!("{} MiB", n / MIB);
    }
    let tenths = (n * 10).div_ceil(MIB);
    if tenths.is_multiple_of(10) {
        format!("{} MiB", tenths / 10)
    } else {
        format!("{}.{} MiB", tenths / 10, tenths % 10)
    }
}

/// A build the person stopped before it started, for lack of memory; the hint names
/// the budget that fits the free memory.
pub(crate) fn stopped_problem(available: usize) -> Problem {
    let fit = gd_host::fitting_budget(available) / MIB;
    let hint = if fit == 0 {
        low_memory_report(0, available, false).hints.join(" ")
    } else {
        format!("Build again with --budget={fit}, which fits the free memory.")
    };
    Problem::new(
        "host.memory.low",
        "BUILD NOT STARTED",
        "",
        "You chose to stop the build: its budget is larger than the memory this machine \
         has free.",
        hint,
        3,
    )
}

#[cfg(test)]
mod tests;
