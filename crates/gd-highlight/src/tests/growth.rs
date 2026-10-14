//! Ported from `growth_test.odin`: scanning work grows linearly with the input.
//!
//! The tests count dispatches and cursor inspections rather than elapsed time, which
//! varies with the machine.

use super::robust::OPENERS;
use crate::scanner::WORK;
use crate::*;

fn scan_work(lexer: Lexer, code: &str) -> u64 {
    WORK.with(|w| w.set(0));
    scan_tokens(lexer, code).for_each(drop);
    WORK.with(|w| w.get())
}

#[test]
fn huge_lines_have_bounded_scanner_work() {
    const SIZE: usize = 64 * 1024;
    for opener in OPENERS {
        let code: String = opener.chars().cycle().take(SIZE).collect();
        for lexer in Lexer::ALL {
            let small = scan_work(lexer, &code[..SIZE / 4]);
            let large = scan_work(lexer, &code);
            // Linear growth is 4x; quadratic is 16x. Allow fixed lookahead and tails.
            assert!(
                large <= 8 * small + 4096,
                "{lexer:?} on repeated {opener:?}: work grew from {small} to {large} for 4x input"
            );
        }
    }
}

/// Shows that repeated whole-line lookahead is caught by the work probe.
#[test]
fn work_probe_detects_repeated_whole_line_searches() {
    let costs = [128, 512].map(|size| {
        let code = "*".repeat(size);
        let mut s = scan_tokens(Lexer::Css, &code);
        WORK.with(|w| w.set(0));
        while s.pos < s.limit {
            let _ = s.line_end(s.pos);
            s.emit_n(1, TokenKind::Text);
        }
        WORK.with(|w| w.get())
    });
    assert!(costs[1] > 8 * costs[0] + 4096);
}
