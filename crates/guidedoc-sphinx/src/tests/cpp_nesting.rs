//! Tests of deep C++ nesting. The C++ parser is recursive
//! descent over nested templates, declarators, and expressions. Nesting past max_depth,
//! or past the reader's stack budget when max_depth is huge, is a Limit, never a stack
//! overflow.

use guidedoc::default_limits;

use crate::tests::*;

fn nested(before: &str, open: &str, n: usize, middle: &str, close: &str, after: &str) -> String {
    format!("{before}{}{middle}{}{after}", open.repeat(n), close.repeat(n))
}

/// Reads with `max_depth` on a thread with a large stack: the codes and whether the
/// read succeeded.
fn read_limited(text: String, max_depth: usize) -> (String, bool) {
    let worker = std::thread::Builder::new().stack_size(256 << 20).spawn(move || {
        let mut request = guidedoc::default_request("test.rst", &text, "sphinx", "html");
        request.limits.max_depth = max_depth;
        // Debug builds have larger frames; release builds hold the default budget.
        if cfg!(debug_assertions) {
            request.limits.stack_bytes = 16 * guidedoc::DEFAULT_STACK_BYTES;
        }
        let r = read_request(&request, &crate::READER);
        (r.codes, r.ok)
    });
    worker.expect("thread").join().expect("no panic")
}

fn deep_declarations(n: usize) -> Vec<String> {
    vec![
        nested(".. cpp:function:: void f(", "A<", n, "int", ">", ")\n"),
        nested(".. cpp:function:: void f(int ", "*", n, "p", "", ")\n"),
        nested(".. cpp:function:: void f(int a = ", "(", n, "1", ")", ")\n"),
        nested(".. cpp:member:: int x = ", "{", n, "1", "}", "\n"),
        nested(".. cpp:member:: int x = ", "-", n, "1", "", "\n"),
        nested(".. cpp:function:: ", "template<", n, "typename T", "> class", " void f()\n"),
        nested("See :cpp:func:`", "A<", n, "int", ">", "::f`.\n"),
    ]
}

#[test]
fn deeply_nested_cpp_declarations_hit_the_depth_limit() {
    for text in deep_declarations(1000) {
        let head: String = text.chars().take(40).collect();
        let (codes, ok) = read_limited(text, default_limits().max_depth);
        assert!(!ok && codes.contains("core.limit.depth"), "{head}: {codes}");
    }
}

#[test]
fn cpp_nesting_past_a_huge_depth_limit_hits_the_stack_limit() {
    for text in deep_declarations(40_000) {
        let head: String = text.chars().take(40).collect();
        let (codes, ok) = read_limited(text, 10_000_000);
        assert!(!ok && codes.contains("core.limit.stack"), "{head}: {codes}");
    }
}

#[test]
fn moderately_nested_cpp_declarations_still_parse() {
    for text in deep_declarations(12) {
        let head: String = text.chars().take(40).collect();
        let (codes, _) = read_limited(text, default_limits().max_depth);
        assert!(!codes.contains("core.limit"), "{head}: {codes}");
    }
}

/// A fold is tried only when its "..." is there, so nested parentheses take linear time.
#[test]
fn nested_parentheses_in_cpp_expressions_parse_in_linear_time() {
    let text = nested(".. cpp:function:: void f(int a = ", "(", 60, "1", ")", ")\n");
    let (codes, ok) = read_limited(text, default_limits().max_depth);
    assert!(ok && codes.is_empty(), "{codes}");
}
