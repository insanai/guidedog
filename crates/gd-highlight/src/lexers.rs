//! The lexer registry: languages, their aliases, file patterns and MIME types, and the
//! choices Sphinx makes between them.

use crate::code::{code_spec, code_step};
use crate::data::{ini_step, json_step, toml_step, yaml_step};
use crate::docs::{markdown_step, rst_step};
use crate::markup::{css_step, html_step, xml_step};
use crate::misc::{diff_step, guess_lexer, text_step};
use crate::python::{pycon_step, pytb_step, python_step};
use crate::scanner::{Scanner, State, has_errors, trim_spaces};
use crate::shell::{bash_step, console_step, dockerfile_step, makefile_step};
use crate::tex::{latex_step, typst_step};

/// A language. [`Lexer::Default`] and [`Lexer::Guess`] are Sphinx's pseudo languages,
/// which scanning resolves against the code (see [`resolve_lexer`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Lexer {
    /// Plain text: `text`, `none`.
    #[default]
    Text,
    /// Python.
    Python,
    /// Python console sessions (`pycon`).
    Pycon,
    /// Python tracebacks (`pytb`).
    Pytb,
    /// C.
    C,
    /// C++.
    Cpp,
    /// Rust.
    Rust,
    /// Go.
    Go,
    /// Java.
    Java,
    /// Kotlin.
    Kotlin,
    /// C#.
    Csharp,
    /// Swift.
    Swift,
    /// Zig.
    Zig,
    /// Odin.
    Odin,
    /// JavaScript.
    Javascript,
    /// TypeScript.
    Typescript,
    /// JSON.
    Json,
    /// YAML.
    Yaml,
    /// TOML.
    Toml,
    /// INI and similar configuration files.
    Ini,
    /// XML.
    Xml,
    /// HTML, with `<script>` and `<style>` lexed as JavaScript and CSS.
    Html,
    /// CSS.
    Css,
    /// Bash.
    Bash,
    /// Shell sessions (`console`).
    Console,
    /// PowerShell.
    Powershell,
    /// SQL.
    Sql,
    /// Unified and context diffs.
    Diff,
    /// Makefiles.
    Makefile,
    /// Dockerfiles.
    Dockerfile,
    /// reStructuredText.
    Rst,
    /// Markdown and MyST.
    Markdown,
    /// TeX and LaTeX.
    Latex,
    /// Typst.
    Typst,
    /// Graphviz.
    Dot,
    /// Sphinx's `default`: Python, a Python session, or text.
    Default,
    /// Sphinx's `guess`: picked from the code.
    Guess,
}

impl Lexer {
    /// Every lexer, in declaration order.
    pub const ALL: [Lexer; 37] = [
        Lexer::Text,
        Lexer::Python,
        Lexer::Pycon,
        Lexer::Pytb,
        Lexer::C,
        Lexer::Cpp,
        Lexer::Rust,
        Lexer::Go,
        Lexer::Java,
        Lexer::Kotlin,
        Lexer::Csharp,
        Lexer::Swift,
        Lexer::Zig,
        Lexer::Odin,
        Lexer::Javascript,
        Lexer::Typescript,
        Lexer::Json,
        Lexer::Yaml,
        Lexer::Toml,
        Lexer::Ini,
        Lexer::Xml,
        Lexer::Html,
        Lexer::Css,
        Lexer::Bash,
        Lexer::Console,
        Lexer::Powershell,
        Lexer::Sql,
        Lexer::Diff,
        Lexer::Makefile,
        Lexer::Dockerfile,
        Lexer::Rst,
        Lexer::Markdown,
        Lexer::Latex,
        Lexer::Typst,
        Lexer::Dot,
        Lexer::Default,
        Lexer::Guess,
    ];
}

/// A lexer's static description.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LexerInfo {
    /// Pygments' lexer name: `"Python"`.
    pub name: &'static str,
    /// The names code blocks use; the first is the canonical one.
    pub aliases: &'static [&'static str],
    /// Glob patterns of file names: `"*.py"`, `"Makefile"`.
    pub filenames: &'static [&'static str],
    /// MIME types.
    pub mimetypes: &'static [&'static str],
}

const fn info(
    name: &'static str,
    aliases: &'static [&'static str],
    filenames: &'static [&'static str],
    mimetypes: &'static [&'static str],
) -> LexerInfo {
    LexerInfo { name, aliases, filenames, mimetypes }
}

/// The lexer's static description: name, aliases, file patterns and MIME types.
pub fn lexer_info(lexer: Lexer) -> &'static LexerInfo {
    &LEXER_INFOS[lexer as usize]
}

/// Pygments' name for the lexer: `"Python"`, `"C++"`.
pub fn lexer_name(lexer: Lexer) -> &'static str {
    lexer_info(lexer).name
}

/// The lexer's canonical alias: `"python"`, `"cpp"`.
pub fn lexer_alias(lexer: Lexer) -> &'static str {
    lexer_info(lexer).aliases[0]
}

/// Runs one step of the state's lexer.
pub(crate) fn step(s: &mut Scanner<'_>, st: &mut State) {
    match st.lexer {
        Lexer::Text | Lexer::Guess => text_step(s, st),
        Lexer::Python | Lexer::Default => python_step(s, st),
        Lexer::Pycon => pycon_step(s, st),
        Lexer::Pytb => pytb_step(s, st),
        Lexer::C
        | Lexer::Cpp
        | Lexer::Rust
        | Lexer::Go
        | Lexer::Java
        | Lexer::Kotlin
        | Lexer::Csharp
        | Lexer::Swift
        | Lexer::Zig
        | Lexer::Odin
        | Lexer::Javascript
        | Lexer::Typescript
        | Lexer::Powershell
        | Lexer::Sql
        | Lexer::Dot => match code_spec(st.lexer) {
            Some(spec) => code_step(s, st, spec),
            None => text_step(s, st),
        },
        Lexer::Json => json_step(s, st),
        Lexer::Yaml => yaml_step(s, st),
        Lexer::Toml => toml_step(s, st),
        Lexer::Ini => ini_step(s, st),
        Lexer::Xml => xml_step(s, st),
        Lexer::Html => html_step(s, st),
        Lexer::Css => css_step(s, st),
        Lexer::Bash => bash_step(s, st),
        Lexer::Console => console_step(s, st),
        Lexer::Diff => diff_step(s, st),
        Lexer::Makefile => makefile_step(s, st),
        Lexer::Dockerfile => dockerfile_step(s, st),
        Lexer::Rst => rst_step(s, st),
        Lexer::Markdown => markdown_step(s, st),
        Lexer::Latex => latex_step(s, st),
        Lexer::Typst => typst_step(s, st),
    }
}

/// Finds a lexer by alias (`"py"`, `"c++"`), as Pygments' `get_lexer_by_name` does,
/// ignoring case; failing that by file name (`"a/setup.py"`, `"Makefile"`) or MIME type.
/// `None` when nothing matches.
pub fn find_lexer(name: &str) -> Option<Lexer> {
    let n = trim_spaces(name.as_bytes());
    if n.is_empty() {
        return None;
    }
    let fold = |list: &[&str]| list.iter().any(|w| w.as_bytes().eq_ignore_ascii_case(n));
    Lexer::ALL
        .into_iter()
        .find(|&l| fold(lexer_info(l).aliases))
        .or_else(|| find_lexer_for_filename(std::str::from_utf8(n).unwrap_or("")))
        .or_else(|| Lexer::ALL.into_iter().find(|&l| fold(lexer_info(l).mimetypes)))
}

/// Matches the base name of `path` against the lexers' file patterns.
pub fn find_lexer_for_filename(path: &str) -> Option<Lexer> {
    let base = path.rsplit(['/', '\\']).next().unwrap_or(path);
    if base.is_empty() {
        return None;
    }
    Lexer::ALL.into_iter().find(|&l| {
        lexer_info(l).filenames.iter().any(|p| glob_match(p.as_bytes(), base.as_bytes()))
    })
}

/// Makes Sphinx's choice for a code block.
///
/// [`Lexer::Default`] is Python, or a Python console session when the code starts with
/// `>>>`, and plain text when that does not lex cleanly. Python code that starts with
/// `>>>` is a session. [`Lexer::Guess`] guesses from the code. Every other lexer is itself.
pub fn resolve_lexer(lexer: Lexer, code: &str) -> Lexer {
    let session = |code: &str| if code.starts_with(">>>") { Lexer::Pycon } else { Lexer::Python };
    match lexer {
        Lexer::Default => {
            let l = session(code);
            if has_errors(l, code) { Lexer::Text } else { l }
        }
        Lexer::Python => session(code),
        Lexer::Guess => guess_lexer(code),
        Lexer::Text
        | Lexer::Pycon
        | Lexer::Pytb
        | Lexer::C
        | Lexer::Cpp
        | Lexer::Rust
        | Lexer::Go
        | Lexer::Java
        | Lexer::Kotlin
        | Lexer::Csharp
        | Lexer::Swift
        | Lexer::Zig
        | Lexer::Odin
        | Lexer::Javascript
        | Lexer::Typescript
        | Lexer::Json
        | Lexer::Yaml
        | Lexer::Toml
        | Lexer::Ini
        | Lexer::Xml
        | Lexer::Html
        | Lexer::Css
        | Lexer::Bash
        | Lexer::Console
        | Lexer::Powershell
        | Lexer::Sql
        | Lexer::Diff
        | Lexer::Makefile
        | Lexer::Dockerfile
        | Lexer::Rst
        | Lexer::Markdown
        | Lexer::Latex
        | Lexer::Typst
        | Lexer::Dot => lexer,
    }
}

/// The alias closest to a name [`find_lexer`] does not know, for "Did you mean" hints;
/// `None` when nothing is close.
pub fn suggest_lexer(name: &str) -> Option<&'static str> {
    let n = trim_spaces(name.as_bytes());
    let mut best = None;
    let mut best_distance = 3;
    for l in Lexer::ALL {
        for alias in lexer_info(l).aliases {
            let d = edit_distance(n, alias.as_bytes());
            if d < best_distance && d <= (alias.len() / 3).max(1) {
                best = Some(*alias);
                best_distance = d;
            }
        }
    }
    best
}

/// Matches `*` (any run) and `?` (one byte); everything else is literal.
fn glob_match(pattern: &[u8], name: &[u8]) -> bool {
    let (mut p, mut n) = (0, 0);
    let mut star: Option<usize> = None;
    let mut mark = 0;
    while n < name.len() {
        if p < pattern.len() && (pattern[p] == b'?' || pattern[p] == name[n]) {
            p += 1;
            n += 1;
        } else if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            mark = n;
            p += 1;
        } else if let Some(s) = star {
            p = s + 1;
            mark += 1;
            n = mark;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }
    p == pattern.len()
}

/// The Levenshtein distance of short ASCII names, ignoring case; names longer than 32
/// bytes count as far apart.
pub(crate) fn edit_distance(a: &[u8], b: &[u8]) -> usize {
    if a.len() > 32 || b.len() > 32 {
        return 99;
    }
    let mut row = [0usize; 33];
    for (j, r) in row.iter_mut().enumerate().take(b.len() + 1) {
        *r = j;
    }
    for i in 1..=a.len() {
        let mut diagonal = row[0];
        row[0] = i;
        for j in 1..=b.len() {
            let above = row[j];
            let cost = usize::from(!a[i - 1].eq_ignore_ascii_case(&b[j - 1]));
            row[j] = (row[j] + 1).min(row[j - 1] + 1).min(diagonal + cost);
            diagonal = above;
        }
    }
    row[b.len()]
}

#[rustfmt::skip]
static LEXER_INFOS: [LexerInfo; 37] = [
    info("Text only", &["text", "none", "plain", "plaintext"], &["*.txt"], &["text/plain"]),
    info("Python", &["python", "py", "python3", "py3", "sage", "bazel", "starlark", "pyi"],
        &["*.py", "*.pyw", "*.pyi", "*.jy", "*.sage", "*.sc", "SConstruct", "SConscript",
            "*.bzl", "BUCK", "BUILD", "BUILD.bazel", "WORKSPACE", "*.tac"],
        &["text/x-python", "application/x-python", "text/x-python3", "application/x-python3"]),
    info("Python console session", &["pycon", "python-console", "pycon3"], &[],
        &["text/x-python-doctest"]),
    info("Python Traceback", &["pytb", "py3tb", "py2tb"], &["*.pytb", "*.py3tb"],
        &["text/x-python-traceback", "text/x-python3-traceback"]),
    info("C", &["c"], &["*.c", "*.h", "*.idc"], &["text/x-chdr", "text/x-csrc"]),
    info("C++", &["cpp", "c++", "cxx", "hpp"], &["*.cpp", "*.hpp", "*.c++", "*.h++", "*.cc",
        "*.hh", "*.cxx", "*.hxx", "*.C", "*.H", "*.cp", "*.CPP", "*.tpp"],
        &["text/x-c++hdr", "text/x-c++src"]),
    info("Rust", &["rust", "rs"], &["*.rs", "*.rs.in"], &["text/rust", "text/x-rust"]),
    info("Go", &["go", "golang"], &["*.go"], &["text/x-gosrc"]),
    info("Java", &["java"], &["*.java"], &["text/x-java"]),
    info("Kotlin", &["kotlin", "kt"], &["*.kt", "*.kts"], &["text/x-kotlin"]),
    info("C#", &["csharp", "c#", "cs"], &["*.cs"], &["text/x-csharp"]),
    info("Swift", &["swift"], &["*.swift"], &["text/x-swift"]),
    info("Zig", &["zig"], &["*.zig"], &["text/zig"]),
    info("Odin", &["odin"], &["*.odin"], &["text/x-odin"]),
    info("JavaScript", &["javascript", "js", "mjs", "cjs"], &["*.js", "*.jsm", "*.mjs",
        "*.cjs"], &["application/javascript", "application/x-javascript", "text/x-javascript",
        "text/javascript"]),
    info("TypeScript", &["typescript", "ts"], &["*.ts", "*.mts", "*.cts"],
        &["application/x-typescript", "text/x-typescript"]),
    info("JSON", &["json", "json-object", "jsonc"], &["*.json", "*.jsonl", "*.ndjson",
        "Pipfile.lock"], &["application/json", "application/json-object",
        "application/x-ndjson", "application/jsonl", "application/json-seq"]),
    info("YAML", &["yaml", "yml"], &["*.yaml", "*.yml"], &["text/x-yaml"]),
    info("TOML", &["toml"], &["*.toml", "Pipfile", "poetry.lock"], &["application/toml"]),
    info("INI", &["ini", "cfg", "dosini"], &["*.ini", "*.cfg", "*.inf", ".editorconfig"],
        &["text/x-ini", "text/inf"]),
    info("XML", &["xml"], &["*.xml", "*.xsl", "*.rss", "*.xslt", "*.xsd", "*.wsdl", "*.wsf",
        "*.svg"], &["text/xml", "application/xml", "image/svg+xml", "application/rss+xml",
        "application/atom+xml"]),
    info("HTML", &["html", "htm", "xhtml"], &["*.html", "*.htm", "*.xhtml"],
        &["text/html", "application/xhtml+xml"]),
    info("CSS", &["css"], &["*.css"], &["text/css"]),
    info("Bash", &["bash", "sh", "ksh", "zsh", "shell"], &["*.sh", "*.ksh", "*.bash",
        "*.ebuild", "*.eclass", "*.zsh", ".bashrc", "bashrc", ".bash_*", "bash_*", "zshrc",
        ".zshrc", ".kshrc", "kshrc", "PKGBUILD"], &["application/x-sh",
        "application/x-shellscript", "text/x-shellscript"]),
    info("Bash Session", &["console", "shell-session", "bash-session", "sh-session",
        "shellsession"], &["*.sh-session", "*.shell-session"],
        &["application/x-shell-session", "application/x-sh-session"]),
    info("PowerShell", &["powershell", "pwsh", "posh", "ps1", "psm1"], &["*.ps1", "*.psm1"],
        &["text/x-powershell"]),
    info("SQL", &["sql"], &["*.sql"], &["text/x-sql"]),
    info("Diff", &["diff", "udiff", "patch"], &["*.diff", "*.patch"],
        &["text/x-diff", "text/x-patch"]),
    info("Makefile", &["make", "makefile", "mf", "bsdmake"], &["*.mak", "*.mk", "Makefile",
        "makefile", "Makefile.*", "GNUmakefile"], &["text/x-makefile"]),
    info("Docker", &["docker", "dockerfile"], &["Dockerfile", "*.docker", "Containerfile"],
        &["text/x-dockerfile-config"]),
    info("reStructuredText", &["restructuredtext", "rst", "rest"], &["*.rst", "*.rest"],
        &["text/x-rst", "text/prs.fallenstein.rst"]),
    info("Markdown", &["markdown", "md", "myst"], &["*.md", "*.markdown"],
        &["text/x-markdown"]),
    info("TeX", &["tex", "latex"], &["*.tex", "*.aux", "*.toc", "*.sty", "*.cls"],
        &["text/x-tex", "text/x-latex"]),
    info("Typst", &["typst", "typ"], &["*.typ"], &["text/x-typst"]),
    info("Graphviz", &["graphviz", "dot", "gv"], &["*.gv", "*.dot"],
        &["text/x-graphviz", "text/vnd.graphviz"]),
    info("Default", &["default"], &[], &[]),
    info("Guess", &["guess"], &[], &[]),
];
