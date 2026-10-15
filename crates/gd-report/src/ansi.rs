//! ANSI styling for terminals: the style codes reports and command lines use, text in a
//! style, and whether a stream should be styled at all. Words never depend on styling,
//! so a caller passes `color: false` and gets plain text.
//!
//! ```
//! use gd_report::ansi;
//! assert_eq!(ansi::styled("ok", ansi::GREEN, true), "\x1b[32mok\x1b[0m");
//! assert_eq!(ansi::styled("ok", ansi::GREEN, false), "ok");
//! ```

use crate::SymbolEnv;

/// Bold red, for errors.
pub const BOLD_RED: &str = "1;31";
/// Bold yellow, for warnings.
pub const BOLD_YELLOW: &str = "1;33";
/// Bold cyan, for notes and names.
pub const BOLD_CYAN: &str = "1;36";
/// Cyan, for places.
pub const CYAN: &str = "36";
/// Red, for carets and failures.
pub const RED: &str = "31";
/// Green, for hints and success.
pub const GREEN: &str = "32";
/// Faint, for codes and labels.
pub const FAINT: &str = "2";

/// `text` in the style `code` (one of this module's constants) when `color`, else `text`
/// as it is; [`write_styled`] appends to a buffer instead.
pub fn styled(text: &str, code: &str, color: bool) -> String {
    let mut out = String::with_capacity(text.len() + 12);
    write_styled(&mut out, text, code, color);
    out
}

/// Appends `text` to `out` in the style `code` when `color`, else as it is.
pub fn write_styled(out: &mut String, text: &str, code: &str, color: bool) {
    if color {
        out.push_str("\x1b[");
        out.push_str(code);
        out.push('m');
        out.push_str(text);
        out.push_str("\x1b[0m");
    } else {
        out.push_str(text);
    }
}

/// Whether `--color=auto` styles a stream: only a terminal (`is_terminal`), and only when
/// `env` has no `NO_COLOR` and `TERM` is not `dumb`.
pub fn color_for(is_terminal: bool, env: &SymbolEnv) -> bool {
    is_terminal && !env.no_color && env.term.as_deref() != Some("dumb")
}

/// [`color_for`] with this process's environment.
pub fn terminal_color(is_terminal: bool) -> bool {
    is_terminal && color_for(true, &SymbolEnv::from_process())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_plain_terminal_is_styled() {
        let env = SymbolEnv::default();
        assert!(color_for(true, &env));
        assert!(!color_for(false, &env));
        assert!(!color_for(true, &SymbolEnv { no_color: true, ..SymbolEnv::default() }));
        let dumb = SymbolEnv { term: Some("dumb".into()), ..SymbolEnv::default() };
        assert!(!color_for(true, &dumb));
        let xterm = SymbolEnv { term: Some("xterm".into()), ..SymbolEnv::default() };
        assert!(color_for(true, &xterm));
    }

    #[test]
    fn styling_wraps_the_text_in_codes() {
        assert_eq!(styled("x", BOLD_CYAN, true), "\x1b[1;36mx\x1b[0m");
        assert_eq!(styled("x", BOLD_CYAN, false), "x");
    }
}
