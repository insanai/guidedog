use super::*;

/// A fake terminal: it answers with its lines in turn, and keeps what was shown.
struct Script {
    answers: Vec<&'static str>,
    shown: Arc<Mutex<String>>,
}

impl Ask for Script {
    fn show(&mut self, text: &str) {
        if let Ok(mut shown) = self.shown.lock() {
            shown.push_str(text);
        }
    }

    fn answer(&mut self) -> Option<String> {
        (!self.answers.is_empty()).then(|| self.answers.remove(0).to_string())
    }
}

fn scripted(answers: &[&'static str], available: usize) -> (MemoryState, Arc<Mutex<String>>) {
    let shown = Arc::new(Mutex::new(String::new()));
    let ask = Script { answers: answers.to_vec(), shown: Arc::clone(&shown) };
    let state =
        MemoryState { ask: Box::new(ask), color: false, available: Some(available), raised: None };
    (state, shown)
}

fn text(shown: &Arc<Mutex<String>>) -> String {
    shown.lock().map(|s| s.clone()).unwrap_or_default()
}

/// Measured zero must not offer a budget that fits the free memory, nor `--budget=0`.
#[test]
fn empty_memory_measurements_offer_honest_choices() {
    let (mut state, shown) = scripted(&["l", "s"], 0);
    assert_eq!(start_question(&mut state, 1024 * MIB), Start::Stop);
    let shown = text(&shown);
    assert!(!shown.contains("[l]") && !shown.contains("--budget=0"), "{shown}");
    assert!(shown.contains("[c/s,"), "{shown}");
    assert!(!stopped_problem(0).hint.contains("--budget=0"));
}

/// At its budget, a build asks: raise the budget (to the value it offers, or one given),
/// or stop; an answer it cannot use is asked again, and the end of input stops.
#[test]
fn refusals_ask_what_to_do() {
    let r = Refusal { need: 60 * MIB, used: 3 * MIB, limit: 4 * MIB };
    let (mut state, shown) = scripted(&[""], 4096 * MIB);
    assert_eq!(ask_refusal(&mut state, &r), RefusalChoice::Raise(256 * MIB));
    let shown = text(&shown);
    assert!(shown.contains("MEMORY BUDGET REACHED") && shown.contains("60 MiB more memory"));
    assert!(shown.contains("Raise the budget to 256 MiB"), "{shown}");
    let (mut state, shown) = scripted(&["maybe", "s"], 4096 * MIB);
    assert_eq!(ask_refusal(&mut state, &r), RefusalChoice::Stop);
    assert!(text(&shown).contains("Please answer"));
    let (mut state, _) = scripted(&["200"], 4096 * MIB);
    assert_eq!(ask_refusal(&mut state, &r), RefusalChoice::Raise(200 * MIB));
    // Less than needed: asked again, then the input ends.
    let (mut state, _) = scripted(&["10"], 4096 * MIB);
    assert_eq!(ask_refusal(&mut state, &r), RefusalChoice::Stop);
    // More than the free memory: the build says the system swaps.
    let (mut state, shown) = scripted(&[""], 32 * MIB);
    assert!(matches!(ask_refusal(&mut state, &r), RefusalChoice::Raise(_)));
    assert!(text(&shown).contains("will swap"));
}

/// A budget larger than the free memory is asked about before the build starts.
#[test]
fn budgets_over_free_memory_are_asked_about() {
    for (answer, expected) in
        [("", Start::Continue), ("l", Start::Lower(528 * MIB)), ("s", Start::Stop)]
    {
        let (mut state, shown) = scripted(&[answer], 600 * MIB);
        assert_eq!(start_question(&mut state, 1024 * MIB), expected);
        assert!(text(&shown).contains("LITTLE MEMORY FREE"));
    }
    let (mut state, _) = scripted(&["300"], 600 * MIB);
    assert_eq!(start_question(&mut state, 1024 * MIB), Start::Lower(300 * MIB));
}

/// The budget a question offers is the one every hint names.
#[test]
fn questions_and_hints_name_the_same_budget() {
    let r = Refusal { need: 60 * MIB, used: 3 * MIB, limit: 4 * MIB };
    let (mut state, shown) = scripted(&["r"], 4096 * MIB);
    assert_eq!(ask_refusal(&mut state, &r), RefusalChoice::Raise(256 * MIB));
    let shown = text(&shown);
    assert!(shown.contains("Next time, choose ahead with --budget=256."), "{shown}");
    assert_eq!(
        low_memory_report(1024 * MIB, 600 * MIB, false).hints,
        ["Lower the budget with --budget=528, which fits the free memory."]
    );
    assert_eq!(
        low_memory_report(1024 * MIB, 600 * MIB, true).hints,
        ["Next time, choose ahead with --budget=528."]
    );
    assert_eq!(
        stopped_problem(600 * MIB).hint,
        "Build again with --budget=528, which fits the free memory."
    );
    assert_eq!(mib_text(MIB + 1), "1.1 MiB");
    assert_eq!(mib_text(3 * MIB), "3 MiB");
}
