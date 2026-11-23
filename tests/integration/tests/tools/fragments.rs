//! `guidedog convert` for a host that shows the HTML inside its own page: formulas as
//! MathML (`--math=mathml`), pictures drawn into the fragment (`--embed-pictures`),
//! the picture bounds, formulas written as MathML, and `--keep-going`.

use guidedog_tests::{Scratch, guidedog, has, lacks};

use crate::flat;

const FRAGMENT: [&str; 7] = ["--from", "sphinx", "--to", "html", "--fragment", "-o", "-"];

/// `guidedog convert <file> ... --fragment -o - <more> --color=never`, expecting `status`.
fn fragment(dir: &Scratch, status: i32, file: &str, more: &[&str]) -> guidedog_tests::Run {
    let mut args = vec!["convert", file];
    args.extend_from_slice(&FRAGMENT);
    args.extend_from_slice(more);
    args.push("--color=never");
    guidedog(&dir.path, status, &args)
}

/// The bytes of the `n`th embedded picture of `html` (from 0), decoded.
fn picture(html: &str, n: usize) -> Vec<u8> {
    let start = "base64,";
    let part = html.split(start).nth(n + 1).unwrap_or_else(|| panic!("no picture {n}: {html}"));
    let encoded = &part[..part.find('"').unwrap()];
    gd_json::base64::decode(encoded).unwrap()
}

#[test]
fn formulas_become_mathml_and_untranslatable_ones_stay_latex() {
    let dir = Scratch::new("fragment-math");
    dir.write(
        "page.rst",
        "Inline :math:`a^2 + b^2 = c^2` and :math:`\\color{red} x`.\n\n.. math::\n   :label: \
         euler\n\n   e^{i\\pi} + 1 = 0\n",
    );
    let default = fragment(&dir, 0, "page.rst", &[]);
    has(&default.out, "<span class=\"math\">\\(a^2 + b^2 = c^2\\)</span>");
    has(&default.out, "<div id=\"equation-euler\" class=\"math\">\\[e^{i\\pi} + 1 = 0\\]</div>");
    lacks(&default.out, "<math");
    assert!(default.err.is_empty(), "{}", default.err);
    let r = fragment(&dir, 0, "page.rst", &["--math=mathml"]);
    has(
        &r.out,
        "<span class=\"math\"><math><msup><mi>𝑎</mi><mn>2</mn></msup><mo>+</mo><msup>\
         <mi>𝑏</mi><mn>2</mn></msup><mo>=</mo><msup><mi>𝑐</mi><mn>2</mn></msup></math></span>",
    );
    has(
        &r.out,
        "<div id=\"equation-euler\" class=\"math\"><math display=\"block\"><msup><mi>𝑒</mi>\
         <mrow><mi>𝑖</mi><mi>𝜋</mi></mrow></msup><mo>+</mo><mn>1</mn><mo>=</mo><mn>0</mn>\
         </math></div>",
    );
    has(&r.out, "<span class=\"math\">\\(\\color{red} x\\)</span>");
    lacks(&r.out, "style=");
    lacks(&r.out, "<script");
    has(&r.err, "mathml.math.unsupported");
    has(&r.err, "page.rst:1:");
    has(&flat(&r.err), "I cannot translate \\color in this LaTeX formula to MathML");
    // A complete page carries the style rules; a wrong word is a usage error.
    let page = ["convert", "page.rst", "--from", "sphinx", "-o", "p.html", "--math=mathml"];
    guidedog(&dir.path, 0, &page);
    has(&dir.read("p.html"), "mtable.multiline-equation mtd");
    let r = guidedog(&dir.path, 2, &["convert", "page.rst", "-o", "x.html", "--math=katex"]);
    has(&r.err, "UNKNOWN MATH OUTPUT");
    let r = guidedog(&dir.path, 2, &["convert", "page.rst", "-o", "p.pdf", "--math=mathml"]);
    has(&r.err, "OPTION NEEDS HTML OUTPUT");
}

#[test]
fn pictures_are_drawn_into_the_fragment() {
    let dir = Scratch::new("fragment-pictures");
    dir.write(
        "page.rst",
        ".. graphviz::\n   :caption: A small graph.\n   :alt: a points to b\n\n   digraph { a -> \
         b }\n\n.. typst-diagram::\n   :alt: A circle\n\n   #circle(radius: 10pt)\n\nMath \
         :math:`x`.\n",
    );
    let plain = fragment(&dir, 0, "page.rst", &["--offline"]);
    has(&plain.out, "highlight-dot");
    has(&plain.out, "highlight-typst");
    lacks(&plain.out, "<img");
    let args = ["--offline", "--embed-pictures", "--math=mathml", "--embedded-fonts-only"];
    let r = fragment(&dir, 0, "page.rst", &args);
    assert!(r.err.is_empty(), "{}", r.err);
    has(&r.out, "<figure");
    has(&r.out, "<img src=\"data:image/svg+xml;base64,");
    has(&r.out, "alt=\"a points to b\" class=\"graphviz\" />");
    has(&r.out, "<figcaption>A small graph.</figcaption>");
    has(&r.out, "alt=\"A circle\" class=\"typst-diagram\" />");
    has(&r.out, "<math><mi>𝑥</mi></math>");
    lacks(&r.out, "highlight-dot");
    let graph = String::from_utf8(picture(&r.out, 0)).unwrap();
    has(&graph, "<svg");
    has(&graph, "<title>a</title>");
    let circle = String::from_utf8(picture(&r.out, 1)).unwrap();
    assert!(circle.starts_with("<svg"), "{circle}");
    assert!(std::fs::read_dir(&dir.path).unwrap().count() <= 2, "a file was written");
}

#[test]
fn markdown_fences_and_scores_are_pictures_too() {
    let dir = Scratch::new("fragment-markdown");
    dir.write(
        "page.md",
        "```dot\ndigraph { a -> b }\n```\n\n```{lilypond}\n\\fixed c' { c4 d e f }\n```\n",
    );
    let args = [
        "convert",
        "page.md",
        "--from",
        "myst",
        "--to",
        "html",
        "--fragment",
        "-o",
        "-",
        "--embed-pictures",
        "--embedded-fonts-only",
    ];
    let r = guidedog(&dir.path, 0, &args);
    has(&r.out, "class=\"graphviz\" />");
    has(&r.out, "class=\"lilypond-music\" />");
    assert_eq!(r.out.matches("data:image/svg+xml;base64,").count(), 2, "{}", r.out);
}

#[test]
fn a_picture_that_fails_or_passes_a_bound_stays_code_with_a_finding() {
    let dir = Scratch::new("fragment-limits");
    dir.write("secret.txt", "secret\n");
    dir.write(
        "page.rst",
        ".. graphviz::\n\n   digraph { a -> b }\n\n.. graphviz::\n\n   digraph { c -> d }\n\n\
         .. graphviz::\n\n   digraph { broken\n\n.. typst-diagram::\n\n   #unknown-function()\n",
    );
    let r = fragment(&dir, 0, "page.rst", &["--offline", "--embed-pictures"]);
    assert_eq!(r.out.matches("<img").count(), 2, "{}", r.out);
    has(&r.out, "digraph { broken");
    has(&r.out, "#unknown-function()");
    has(&r.err, "embed.graphviz");
    has(&r.err, "embed.typst-diagram");
    has(&r.err, "page.rst:9:");
    let r = fragment(&dir, 0, "page.rst", &["--offline", "--embed-pictures", "--max-pictures=1"]);
    assert_eq!(r.out.matches("<img").count(), 1, "{}", r.out);
    has(&r.out, "digraph { c -&gt; d }");
    has(&r.err, "embed.limit");
    has(&flat(&r.err), "the document has more than 1 pictures");
    has(&r.err, "--max-pictures");
    let args = ["--offline", "--embed-pictures", "--max-picture-kib=1"];
    let r = fragment(&dir, 0, "page.rst", &args);
    lacks(&r.out, "<img");
    has(&flat(&r.err), "the pictures together pass 1 KiB");
    let r = guidedog(&dir.path, 2, &["convert", "page.rst", "-o", "x.html", "--max-pictures=3"]);
    has(&r.err, "Add --embed-pictures.");
}

#[test]
fn mathml_written_by_the_author_is_filtered() {
    let dir = Scratch::new("fragment-authored");
    dir.write(
        "page.rst",
        ".. math::\n   :notation: mathml\n   :label: half\n\n   <math xmlns=\"http://www.w3.org/\
         1998/Math/MathML\"><mfrac><mi onclick=\"x()\">a</mi><mn>2</mn></mfrac><script>alert(1)\
         </script></math>\n\n.. math::\n   :notation: mathml\n\n   <mi>no root</mi>\n",
    );
    for more in [&[][..], &["--math=mathml"][..]] {
        let r = fragment(&dir, 0, "page.rst", more);
        has(
            &r.out,
            "<div id=\"equation-half\" class=\"math\"><math display=\"block\"><mfrac><mi>a</mi>\
             <mn>2</mn></mfrac></math></div>",
        );
        has(&r.out, "<div class=\"math\"><pre>&lt;mi&gt;no root&lt;/mi&gt;</pre></div>");
        lacks(&r.out, "onclick");
        lacks(&r.out, "alert");
        has(&r.err, "html.mathml.dropped");
        has(&r.err, "html.mathml.invalid");
    }
    let args = ["convert", "page.rst", "--from", "sphinx", "--to", "rst", "-o", "-"];
    has(&guidedog(&dir.path, 0, &args).out, "   :notation: mathml\n");
    let args = ["convert", "page.rst", "--from", "sphinx", "-o", "page.pdf", "--color=never"];
    has(&guidedog(&dir.path, 0, &args).err, "typst.math.mathml");
}

#[test]
fn keep_going_writes_the_fragment_and_reports_the_problems() {
    let dir = Scratch::new("fragment-keep-going");
    dir.write(
        "page.rst",
        "Before.\n\n.. nosuch:: x\n\n:nosuchrole:`y` and missing_.\n\n.. include:: gone.rst\n\n\
         .. literalinclude:: gone.py\n\nAfter.\n",
    );
    let r = fragment(&dir, 1, "page.rst", &[]);
    assert!(r.out.is_empty(), "{}", r.out);
    has(&flat(&r.err), "No output was written.");
    let r = fragment(&dir, 0, "page.rst", &["--keep-going"]);
    has(&r.out, "<p>Before.</p>");
    has(&r.out, "<span id=\"id2\" class=\"problematic\">missing</span>");
    has(&r.out, "<p>After.</p>");
    for code in [
        "rst.directive.unknown",
        "rst.role.unknown",
        "core.include.missing",
        "rst.reference.unresolved",
    ] {
        has(&r.err, code);
    }
    lacks(&flat(&r.err), "No output was written.");
}
