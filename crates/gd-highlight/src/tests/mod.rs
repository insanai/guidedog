//! Tests, ported from the Odin package's `*_test.odin` files.

mod code;
mod data;
mod docs;
mod growth;
mod html;
mod markup;
mod problems;
mod python;
mod robust;
mod shell;
mod styles;
mod tables;

use crate::{Lexer, kind_class, scan_tokens, tokenize, write_code_html};

/// Writes tokens as `[class]text`, with newlines as `\n` and Text as `[]`; for example
/// `[k]def[w] [nf]f[p]():`.
pub(crate) fn dump(lexer: Lexer, code: &str) -> String {
    let mut out = String::new();
    for t in tokenize(lexer, code) {
        out.push('[');
        out.push_str(kind_class(t.kind));
        out.push(']');
        out.push_str(&code[t.start as usize..t.end as usize].replace('\n', "\\n"));
    }
    out
}

/// Checks the tokens of `code` against `expected`, and their coverage.
#[track_caller]
pub(crate) fn expect_tokens(lexer: Lexer, code: &str, expected: &str) {
    let got = dump(lexer, code);
    assert_eq!(got, expected, "{lexer:?} tokens differ for {code:?}");
    expect_coverage(lexer, code);
}

/// Checks that tokens are contiguous, non-empty, merged and cover `code`.
#[track_caller]
pub(crate) fn expect_coverage(lexer: Lexer, code: &str) {
    let mut at = 0u32;
    let mut previous = None;
    for t in scan_tokens(lexer, code) {
        assert!(
            t.start == at && t.end > t.start,
            "{lexer:?}: token {t:?} does not continue at {at}"
        );
        assert_ne!(previous, Some(t.kind), "{lexer:?}: two {:?} tokens in a row at {at}", t.kind);
        assert!(
            code.is_char_boundary(t.end as usize),
            "{lexer:?}: token {t:?} splits a character"
        );
        previous = Some(t.kind);
        at = t.end;
    }
    assert_eq!(at as usize, code.len(), "{lexer:?}: tokens end early");
}

pub(crate) fn html(lexer: Lexer, code: &str) -> String {
    let mut out = String::new();
    write_code_html(lexer, code, &mut out);
    out
}

/// A short program in every language.
pub(crate) fn sample(lexer: Lexer) -> &'static str {
    match lexer {
        Lexer::Text => "plain <text> & more\n",
        Lexer::Python => concat!(
            "@dataclass\nclass P:\n    \"\"\"Doc.\"\"\"\n",
            "    def f(self, x=1.5):\n        return f\"{x!r:>4}\" % {'a': None}\n"
        ),
        Lexer::Pycon => concat!(
            ">>> 1 + 1\n2\n>>> raise ValueError('x')\n",
            "Traceback (most recent call last):\n  File \"<stdin>\", line 1, in <module>\n",
            "ValueError: x\n"
        ),
        Lexer::Pytb => concat!(
            "Traceback (most recent call last):\n",
            "  File \"a.py\", line 1, in <module>\n    f()\n    ^^^\n",
            "NameError: name 'f' is not defined\n"
        ),
        Lexer::C => concat!(
            "#include <stdio.h>\n/* c */\n",
            "int main(void) { return printf(\"%d\\n\", 0x1F); }\n"
        ),
        Lexer::Cpp => "template<typename T> class V { std::string s = R\"(raw)\"; };\n",
        Lexer::Rust => {
            "#[derive(Debug)]\nfn f<'a>(s: &'a str) -> char { println!(\"{}\", s); 'x' }\n"
        }
        Lexer::Go => {
            "package main\nimport \"fmt\"\nfunc main() { fmt.Println(`raw`, 'r', 1.5) }\n"
        }
        Lexer::Java => "@Override\npublic class A { public static void main(String[] a) {} }\n",
        Lexer::Kotlin => "fun main() { val n = 1; println(\"n=$n ${n + 1}\") }\n",
        Lexer::Csharp => {
            "using System;\nclass P { static void Main() => Console.WriteLine($\"{1}\"); }\n"
        }
        Lexer::Swift => "import Foundation\nfunc f(_ x: Int) -> String { \"\\(x)\" }\n",
        Lexer::Zig => {
            "const std = @import(\"std\");\npub fn main() void { const s = \n\\\\x\n; }\n"
        }
        Lexer::Odin => "package m\nmain :: proc() { x := #load(\"f\"); s := `raw` }\n",
        Lexer::Javascript => "const r = /a+/g; let t = `x ${1}`; console.log('a', \"b\");\n",
        Lexer::Typescript => "interface P { n: number }\n@c class C implements P { n = 1 }\n",
        Lexer::Json => "{\"a\": [1, 2.5, true, null, \"s\"], \"b\": {}}\n",
        Lexer::Yaml => "---\na: 1 # c\nb:\n  - x\n  - {y: [z]}\nc: |\n  text\n",
        Lexer::Toml => "[t]\na = \"s\"\nb = [1, 2.5]\nc = 1979-05-27\n",
        Lexer::Ini => "[s]\nk = v ; c\n",
        Lexer::Xml => "<?xml version=\"1.0\"?>\n<a b=\"c\">&amp;<!-- d --></a>\n",
        Lexer::Html => "<!DOCTYPE html>\n<p class=\"a\">x</p><script>let a = 1;</script>\n",
        Lexer::Css => "a:hover { color: #fff; margin: 0 1.5em !important; }\n",
        Lexer::Bash => concat!(
            "#!/bin/bash\nfor f in *.txt; do echo \"$f ${x:-y}\" | wc -l; done\n",
            "cat <<E\nb\nE\n"
        ),
        Lexer::Console => "$ ls -la\ntotal 0\n(venv) $ pip install x\n",
        Lexer::Powershell => "$x = Get-Item \"a\"; if ($x -eq $null) { Write-Host 'no' }\n",
        Lexer::Sql => "SELECT a, COUNT(*) FROM t WHERE b = 'x' GROUP BY a; -- c\n",
        Lexer::Diff => "--- a\n+++ b\n@@ -1 +1 @@\n-x\n+y\n",
        Lexer::Makefile => "all: a.o\n\t$(CC) -o $@ $^\n",
        Lexer::Dockerfile => "FROM alpine AS base\nRUN apk add git\nCMD [\"sh\"]\n",
        Lexer::Rst => "Title\n=====\n\n.. code-block:: python\n\n   x = 1\n\n*a* ``b``\n",
        Lexer::Markdown => "# T\n\n- *a* `b` [c](d)\n\n```python\nx = 1\n```\n",
        Lexer::Latex => "\\section{A} % c\n$x^2$\n",
        Lexer::Typst => "= T\n#let x = 1\n*b* $x$ #f(a)[b]\n",
        Lexer::Dot => "digraph { a -> b [label=\"x\"]; }\n",
        Lexer::Default => "print('x')\n",
        Lexer::Guess => "#!/bin/sh\necho hi\n",
    }
}
