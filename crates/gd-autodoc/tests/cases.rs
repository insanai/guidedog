//! The verification project whose autodoc output Sphinx 9.1 made (tests/data): its
//! Python source as data, which the analyser reads and nothing runs. Each directive
//! Sphinx logged is run through `document` and compared line by line.

mod common;

use gd_autodoc::{Config, DirectiveOption, MemberOrder, Request, Typehints, document};

const CASES_INIT: &str = r#""""Cases package."""

from .shapes import Shape as Shape
from .shapes import Square

__all__ = ["Shape", "Square", "helper"]


def helper():
    """Helper."""
"#;

const CASES_SHAPES: &str = r#"from __future__ import annotations

import typing as t

T = t.TypeVar("T")


class Shape(t.Generic[T]):
    """A shape.

    :param sides: How many sides.
    """

    sides: int
    """Number of sides."""

    def __init__(self, sides: int, name: str | None = None) -> None:
        self.sides = sides

    def area(self, scale: float = 1.0) -> float:
        """The area.

        :param scale: A factor.
        """
        return 0.0


class Square(Shape[int]):
    """A square."""

    def area(self, scale: float = 1.0) -> float:
        return 1.0
"#;

const CASES_BASICS: &str = r#""""Basics module.

More text.
"""

import enum
import typing as t
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .shapes import Shape

#: A constant.
LIMIT = 10
NAMES = ["a", "b"]
"""Names docstring."""
MAPPING = {"b": 2, "a": 1}  #: sorted mapping
FLOAT = 1e3  #: float
NEG = -5  #: neg
TUP = (1,)  #: tuple
Alias = t.Union[int, str]  #: an alias
annotated: int  #: annotated only
TEXT: str = "it's"  #: text
SENTINEL = object()  #: a sentinel


class Plain:
    """Plain class."""


class WithInit:
    """With init.

    Extra.
    """

    #: class attribute
    shared: t.ClassVar[int] = 1

    def __init__(self, a, b=2, *args, c: int = 3, d, **kw) -> None:
        """Init doc."""
        self.a = a  #: attribute a
        #: attribute b
        self.b: int = b


class MyError(Exception):
    """An error."""


class DictSub(dict):
    """A dict."""


class Color(enum.Enum):
    """Colors."""

    RED = 1  #: red
    GREEN = 2


class Child(WithInit):
    """Child."""

    def method(self, x: "Shape", y: t.Optional[int] = None) -> int:
        """Method."""

    @classmethod
    def make(cls, y=None):
        """Make."""

    @staticmethod
    def util(z, /, w):
        """Util."""

    @property
    def prop(self) -> str:
        """Prop."""

    @prop.setter
    def prop(self, value):
        pass

    async def run(self):
        """Run."""

    def _private(self):
        """Private."""

    def __special__(self):
        """Special."""

    def undocumented(self):
        pass

    def with_default(self, color=Color.RED, cls=Plain, f=len, s=LIMIT * 2):
        """Defaults."""


def func(a, b: int = 1, *, c=None) -> str:
    """Func doc."""


def deco(f):
    """A decorator."""
    return f


def sig_in_doc(*args):
    """sig_in_doc(a, b) -> int

    Real doc.
    """


@t.overload
def over(x: int) -> int: ...
@t.overload
def over(x: str) -> str: ...
def over(x):
    """Overloaded."""


def multi(
    a={
        "x": 1,
    },
    b=(1,
       2),
):
    """Multi-line defaults."""
"#;

/// One block of a capture: the directive as written, then what Sphinx generated.
#[derive(Default)]
struct Case {
    input: Vec<String>,
    expected: Vec<String>,
}

fn parse_cases(text: &str) -> Vec<Case> {
    let mut cases: Vec<Case> = Vec::new();
    let mut output = false;
    for line in text.lines() {
        if line.starts_with("=== INPUT") {
            cases.push(Case::default());
            output = false;
        } else if line == "=== OUTPUT" {
            output = true;
        } else if let Some(c) = cases.last_mut() {
            let list = if output { &mut c.expected } else { &mut c.input };
            list.push(line.to_string());
        }
    }
    cases
}

/// Turns a directive as written (`.. autoclass:: name` and option lines) into a request.
fn request_of(c: &Case) -> Request {
    let head = &c.input[0];
    let mark = head.find("::").expect("a directive");
    let mut r = Request {
        directive: head[3..mark].trim().to_string(),
        argument: head[mark + 2..].trim().to_string(),
        ..Request::default()
    };
    for l in &c.input[1..] {
        let t = l.trim();
        let Some(rest) = t.strip_prefix(':') else { continue };
        let end = rest.find(':').expect("an option ends with a colon");
        r.options.push(DirectiveOption::new(&rest[..end], rest[end + 1..].trim()));
    }
    r
}

fn run_cases(capture: &str, config: &Config) {
    let root = common::temp_root(&[
        ("cases/__init__.py", CASES_INIT),
        ("cases/shapes.py", CASES_SHAPES),
        ("cases/basics.py", CASES_BASICS),
    ]);
    let mut session = common::session(&root);
    let cases = parse_cases(capture);
    assert_eq!(cases.len(), 13);
    let mut failures = Vec::new();
    for c in &cases {
        let out = document(&mut session, config, &request_of(c));
        let got = out.lines.join("\n");
        let want = c.expected.join("\n");
        if got.trim_end_matches('\n') != want.trim_end_matches('\n') {
            failures.push(format!("{}\n--- Sphinx:\n{want}\n--- Guidedog:\n{got}", c.input[0]));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn cases_match_sphinx_with_signature_typehints() {
    run_cases(include_str!("data/cases-signature.txt"), &Config::default());
}

#[test]
fn cases_match_sphinx_with_description_typehints() {
    let config = Config {
        autodoc_typehints: Typehints::Description,
        autodoc_preserve_defaults: true,
        autodoc_member_order: MemberOrder::Bysource,
        // Sphinx adds the fields to the parsed field lists, not to its text.
        typehint_fields: false,
        ..Config::default()
    };
    run_cases(include_str!("data/cases-description.txt"), &config);
}
