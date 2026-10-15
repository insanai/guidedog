//! Deep trees end in a Limit, never in a stack overflow.

use guidedoc::{NodeKind as K, Policy, Status};

use super::Tb;

const DEEP_QUOTES: usize = 200_000;

fn build_deep(t: &mut Tb<'_, '_>) -> Result<(), guidedoc::Fail> {
    let mut open = Vec::with_capacity(DEEP_QUOTES);
    for _ in 0..DEEP_QUOTES {
        open.push(t.open(K::BlockQuote, 0)?);
    }
    t.para("deep")?;
    while let Some(id) = open.pop() {
        t.close(id)?;
    }
    Ok(())
}

/// With `max_depth` beyond any thread's stack, the stack guard turns a tree nested
/// deeper than the stack budget holds into a Limit.
#[test]
fn deep_trees_past_a_huge_depth_limit_hit_the_stack_limit() {
    let run = || {
        let limits = guidedoc::Limits { max_depth: 1_000_000, ..guidedoc::Limits::default() };
        super::render_limited(build_deep, Policy::default(), limits)
    };
    // Debug builds have large frames: give the conversion room beyond its budget.
    let r = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(run)
        .expect("thread")
        .join()
        .expect("no panic");
    assert_eq!(r.status, Status::Limit);
    let primary = r.reports.iter().find(|d| d.severity == guidedoc::Severity::Error);
    assert_eq!(primary.map(|d| d.code), Some(guidedoc::CORE_STACK.code));
}
