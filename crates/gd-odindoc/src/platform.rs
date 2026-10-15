//! The targets a declaration exists for, as Odin decides which files and `when`
//! branches a build compiles:
//!
//! - A file whose name ends in `_<os>`, `_<arch>`, or `_<os>_<arch>` before `.odin`
//!   builds only for those targets. `_haiku` and `_essence` name systems this Odin cannot
//!   target, so such files build for none. Any other suffix is part of the name.
//! - `#+build` lines (and the legacy `//+build` comment) narrow that further, as Odin's
//!   parser reads them (see `tags.rs`).
//! - At file scope, `when` conditions made of `ODIN_OS` and `ODIN_ARCH` compared with `==`
//!   or `!=` to `.Name`, joined with `&&`, `||`, `!` and parentheses, choose their branch
//!   per target; any other condition keeps both branches for the file's targets.
//!
//! Only the combinations Odin can build are targets (`Targets::SUPPORTED`).

use crate::ast::{Expr, File, NodeId};
use crate::tokenizer::Tk;

/// An operating system Odin builds for, in Odin's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Os {
    /// Microsoft Windows (`windows`).
    Windows = 1,
    /// macOS and the other Apple systems (`darwin`).
    Darwin,
    /// Linux (`linux`).
    Linux,
    /// FreeBSD (`freebsd`).
    FreeBSD,
    /// OpenBSD (`openbsd`).
    OpenBSD,
    /// NetBSD (`netbsd`).
    NetBSD,
    /// The WebAssembly System Interface (`wasi`).
    Wasi,
    /// JavaScript hosts for WebAssembly (`js`).
    Js,
    /// The Orca platform (`orca`).
    Orca,
    /// No operating system (`freestanding`).
    Freestanding,
}

/// A processor architecture Odin builds for, in Odin's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Arch {
    /// 64-bit x86 (`amd64`).
    Amd64 = 1,
    /// 32-bit x86 (`i386`).
    I386,
    /// 32-bit ARM (`arm32`).
    Arm32,
    /// 64-bit ARM (`arm64`).
    Arm64,
    /// 32-bit WebAssembly (`wasm32`).
    Wasm32,
    /// 64-bit WebAssembly with 32-bit pointers (`wasm64p32`).
    Wasm64p32,
    /// 64-bit RISC-V (`riscv64`).
    Riscv64,
}

impl Os {
    /// Every system, in Odin's order.
    pub const ALL: [Os; 10] = [
        Os::Windows,
        Os::Darwin,
        Os::Linux,
        Os::FreeBSD,
        Os::OpenBSD,
        Os::NetBSD,
        Os::Wasi,
        Os::Js,
        Os::Orca,
        Os::Freestanding,
    ];

    /// The system's name as Odin's runtime spells it ("Windows", "FreeBSD", "WASI").
    pub fn display_name(self) -> &'static str {
        match self {
            Os::Windows => "Windows",
            Os::Darwin => "Darwin",
            Os::Linux => "Linux",
            Os::FreeBSD => "FreeBSD",
            Os::OpenBSD => "OpenBSD",
            Os::NetBSD => "NetBSD",
            Os::Wasi => "WASI",
            Os::Js => "JS",
            Os::Orca => "Orca",
            Os::Freestanding => "Freestanding",
        }
    }

    /// The system named `name` in any case ("linux", "Windows"), if Odin knows it.
    pub fn named(name: &str) -> Option<Os> {
        Os::ALL.into_iter().find(|o| o.display_name().eq_ignore_ascii_case(name))
    }
}

impl Arch {
    /// Every architecture, in Odin's order.
    pub const ALL: [Arch; 7] = [
        Arch::Amd64,
        Arch::I386,
        Arch::Arm32,
        Arch::Arm64,
        Arch::Wasm32,
        Arch::Wasm64p32,
        Arch::Riscv64,
    ];

    /// The architecture's name, as Odin spells it ("amd64").
    pub fn name(self) -> &'static str {
        match self {
            Arch::Amd64 => "amd64",
            Arch::I386 => "i386",
            Arch::Arm32 => "arm32",
            Arch::Arm64 => "arm64",
            Arch::Wasm32 => "wasm32",
            Arch::Wasm64p32 => "wasm64p32",
            Arch::Riscv64 => "riscv64",
        }
    }

    /// The architecture named `name` exactly ("arm64"), if Odin knows it.
    pub fn named(name: &str) -> Option<Arch> {
        Arch::ALL.into_iter().find(|a| a.name() == name)
    }

    fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

/// A set of (system, architecture) pairs: per system, a bit for each architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Targets([u8; 11]);

const ALL_ARCHS: u8 = 0b1111_1110;

impl Targets {
    /// The targets Odin can build for.
    pub const SUPPORTED: Targets = Targets([
        0,
        0b0001_0110, // Windows: amd64, i386, arm64
        0b0001_0010, // Darwin: amd64, arm64
        0b1001_1110, // Linux: amd64, i386, arm32, arm64, riscv64
        0b0001_0110, // FreeBSD: amd64, i386, arm64
        0b0001_0010, // OpenBSD: amd64, arm64
        0b0001_0010, // NetBSD: amd64, arm64
        0b0110_0000, // WASI: wasm32, wasm64p32
        0b0110_0000, // JS: wasm32, wasm64p32
        0b0010_0000, // Orca: wasm32
        0b1111_1110, // Freestanding: every architecture
    ]);

    /// No targets.
    pub const NONE: Targets = Targets([0; 11]);

    /// Whether the set holds `os` on `arch`.
    pub fn contains(&self, os: Os, arch: Arch) -> bool {
        self.0[os as usize] & arch.bit() != 0
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|&b| b == 0)
    }

    /// The pairs in both sets.
    pub fn and(self, other: Targets) -> Targets {
        Targets(std::array::from_fn(|i| self.0[i] & other.0[i]))
    }

    /// The pairs in either set.
    pub fn or(self, other: Targets) -> Targets {
        Targets(std::array::from_fn(|i| self.0[i] | other.0[i]))
    }

    /// The supported targets of one system.
    pub(crate) fn of_os(os: Os) -> Targets {
        let mut out = Targets::NONE;
        out.0[os as usize] = Targets::SUPPORTED.0[os as usize];
        out
    }

    /// The supported targets of one architecture, on every system.
    pub(crate) fn of_arch(arch: Arch) -> Targets {
        Targets(std::array::from_fn(|i| Targets::SUPPORTED.0[i] & arch.bit()))
    }

    /// The pairs whose system is in `os` and architecture in `archs` (bit sets as
    /// `#+build` lines make them: bit `os as usize`, bit `arch as u8`).
    pub(crate) fn from_bits(os: u16, archs: u8) -> Targets {
        Targets(std::array::from_fn(|i| {
            let on = os & (1 << i) != 0;
            if on { Targets::SUPPORTED.0[i] & archs } else { 0 }
        }))
    }

    /// The targets named for people, relative to `all` (a package's targets): "" when
    /// they are all of them; else each system in Odin's order, with its architectures in
    /// parentheses when it has only some of those `all` gives it.
    pub fn display(self, all: Targets) -> String {
        let mut out = String::new();
        if self == all {
            return out;
        }
        for os in Os::ALL {
            let archs = self.0[os as usize];
            if archs == 0 {
                continue;
            }
            if !out.is_empty() {
                out.push_str(", ");
            }
            out.push_str(os.display_name());
            if archs == all.0[os as usize] {
                continue;
            }
            let names: Vec<&str> =
                Arch::ALL.into_iter().filter(|a| archs & a.bit() != 0).map(Arch::name).collect();
            out.push_str(" (");
            out.push_str(&names.join(", "));
            out.push(')');
        }
        out
    }
}

impl std::ops::Not for Targets {
    type Output = Targets;

    /// Every supported pair outside the set.
    fn not(self) -> Targets {
        Targets(std::array::from_fn(|i| Targets::SUPPORTED.0[i] & !self.0[i]))
    }
}

/// Every architecture bit, for a `#+build` line that names none.
pub(crate) const EVERY_ARCH: u8 = ALL_ARCHS;

/// The targets a file name's suffix allows: `_os`, `_arch`, or `_os_arch`.
pub(crate) fn suffix_targets(filename: &str) -> Targets {
    let stem = filename.strip_suffix(".odin").unwrap_or(filename);
    let parts: Vec<&str> = stem.split('_').collect();
    let Some((&last, rest)) = parts.split_last() else {
        return Targets::SUPPORTED;
    };
    if rest.is_empty() {
        return Targets::SUPPORTED;
    }
    if last == "haiku" || last == "essence" {
        return Targets::NONE;
    }
    if let Some(os) = os_suffix(last) {
        return Targets::of_os(os);
    }
    let Some(arch) = Arch::named(last) else {
        return Targets::SUPPORTED;
    };
    if let [.., before] = rest
        && rest.len() >= 2
    {
        if *before == "haiku" || *before == "essence" {
            return Targets::NONE;
        }
        if let Some(os) = os_suffix(before) {
            return Targets::of_os(os).and(Targets::of_arch(arch));
        }
    }
    Targets::of_arch(arch)
}

/// The system a lowercase file-name suffix names ("windows", "freebsd").
fn os_suffix(name: &str) -> Option<Os> {
    Os::ALL.into_iter().find(|o| o.display_name().to_ascii_lowercase() == name)
}

/// How deeply a `when` condition is followed: `a && b && ...` is a left-nested tree as
/// deep as it is long.
pub(crate) const MAX_WHEN_DEPTH: u32 = 64;

/// The targets a `when` condition holds for; `None` when it is not made of `ODIN_OS`
/// and `ODIN_ARCH` comparisons alone, or nests deeper than `MAX_WHEN_DEPTH`.
pub(crate) fn when_targets(file: &File, src: &str, id: NodeId, depth: u32) -> Option<Targets> {
    if depth > MAX_WHEN_DEPTH {
        return None;
    }
    match file.node(id).kind {
        Expr::Paren(inner) => when_targets(file, src, inner, depth + 1),
        Expr::Unary { op: Tk::Not, expr: Some(inner) } => {
            Some(!when_targets(file, src, inner, depth + 1)?)
        }
        Expr::Binary { op: Tk::CmpAnd, left, right } => {
            let l = when_targets(file, src, left, depth + 1)?;
            Some(l.and(when_targets(file, src, right, depth + 1)?))
        }
        Expr::Binary { op: Tk::CmpOr, left, right } => {
            let l = when_targets(file, src, left, depth + 1)?;
            Some(l.or(when_targets(file, src, right, depth + 1)?))
        }
        Expr::Binary { op: Tk::CmpEq, left, right } => comparison(file, src, left, right),
        Expr::Binary { op: Tk::NotEq, left, right } => Some(!comparison(file, src, left, right)?),
        _ => None,
    }
}

/// The targets of `ODIN_OS == .Name` or `ODIN_ARCH == .name`, either way round.
fn comparison(file: &File, src: &str, left: NodeId, right: NodeId) -> Option<Targets> {
    let (mut subject, mut value) = (left, right);
    if matches!(file.node(value).kind, Expr::Ident { .. }) {
        (subject, value) = (right, left);
    }
    let Expr::Ident { .. } = file.node(subject).kind else {
        return None;
    };
    let Expr::ImplicitSelector { field } = file.node(value).kind else {
        return None;
    };
    let text = |id: NodeId| {
        let s = file.node(id).span;
        src.get(s.start as usize..s.end as usize).unwrap_or("")
    };
    let name = text(field);
    match text(subject) {
        // A system this Odin does not know has no targets.
        "ODIN_OS" => Some(Os::named(name).map_or(Targets::NONE, Targets::of_os)),
        "ODIN_ARCH" => Some(Arch::named(name).map_or(Targets::NONE, Targets::of_arch)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suffixes_choose_targets() {
        assert_eq!(suffix_targets("file.odin"), Targets::SUPPORTED);
        assert_eq!(suffix_targets("file_linux.odin"), Targets::of_os(Os::Linux));
        assert_eq!(suffix_targets("file_haiku.odin"), Targets::NONE);
        let darwin_arm = suffix_targets("file_darwin_arm64.odin");
        assert_eq!(darwin_arm.display(Targets::SUPPORTED), "Darwin (arm64)");
        assert_eq!(suffix_targets("big_endian.odin"), Targets::SUPPORTED);
        assert_eq!(!!Targets::of_os(Os::Windows), Targets::of_os(Os::Windows));
    }
}
