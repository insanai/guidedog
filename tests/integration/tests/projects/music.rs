//! Native music through real readers, builders, assets and embedded PDF compilation.

use guidedog_tests::{Scratch, has, lacks};

use crate::support::{both, build, project};

fn setup(name: &str) -> Scratch {
    let dir = project(name);
    dir.write(
        "conf.toml",
        &format!(
            "{}\npdf_packages = 'offline'\npdf_fonts = 'embedded'\nnumfig = true\n",
            dir.read("conf.toml")
                .replace("source_suffix = [\".rst\"]", "source_suffix = [\".rst\", \".md\"]")
        ),
    );
    dir
}

#[test]
fn rst_and_myst_music_render_in_html_and_real_pdf() {
    let dir = setup("music-both");
    dir.write(
        "index.rst",
        concat!(
            "Music\n=====\n\n.. toctree::\n\n   other\n\n.. lilypond::\n",
            "   :caption: A *melody*\n   :name: melody\n   :alt: Four notes\n\n",
            "   \\relative c' { \\time 4/4 c4 d e f | }\n\nSee :numref:`melody`.\n"
        ),
    );
    dir.write(
        "other.md",
        concat!(
            "# Other\n\n```{lilypond}\n:format: png\n\n",
            "\\relative c' { c4 d e f | }\n```\n"
        ),
    );
    build(&dir.path, 0, &["html", "-n", "-W"]);
    let html = dir.read("_build/html/index.html");
    has(&html, "<picture>");
    has(&html, "media=\"(max-width: 40rem)\"");
    has(&html, "alt=\"Four notes\"");
    has(&html, "id=\"melody\"");
    has(&html, "<em>melody</em>");
    has(&html, "href=\"#melody\">Fig. ");
    let other = dir.read("_build/html/other.html");
    has(&other, "lilypond-");
    has(&other, ".png");
    has(&other, "Music for 1 staff.");
    assert_eq!(other.matches(" 1.5x").count(), 2);
    has(&other, "<img srcset=");
    lacks(&html, " 1.5x");
    build(&dir.path, 0, &["pdf", "-n", "-W"]);
    has(&dir.read("_build/pdf/book-en.typ"), "lilypond-");
    assert!(std::fs::read(dir.path.join("_build/pdf/demo-en.pdf")).unwrap().starts_with(b"%PDF-"));
}

#[test]
fn confined_includes_invalidate_music_and_untrusted_inline_remains_available() {
    let dir = setup("music-includes");
    dir.write("index.rst", "Music\n=====\n\n.. lilypond:: sub/main.ly\n");
    dir.write("sub/main.ly", "\\include \"notes.ly\"\n\\relative c' { \\notes }\n");
    dir.write("sub/notes.ly", "notes = { c4 d e f | }\n");
    build(&dir.path, 0, &["html", "-W"]);
    let first = dir.read("_build/html/index.html");
    has(&first, "lilypond-");
    dir.write("sub/notes.ly", "notes = { c4 f e d | }\n");
    build(&dir.path, 0, &["html", "-W"]);
    assert_ne!(first, dir.read("_build/html/index.html"));
    let refusal = build(&dir.path, 0, &["html", "--untrusted"]);
    has(&both(&refusal), "disabled in --untrusted");
    lacks(&dir.read("_build/html/index.html"), "_images/lilypond-");
    dir.write("index.rst", "Music\n=====\n\n.. lilypond::\n\n   { c'4 d' e' f' | }\n");
    build(&dir.path, 0, &["html", "--untrusted", "-W"]);
    has(&dir.read("_build/html/index.html"), "_images/lilypond-");
}

#[test]
fn systems_break_across_pdf_pages_and_caption_appears_once() {
    let dir = setup("music-systems");
    let source = "c'4 d' e' f' | \\break ".repeat(16);
    dir.write("index.rst", &format!(
        "Music\n=====\n\n.. lilypond::\n   :caption: Long melody\n   :name: long\n\n   {{ {source} }}\n"));
    build(&dir.path, 0, &["pdf", "-W"]);
    let source = dir.read("_build/pdf/book-en.typ");
    has(&source, "#block(breakable: true)");
    has(&source, "#layout(size =>");
    assert_eq!(source.matches("Long melody").count(), 1);
    assert!(source.matches("lilypond-").count() >= 16);
}

#[test]
fn invalid_music_warns_at_its_original_source_line_and_strict_build_keeps_output() {
    let dir = setup("music-diagnostics");
    dir.write(
        "index.rst",
        "Music\n=====\n\nA paragraph.\n\n.. lilypond::\n\n   { c'4 \\unknown }\n",
    );
    let failed = build(&dir.path, 1, &["html", "-W"]);
    let output = both(&failed);
    has(&output, "UNKNOWN MUSIC COMMAND");
    has(&output, "index.rst:8:");
    build(&dir.path, 0, &["html"]);
    lacks(&dir.read("_build/html/index.html"), "_images/lilypond-");
}

#[test]
fn creating_a_missing_nested_include_rebuilds_the_page() {
    let dir = setup("music-missing");
    dir.write(
        "index.rst",
        concat!("Music\n=====\n\n.. lilypond::\n\n", "   \\include \"notes.ly\"\n   \\notes\n"),
    );
    build(&dir.path, 0, &["html"]);
    lacks(&dir.read("_build/html/index.html"), "_images/lilypond-");
    dir.write("notes.ly", "notes = { c'4 d' e' f' | }\n");
    build(&dir.path, 0, &["html", "-W"]);
    has(&dir.read("_build/html/index.html"), "_images/lilypond-");
}

#[cfg(unix)]
#[test]
fn music_files_and_includes_refuse_escaping_symbolic_links() {
    let dir = setup("music-symlink");
    let outside = Scratch::new("music-private");
    outside.write("secret.ly", "{ c'1 }\n");
    std::os::unix::fs::symlink(outside.path.join("secret.ly"), dir.path.join("score.ly")).unwrap();
    for source in
        [".. lilypond:: score.ly\n", concat!(".. lilypond::\n\n", "   \\include \"score.ly\"\n")]
    {
        dir.write("index.rst", &format!("Music\n=====\n\n{source}"));
        let result = build(&dir.path, 1, &["html", "-W"]);
        has(&both(&result), "inside the source roots");
    }
}

#[test]
fn generated_music_descriptions_follow_the_projects_ui_language() {
    let dir = setup("music-localized-alt");
    dir.write(
        "index.rst",
        concat!("Music\n=====\n\n.. lilypond::\n\n", "   { \\key g \\major c'4 d' e' f' | }\n"),
    );
    for (language, staff, mode) in
        [("de", "Notensystem", "Dur"), ("hi", "स्वर-पंक्ति", "मेजर"), ("ko", "보표", "장조")]
    {
        build(&dir.path, 0, &["html", "-W", "-D", &format!("language={language}")]);
        let html = dir.read("_build/html/index.html");
        has(&html, staff);
        has(&html, mode);
    }
}

#[test]
fn myst_and_included_rst_music_diagnostics_keep_the_original_file_and_line() {
    let dir = setup("music-mapped-sources");
    dir.write("index.rst", "Music\n=====\n\n.. toctree::\n\n   other\n");
    dir.write("other.md", "# Other\n\n```{lilypond}\n\n{ c'4^\"é\" \\unknown }\n```\n");
    let failed = build(&dir.path, 1, &["html", "-W"]);
    let output = both(&failed);
    has(&output, "other.md:5:");
    has(&output, "{ c'4^\"é\" \\unknown }");
    dir.write("other.md", "# Other\n\nOrdinary text.\n");
    dir.write("index.rst", "Music\n=====\n\n.. include:: parts/fragment.rst\n");
    dir.write("parts/fragment.rst", ".. lilypond::\n\n   { c'4\n     \\unknown }\n");
    let failed = build(&dir.path, 1, &["html", "-W"]);
    has(&both(&failed), "parts/fragment.rst:4:");
}
