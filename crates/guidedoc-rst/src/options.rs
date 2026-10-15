//! Checks of directive option values against their types.

use crate::lines::{is_space, trim_space};
use crate::names::{all_digits, parse_int};
use crate::specs::{OptSpec, OptType, equal_fold};
use crate::substitutions::unicode_code;

/// Checks value `v` against the option's type: `None` when it is valid, else why not.
pub fn option_problem(s: &OptSpec, v: &str) -> Option<&'static str> {
    let bad = |valid: bool, why: &'static str| if valid { None } else { Some(why) };
    match s.kind {
        OptType::Flag => bad(v.is_empty(), "the option takes no value"),
        OptType::Text | OptType::Encoding => None,
        OptType::TextRequired | OptType::Path | OptType::Name => {
            bad(!v.is_empty(), "a value is required")
        }
        OptType::Int => bad(is_int(v, true), "expected an integer"),
        OptType::NonnegInt => bad(is_int(v, false), "expected a nonnegative integer"),
        OptType::PosInt => {
            bad(is_int(v, false) && parse_int(v) >= 1, "expected a positive integer")
        }
        OptType::OptInt => bad(v.is_empty() || is_int(v, false), "expected an integer or nothing"),
        OptType::Length => bad(is_length(v, false), "expected a length, such as 12px or 3em"),
        OptType::LengthPercent => bad(is_length(v, true), "expected a length or a percentage"),
        OptType::Figwidth => {
            bad(v == "image" || is_length(v, true), "expected a length, a percentage, or image")
        }
        OptType::Percentage => {
            let w = v.strip_suffix('%').map_or(v, trim_space);
            bad(is_int(w, false), "expected a percentage")
        }
        OptType::Class => bad(valid_classes(v), "expected class names"),
        OptType::Choice => bad(is_choice(v, s.choices), "the value is not one of the choices"),
        OptType::Char => bad(is_char_option(v), "expected one character, tab, or space"),
        OptType::Widths => bad(is_widths(v), "expected auto, grid, or positive integers"),
    }
}

fn is_int(v: &str, signed: bool) -> bool {
    let s = if signed { v.strip_prefix(['-', '+']).unwrap_or(v) } else { v };
    all_digits(s) && s.len() <= 12
}

fn is_length(v: &str, percent: bool) -> bool {
    let b = v.as_bytes();
    let mut i = 0;
    let mut digits = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        digits += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return false;
    }
    match trim_space(&v[i..]) {
        "" | "em" | "ex" | "ch" | "rem" | "vw" | "vh" | "vmin" | "vmax" | "px" | "in" | "cm"
        | "mm" | "Q" | "pt" | "pc" => true,
        "%" => percent,
        _ => false,
    }
}

fn valid_classes(v: &str) -> bool {
    let mut n = 0;
    for word in v.split(|c: char| c.is_ascii() && is_space(c as u8)).filter(|w| !w.is_empty()) {
        if !word.bytes().any(|c| (c | 0x20).is_ascii_lowercase()) {
            return false;
        }
        n += 1;
    }
    n > 0
}

fn is_choice(v: &str, choices: &str) -> bool {
    choices.split(' ').any(|c| equal_fold(v, c))
}

fn is_char_option(v: &str) -> bool {
    v == "tab" || v == "space" || v.chars().count() == 1 || unicode_code(v).is_some()
}

fn is_widths(v: &str) -> bool {
    if v == "auto" || v == "grid" {
        return true;
    }
    let mut n = 0;
    for part in v.split([',', ' ']).filter(|p| !p.is_empty()) {
        if !all_digits(part) || parse_int(part) < 1 {
            return false;
        }
        n += 1;
    }
    n > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specs::{choice, opt};

    #[test]
    fn values_are_checked_by_type() {
        assert!(option_problem(&opt("w", OptType::Length), "12 px").is_none());
        assert!(option_problem(&opt("w", OptType::Length), "12%").is_some());
        assert!(option_problem(&opt("w", OptType::LengthPercent), "50 %").is_none());
        assert!(option_problem(&opt("s", OptType::Percentage), "50 %").is_none());
        assert!(option_problem(&opt("c", OptType::Class), "a 1").is_some());
        assert!(option_problem(&choice("a", "left right"), "LEFT").is_none());
        assert!(option_problem(&opt("w", OptType::Widths), "1, 2 3").is_none());
        assert!(option_problem(&opt("w", OptType::Widths), "0 1").is_some());
        assert!(option_problem(&opt("d", OptType::Char), "0x2c").is_none());
        assert!(option_problem(&opt("i", OptType::Int), "-3").is_none());
    }
}
