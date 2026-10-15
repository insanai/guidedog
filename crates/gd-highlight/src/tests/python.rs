//! The Python lexers: source, console sessions and tracebacks.

use super::*;
use crate::*;
#[test]
fn python_basics() {
    expect_tokens(
        Lexer::Python,
        "def f(x):\n    return x + 1\n",
        "[k]def[w] [nf]f[p]([n]x[p]):[w]\\n[]    [k]return[] [n]x[] [o]+[] [mi]1[w]\\n",
    );
    expect_tokens(
        Lexer::Python,
        "class A(B):\n    def __init__(self): pass",
        concat!(
            "[k]class[w] [nc]A[p]([n]B[p]):[w]\\n[]    [k]def[w] [fm]__init__[p]([bp]self",
            "[p]):[] [k]pass"
        ),
    );
    expect_tokens(
        Lexer::Python,
        "import os.path as p, sys\nfrom . import x",
        concat!(
            "[kn]import[w] [nn]os.path[] [k]as[] [nn]p[o],[] [nn]sys[w]\\n[kn]from[w] [nn].",
            "[] [kn]import[w] [n]x"
        ),
    );
    expect_tokens(
        Lexer::Python,
        "print(len(a), True, None, a.print, ValueError)",
        concat!(
            "[nb]print[p]([nb]len[p]([n]a[p]),[] [kc]True[p],[] [kc]None[p],[] [n]a[o].[n]print",
            "[p],[] [ne]ValueError[p])"
        ),
    );
    expect_tokens(
        Lexer::Python,
        "x = 0x1F + 1_000 + 1.5e-3 + 2j  # note",
        concat!(
            "[n]x[] [o]=[] [mh]0x1F[] [o]+[] [mi]1_000[] [o]+[] [mf]1.5e-3[] [o]+[] [mi]2j[]  ",
            "[c1]# note"
        ),
    );
    expect_tokens(
        Lexer::Python,
        "#!/usr/bin/env python\nx is not y",
        "[ch]#!/usr/bin/env python[w]\\n[n]x[] [ow]is[] [ow]not[] [n]y",
    );
}

#[test]
fn python_decorators() {
    expect_tokens(
        Lexer::Python,
        "@app.route('/')\n@property\ndef f(): pass\na @ b",
        concat!(
            "[nd]@app.route[p]([s1]'/'[p])[w]\\n[nd]@property[w]\\n[k]def[w] [nf]f[p]():[] ",
            "[k]pass[w]\\n[n]a[] [o]@[] [n]b"
        ),
    );
}

#[test]
fn python_strings() {
    expect_tokens(
        Lexer::Python,
        "s = 'it\\'s' + \"a\\n\" + r'\\d' + b\"x\"",
        concat!(
            "[n]s[] [o]=[] [s1]'it[se]\\'[s1]s'[] [o]+[] [s2]\"a[se]\\n[s2]\"[] [o]+[] ",
            "[sa]r[s1]'\\d'[] [o]+[] [sa]b[s2]\"x\""
        ),
    );
    expect_tokens(
        Lexer::Python,
        "'%s and %(n)d' % x, '{} {0:>4}'.format(y)",
        concat!(
            "[s1]'[si]%s[s1] and [si]%(n)d[s1]'[] [o]%[] [n]x[p],[] [s1]'[si]{}[s1] [si]{0:>4}",
            "[s1]'[o].[n]format[p]([n]y[p])"
        ),
    );
}

#[test]
fn python_triple_quoted_and_docstrings() {
    expect_tokens(
        Lexer::Python,
        "def f():\n    \"\"\"Doc\n    more.\"\"\"\n    x = '''a\nb'''",
        concat!(
            "[k]def[w] [nf]f[p]():[w]\\n    [sd]\"\"\"Doc\\n    more.\"\"\"[w]\\n[]    [n]x[] ",
            "[o]=[] [s1]'''a\\nb'''"
        ),
    );
    expect_tokens(Lexer::Python, "r\"\"\"raw\\\"\"\"\"", "[sa]r[sd]\"\"\"raw\\\"\"\"[s2]\"");
}

#[test]
fn python_f_strings() {
    expect_tokens(
        Lexer::Python,
        "f\"x {y!r:>{w}} {{z}} {a['k']=}\"",
        concat!(
            "[sa]f[s2]\"x [si]{[n]y[si]!r:[s2]>[si]{[n]w[si]}}[s2] [se]{{[s2]z[se]}}[s2] [si]{",
            "[n]a[p][[s1]'k'[p]][si]=}[s2]\""
        ),
    );
    expect_tokens(
        Lexer::Python,
        "f'{x + 1:.2f}' F'''{\ny}'''",
        concat!(
            "[sa]f[s1]'[si]{[n]x[w] [o]+[w] [mi]1[si]:[s1].2f[si]}[s1]'[] [sa]F[s1]'''[si]{[w]",
            "\\n[n]y[si]}[s1]'''"
        ),
    );
}

#[test]
fn python_errors_and_default() {
    expect_tokens(Lexer::Python, "$ pip install x", "[err]$[] [n]pip[] [n]install[] [n]x");
    assert!(has_errors(Lexer::Python, "a = `b`"));
    assert!(!has_errors(Lexer::Python, "It's prose, not code"));
    assert_eq!(resolve_lexer(Lexer::Default, "$ make html"), Lexer::Text);
    assert_eq!(resolve_lexer(Lexer::Default, "print('x')"), Lexer::Python);
    assert_eq!(resolve_lexer(Lexer::Default, ">>> 1 + 1\n2\n"), Lexer::Pycon);
    assert_eq!(resolve_lexer(Lexer::Python, ">>> 1\n"), Lexer::Pycon);
    expect_tokens(Lexer::Default, "$ make", "[]$ make");
}

#[test]
fn pycon_sessions() {
    expect_tokens(
        Lexer::Pycon,
        ">>> x = 1\n>>> print(x)\n1\n",
        "[gp]>>> [n]x[] [o]=[] [mi]1[w]\\n[gp]>>> [nb]print[p]([n]x[p])[w]\\n[go]1\\n",
    );
    expect_tokens(
        Lexer::Pycon,
        ">>> s = '''a\n... b'''\n>>>\n",
        "[gp]>>> [n]s[] [o]=[] [s1]'''a\\n[gp]... [s1]b'''[w]\\n[gp]>>>[w]\\n",
    );
    expect_tokens(
        Lexer::Pycon,
        concat!(
            ">>> 1/0\nTraceback (most recent call last):\n",
            "  File \"<stdin>\", line 1, in <module>\n",
            "ZeroDivisionError: division by zero\n>>> ok\n"
        ),
        concat!(
            "[gp]>>> [mi]1[o]/[mi]0[w]\\n[gt]Traceback (most recent call last):\\n[]  File ",
            "[nb]\"<stdin>\"[], line [m]1[], in [n]<module>[w]\\n[gr]ZeroDivisionError[]: ",
            "[n]division by zero[w]\\n[gp]>>> [n]ok[w]\\n"
        ),
    );
}

#[test]
fn python_tracebacks() {
    expect_tokens(
        Lexer::Pytb,
        concat!(
            "Traceback (most recent call last):\n  File \"a.py\", line 3, in f\n    x = 1 / 0\n",
            "        ~~^~~\nZeroDivisionError: division by zero\n"
        ),
        concat!(
            "[gt]Traceback (most recent call last):\\n[]  File [nb]\"a.py\"[], line [m]3[], in ",
            "[n]f[w]\\n    [n]x[] [o]=[] [mi]1[] [o]/[] [mi]0[w]\\n        [pm]~~^~~[w]\\n",
            "[gr]ZeroDivisionError[]: [n]division by zero[w]\\n"
        ),
    );
    expect_tokens(Lexer::Pytb, "KeyboardInterrupt\n", "[x]KeyboardInterrupt\\n");
}

#[test]
fn python_unicode_names() {
    expect_tokens(
        Lexer::Python,
        "café = 1\nπ2 = café",
        concat!("[n]café[] [o]=[] [mi]1[w]\\n[n]π2[] ", "[o]=[] [n]café"),
    );
    // Arrows, dashes and box drawing are not Python, so "default" shows them as text.
    assert!(has_errors(Lexer::Python, "a → b"));
    assert_eq!(resolve_lexer(Lexer::Default, "src/\n├── a.py\n└── b.py\n"), Lexer::Text);
    assert_eq!(resolve_lexer(Lexer::Default, "x = 'naïve'"), Lexer::Python);
}
