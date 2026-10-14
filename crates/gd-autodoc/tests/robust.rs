//! Python a project documents is the user's text: mutated, every directive over it gives
//! lines or a warning, never a crash.

mod common;

use gd_autodoc::{Config, DirectiveOption, Request, document};

const SEED: &str = r#""""Module doc with :func:`f`."""
from __future__ import annotations
import typing as t
from .sub import thing as other
from dataclasses import dataclass, field

__all__ = ["f", "C", "V"]
V: t.Final[int] = 3  #: The value.
T = t.TypeVar("T", bound="C")

@deco
async def f(a: int, /, b: "str" = 'x', *args: T, c: list[int] | None = None, **kw) -> dict[str, T]:
    """Do f.

    :param a: an int
    """
    return {}

@dataclass(frozen=True)
class C(Base[T], metaclass=Meta):
    """A class."""
    x: int = 1
    y: list[str] = field(default_factory=list)
    def m(self, q=lambda z: z + 1, *, r=(1, 2)) -> "C":
        """Method."""
    @property
    def p(self) -> int: ...
    class Inner(Enum):
        A = 1
"#;

const ALPHABET: &[u8] = b"()[]{}:,.=@*'\"\\#\n \t|<>-+0123456789aTx_";

/// A small deterministic generator (xorshift), so failures reproduce.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n.max(1) as u64) as usize
    }
}

/// The seed with one to five bytes replaced, deleted, or inserted.
fn mutate(rng: &mut Rng) -> Vec<u8> {
    let mut buf = SEED.as_bytes().to_vec();
    for _ in 0..1 + rng.below(5) {
        match rng.below(3) {
            0 => {
                let at = rng.below(buf.len());
                buf[at] = ALPHABET[rng.below(ALPHABET.len())];
            }
            1 => {
                let at = rng.below(buf.len());
                let k = (rng.below(8) + 1).min(buf.len() - at);
                buf.drain(at..at + k);
            }
            _ => {
                let at = rng.below(buf.len() + 1);
                buf.insert(at, ALPHABET[rng.below(ALPHABET.len())]);
            }
        }
    }
    buf
}

#[test]
fn mutated_python_never_crashes() {
    let root = common::temp_root(&[("m.py", "")]);
    let path = root.0.join("m.py");
    let mut rng = Rng(20260930);
    let cases = [
        ("automodule", "m"),
        ("autofunction", "m.f"),
        ("autoclass", "m.C"),
        ("autodata", "m.V"),
        ("automethod", "m.C.m"),
        ("autoattribute", "m.C.x"),
    ];
    let options = vec![DirectiveOption::flag("members"), DirectiveOption::flag("undoc-members")];
    let config = Config::default();
    let mut lines = 0;
    for _ in 0..1500 {
        std::fs::write(&path, mutate(&mut rng)).expect("write");
        let mut session = common::session(&root);
        for (directive, argument) in cases {
            let request = Request {
                directive: directive.into(),
                argument: argument.into(),
                options: options.clone(),
                ..Request::default()
            };
            lines += document(&mut session, &config, &request).lines.len();
        }
    }
    assert!(lines > 0);
}
