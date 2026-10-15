//! The few symbols a command prints around its reports (a tick, a cross, a separator),
//! in Unicode where the terminal can show it and in ASCII where it may not.
//!
//! Terminals that are not UTF-8 show `✓` as mojibake or a question mark, and a dumb
//! terminal or a log reader may not draw it at all; ASCII is always readable.

/// The symbols a command decorates its terminal output with. Plain data: pick
/// [`Symbols::UNICODE`] or [`Symbols::ASCII`], or ask [`terminal_symbols`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Symbols {
    /// Marks a finished step or a passing check.
    pub check: &'static str,
    /// Marks a failed step.
    pub cross: &'static str,
    /// Separates short items on one line.
    pub dot: &'static str,
    /// Ends a shortened text or list.
    pub ellipsis: &'static str,
}

impl Symbols {
    /// The symbols for a UTF-8 terminal.
    pub const UNICODE: Symbols =
        Symbols { check: "✓", cross: "✗", dot: "·", ellipsis: "…" };
    /// The symbols every terminal shows.
    pub const ASCII: Symbols = Symbols { check: "ok", cross: "x", dot: "|", ellipsis: "..." };
}

impl Default for Symbols {
    /// ASCII, which is always safe.
    fn default() -> Symbols {
        Symbols::ASCII
    }
}

/// The environment variables that decide the symbols, read once by the caller so the
/// decision itself is a pure function. Each field is the variable's value, or `None`
/// when it is unset.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolEnv {
    /// `LC_ALL`, which overrides the other locale variables.
    pub lc_all: Option<String>,
    /// `LC_CTYPE`, the locale of the character set.
    pub lc_ctype: Option<String>,
    /// `LANG`, the default locale.
    pub lang: Option<String>,
    /// `TERM`; `dumb` means a terminal without Unicode or styling.
    pub term: Option<String>,
    /// Whether `NO_COLOR` is set: a request for plain output.
    pub no_color: bool,
    /// Whether `WT_SESSION` is set: Windows Terminal, which is always UTF-8.
    pub windows_terminal: bool,
}

impl SymbolEnv {
    /// Reads the variables from the process environment. Never fails: an unreadable
    /// (non-Unicode) value counts as unset.
    pub fn from_process() -> SymbolEnv {
        let var = |name: &str| std::env::var(name).ok();
        SymbolEnv {
            lc_all: var("LC_ALL"),
            lc_ctype: var("LC_CTYPE"),
            lang: var("LANG"),
            term: var("TERM"),
            no_color: std::env::var_os("NO_COLOR").is_some(),
            windows_terminal: std::env::var_os("WT_SESSION").is_some(),
        }
    }
}

/// The symbols for the terminal `env` describes: Unicode only when the locale is UTF-8
/// (or the terminal is Windows Terminal), `TERM` is not `dumb` and `NO_COLOR` is unset.
pub fn symbols_for(env: &SymbolEnv) -> Symbols {
    if env.no_color || env.term.as_deref() == Some("dumb") {
        return Symbols::ASCII;
    }
    if env.windows_terminal || is_utf8_locale(env) { Symbols::UNICODE } else { Symbols::ASCII }
}

/// The symbols for this process's terminal; see [`symbols_for`].
pub fn terminal_symbols() -> Symbols {
    symbols_for(&SymbolEnv::from_process())
}

/// Whether the effective character-set locale names UTF-8. As in C, the first set and
/// non-empty variable of `LC_ALL`, `LC_CTYPE` and `LANG` decides.
fn is_utf8_locale(env: &SymbolEnv) -> bool {
    let locale = [&env.lc_all, &env.lc_ctype, &env.lang]
        .into_iter()
        .flatten()
        .find(|value| !value.is_empty());
    locale.is_some_and(|value| {
        let value = value.to_ascii_lowercase();
        value.contains("utf-8") || value.contains("utf8")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(lang: &str) -> SymbolEnv {
        SymbolEnv { lang: Some(lang.into()), term: Some("xterm".into()), ..Default::default() }
    }

    #[test]
    fn a_utf8_locale_gets_unicode() {
        assert_eq!(symbols_for(&env("en_US.UTF-8")), Symbols::UNICODE);
        assert_eq!(symbols_for(&env("de_DE.utf8")), Symbols::UNICODE);
    }

    #[test]
    fn other_locales_and_plain_terminals_get_ascii() {
        assert_eq!(symbols_for(&env("C")), Symbols::ASCII);
        assert_eq!(symbols_for(&env("en_US.ISO-8859-1")), Symbols::ASCII);
        assert_eq!(symbols_for(&SymbolEnv::default()), Symbols::ASCII);
        let dumb = SymbolEnv { term: Some("dumb".into()), ..env("en_US.UTF-8") };
        assert_eq!(symbols_for(&dumb), Symbols::ASCII);
        let plain = SymbolEnv { no_color: true, ..env("en_US.UTF-8") };
        assert_eq!(symbols_for(&plain), Symbols::ASCII);
    }

    #[test]
    fn lc_all_overrides_lang_and_empty_values_are_skipped() {
        let c = SymbolEnv { lc_all: Some("C".into()), ..env("en_US.UTF-8") };
        assert_eq!(symbols_for(&c), Symbols::ASCII);
        let empty = SymbolEnv { lc_all: Some(String::new()), ..env("en_US.UTF-8") };
        assert_eq!(symbols_for(&empty), Symbols::UNICODE);
        let wt = SymbolEnv { windows_terminal: true, ..SymbolEnv::default() };
        assert_eq!(symbols_for(&wt), Symbols::UNICODE);
    }

    #[test]
    fn ascii_symbols_are_ascii() {
        let s = Symbols::ASCII;
        assert!([s.check, s.cross, s.dot, s.ellipsis].iter().all(|t| t.is_ascii()));
    }
}
