// Guidedog's discussion paper. The PDF composer selects this layout for a record,
// independently of book.typ. A project can override it with _templates/discussion.typ.
// Record metadata arrives as strings, not executable source.
#let book(
  title: "", author: "", version: "", date: "", lang: "en", paper: "a4",
  numbering: false, version-label: "Version", logo: none, copyright: "",
  record: (id: "Discussion", catalog: "", fields: ()),
  accent: rgb("#285b60"),
  serif: ("Libertinus Serif", "New Computer Modern"),
  sans: ("Libertinus Sans", "Libertinus Serif"),
  mono: ("DejaVu Sans Mono",),
  size: 10.5pt,
  body,
) = {
  let ink = rgb("#20282b")
  let quiet = rgb("#526269")
  let rule = rgb("#d4dddf")
  let wash = rgb("#f3f6f6")
  let sans-text = text.with(font: sans, number-type: "lining")
  let small = sans-text.with(size: 8pt, fill: quiet)
  let parts = lang.replace("_", "-").split("-")
  let code = lower(parts.first())
  if code.len() < 2 or code.len() > 3 or code.match(regex("^[a-z]+$")) == none { code = "en" }
  let region = if parts.len() > 1 and parts.at(1).match(regex("^[A-Za-z]{2}$")) != none {
    upper(parts.at(1))
  } else { none }

  state("gd-math-sans").update(_ => sans)
  set document(title: title, author: if author == "" { () } else { author })
  set text(font: serif, size: size, fill: ink, lang: code, region: region,
    hyphenate: true, number-type: "lining", costs: (widow: 100%, orphan: 100%))
  set par(justify: true, linebreaks: "optimized", leading: 0.58em, spacing: 0.7em,
    first-line-indent: 0em)
  set block(spacing: 0.9em)
  set page(paper: paper, margin: (x: 25mm, top: 23mm, bottom: 23mm),
    numbering: "1", header-ascent: 30%, footer-descent: 35%,
    header: context {
      if here().page() > 1 {
        block(width: 100%, below: 4mm, {
          grid(columns: (auto, 1fr), column-gutter: 8mm,
            small(weight: "bold", record.id), align(end, small(record.catalog)))
          v(2mm)
          line(length: 100%, stroke: 0.4pt + rule)
        })
      }
    },
    footer: context {
      line(length: 100%, stroke: 0.4pt + rule)
      v(2mm)
      grid(columns: (1fr, auto), column-gutter: 8mm,
        small(record.id),
        small[#counter(page).display() / #counter(page).final().first()])
    })

  // The title is the source heading, so its anchor and PDF outline stay intact.
  // Its metadata replaces the source field list, never a second title or a cover.
  let front-matter() = {
    set text(weight: "regular")
    let cells = record.fields.map(pair => block(breakable: true, {
      small(size: 7.4pt, weight: "bold", tracking: 0.07em, upper(pair.first()))
      v(0.7mm)
      let value = pair.last()
      let shown = if value.starts-with("https://") or value.starts-with("http://") {
        link(value, value)
      } else { value }
      sans-text(size: 9pt, fill: ink, shown)
    }))
    block(width: 100%, above: 5mm, below: 7mm, inset: (y: 4mm),
      stroke: (top: 0.5pt + rule, bottom: 0.5pt + rule), breakable: true,
      grid(columns: (1fr, 1fr), column-gutter: 9mm, row-gutter: 3mm, ..cells))
  }
  let section-number(..values) = if values.pos().len() > 1 {
    std.numbering("1.1.", ..values.pos().slice(1))
  }
  set heading(numbering: section-number)
  show heading.where(level: 1): set heading(numbering: none)
  show heading: set text(font: sans, fill: ink, hyphenate: false)
  show heading: set par(justify: false, first-line-indent: 0em)
  show heading.where(level: 1): it => context {
    let first = query(heading.where(level: 1)).first().location() == it.location()
    if first {
      block(above: 0pt, below: 3mm, sticky: true, {
        sans-text(size: 8pt, weight: "bold", tracking: 0.1em, fill: accent, record.id)
        v(3mm)
        sans-text(size: 23pt, weight: "bold", tracking: -0.015em,
          par(leading: 0.3em, it.body))
      })
      front-matter()
    } else {
      block(above: 1.5em, below: 0.6em, sticky: true,
        sans-text(size: 14pt, weight: "bold", it.body))
    }
  }
  show heading: it => if it.level > 1 {
    context {
      let counters = counter(heading).at(it.location()).slice(1)
      let number = std.numbering("1.1.", ..counters)
      block(above: if it.level == 2 { 1.7em } else { 1.1em }, below: 0.6em, sticky: true,
        sans-text(size: if it.level == 2 { 11.5pt } else { 10.5pt }, weight: "bold", {
          text(fill: accent, number)
          h(0.65em)
          it.body
        }))
    }
  } else { it }

  show raw: set text(font: mono, hyphenate: false, ligatures: false)
  show raw.where(block: false): set text(size: 0.88em)
  show raw.where(block: true): it => block(width: 100%, breakable: true,
    fill: wash, stroke: (left: 1.5pt + accent.lighten(35%)), inset: 9pt,
    text(size: 0.82em, it))
  show figure.where(kind: raw): set block(breakable: true)
  show figure.where(kind: table): set block(breakable: true)
  show figure.caption: set text(font: sans, size: 0.86em, fill: quiet)
  show table: set text(size: 0.92em)
  set table(inset: (x: 6pt, y: 5pt), stroke: 0.4pt + rule)
  set list(indent: 0.6em, body-indent: 0.6em)
  set enum(indent: 0.4em, body-indent: 0.6em)
  show link: set text(fill: accent)
  show quote.where(block: true): set block(inset: (left: 12pt),
    stroke: (left: 1pt + rule), breakable: true)
  set footnote.entry(separator: line(length: 22%, stroke: 0.4pt + rule), gap: 0.5em)
  show footnote.entry: set text(size: 0.83em)

  // Start the paper immediately. No cover, colophon, recto breaks or automatic contents.
  body
}
