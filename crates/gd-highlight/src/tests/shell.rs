//! The shell lexers: Bash, console sessions, Makefiles and Dockerfiles.

use super::*;
use crate::*;
#[test]
fn bash_commands() {
    expect_tokens(
        Lexer::Bash,
        "echo \"Hello $USER\" 'x' > out.txt # note\n",
        concat!(
            "[nb]echo[w] [s2]\"Hello [nv]$USER[s2]\"[w] [s1]'x'[w] []>[w] []out.txt[w] ",
            "[c1]# note[w]\\n"
        ),
    );
    expect_tokens(
        Lexer::Bash,
        "if [ -f a ]; then\n  FOO=1 ls -la && x=$(pwd)\nfi",
        concat!(
            "[k]if[w] [o][[w] []-f[w] []a[w] [o]][p];[w] [k]then[w]\\n  [nv]FOO[o]=[m]1[w] ",
            "[]ls[w] []-la[w] [o]&&[w] [nv]x[o]=[k]$([nb]pwd[k])[w]\\n[k]fi"
        ),
    );
    expect_tokens(
        Lexer::Bash,
        "echo ${HOME:-/tmp} $1 $((1 + 2)) `date`",
        concat!(
            "[nb]echo[w] [si]${[nv]HOME[k]:-[p]/[nv]tmp[si]}[w] [nv]$1[w] [k]$(([m]1[w] []+",
            "[w] [m]2[k]))[w] [sb]`[]date[sb]`"
        ),
    );
    expect_tokens(Lexer::Bash, "#!/bin/sh\na\\ b", "[ch]#!/bin/sh[w]\\n[]a[se]\\ []b");
}

#[test]
fn bash_heredocs() {
    expect_tokens(
        Lexer::Bash,
        "cat <<EOF > f\nline $x\nEOF\necho done",
        concat!(
            "[]cat[w] [sh]<<EOF[w] []>[w] []f[w]\\n[sh]line $x[w]\\n[sh]EOF[w]\\n[nb]echo[w] ",
            "[k]done"
        ),
    );
    expect_tokens(
        Lexer::Bash,
        "cat <<-'END'\n\tbody\n\tEND\nx",
        "[]cat[w] [sh]<<-'END'[w]\\n[sh]\tbody[w]\\n[sh]\tEND[w]\\n[]x",
    );
    expect_tokens(Lexer::Bash, "cat <<EOF\nnever ends", "[]cat[w] [sh]<<EOF[w]\\n[sh]never ends");
}

#[test]
fn console_sessions() {
    expect_tokens(
        Lexer::Console,
        "$ echo hi\nhi\n$ ls\na  b\n",
        "[gp]$ [nb]echo[w] []hi[w]\\n[go]hi\\n[gp]$ []ls[w]\\n[go]a  b\\n",
    );
    expect_tokens(
        Lexer::Console,
        "(venv) user@host:~/src$ pip install \\\n> guidedog\nok\n",
        concat!(
            "[gp](venv) user@host:~/src$ []pip[w] []install[w] [se]\\\\n[gp]> []guidedog[w]\\n",
            "[go]ok\\n"
        ),
    );
    expect_tokens(
        Lexer::Console,
        "# apt update\n% make\n[me@box dir]$ cat <<X\nhi\nX\n",
        concat!(
            "[gp]# []apt[w] []update[w]\\n[gp]% []make[w]\\n[gp][me@box dir]$ []cat[w] ",
            "[sh]<<X[w]\\n[sh]hi[w]\\n[sh]X[w]\\n"
        ),
    );
    assert_eq!(crate::shell::prompt_length(b"$"), 1);
    assert_eq!(crate::shell::prompt_length(b"sh-5.2$ ls"), 8);
    assert_eq!(crate::shell::prompt_length(b"100% done"), 0);
    assert_eq!(crate::shell::prompt_length(b"$HOME is set"), 0);
}

#[test]
fn makefile_rules() {
    expect_tokens(
        Lexer::Makefile,
        "CC := gcc\n# c\nall: main.o\n\t$(CC) -o $@ $^\n",
        concat!(
            "[nv]CC[w] [o]:=[w] []gcc[w]\\n[c]# c[w]\\n[nf]all[o]:[w] [n]main[].[n]o[w]\\n\t",
            "[k]$([]CC[k])[w] []-o[w] [nv]$@[w] []$^[w]\\n"
        ),
    );
    expect_tokens(
        Lexer::Makefile,
        "ifeq ($(OS),x)\nendif",
        "[k]ifeq[w] []([k]$([nv]OS[k])[],x)[w]\\n[k]endif",
    );
}

#[test]
fn dockerfile_instructions() {
    expect_tokens(
        Lexer::Dockerfile,
        concat!(
            "FROM python:3.12 AS base\n# c\nWORKDIR /app\nRUN pip install -r req.txt \\\n",
            "    && echo ok\nCMD [\"python\", \"app.py\"]\n"
        ),
        concat!(
            "[k]FROM[w] [s]python:3.12[w] [k]AS[w] [s]base[w]\\n[c]# c[w]\\n[k]WORKDIR[w] ",
            "[s]/app[w]\\n[k]RUN[w] []pip[w] []install[w] []-r[w] []req.txt[w] [se]\\\\n[w]",
            "    [o]&&[w] [nb]echo[w] []ok[w]\\n[k]CMD[w] [p][[s2]\"python\"[p],[w] ",
            "[s2]\"app.py\"[p]][w]\\n"
        ),
    );
}
