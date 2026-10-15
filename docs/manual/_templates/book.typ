// Guidedog's PDF book template. It is ordinary Typst: edit it freely, or delete it to
// use Guidedog's built-in copy. The build calls it as
//
//   #show: book.with(title: ..., author: ..., version: ..., ...)
//
// followed by the chapters, so every parameter below is available. Typst packages may
// be imported here, for example #import "@preview/cetz:0.4.2".
// The Templates chapter of the Guidedog manual lists what the build passes in.
//
// The design: a serif text face with old-style figures, set justified with Knuth-Plass
// line breaking and hyphenation in the document's language; bold serif chapter titles; a
// two-sided page with a wider outer margin; chapters opening on a right-hand page.
#let book(
  title: "",
  author: "",
  version: "",
  date: "",
  lang: "en",
  paper: "a4",
  logo: none,
  copyright: "",
  accent: rgb("#216553"),
  numbering: false, // The build numbers headings itself; this says whether it did.
  version-label: "Version", // "Version" in the document language.
  // Fonts, first available wins. This project bundles Libertinus through pdf_font_paths.
  // Typst supplies New Computer Modern and DejaVu Sans Mono as fallback faces.
  // The sans-serif list ends with a serif for restricted font collections.
  serif: ("Libertinus Serif", "New Computer Modern"),
  sans: ("Libertinus Sans", "Source Sans 3", "Fira Sans", "Noto Sans", "Liberation Sans",
    "DejaVu Sans", "Libertinus Serif"),
  mono: ("DejaVu Sans Mono",),
  size: 11.2pt,
  // Follow cross-references to other pages with their page, as "Deployment (p. 42)":
  // set to n => [p. #n], in the document language.
  page-ref: none,
  body,
) = {
  // One place names the sans-serif list, so Typst reports a missing family once.
  let sans-text = text.with(font: sans)
  let ink = luma(28)
  let quiet = luma(110)
  let rule = luma(175)

  // Language: "pt_BR" is lang "pt" and region "BR"; anything unusable is English, so
  // hyphenation always has patterns.
  let parts = lang.replace("_", "-").split("-")
  let code = lower(parts.at(0))
  if code.len() < 2 or code.len() > 3 or code.match(regex("^[a-z]+$")) == none { code = "en" }
  let region = if parts.len() > 1 and parts.at(1).match(regex("^[A-Za-z]{2}$")) != none {
    upper(parts.at(1))
  } else { none }

  // Regional CJK faces preserve regional glyph shapes. The Devanagari face
  // supplies Hindi shaping; Latin prose keeps the book's Libertinus typography.
  let script = (zh: "Noto Serif CJK SC", ja: "Noto Serif CJK JP",
    ko: "Noto Serif CJK KR", hi: "Noto Serif Devanagari").at(code, default: none)
  if script != none {
    serif = serif + (script,)
    sans = sans + (script,)
    sans-text = text.with(font: sans)
  }
  let edition = (
    en: ("Guidedog Manual", "READING EDITION", "Manual and complete Rust API reference",
      "SOURCE", "Write with meaning.", "STRUCTURE", "Make relations clear.",
      "PUBLICATION", "Read on screen and paper."),
    zh: ("Guidedog 手册", "阅读版", "手册与完整 Rust API 参考",
      "源文件", "写出明确的含义。", "结构", "理清各部分的关系。", "发布", "屏幕与纸上都易读。"),
    ja: ("Guidedog マニュアル", "読書版", "マニュアルと完全版 Rust API リファレンス",
      "ソース", "意味を伝える。", "構造", "関係を明確にする。", "公開", "画面でも紙でも読む。"),
    ko: ("Guidedog 설명서", "읽기용 판", "설명서와 전체 Rust API 참조",
      "소스", "의미를 담아 쓴다.", "구조", "관계를 명확히 한다.", "게시", "화면과 종이에서 읽는다."),
    hi: ("Guidedog मैनुअल", "पठन संस्करण", "मैनुअल और संपूर्ण Rust API संदर्भ",
      "स्रोत", "अर्थ के साथ लिखें।", "संरचना", "संबंध स्पष्ट करें।", "प्रकाशन", "स्क्रीन और काग़ज़ पर पढ़ें।"),
    es: ("Manual de Guidedog", "EDICIÓN DE LECTURA", "Manual y referencia completa de la API de Rust",
      "FUENTE", "Escribir con sentido.", "ESTRUCTURA", "Aclarar las relaciones.",
      "PUBLICACIÓN", "Leer en pantalla y papel."),
    de: ("Guidedog-Handbuch", "LESEAUSGABE", "Handbuch und vollständige Rust-API-Referenz",
      "QUELLE", "Mit Bedeutung schreiben.", "STRUKTUR", "Beziehungen klären.",
      "VERÖFFENTLICHUNG", "Auf Bildschirm und Papier lesen."),
  ).at(code, default: none)
  if edition == none { edition = ("Guidedog Manual", "READING EDITION",
    "Manual and complete Rust API reference", "SOURCE", "Write with meaning.",
    "STRUCTURE", "Make relations clear.", "PUBLICATION", "Read on screen and paper.") }
  if title == "Guidedog Manual" { title = edition.at(0) }

  state("gd-math-sans").update(_ => sans)
  set document(title: title, author: if author == "" { () } else { author })
  set text(font: serif, size: size, fill: ink, lang: code, region: region,
    hyphenate: code not in ("zh", "ja", "ko"), number-type: "old-style", ligatures: true, kerning: true, overhang: true,
    // Knuth-Plass weighs these against loose lines: avoid runts (a lone short word on
    // a paragraph's last line), widows, and orphans; hyphenate at normal cost.
    costs: (hyphenation: 100%, runt: 220%, widow: 100%, orphan: 100%))
  set par(justify: true, linebreaks: "optimized", leading: 0.61em, spacing: 0.7em,
    first-line-indent: 1.15em,
    // Micro-typography: word spaces flex a little, letters a hair, so lines fill
    // without rivers.
    justification-limits: (spacing: (min: 85%, max: 130%),
      tracking: (min: -0.012em, max: 0.018em)))
  set block(spacing: 1.05em)
  set list(indent: 0.6em, body-indent: 0.6em, marker: ([•], [–], [·]))
  set enum(indent: 0.4em, body-indent: 0.6em)
  set terms(spacing: 1.05em)
  show math.equation: set text(number-type: "lining")
  show math.equation.where(block: true): set block(spacing: 1.1em)

  // ---- Headings -------------------------------------------------------------------
  show heading: set text(fill: ink, hyphenate: false, number-type: "lining")
  show heading: set text(font: sans)
  show heading: set par(justify: false, first-line-indent: 0em)
  set heading(numbering: none)
  // A break to the next right-hand page; the markers let the page furniture know a
  // page between them was left blank.
  let recto() = {
    [#metadata(none) <gd-part-end>]
    pagebreak(weak: true, to: "odd")
    [#metadata(none) <gd-part-start>]
  }
  let marker(it) = if it.numbering != none {
    std.numbering(it.numbering, ..counter(heading).at(it.location()))
  }
  // A chapter opens on a right-hand page with its number set large above the title.
  // Headings that are not in the outline (the root document's sections, a contents
  // directive's title) are set as sections instead.
  show heading.where(level: 1): it => {
    if not it.outlined {
      block(above: 1.8em, below: 0.9em, sticky: true,
        text(size: 1.35em, weight: "bold", it.body))
      return
    }
    recto()
    let number = marker(it)
    block(width: 100%, above: 0pt, below: 2.4em, sticky: true, {
      v(22mm)
      if number != none {
        text(size: 44pt, weight: "light", fill: accent, number)
        v(4mm)
      } else {
        line(length: 18mm, stroke: 2.2pt + accent)
        v(7mm)
      }
      set par(leading: 0.3em)
      text(font: serif, size: 30pt, weight: "bold", tracking: -0.015em, it.body)
      v(5mm)
      line(length: 100%, stroke: 0.5pt + rule)
    })
  }
  show heading.where(level: 2): it => block(above: 2.1em, below: 0.95em, sticky: true,
    text(size: 1.2em, weight: "bold", {
      let number = marker(it)
      if number != none { text(fill: accent, number); h(0.7em) }
      it.body
    }))
  show heading.where(level: 3): it => block(above: 1.7em, below: 0.8em, sticky: true,
    text(size: 1.08em, weight: "bold", {
      let number = marker(it)
      if number != none { text(fill: accent, number); h(0.6em) }
      it.body
    }))
  show heading.where(level: 4): it => block(above: 1.4em, below: 0.7em, sticky: true,
    text(size: 1em, weight: "bold", fill: luma(60), it.body))
  show heading: it => if it.level > 4 {
    block(above: 1.3em, below: 0.6em, sticky: true,
      text(font: serif, style: "italic", weight: "regular", size: 1em, it.body))
  } else { it }

  // ---- Code -------------------------------------------------------------------------
  show raw: set text(font: mono, hyphenate: false, ligatures: false, number-type: auto)
  show raw.where(block: false): it => {
    // Mono type looks larger than the serif at the same size.
    set text(size: 0.86em)
    box(fill: accent.lighten(94%).desaturate(60%), outset: (y: 0.22em),
      inset: (x: 0.16em), radius: 1.5pt, it)
  }
  show raw.where(block: true): it => {
    set text(size: 0.8em)
    set par(justify: false, first-line-indent: 0em, leading: 0.5em, hanging-indent: 2em)
    // Each line is its own paragraph, so a line too long for the measure wraps with
    // its continuation indented; a zero-width space gives an empty line its height.
    show linebreak: none
    show raw.line: line => block(spacing: 0.5em, { line; if line.text == "" { sym.zws } })
    block(width: 100%, fill: rgb("#f6f5f1"), stroke: (left: 1.5pt + accent.lighten(55%)),
      inset: (left: 9pt, right: 8pt, y: 7pt), radius: (right: 2.5pt), breakable: true,
      above: 1.1em, below: 1.2em, it)
  }

  // ---- Links and references ---------------------------------------------------------
  let plain(c) = if c.has("text") { c.text } else if c.has("children") {
    c.children.map(plain).join(default: "")
  } else { "" }
  // A dark blue-green that reads as text in print and as a link on screen. A URL shown
  // as itself is never hyphenated.
  show link: it => {
    set text(fill: accent.darken(8%))
    if type(it.dest) == str and it.dest.ends-with(plain(it.body)) {
      set text(hyphenate: false)
      it
    } else { it }
  }
  state("gd-page-ref").update(_ => page-ref)

  // ---- Figures, tables, captions ------------------------------------------------------
  set figure(numbering: none, gap: 0.9em) // The build numbers figures as Sphinx does.
  show figure: set block(above: 1.6em, below: 1.6em)
  show figure.caption: it => {
    set text(size: 0.88em)
    set par(justify: false, first-line-indent: 0em)
    show strong: it => sans-text(fill: accent, size: 0.95em, it)
    block(width: 100%, it.body)
  }
  // A code block's caption is a small label just above it, like a file name tab.
  show figure.where(kind: raw): set figure(gap: 0.5em)
  show figure.where(kind: raw): it => {
    show figure.caption: c => block(width: 100%, sans-text(size: 0.8em, fill: quiet,
      hyphenate: false, c.body))
    it
  }
  // Cells align to the start even inside a figure, which centers its body.
  set table(stroke: none, inset: (x: 0.55em, y: 0.42em), align: start)
  set table.hline(stroke: 0.5pt + ink)
  show table: set text(size: 0.92em, number-type: "lining", number-width: "tabular")
  show table: set par(justify: false, first-line-indent: 0em, leading: 0.55em)
  // Booktabs: a heavy rule above and below, a light one under the head rows, which the
  // build draws, and no vertical rules. Head rows are bold.
  show table: it => {
    // Tables of the gd- helpers (field lists, grammars) have an auto column and no rules.
    if type(it.columns) == array and auto in it.columns { return it }
    let head = it.children.filter(c => c.func() == table.header)
    let columns = if type(it.columns) == int { it.columns } else { it.columns.len() }
    let rows = if head.len() == 0 { 0 } else {
      let spans = head.first().children.filter(c => c.func() != table.hline)
        .map(c => if c.func() == table.cell { c.at("colspan", default: 1) } else { 1 })
      calc.ceil(spans.sum(default: 0) / calc.max(columns, 1))
    }
    show table.cell: c => if c.y < rows { set text(weight: "bold"); c } else { c }
    block(stroke: (top: 0.9pt + ink, bottom: 0.9pt + ink), inset: (y: 0.15em),
      breakable: true, it)
  }
  show table.cell: set block(spacing: 0.6em)

  // ---- Quotes and footnotes ---------------------------------------------------------
  set quote(block: true)
  show quote.where(block: true): it => pad(left: 1.6em, right: 1.6em,
    block(above: 1.2em, below: 1.2em, {
      set text(size: 0.95em)
      it.body
      if it.attribution != none {
        set par(first-line-indent: 0em)
        align(end, text(size: 0.95em, fill: quiet)[— #it.attribution])
      }
    }))
  set footnote.entry(separator: line(length: 25%, stroke: 0.5pt + rule), clearance: 1em,
    gap: 0.5em, indent: 0em)
  show footnote.entry: set text(size: 0.84em)
  show footnote.entry: set par(first-line-indent: 0em, leading: 0.5em)

  // ---- Title page and colophon ------------------------------------------------------
  let label-style(body) = sans-text(size: 9pt, weight: "bold", tracking: 0.12em,
    fill: accent, upper(body))
  // A quiet book cover: the same ink, typefaces and grid as the reading pages.
  page(paper: paper, margin: (x: 24mm, top: 24mm, bottom: 25mm), header: none,
    footer: none, numbering: none, fill: rgb("#f7f3e8"))[
    #set par(justify: false, first-line-indent: 0em)
    #set text(hyphenate: false)
    #grid(columns: (1fr, auto), label-style[GUIDEDOG / #edition.at(1)],
      sans-text(size: 9pt, fill: quiet, version))
    #v(6mm)
    #line(length: 100%, stroke: 0.65pt + accent)
    #v(29mm)
    // Fit short translated titles before wrapping. This avoids leaving a single
    // CJK character on another line; long titles retain a readable minimum size.
    #block(width: 92%, layout(area => context {
      let cover-text = text.with(font: serif, weight: "bold", fill: ink,
        tracking: -0.018em)
      let natural = measure(cover-text(size: 48pt, title)).width
      let fitted = if natural > 0pt {
        48pt * calc.clamp(area.width / natural, 40 / 48, 1)
      } else { 48pt }
      cover-text(size: fitted, par(leading: 0.32em, title))
    }))
    #v(10mm)
    #sans-text(size: 15pt, fill: accent)[#edition.at(2)]
    #v(1fr)
    #grid(columns: (1fr, 1fr, 1fr), column-gutter: 7mm,
      block(stroke: (top: 2pt + accent), inset: (top: 4mm))[
        #label-style[01 / #edition.at(3)] \ #text(size: 12pt)[#edition.at(4)]
      ],
      block(stroke: (top: 2pt + accent), inset: (top: 4mm))[
        #label-style[02 / #edition.at(5)] \ #text(size: 12pt)[#edition.at(6)]
      ],
      block(stroke: (top: 2pt + accent), inset: (top: 4mm))[
        #label-style[03 / #edition.at(7)] \ #text(size: 12pt)[#edition.at(8)]
      ])
    #v(22mm)
    #line(length: 100%, stroke: 0.65pt + rule)
    #v(5mm)
    #grid(columns: (1fr, auto), sans-text(size: 9.5pt, author),
      sans-text(size: 9.5pt, fill: quiet, number-type: "lining", date))
  ]
  page(paper: paper, margin: (x: 24mm, top: 26mm, bottom: 28mm), header: none,
    footer: none, numbering: none)[
    #set par(justify: false, first-line-indent: 0em, spacing: 0.9em)
    #set text(size: 9pt, fill: quiet, number-type: "lining")
    #v(1fr)
    #text(fill: ink, style: "italic", title)
    #if version != "" [ \ #version-label #version]
    #if copyright != "" [ \ © #copyright]
    #if author != "" and copyright == "" [ \ #author]
    #if date != "" [ \ #date]
  ]

  // ---- The page ----------------------------------------------------------------------
  // A 128 mm reading measure, placed toward the spine
  // with the larger margins outside and at the foot.
  let letter = paper == "us-letter"
  let margins = (inside: 28mm, outside: if letter { 58mm } else { 54mm },
    top: 25mm, bottom: if letter { 30mm } else { 32mm })
  let running = sans-text.with(size: 7.6pt, fill: quiet, tracking: 0.08em,
    number-type: "lining")
  let folio = sans-text.with(size: 8.5pt, weight: "bold", fill: ink,
    number-type: "lining")
  // Every page's head asks which pages open a part, which are left blank, and which
  // chapter and section a page is in. The answers come from one table of pages, made
  // once, at the end of the book, and kept in a state that each head reads (its final
  // value, from the previous layout pass) and searches by page: a query over the whole
  // book on every page would cost memory and time in proportion to both.
  let page-marks = state("gd-page-marks", none)
  let marks() = {
    let pages(it) = (it.location().page(), it)
    (starts: query(<gd-part-start>).map(pages),
      // Part starts and ends in document order, as (page, is an end).
      parts: query(selector(<gd-part-start>).or(<gd-part-end>))
        .map(it => (it.location().page(), it.label == <gd-part-end>)),
      chapters: query(heading.where(level: 1, outlined: true)).map(pages),
      sections: query(heading.where(level: 2, outlined: true)).map(pages))
  }
  // The index of the first (page, item) pair, in page order, on page pg or later.
  let from(pairs, pg) = {
    let (lo, hi) = (0, pairs.len())
    while lo < hi {
      let mid = calc.quo(lo + hi, 2)
      if pairs.at(mid).first() < pg { lo = mid + 1 } else { hi = mid }
    }
    lo
  }
  // A page that opens a chapter, the contents, or the main matter.
  let opening(m, pg) = {
    let i = from(m.starts, pg)
    i < m.starts.len() and m.starts.at(i).first() == pg
  }
  // A page left blank so a chapter can open on the right shows nothing at all: the
  // last part marker before it is an end, and the next begins a part on a later page.
  let blank(m, pg) = {
    let i = from(m.parts, pg)
    if i == 0 or not m.parts.at(i - 1).last() { return false }
    i < m.parts.len() and m.parts.at(i).first() > pg
  }
  // The last chapter that began on an earlier page.
  let chapter-of(m, pg) = {
    let i = from(m.chapters, pg)
    if i > 0 { m.chapters.at(i - 1).last() }
  }
  let chapter-name(hd) = {
    let number = marker(hd)
    if number != none { number; h(0.9em) }
    upper(hd.body)
  }
  // The first section to begin on the page, or else the last one before it within
  // the chapter.
  let section-name(m, pg, chapter) = {
    let i = from(m.sections, pg)
    if i < m.sections.len() and m.sections.at(i).first() == pg {
      return m.sections.at(i).last()
    }
    if i == 0 { return none }
    let (page, before) = m.sections.at(i - 1)
    if chapter == none or page >= chapter.location().page() { before }
  }
  set page(paper: paper, margin: margins, binding: left, numbering: "i",
    header-ascent: 38%, footer-descent: 40%,
    header: context {
      let pg = here().page()
      let m = page-marks.final()
      if m == none { return } // the first layout pass.
      if opening(m, pg) or blank(m, pg) { return }
      let chapter = chapter-of(m, pg)
      let number = folio(counter(page).display())
      if calc.even(pg) {
        number; h(1.4em)
        if chapter != none { running(chapter-name(chapter)) }
      } else {
        let section = section-name(m, pg, chapter)
        h(1fr)
        if section != none { running(upper(section.body)) } else if chapter != none {
          running(chapter-name(chapter))
        }
        h(1.4em); number
      }
    },
    footer: context {
      let pg = here().page()
      let m = page-marks.final()
      if m == none { return }
      if opening(m, pg) and not blank(m, pg) {
        align(center, folio(counter(page).display()))
      }
    })

  // ---- Contents -----------------------------------------------------------------------
  context {
    if query(heading.where(outlined: true)).len() > 0 {
      recto()
      {
        show heading: it => block(width: 100%, below: 2.6em, {
          v(22mm)
          line(length: 18mm, stroke: 2.2pt + accent)
          v(7mm)
          sans-text(size: 24pt, weight: "bold", hyphenate: false, it.body)
        })
        set par(justify: false, first-line-indent: 0em)
        set outline.entry(fill: box(width: 1fr, repeat(text(fill: rule, size: 0.8em)[.],
          gap: 0.45em)))
        show outline.entry: set text(number-type: "lining", number-width: "tabular")
        show outline.entry.where(level: 1): it => {
          v(1.05em, weak: true)
          link(it.element.location(), sans-text(weight: "bold", size: 0.95em, {
            let number = it.prefix()
            if number != none { box(width: 2.2em, text(fill: accent, number)) }
            it.body()
            h(1fr)
            it.page()
          }))
        }
        show outline.entry.where(level: 2): set block(spacing: 0.5em)
        outline(depth: 2, indent: auto)
      }
    }
  }

  // ---- The main matter ----------------------------------------------------------------
  recto()
  // The count restarts before the numbering changes, or the first page's head would
  // still show the old count.
  counter(page).update(1)
  set page(numbering: "1")
  body
  context page-marks.update(marks())
}
